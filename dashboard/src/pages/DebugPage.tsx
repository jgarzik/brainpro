import { useState, useRef, useEffect } from "react";
import { Send, Trash2, Download } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Input, Textarea } from "@/components/ui/Input";
import { Badge } from "@/components/ui/Badge";
import { useToast } from "@/components/ui";
import { getWebSocket } from "@/api/websocket";
import { useWebSocket, useAllEvents } from "@/api/hooks";
import type { ClientEvent } from "@/types/protocol";

interface LogEntry {
  id: string;
  timestamp: number;
  direction: "in" | "out";
  type: string;
  data: unknown;
}

export default function DebugPage() {
  const { isConnected } = useWebSocket();
  const toast = useToast();
  const logEndRef = useRef<HTMLDivElement>(null);

  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [method, setMethod] = useState("health.status");
  const [params, setParams] = useState("{}");
  const [response, setResponse] = useState<unknown>(null);
  const [loading, setLoading] = useState(false);

  // Log incoming events
  const handleEvent = (event: ClientEvent) => {
    const entry: LogEntry = {
      id: crypto.randomUUID(),
      timestamp: Date.now(),
      direction: "in",
      type: event.event,
      data: event.data,
    };
    setLogs((prev) => [...prev.slice(-99), entry]);
  };

  useAllEvents(handleEvent);

  // Auto-scroll logs
  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  const handleSend = async () => {
    if (!isConnected) {
      toast.error("Not connected");
      return;
    }

    let parsedParams: unknown;
    try {
      parsedParams = JSON.parse(params);
    } catch {
      toast.error("Invalid JSON in params");
      return;
    }

    // Log outgoing request
    const outEntry: LogEntry = {
      id: crypto.randomUUID(),
      timestamp: Date.now(),
      direction: "out",
      type: method,
      data: parsedParams,
    };
    setLogs((prev) => [...prev.slice(-99), outEntry]);

    setLoading(true);
    setResponse(null);

    try {
      const ws = getWebSocket();
      const result = await ws.send(method, parsedParams);
      setResponse(result);

      // Log response
      const inEntry: LogEntry = {
        id: crypto.randomUUID(),
        timestamp: Date.now(),
        direction: "in",
        type: `${method}:response`,
        data: result,
      };
      setLogs((prev) => [...prev.slice(-99), inEntry]);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Request failed";
      toast.error(errorMsg);
      setResponse({ error: errorMsg });
    } finally {
      setLoading(false);
    }
  };

  const handleClearLogs = () => {
    setLogs([]);
  };

  const handleDownloadLogs = () => {
    const data = JSON.stringify(logs, null, 2);
    const blob = new Blob([data], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `brainpro-logs-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
        Debug
      </h1>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* RPC Tester */}
        <Card title="RPC Tester">
          <div className="space-y-4">
            <Input
              label="Method"
              value={method}
              onChange={(e) => setMethod(e.target.value)}
              placeholder="e.g., health.status"
            />

            <Textarea
              label="Params (JSON)"
              value={params}
              onChange={(e) => setParams(e.target.value)}
              rows={4}
              className="font-mono text-sm"
            />

            <Button
              onClick={handleSend}
              loading={loading}
              disabled={!isConnected}
            >
              <Send className="h-4 w-4" />
              Send
            </Button>

            {response !== null && (
              <div>
                <h4 className="mb-2 text-sm font-medium text-gray-700 dark:text-gray-300">
                  Response
                </h4>
                <pre className="max-h-64 overflow-auto rounded-lg bg-gray-50 p-3 text-xs dark:bg-gray-900">
                  {JSON.stringify(response, null, 2)}
                </pre>
              </div>
            )}
          </div>
        </Card>

        {/* WebSocket Log */}
        <Card
          title="WebSocket Log"
          actions={
            <div className="flex gap-2">
              <Button variant="ghost" size="sm" onClick={handleDownloadLogs}>
                <Download className="h-4 w-4" />
              </Button>
              <Button variant="ghost" size="sm" onClick={handleClearLogs}>
                <Trash2 className="h-4 w-4" />
              </Button>
            </div>
          }
        >
          <div className="h-96 overflow-y-auto rounded-lg bg-gray-50 p-2 dark:bg-gray-900">
            {logs.length > 0 ? (
              <div className="space-y-1 font-mono text-xs">
                {logs.map((log) => (
                  <div
                    key={log.id}
                    className={`rounded p-1.5 ${
                      log.direction === "out"
                        ? "bg-blue-50 dark:bg-blue-900/20"
                        : "bg-gray-100 dark:bg-gray-800"
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <Badge
                        variant={log.direction === "out" ? "info" : "neutral"}
                      >
                        {log.direction === "out" ? "OUT" : "IN"}
                      </Badge>
                      <span className="font-medium">{log.type}</span>
                      <span className="ml-auto text-gray-400">
                        {new Date(log.timestamp).toLocaleTimeString()}
                      </span>
                    </div>
                    <pre className="mt-1 max-h-24 overflow-auto text-gray-600 dark:text-gray-400">
                      {JSON.stringify(log.data, null, 2)}
                    </pre>
                  </div>
                ))}
                <div ref={logEndRef} />
              </div>
            ) : (
              <p className="py-8 text-center text-sm text-gray-400">
                No messages yet
              </p>
            )}
          </div>
        </Card>
      </div>

      {/* Connection Info */}
      <Card title="Connection Info">
        <div className="grid gap-4 sm:grid-cols-3 text-sm">
          <div>
            <span className="text-gray-500 dark:text-gray-400">Status</span>
            <div className="mt-1">
              <Badge variant={isConnected ? "success" : "error"} dot>
                {isConnected ? "Connected" : "Disconnected"}
              </Badge>
            </div>
          </div>
          <div>
            <span className="text-gray-500 dark:text-gray-400">Session ID</span>
            <p className="mt-1 font-mono text-xs">
              {getWebSocket().getSessionId() ?? "None"}
            </p>
          </div>
          <div>
            <span className="text-gray-500 dark:text-gray-400">
              Log Entries
            </span>
            <p className="mt-1 font-medium">{logs.length}</p>
          </div>
        </div>
      </Card>
    </div>
  );
}
