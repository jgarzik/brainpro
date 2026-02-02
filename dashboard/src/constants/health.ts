/**
 * Health and circuit breaker constants matching src/circuit_breaker.rs
 */

export const CIRCUIT_STATES = {
  CLOSED: "closed",
  OPEN: "open",
  HALF_OPEN: "half_open",
} as const;

export type CircuitState = (typeof CIRCUIT_STATES)[keyof typeof CIRCUIT_STATES];

export const HEALTH_STATES = {
  HEALTHY: "healthy",
  DEGRADED: "degraded",
  UNHEALTHY: "unhealthy",
} as const;

export type HealthState = (typeof HEALTH_STATES)[keyof typeof HEALTH_STATES];

export const BACKENDS = {
  VENICE: "venice",
  OPENAI: "openai",
  ANTHROPIC: "claude",
  OLLAMA: "ollama",
} as const;

export type BackendName = (typeof BACKENDS)[keyof typeof BACKENDS];

/** Default circuit breaker thresholds */
export const CIRCUIT_BREAKER_DEFAULTS = {
  FAILURE_THRESHOLD: 5,
  RECOVERY_TIMEOUT_SECS: 30,
  HALF_OPEN_PROBES: 3,
} as const;

/** Health check decision results */
export const CIRCUIT_DECISIONS = {
  ALLOW: "allow",
  REJECT: "reject",
  PROBE: "probe",
} as const;

export type CircuitDecision =
  (typeof CIRCUIT_DECISIONS)[keyof typeof CIRCUIT_DECISIONS];
