//! Telegram channel plugin using teloxide.

pub mod formatting;

use super::config::TelegramConfig;
use super::plugin::{
    ApprovalRequest, ApprovalResponse, ChannelContext, ChannelPlugin, ChannelTarget,
    InboundMessage, MessageId, OutboundMessage,
};
use anyhow::Result;
use std::sync::Arc;
use teloxide::{
    dispatching::{Dispatcher, UpdateFilterExt},
    dptree,
    payloads::SendMessageSetters,
    prelude::*,
    types::{
        CallbackQuery, ChatId, InlineKeyboardButton, InlineKeyboardMarkup,
        MessageId as TgMessageId, ParseMode, Update,
    },
    Bot,
};
use tokio::sync::RwLock;

/// Telegram channel plugin
pub struct TelegramChannel {
    /// Configuration
    config: TelegramConfig,
    /// Bot instance (initialized on start)
    bot: RwLock<Option<Bot>>,
    /// Shutdown signal
    shutdown_tx: RwLock<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl TelegramChannel {
    /// Create a new Telegram plugin
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            bot: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
        }
    }

    /// Get the bot instance
    async fn get_bot(&self) -> Result<Bot> {
        let bot = self.bot.read().await;
        bot.clone()
            .ok_or_else(|| anyhow::anyhow!("Telegram bot not initialized"))
    }
}

#[async_trait::async_trait]
impl ChannelPlugin for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(&self, ctx: Arc<ChannelContext>) -> Result<()> {
        let token = self
            .config
            .resolve_bot_token()
            .ok_or_else(|| anyhow::anyhow!("Telegram bot token not configured"))?;

        let bot = Bot::new(token);

        // Store bot instance
        *self.bot.write().await = Some(bot.clone());

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        // Clone context for handlers
        let message_tx = ctx.message_tx.clone();
        let approval_tx = ctx.approval_tx.clone();

        // Build dispatcher
        let handler = dptree::entry()
            .branch(Update::filter_message().endpoint(handle_message))
            .branch(Update::filter_callback_query().endpoint(handle_callback));

        let mut dispatcher = Dispatcher::builder(bot.clone(), handler)
            .dependencies(dptree::deps![message_tx, approval_tx])
            .build();

        // Get shutdown token before spawning
        let shutdown_token = dispatcher.shutdown_token();

        // Run dispatcher in background
        tokio::spawn(async move {
            tokio::select! {
                _ = dispatcher.dispatch() => {}
                _ = &mut shutdown_rx => {
                    shutdown_token.shutdown().ok();
                }
            }
        });

        eprintln!("[telegram] Bot started");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
        }
        *self.bot.write().await = None;
        eprintln!("[telegram] Bot stopped");
        Ok(())
    }

    async fn send_message(
        &self,
        target: &ChannelTarget,
        msg: &OutboundMessage,
    ) -> Result<MessageId> {
        let bot = self.get_bot().await?;
        let chat_id = target.chat_id.parse::<i64>()?;

        let content = if msg.markdown {
            formatting::to_telegram_markdown(&msg.content)
        } else {
            formatting::escape_markdown(&msg.content)
        };

        // Truncate if needed
        let content = formatting::truncate_for_telegram(&content, 4000);

        let result = if let Some(edit_id) = &msg.edit_message_id {
            // Edit existing message
            let msg_id = TgMessageId(edit_id.parse::<i32>()?);
            bot.edit_message_text(ChatId(chat_id), msg_id, &content)
                .parse_mode(ParseMode::MarkdownV2)
                .await?;
            edit_id.clone()
        } else {
            // Send new message
            let mut request = bot.send_message(ChatId(chat_id), &content);

            if msg.markdown {
                request = request.parse_mode(ParseMode::MarkdownV2);
            }

            // Note: reply_to is not used in this version of teloxide
            // Future versions may support reply parameters

            let sent = request.await?;
            sent.id.0.to_string()
        };

        Ok(MessageId {
            id: result,
            channel: "telegram".to_string(),
        })
    }

    async fn send_approval_request(
        &self,
        target: &ChannelTarget,
        req: &ApprovalRequest,
    ) -> Result<MessageId> {
        let bot = self.get_bot().await?;
        let chat_id = target.chat_id.parse::<i64>()?;

        let text = req.format_message();
        let text = formatting::to_telegram_markdown(&text);

        // Create inline keyboard
        let keyboard = InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback("✅ Approve", req.approve_callback()),
            InlineKeyboardButton::callback("❌ Deny", req.deny_callback()),
        ]]);

        let sent = bot
            .send_message(ChatId(chat_id), text)
            .parse_mode(ParseMode::MarkdownV2)
            .reply_markup(keyboard)
            .await?;

        Ok(MessageId {
            id: sent.id.0.to_string(),
            channel: "telegram".to_string(),
        })
    }

    async fn edit_message(
        &self,
        target: &ChannelTarget,
        message_id: &str,
        msg: &OutboundMessage,
    ) -> Result<()> {
        let bot = self.get_bot().await?;
        let chat_id = target.chat_id.parse::<i64>()?;
        let msg_id = TgMessageId(message_id.parse::<i32>()?);

        let content = if msg.markdown {
            formatting::to_telegram_markdown(&msg.content)
        } else {
            formatting::escape_markdown(&msg.content)
        };

        let content = formatting::truncate_for_telegram(&content, 4000);

        bot.edit_message_text(ChatId(chat_id), msg_id, content)
            .parse_mode(ParseMode::MarkdownV2)
            .await?;

        Ok(())
    }

    async fn delete_message(&self, target: &ChannelTarget, message_id: &str) -> Result<()> {
        let bot = self.get_bot().await?;
        let chat_id = target.chat_id.parse::<i64>()?;
        let msg_id = TgMessageId(message_id.parse::<i32>()?);

        bot.delete_message(ChatId(chat_id), msg_id).await?;
        Ok(())
    }
}

/// Handle incoming messages
async fn handle_message(
    _bot: Bot,
    msg: Message,
    message_tx: tokio::sync::mpsc::UnboundedSender<InboundMessage>,
) -> ResponseResult<()> {
    // Extract text content
    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()), // Ignore non-text messages
    };

    // Build target
    let target = ChannelTarget {
        channel: "telegram".to_string(),
        chat_id: msg.chat.id.0.to_string(),
        user_id: msg.from.as_ref().map(|u| u.id.0.to_string()),
        username: msg.from.as_ref().and_then(|u| u.username.clone()),
    };

    // Forward to manager
    let inbound = InboundMessage {
        target,
        content: text,
        message_id: msg.id.0.to_string(),
    };

    let _ = message_tx.send(inbound);

    Ok(())
}

/// Handle callback queries (button clicks)
async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    approval_tx: tokio::sync::mpsc::UnboundedSender<ApprovalResponse>,
) -> ResponseResult<()> {
    // Parse callback data
    let data = match &q.data {
        Some(d) => d,
        None => return Ok(()),
    };

    let (action, turn_id, tool_call_id) = match ApprovalRequest::parse_callback(data) {
        Some(parsed) => parsed,
        None => return Ok(()),
    };

    // Get message info
    let message = match &q.message {
        Some(m) => m,
        None => return Ok(()),
    };

    let chat_id = message.chat().id.0;

    // Build target
    let target = ChannelTarget {
        channel: "telegram".to_string(),
        chat_id: chat_id.to_string(),
        user_id: Some(q.from.id.0.to_string()),
        username: q.from.username.clone(),
    };

    // Send approval response
    let response = ApprovalResponse {
        turn_id,
        tool_call_id,
        approved: action == "approve",
        source: target,
    };

    let _ = approval_tx.send(response);

    // Answer callback and edit message
    bot.answer_callback_query(&q.id).await?;

    let status = if action == "approve" {
        "✅ Approved"
    } else {
        "❌ Denied"
    };

    // Try to edit the message to remove buttons
    if let Some(msg) = q.message {
        if let Some(text) = msg.regular_message().and_then(|m| m.text()) {
            let new_text = format!("{}\n\n_{}_", text, status);
            let _ = bot
                .edit_message_text(
                    msg.chat().id,
                    msg.id(),
                    formatting::to_telegram_markdown(&new_text),
                )
                .parse_mode(ParseMode::MarkdownV2)
                .await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_target_creation() {
        let target = ChannelTarget::telegram(12345, Some(67890));
        assert_eq!(target.channel, "telegram");
        assert_eq!(target.chat_id, "12345");
        assert_eq!(target.user_id, Some("67890".to_string()));
    }
}
