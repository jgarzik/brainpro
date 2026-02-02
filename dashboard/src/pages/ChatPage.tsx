import { useState, useCallback, useRef, useEffect } from "react";
import { clsx } from "clsx";
import { Send, Loader2, Check, X, AlertCircle } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { Textarea } from "@/components/ui/Input";
import { useToast } from "@/components/ui";
import { api } from "@/api/client";
import { useWebSocket, useEvent } from "@/api/hooks";
import {
  useSessionStore,
  useCurrentSession,
  useCurrentMessages,
} from "@/store/sessionStore";
import { AGENT_EVENTS } from "@/constants/events";
import { truncateId } from "@/utils";
import type { Message, ToolCall } from "@/types/session";
import type { ClientEvent } from "@/types/protocol";

function MessageBubble({ message }: { message: Message }) {
  const isUser = message.role === "user";

  return (
    <div className={clsx("flex", isUser ? "justify-end" : "justify-start")}>
      <div
        className={clsx(
          "max-w-[80%] rounded-lg px-4 py-2",
          isUser
            ? "bg-blue-600 text-white"
            : "bg-gray-100 text-gray-900 dark:bg-gray-800 dark:text-gray-100",
        )}
      >
        <p className="whitespace-pre-wrap text-sm">{message.content}</p>
        {message.tool_calls && message.tool_calls.length > 0 && (
          <div className="mt-2 space-y-2">
            {message.tool_calls.map((tc) => (
              <ToolCallCard key={tc.id} toolCall={tc} />
            ))}
          </div>
        )}
        {message.streaming && (
          <span className="ml-1 inline-block h-2 w-2 animate-pulse rounded-full bg-current" />
        )}
      </div>
    </div>
  );
}

function ToolCallCard({ toolCall }: { toolCall: ToolCall }) {
  const statusIcon = {
    pending: <Loader2 className="h-3 w-3 animate-spin" />,
    running: <Loader2 className="h-3 w-3 animate-spin" />,
    completed: <Check className="h-3 w-3 text-emerald-500" />,
    failed: <X className="h-3 w-3 text-red-500" />,
    denied: <AlertCircle className="h-3 w-3 text-amber-500" />,
  };

  return (
    <div className="rounded border border-gray-200 bg-white p-2 text-xs dark:border-gray-700 dark:bg-gray-900">
      <div className="flex items-center gap-2">
        {statusIcon[toolCall.status]}
        <span className="font-medium">{toolCall.name}</span>
        {toolCall.duration_ms && (
          <span className="text-gray-400">{toolCall.duration_ms}ms</span>
        )}
      </div>
      {toolCall.error && <p className="mt-1 text-red-500">{toolCall.error}</p>}
    </div>
  );
}

function ApprovalPrompt() {
  const approval = useSessionStore((s) => s.pendingApproval);
  const setPendingApproval = useSessionStore((s) => s.setPendingApproval);
  const toast = useToast();

  if (!approval) return null;

  const handleApprove = async (approved: boolean) => {
    try {
      await api.tool.approve({
        session_id: approval.session_id,
        turn_id: approval.turn_id,
        tool_call_id: approval.tool_call_id,
        approved,
      });
      setPendingApproval(null);
    } catch {
      toast.error("Failed to respond to approval request");
    }
  };

  return (
    <Card className="border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-900/20">
      <div className="flex items-start gap-3">
        <AlertCircle className="mt-0.5 h-5 w-5 text-amber-500" />
        <div className="flex-1">
          <h4 className="font-medium text-amber-800 dark:text-amber-200">
            Tool requires approval
          </h4>
          <p className="mt-1 text-sm text-amber-700 dark:text-amber-300">
            <code className="font-mono">{approval.tool_name}</code>
          </p>
          <pre className="mt-2 max-h-32 overflow-auto rounded bg-amber-100 p-2 text-xs dark:bg-amber-900/30">
            {JSON.stringify(approval.args, null, 2)}
          </pre>
          <div className="mt-3 flex gap-2">
            <Button size="sm" onClick={() => handleApprove(true)}>
              Approve
            </Button>
            <Button
              size="sm"
              variant="danger"
              onClick={() => handleApprove(false)}
            >
              Deny
            </Button>
          </div>
        </div>
      </div>
    </Card>
  );
}

function ChatInput({
  onSend,
  disabled,
}: {
  onSend: (message: string) => void;
  disabled: boolean;
}) {
  const [input, setInput] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (input.trim() && !disabled) {
      onSend(input.trim());
      setInput("");
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit(e);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="flex gap-2">
      <Textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Type a message..."
        rows={2}
        className="flex-1 resize-none"
        disabled={disabled}
      />
      <Button type="submit" disabled={disabled || !input.trim()}>
        <Send className="h-4 w-4" />
      </Button>
    </form>
  );
}

export default function ChatPage() {
  const { isConnected } = useWebSocket();
  const session = useCurrentSession();
  const messages = useCurrentMessages();
  const streaming = useSessionStore((s) => s.streaming);
  const streamBuffer = useSessionStore((s) => s.streamBuffer);
  const addMessage = useSessionStore((s) => s.addMessage);
  const setStreaming = useSessionStore((s) => s.setStreaming);
  const appendStreamBuffer = useSessionStore((s) => s.appendStreamBuffer);
  const clearStreamBuffer = useSessionStore((s) => s.clearStreamBuffer);
  const setCurrentSession = useSessionStore((s) => s.setCurrentSession);
  const addSession = useSessionStore((s) => s.addSession);
  const setPendingApproval = useSessionStore((s) => s.setPendingApproval);

  const toast = useToast();
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Use refs to avoid stale closures in event handlers
  const streamBufferRef = useRef(streamBuffer);
  useEffect(() => {
    streamBufferRef.current = streamBuffer;
  }, [streamBuffer]);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamBuffer]);

  // Handle streaming events - use functional updates to avoid stale closures
  const handleTokenDelta = useCallback(
    (event: ClientEvent) => {
      const data = event.data as { content?: string };
      if (data.content) {
        appendStreamBuffer(data.content);
      }
    },
    [appendStreamBuffer],
  );

  const handleMessage = useCallback(
    (event: ClientEvent) => {
      const data = event.data as {
        message_id?: string;
        content?: string;
        role?: string;
      };
      const sessionId = event.session_id;
      if (sessionId && data.message_id && data.content) {
        // Use ref to get current streamBuffer value (avoids stale closure)
        addMessage(sessionId, {
          id: data.message_id,
          role: (data.role as "assistant") ?? "assistant",
          content: streamBufferRef.current + data.content,
          timestamp: Date.now(),
        });
        clearStreamBuffer();
      }
    },
    [addMessage, clearStreamBuffer],
  );

  const handleDone = useCallback(() => {
    setStreaming(false);
    clearStreamBuffer();
  }, [setStreaming, clearStreamBuffer]);

  const handleAwaitingApproval = useCallback(
    (event: ClientEvent) => {
      const data = event.data as {
        turn_id: string;
        tool_call_id: string;
        tool_name: string;
        args: unknown;
      };
      if (event.session_id) {
        setPendingApproval({
          session_id: event.session_id,
          turn_id: data.turn_id,
          tool_call_id: data.tool_call_id,
          tool_name: data.tool_name,
          args: data.args,
        });
      }
    },
    [setPendingApproval],
  );

  useEvent(AGENT_EVENTS.TOKEN_DELTA, handleTokenDelta);
  useEvent(AGENT_EVENTS.MESSAGE, handleMessage);
  useEvent(AGENT_EVENTS.DONE, handleDone);
  useEvent(AGENT_EVENTS.AWAITING_APPROVAL, handleAwaitingApproval);

  const handleSend = async (message: string) => {
    if (!isConnected) {
      toast.error("Not connected to gateway");
      return;
    }

    try {
      // Add user message immediately
      const userMsgId = crypto.randomUUID();
      const sessionId = session?.id;

      if (sessionId) {
        addMessage(sessionId, {
          id: userMsgId,
          role: "user",
          content: message,
          timestamp: Date.now(),
        });
      }

      setStreaming(true);

      const response = await api.chat.send({
        message,
        session_id: sessionId,
      });

      // If new session was created
      if (!sessionId && response.session_id) {
        addSession({
          id: response.session_id,
          status: "active",
          created_at: Date.now(),
          updated_at: Date.now(),
          turn_count: 1,
          total_tokens: 0,
          total_cost_usd: 0,
          turns: [],
        });
        setCurrentSession(response.session_id);
        addMessage(response.session_id, {
          id: userMsgId,
          role: "user",
          content: message,
          timestamp: Date.now(),
        });
      }
    } catch (err) {
      setStreaming(false);
      toast.error(
        err instanceof Error ? err.message : "Failed to send message",
      );
    }
  };

  return (
    <div className="flex h-full gap-4">
      {/* Chat panel */}
      <div className="flex flex-1 flex-col">
        <Card className="flex flex-1 flex-col" padding="sm">
          {/* Messages */}
          <div className="flex-1 space-y-4 overflow-y-auto p-4">
            {messages.map((msg) => (
              <MessageBubble key={msg.id} message={msg} />
            ))}
            {streaming && streamBuffer && (
              <div className="flex justify-start">
                <div className="max-w-[80%] rounded-lg bg-gray-100 px-4 py-2 text-gray-900 dark:bg-gray-800 dark:text-gray-100">
                  <p className="whitespace-pre-wrap text-sm">
                    {streamBuffer}
                    <span className="ml-1 inline-block h-2 w-2 animate-pulse rounded-full bg-current" />
                  </p>
                </div>
              </div>
            )}
            <div ref={messagesEndRef} />
          </div>

          {/* Approval prompt */}
          <div className="p-4 pt-0">
            <ApprovalPrompt />
          </div>

          {/* Input */}
          <div className="border-t border-gray-200 p-4 dark:border-gray-700">
            <ChatInput
              onSend={handleSend}
              disabled={!isConnected || streaming}
            />
          </div>
        </Card>
      </div>

      {/* Session info sidebar */}
      <div className="w-72">
        <Card title="Session Info">
          {session ? (
            <div className="space-y-3 text-sm">
              <div>
                <span className="text-gray-500 dark:text-gray-400">ID:</span>
                <code className="ml-2 font-mono text-xs">
                  {truncateId(session.id)}
                </code>
              </div>
              <div>
                <span className="text-gray-500 dark:text-gray-400">
                  Status:
                </span>
                <Badge
                  className="ml-2"
                  variant={session.status === "active" ? "success" : "neutral"}
                >
                  {session.status}
                </Badge>
              </div>
              <div>
                <span className="text-gray-500 dark:text-gray-400">Turns:</span>
                <span className="ml-2">{session.turn_count}</span>
              </div>
              <div>
                <span className="text-gray-500 dark:text-gray-400">Cost:</span>
                <span className="ml-2">
                  ${session.total_cost_usd.toFixed(4)}
                </span>
              </div>
            </div>
          ) : (
            <p className="text-sm text-gray-500 dark:text-gray-400">
              No active session. Send a message to start.
            </p>
          )}
        </Card>
      </div>
    </div>
  );
}
