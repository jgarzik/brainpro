import { useEffect, useState } from "react";
import { useParams, Link } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { PageSpinner } from "@/components/ui/Spinner";
import { api } from "@/api/client";
import { useWebSocket } from "@/api/hooks";
import { useSessionStore } from "@/store/sessionStore";
import { ROUTES } from "@/constants/routes";
import { formatCost } from "@/types/cost";

export default function SessionDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { isConnected } = useWebSocket();
  const sessions = useSessionStore((s) => s.sessions);
  const messages = useSessionStore((s) => s.messages);
  const addSession = useSessionStore((s) => s.addSession);
  const setMessages = useSessionStore((s) => s.setMessages);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const session = id ? sessions.get(id) : undefined;
  const sessionMessages = id ? (messages.get(id) ?? []) : [];

  useEffect(() => {
    if (!isConnected || !id) return;

    const fetchSession = async () => {
      setLoading(true);
      setError(null);
      try {
        const data = await api.session.get(id);
        addSession(data);
        setMessages(id, data.messages);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load session");
      } finally {
        setLoading(false);
      }
    };

    if (!session) {
      fetchSession();
    }
  }, [isConnected, id, session, addSession, setMessages]);

  if (loading) {
    return <PageSpinner />;
  }

  if (error) {
    return (
      <div className="space-y-4">
        <Link
          to={ROUTES.SESSIONS}
          className="inline-flex items-center gap-2 text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Sessions
        </Link>
        <Card>
          <p className="text-center text-red-600 dark:text-red-400">{error}</p>
        </Card>
      </div>
    );
  }

  if (!session) {
    return (
      <div className="space-y-4">
        <Link
          to={ROUTES.SESSIONS}
          className="inline-flex items-center gap-2 text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
        >
          <ArrowLeft className="h-4 w-4" />
          Back to Sessions
        </Link>
        <Card>
          <p className="text-center text-gray-500 dark:text-gray-400">
            Session not found
          </p>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <Link
        to={ROUTES.SESSIONS}
        className="inline-flex items-center gap-2 text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
      >
        <ArrowLeft className="h-4 w-4" />
        Back to Sessions
      </Link>

      <div className="flex items-center gap-4">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
          Session <code className="font-mono">{session.id.slice(0, 12)}</code>
        </h1>
        <Badge
          variant={
            session.status === "active"
              ? "success"
              : session.status === "ended"
                ? "neutral"
                : "warning"
          }
        >
          {session.status}
        </Badge>
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        {/* Stats */}
        <div className="space-y-4">
          <Card title="Statistics">
            <div className="space-y-3 text-sm">
              <div className="flex justify-between">
                <span className="text-gray-500 dark:text-gray-400">Turns</span>
                <span>{session.turn_count}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500 dark:text-gray-400">
                  Total Tokens
                </span>
                <span>{session.total_tokens.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500 dark:text-gray-400">
                  Total Cost
                </span>
                <span>{formatCost(session.total_cost_usd)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500 dark:text-gray-400">
                  Created
                </span>
                <span className="text-xs">
                  {new Date(session.created_at).toLocaleString()}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500 dark:text-gray-400">
                  Updated
                </span>
                <span className="text-xs">
                  {new Date(session.updated_at).toLocaleString()}
                </span>
              </div>
            </div>
          </Card>

          {session.agent_id && (
            <Card title="Agent">
              <code className="font-mono text-sm">{session.agent_id}</code>
            </Card>
          )}
        </div>

        {/* Messages */}
        <div className="lg:col-span-2">
          <Card title="Message History">
            {sessionMessages.length > 0 ? (
              <div className="max-h-[600px] space-y-3 overflow-y-auto">
                {sessionMessages.map((msg) => (
                  <div
                    key={msg.id}
                    className={`rounded-lg p-3 ${
                      msg.role === "user"
                        ? "bg-blue-50 dark:bg-blue-900/20"
                        : msg.role === "assistant"
                          ? "bg-gray-50 dark:bg-gray-800"
                          : "bg-amber-50 dark:bg-amber-900/20"
                    }`}
                  >
                    <div className="mb-1 flex items-center justify-between">
                      <Badge variant={msg.role === "user" ? "info" : "neutral"}>
                        {msg.role}
                      </Badge>
                      <span className="text-xs text-gray-400">
                        {new Date(msg.timestamp).toLocaleTimeString()}
                      </span>
                    </div>
                    <p className="whitespace-pre-wrap text-sm">{msg.content}</p>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-gray-500 dark:text-gray-400">
                No messages
              </p>
            )}
          </Card>
        </div>
      </div>
    </div>
  );
}
