import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Plus, Trash2, ExternalLink } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { Tabs, TabPanel } from "@/components/ui/Tabs";
import { useToast } from "@/components/ui";
import { api } from "@/api/client";
import { useWebSocket } from "@/api/hooks";
import { useSessionStore, useAllSessions } from "@/store/sessionStore";
import { sessionDetailPath } from "@/constants/routes";
import { formatCost } from "@/types/cost";
import { getStatusVariant, truncateId } from "@/utils";

const statusTabs = [
  { key: "all", label: "All" },
  { key: "active", label: "Active" },
  { key: "ended", label: "Ended" },
  { key: "stuck", label: "Stuck" },
];

export default function SessionsPage() {
  const { isConnected } = useWebSocket();
  const sessions = useAllSessions();
  const addSession = useSessionStore((s) => s.addSession);
  const removeSession = useSessionStore((s) => s.removeSession);
  const setCurrentSession = useSessionStore((s) => s.setCurrentSession);
  const toast = useToast();

  const [activeTab, setActiveTab] = useState("all");
  const [loading, setLoading] = useState(false);

  // Fetch sessions on mount
  useEffect(() => {
    if (!isConnected) return;

    const fetchSessions = async () => {
      try {
        const list = await api.session.list();
        for (const session of list) {
          addSession(session);
        }
      } catch (error) {
        console.warn("Failed to fetch sessions:", error);
      }
    };

    fetchSessions();
  }, [isConnected, addSession]);

  const filteredSessions = sessions.filter((s) => {
    if (activeTab === "all") return true;
    return s.status === activeTab;
  });

  const handleCreateSession = async () => {
    if (!isConnected) {
      toast.error("Not connected to gateway");
      return;
    }

    setLoading(true);
    try {
      const session = await api.session.create();
      addSession(session);
      setCurrentSession(session.id);
      toast.success("Session created");
    } catch {
      toast.error("Failed to create session");
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteSession = async (sessionId: string) => {
    // TODO: Add API call when available
    removeSession(sessionId);
    toast.info("Session removed");
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
          Sessions
        </h1>
        <Button
          onClick={handleCreateSession}
          loading={loading}
          disabled={!isConnected}
        >
          <Plus className="h-4 w-4" />
          New Session
        </Button>
      </div>

      <Tabs items={statusTabs} activeKey={activeTab} onChange={setActiveTab} />

      <TabPanel active={true}>
        {filteredSessions.length > 0 ? (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {filteredSessions.map((session) => (
              <Card key={session.id} padding="md">
                <div className="flex items-start justify-between">
                  <div>
                    <code className="font-mono text-sm">
                      {truncateId(session.id)}
                    </code>
                    <Badge
                      className="ml-2"
                      variant={getStatusVariant(session.status)}
                    >
                      {session.status}
                    </Badge>
                  </div>
                  <div className="flex gap-1">
                    <Link
                      to={sessionDetailPath(session.id)}
                      className="rounded p-1 text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-gray-800"
                    >
                      <ExternalLink className="h-4 w-4" />
                    </Link>
                    <button
                      type="button"
                      onClick={() => handleDeleteSession(session.id)}
                      aria-label="Delete session"
                      className="rounded p-1 text-gray-400 hover:bg-red-100 hover:text-red-600 dark:hover:bg-red-900/20"
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  </div>
                </div>

                <div className="mt-3 grid grid-cols-2 gap-2 text-sm">
                  <div>
                    <span className="text-gray-500 dark:text-gray-400">
                      Turns:
                    </span>
                    <span className="ml-1">{session.turn_count}</span>
                  </div>
                  <div>
                    <span className="text-gray-500 dark:text-gray-400">
                      Cost:
                    </span>
                    <span className="ml-1">
                      {formatCost(session.total_cost_usd)}
                    </span>
                  </div>
                  <div className="col-span-2">
                    <span className="text-gray-500 dark:text-gray-400">
                      Created:
                    </span>
                    <span className="ml-1 text-xs">
                      {new Date(session.created_at).toLocaleString()}
                    </span>
                  </div>
                </div>
              </Card>
            ))}
          </div>
        ) : (
          <Card>
            <p className="text-center text-gray-500 dark:text-gray-400">
              No sessions found. Create one to get started.
            </p>
          </Card>
        )}
      </TabPanel>
    </div>
  );
}
