import { useEffect } from "react";
import { Activity, Layers, DollarSign, Zap } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { useHealthStore, useAllBackends } from "@/store/healthStore";
import { useAllSessions } from "@/store/sessionStore";
import { useCostStore } from "@/store/costStore";
import { useRecentEvents } from "@/store/eventStore";
import { api } from "@/api/client";
import { useWebSocket } from "@/api/hooks";
import { formatCost } from "@/types/cost";
import { HEALTH_CHECK_INTERVAL_MS } from "@/constants/api";
import {
  formatUptime,
  getCircuitBreakerVariant,
  getHealthStateColor,
} from "@/utils";

function StatCard({
  title,
  value,
  icon: Icon,
  description,
}: {
  title: string;
  value: string | number;
  icon: React.ComponentType<{ className?: string }>;
  description?: string;
}) {
  return (
    <Card>
      <div className="flex items-start justify-between">
        <div>
          <p className="text-sm font-medium text-gray-500 dark:text-gray-400">
            {title}
          </p>
          <p className="mt-1 text-2xl font-semibold text-gray-900 dark:text-gray-100">
            {value}
          </p>
          {description && (
            <p className="mt-1 text-xs text-gray-400 dark:text-gray-500">
              {description}
            </p>
          )}
        </div>
        <div className="rounded-lg bg-blue-100 p-2 dark:bg-blue-900/30">
          <Icon className="h-5 w-5 text-blue-600 dark:text-blue-400" />
        </div>
      </div>
    </Card>
  );
}

export default function OverviewPage() {
  const { isConnected } = useWebSocket();
  const { status, uptimeSecs, activeSessions, setHealthStatus, setLoading } =
    useHealthStore();
  const backends = useAllBackends();
  const sessions = useAllSessions();
  const totalCost = useCostStore((s) => s.totalCostUsd);
  const recentEvents = useRecentEvents(5);

  // Fetch health status periodically
  useEffect(() => {
    if (!isConnected) return;

    const fetchHealth = async () => {
      setLoading(true);
      try {
        const health = await api.health.status();
        setHealthStatus(health);
      } catch (error) {
        console.warn("Failed to fetch health status:", error);
      }
    };

    fetchHealth();
    const interval = setInterval(fetchHealth, HEALTH_CHECK_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [isConnected, setHealthStatus, setLoading]);

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
        Overview
      </h1>

      {/* Stats grid */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="System Status"
          value={status.charAt(0).toUpperCase() + status.slice(1)}
          icon={Activity}
          description={`Uptime: ${formatUptime(uptimeSecs)}`}
        />
        <StatCard
          title="Active Sessions"
          value={activeSessions}
          icon={Layers}
          description={`${sessions.length} total sessions`}
        />
        <StatCard
          title="Total Cost"
          value={formatCost(totalCost)}
          icon={DollarSign}
        />
        <StatCard
          title="Backends"
          value={`${backends.filter((b) => b.state === "healthy").length}/${backends.length}`}
          icon={Zap}
          description="Healthy backends"
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Backend health */}
        <Card title="Backend Health">
          {backends.length > 0 ? (
            <div className="space-y-3">
              {backends.map((backend) => (
                <div
                  key={backend.backend}
                  className="flex items-center justify-between rounded-lg border border-gray-200 p-3 dark:border-gray-700"
                >
                  <div className="flex items-center gap-3">
                    <div
                      className={`h-2.5 w-2.5 rounded-full ${getHealthStateColor(backend.state)}`}
                    />
                    <span className="font-medium">{backend.backend}</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge
                      variant={getCircuitBreakerVariant(backend.circuit.state)}
                    >
                      {backend.circuit.state}
                    </Badge>
                    {backend.avg_latency_ms && (
                      <span className="text-xs text-gray-400">
                        {backend.avg_latency_ms}ms
                      </span>
                    )}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-gray-500 dark:text-gray-400">
              No backend data available
            </p>
          )}
        </Card>

        {/* Recent events */}
        <Card title="Recent Events">
          {recentEvents.length > 0 ? (
            <div className="space-y-2">
              {recentEvents.map((event) => (
                <div
                  key={event.seq}
                  className="flex items-center justify-between rounded border border-gray-200 p-2 text-sm dark:border-gray-700"
                >
                  <div className="flex items-center gap-2">
                    <Badge variant="info">{event.subsystem}</Badge>
                    <span className="font-mono text-xs">{event.type}</span>
                  </div>
                  <span className="text-xs text-gray-400">
                    {new Date(event.timestamp_ms).toLocaleTimeString()}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-gray-500 dark:text-gray-400">
              No recent events
            </p>
          )}
        </Card>
      </div>
    </div>
  );
}
