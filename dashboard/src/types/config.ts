/**
 * Configuration types
 */

import type { BackendName } from "@/constants/health";

/** Permission rule action */
export type PermissionAction = "allow" | "ask" | "deny";

/** Permission rule */
export interface PermissionRule {
  tool: string;
  action: PermissionAction;
  pattern?: string;
  reason?: string;
}

/** Backend configuration */
export interface BackendConfig {
  name: BackendName;
  api_url: string;
  default_model?: string;
  enabled: boolean;
}

/** Context limits configuration */
export interface ContextLimits {
  max_context_tokens: number;
  max_output_tokens: number;
  reserve_tokens: number;
}

/** Agent loop configuration */
export interface AgentConfig {
  max_turns: number;
  max_iterations_per_turn: number;
  doom_loop_threshold: number;
}

/** Full configuration object */
export interface Config {
  persona: string;
  model: string;
  backend: BackendName;
  backends: Record<string, BackendConfig>;
  policy: {
    mode: string;
    rules: PermissionRule[];
  };
  context: ContextLimits;
  agent: AgentConfig;
  cost: {
    enabled: boolean;
    warn_threshold_usd?: number;
    display_in_stats: boolean;
  };
  circuit_breaker: {
    enabled: boolean;
    failure_threshold: number;
    recovery_timeout_secs: number;
    half_open_probes: number;
  };
}
