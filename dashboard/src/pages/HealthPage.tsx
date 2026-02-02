import { useEffect } from "react";
import { RefreshCw, CheckCircle, AlertTriangle, XCircle } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { api } from "@/api/client";
import { useWebSocket } from "@/api/hooks";
import { useHealthStore, useAllBackends } from "@/store/healthStore";
import { HEALTH_CHECK_INTERVAL_MS } from "@/constants/api";
import { CIRCUIT_STATES } from "@/constants/health";
import {
  formatUptime,
  getCircuitBreakerColor,
  getHealthStateVariant,
} from "@/utils";

function CircuitBreakerVisual({ state }: { state: string }) {
  const states = [
    CIRCUIT_STATES.CLOSED,
    CIRCUIT_STATES.HALF_OPEN,
    CIRCUIT_STATES.OPEN,
  ];
  const currentIdx = states.indexOf(state as (typeof states)[number]);

  return (
    <div className="flex items-center gap-2">
      {states.map((s, idx) => (
        <div key={s} className="flex items-center gap-1">
          <div
            className={`h-3 w-3 rounded-full ${
              idx === currentIdx
                ? getCircuitBreakerColor(s)
                : "bg-gray-200 dark:bg-gray-700"
            }`}
          />
          {idx < states.length - 1 && (
            <div className="h-0.5 w-4 bg-gray-200 dark:bg-gray-700" />
          )}
        </div>
      ))}
      <span className="ml-2 text-xs font-medium capitalize">
        {state.replace("_", "-")}
      </span>
    </div>
  );
}

export default function HealthPage() {
  const { isConnected } = useWebSocket();
  const {
    status,
    uptimeSecs,
    activeSessions,
    pendingRequests,
    loading,
    lastCheck,
    setHealthStatus,
    setLoading,
  } = useHealthStore();
  const backends = useAllBackends();

  const fetchHealth = async () => {
    if (!isConnected) return;
    setLoading(true);
    try {
      const health = await api.health.status();
      setHealthStatus(health);
    } catch (error) {
      console.warn("Failed to fetch health status:", error);
    }
  };

  useEffect(() => {
    if (!isConnected) return;

    fetchHealth();
    const interval = setInterval(fetchHealth, HEALTH_CHECK_INTERVAL_MS);
    return () => clearInterval(interval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isConnected]);

  const StatusIcon =
    status === "healthy"
      ? CheckCircle
      : status === "degraded"
        ? AlertTriangle
        : XCircle;
  const statusColor =
    status === "healthy"
      ? "text-emerald-500"
      : status === "degraded"
        ? "text-amber-500"
        : "text-red-500";

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
          Health
        </h1>
        <Button variant="secondary" onClick={fetchHealth} loading={loading}>
          <RefreshCw className="h-4 w-4" />
          Refresh
        </Button>
      </div>

      {/* Overall status */}
      <Card>
        <div className="flex items-center gap-6">
          <div className="flex items-center gap-3">
            <StatusIcon className={`h-10 w-10 ${statusColor}`} />
            <div>
              <h2 className="text-xl font-semibold capitalize text-gray-900 dark:text-gray-100">
                {status}
              </h2>
              <p className="text-sm text-gray-500 dark:text-gray-400">
                System Status
              </p>
            </div>
          </div>

          <div className="ml-auto grid grid-cols-3 gap-8 text-center">
            <div>
              <p className="text-2xl font-semibold text-gray-900 dark:text-gray-100">
                {formatUptime(uptimeSecs, true)}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400">Uptime</p>
            </div>
            <div>
              <p className="text-2xl font-semibold text-gray-900 dark:text-gray-100">
                {activeSessions}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Active Sessions
              </p>
            </div>
            <div>
              <p className="text-2xl font-semibold text-gray-900 dark:text-gray-100">
                {pendingRequests}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Pending Requests
              </p>
            </div>
          </div>
        </div>

        {lastCheck && (
          <p className="mt-4 text-xs text-gray-400">
            Last updated: {new Date(lastCheck).toLocaleTimeString()}
          </p>
        )}
      </Card>

      {/* Backend cards */}
      <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
        Backends
      </h2>
      <div className="grid gap-4 sm:grid-cols-2">
        {backends.map((backend) => (
          <Card key={backend.backend} title={backend.backend}>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <span className="text-sm text-gray-500 dark:text-gray-400">
                  Health
                </span>
                <Badge
                  variant={getHealthStateVariant(backend.state)}
                  dot
                >
                  {backend.state}
                </Badge>
              </div>

              <div>
                <span className="text-sm text-gray-500 dark:text-gray-400">
                  Circuit Breaker
                </span>
                <div className="mt-1">
                  <CircuitBreakerVisual state={backend.circuit.state} />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <span className="text-gray-500 dark:text-gray-400">
                    Successes
                  </span>
                  <p className="font-medium text-emerald-600 dark:text-emerald-400">
                    {backend.circuit.total_successes}
                  </p>
                </div>
                <div>
                  <span className="text-gray-500 dark:text-gray-400">
                    Failures
                  </span>
                  <p className="font-medium text-red-600 dark:text-red-400">
                    {backend.circuit.total_failures}
                  </p>
                </div>
                <div>
                  <span className="text-gray-500 dark:text-gray-400">
                    Rejections
                  </span>
                  <p className="font-medium">
                    {backend.circuit.total_rejections}
                  </p>
                </div>
                {backend.avg_latency_ms && (
                  <div>
                    <span className="text-gray-500 dark:text-gray-400">
                      Avg Latency
                    </span>
                    <p className="font-medium">{backend.avg_latency_ms}ms</p>
                  </div>
                )}
              </div>

              {backend.last_error && (
                <div className="rounded bg-red-50 p-2 text-xs text-red-700 dark:bg-red-900/20 dark:text-red-300">
                  {backend.last_error}
                </div>
              )}
            </div>
          </Card>
        ))}

        {backends.length === 0 && (
          <Card className="col-span-full">
            <p className="text-center text-gray-500 dark:text-gray-400">
              No backend data available
            </p>
          </Card>
        )}
      </div>
    </div>
  );
}
