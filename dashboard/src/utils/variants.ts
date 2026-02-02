/**
 * UI variant utilities for consistent styling
 */

import type { SessionStatus } from "@/types/session";
import type { CircuitState, HealthState } from "@/constants/health";

/** Badge variant types */
export type BadgeVariant =
  | "success"
  | "warning"
  | "error"
  | "info"
  | "neutral";

/**
 * Get badge variant for session status
 */
export function getStatusVariant(status: SessionStatus): BadgeVariant {
  switch (status) {
    case "active":
      return "success";
    case "ended":
      return "neutral";
    case "stuck":
    case "awaiting_approval":
    case "awaiting_input":
      return "warning";
    case "paused":
      return "info";
    default:
      return "neutral";
  }
}

/**
 * Get badge variant for policy action
 */
export function getActionVariant(
  action: string,
): "success" | "warning" | "error" | "neutral" {
  switch (action) {
    case "allow":
      return "success";
    case "ask":
      return "warning";
    case "deny":
      return "error";
    default:
      return "neutral";
  }
}

/**
 * Get Tailwind color class for circuit breaker state
 */
export function getCircuitBreakerColor(state: CircuitState): string {
  switch (state) {
    case "closed":
      return "bg-emerald-500";
    case "half_open":
      return "bg-amber-500";
    case "open":
      return "bg-red-500";
    default:
      return "bg-gray-500";
  }
}

/**
 * Get badge variant for circuit breaker state
 */
export function getCircuitBreakerVariant(state: CircuitState): BadgeVariant {
  switch (state) {
    case "closed":
      return "success";
    case "half_open":
      return "warning";
    case "open":
      return "error";
    default:
      return "neutral";
  }
}

/**
 * Get Tailwind color class for health state (dot indicator)
 */
export function getHealthStateColor(state: HealthState): string {
  switch (state) {
    case "healthy":
      return "bg-emerald-500";
    case "degraded":
      return "bg-amber-500";
    case "unhealthy":
      return "bg-red-500";
    default:
      return "bg-gray-500";
  }
}

/**
 * Get badge variant for health state
 */
export function getHealthStateVariant(state: HealthState): BadgeVariant {
  switch (state) {
    case "healthy":
      return "success";
    case "degraded":
      return "warning";
    case "unhealthy":
      return "error";
    default:
      return "neutral";
  }
}
