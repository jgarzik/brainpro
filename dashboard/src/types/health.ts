/**
 * Health and circuit breaker types matching src/circuit_breaker.rs
 */

import type {
  CircuitState,
  HealthState,
  BackendName,
} from "@/constants/health";

/** Circuit breaker statistics */
export interface CircuitBreakerStats {
  name: string;
  state: CircuitState;
  consecutive_failures: number;
  total_failures: number;
  total_successes: number;
  total_rejections: number;
}

/** Provider health status */
export interface ProviderHealth {
  backend: BackendName;
  state: HealthState;
  circuit: CircuitBreakerStats;
  avg_latency_ms?: number;
  success_rate?: number;
  last_error?: string;
  last_success_at?: number;
}

/** Overall health status response */
export interface HealthStatus {
  status: HealthState;
  uptime_secs: number;
  active_sessions: number;
  pending_requests: number;
  backends: Record<string, ProviderHealth>;
}

/** Circuit breaker configuration */
export interface CircuitBreakerConfig {
  failure_threshold: number;
  recovery_timeout_secs: number;
  half_open_probes: number;
  enabled: boolean;
}
