//! Discord channel plugin using serenity.

pub mod formatting;

use super::config::DiscordConfig;
use super::plugin::{
    ApprovalRequest, ApprovalResponse, ChannelContext, ChannelPlugin, ChannelTarget,
    InboundMessage, MessageId, OutboundMessage,
};
use anyhow::Result;
use serenity::{
    async_trait as serenity_async_trait,
    builder::{
        CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateMessage, EditMessage,
    },
    client::{Client, Context, EventHandler},
    model::{
        channel::Message,
        gateway::{GatewayIntents, Ready},
        id::{ChannelId, MessageId as DiscordMessageId},
        prelude::Interaction,
    },
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Discord channel plugin
pub struct DiscordChannel {
    /// Configuration
    config: DiscordConfig,
    /// HTTP client (available after start)
    http: RwLock<Option<Arc<serenity::http::Http>>>,
    /// Shutdown signal
    shutdown_tx: RwLock<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl DiscordChannel {
    /// Create a new Discord plugin
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            config,
            http: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
        }
    }

    /// Get the HTTP client
    async fn get_http(&self) -> Result<Arc<serenity::http::Http>> {
        let http = self.http.read().await;
        http.clone()
            .ok_or_else(|| anyhow::anyhow!("Discord client not initialized"))
    }
}

#[async_trait::async_trait]
impl ChannelPlugin for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    async fn start(&self, ctx: Arc<ChannelContext>) -> Result<()> {
        let token = self
            .config
            .resolve_bot_token()
            .ok_or_else(|| anyhow::anyhow!("Discord bot token not configured"))?;

        // Create intents
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        // Create handler
        let handler = DiscordHandler {
            message_tx: ctx.message_tx.clone(),
            approval_tx: ctx.approval_tx.clone(),
            allowed_guilds: self.config.allowed_guilds.clone(),
            allowed_channels: self.config.allowed_channels.clone(),
        };

        // Build client
        let mut client = Client::builder(&token, intents)
            .event_handler(handler)
            .await?;

        // Store HTTP client
        *self.http.write().await = Some(client.http.clone());

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        // Run client in background
        let shard_manager = client.shard_manager.clone();
        tokio::spawn(async move {
            tokio::select! {
                result = client.start() => {
                    if let Err(e) = result {
                        eprintln!("[discord] Client error: {}", e);
                    }
                }
                _ = &mut shutdown_rx => {
                    shard_manager.shutdown_all().await;
                }
            }
        });

        eprintln!("[discord] Bot started");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
        }
        *self.http.write().await = None;
        eprintln!("[discord] Bot stopped");
        Ok(())
    }

    async fn send_message(
        &self,
        target: &ChannelTarget,
        msg: &OutboundMessage,
    ) -> Result<MessageId> {
        let http = self.get_http().await?;
        let channel_id = ChannelId::new(target.chat_id.parse::<u64>()?);

        let content = if msg.markdown {
            formatting::to_discord_markdown(&msg.content)
        } else {
            msg.content.clone()
        };

        // Truncate if needed
        let content = formatting::truncate(&content, 2000);

        let result = if let Some(edit_id) = &msg.edit_message_id {
            // Edit existing message
            let msg_id = DiscordMessageId::new(edit_id.parse::<u64>()?);
            channel_id
                .edit_message(&http, msg_id, EditMessage::new().content(&content))
                .await?;
            edit_id.clone()
        } else {
            // Send new message
            let sent = channel_id
                .send_message(&http, CreateMessage::new().content(&content))
                .await?;
            sent.id.to_string()
        };

        Ok(MessageId {
            id: result,
            channel: "discord".to_string(),
        })
    }

    async fn send_approval_request(
        &self,
        target: &ChannelTarget,
        req: &ApprovalRequest,
    ) -> Result<MessageId> {
        let http = self.get_http().await?;
        let channel_id = ChannelId::new(target.chat_id.parse::<u64>()?);

        // Create embed
        let embed = formatting::create_approval_embed(
            &req.tool_name,
            &req.tool_args,
            req.policy_rule.as_deref(),
        );

        // Create buttons
        let approve_button = CreateButton::new(req.approve_callback())
            .label("Approve")
            .style(serenity::model::application::ButtonStyle::Success);

        let deny_button = CreateButton::new(req.deny_callback())
            .label("Deny")
            .style(serenity::model::application::ButtonStyle::Danger);

        let action_row = CreateActionRow::Buttons(vec![approve_button, deny_button]);

        let sent = channel_id
            .send_message(
                &http,
                CreateMessage::new()
                    .embed(embed)
                    .components(vec![action_row]),
            )
            .await?;

        Ok(MessageId {
            id: sent.id.to_string(),
            channel: "discord".to_string(),
        })
    }

    async fn edit_message(
        &self,
        target: &ChannelTarget,
        message_id: &str,
        msg: &OutboundMessage,
    ) -> Result<()> {
        let http = self.get_http().await?;
        let channel_id = ChannelId::new(target.chat_id.parse::<u64>()?);
        let msg_id = DiscordMessageId::new(message_id.parse::<u64>()?);

        let content = if msg.markdown {
            formatting::to_discord_markdown(&msg.content)
        } else {
            msg.content.clone()
        };

        let content = formatting::truncate(&content, 2000);

        channel_id
            .edit_message(&http, msg_id, EditMessage::new().content(content))
            .await?;

        Ok(())
    }

    async fn delete_message(&self, target: &ChannelTarget, message_id: &str) -> Result<()> {
        let http = self.get_http().await?;
        let channel_id = ChannelId::new(target.chat_id.parse::<u64>()?);
        let msg_id = DiscordMessageId::new(message_id.parse::<u64>()?);

        channel_id.delete_message(&http, msg_id).await?;
        Ok(())
    }
}

/// Discord event handler
struct DiscordHandler {
    message_tx: tokio::sync::mpsc::UnboundedSender<InboundMessage>,
    approval_tx: tokio::sync::mpsc::UnboundedSender<ApprovalResponse>,
    allowed_guilds: Vec<u64>,
    allowed_channels: Vec<u64>,
}

#[serenity_async_trait]
impl EventHandler for DiscordHandler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        eprintln!("[discord] {} is connected!", ready.user.name);
    }

    async fn message(&self, _ctx: Context, msg: Message) {
        // Ignore bot messages
        if msg.author.bot {
            return;
        }

        // Check guild permissions
        if let Some(guild_id) = msg.guild_id {
            if !self.allowed_guilds.is_empty() && !self.allowed_guilds.contains(&guild_id.get()) {
                return;
            }
        }

        // Check channel permissions
        if !self.allowed_channels.is_empty()
            && !self.allowed_channels.contains(&msg.channel_id.get())
        {
            return;
        }

        // Build target
        let target = ChannelTarget {
            channel: "discord".to_string(),
            chat_id: msg.channel_id.to_string(),
            user_id: Some(msg.author.id.to_string()),
            username: Some(msg.author.name.clone()),
        };

        // Forward to manager
        let inbound = InboundMessage {
            target,
            content: msg.content.clone(),
            message_id: msg.id.to_string(),
        };

        let _ = self.message_tx.send(inbound);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Component(component) = interaction {
            // Parse callback data
            let data = &component.data.custom_id;

            let (action, turn_id, tool_call_id) = match ApprovalRequest::parse_callback(data) {
                Some(parsed) => parsed,
                None => return,
            };

            // Build target
            let target = ChannelTarget {
                channel: "discord".to_string(),
                chat_id: component.channel_id.to_string(),
                user_id: Some(component.user.id.to_string()),
                username: Some(component.user.name.clone()),
            };

            // Send approval response
            let response = ApprovalResponse {
                turn_id,
                tool_call_id,
                approved: action == "approve",
                source: target,
            };

            let _ = self.approval_tx.send(response);

            // Respond to interaction
            let status = if action == "approve" {
                "✅ Approved"
            } else {
                "❌ Denied"
            };

            let response_msg = CreateInteractionResponseMessage::new()
                .content(status)
                .ephemeral(true);

            let _ = component
                .create_response(&ctx.http, CreateInteractionResponse::Message(response_msg))
                .await;

            // Disable buttons on original message
            let channel_id = component.channel_id;
            let msg_id = component.message.id;
            let _ = channel_id
                .edit_message(&ctx.http, msg_id, EditMessage::new().components(vec![]))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_target_creation() {
        let target = ChannelTarget::discord(123456789, Some(987654321));
        assert_eq!(target.channel, "discord");
        assert_eq!(target.chat_id, "123456789");
        assert_eq!(target.user_id, Some("987654321".to_string()));
    }
}
