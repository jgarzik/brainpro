/**
 * Event types matching src/events.rs
 */

import type { Subsystem, EventType } from "@/constants/events";

/** Run context for event association */
export interface RunContext {
  session_id: string;
  agent_id?: string;
  turn_number?: number;
}

/** Base event structure */
export interface BaseEvent {
  seq: number;
  timestamp_ms: number;
  subsystem: Subsystem;
  type: EventType;
  run_context?: RunContext;
}

// Model events
export interface ModelUsageEvent extends BaseEvent {
  type: "model_usage";
  backend: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
  duration_ms: number;
}

export interface ModelErrorEvent extends BaseEvent {
  type: "model_error";
  backend: string;
  model: string;
  error_code: string;
  error_message: string;
}

export interface ModelStreamStartEvent extends BaseEvent {
  type: "model_stream_start";
  backend: string;
  model: string;
  request_id: string;
}

export interface ModelStreamEndEvent extends BaseEvent {
  type: "model_stream_end";
  backend: string;
  model: string;
  request_id: string;
  total_tokens: number;
}

// Session events
export interface SessionCreatedEvent extends BaseEvent {
  type: "session_created";
  session_id: string;
  agent_id?: string;
}

export interface SessionResumedEvent extends BaseEvent {
  type: "session_resumed";
  session_id: string;
}

export interface SessionPausedEvent extends BaseEvent {
  type: "session_paused";
  session_id: string;
  reason: string;
}

export interface SessionEndedEvent extends BaseEvent {
  type: "session_ended";
  session_id: string;
  total_turns: number;
  total_cost_usd: number;
}

export interface SessionStuckEvent extends BaseEvent {
  type: "session_stuck";
  session_id: string;
  stuck_reason: string;
  stuck_tool?: string;
}

// Tool events
export interface ToolInvokedEvent extends BaseEvent {
  type: "tool_invoked";
  session_id: string;
  tool_name: string;
  tool_call_id: string;
  args_preview: string;
}

export interface ToolCompletedEvent extends BaseEvent {
  type: "tool_completed";
  session_id: string;
  tool_name: string;
  tool_call_id: string;
  success: boolean;
  duration_ms: number;
}

export interface ToolTimeoutEvent extends BaseEvent {
  type: "tool_timeout";
  session_id: string;
  tool_name: string;
  tool_call_id: string;
  timeout_ms: number;
}

export interface ToolDeniedEvent extends BaseEvent {
  type: "tool_denied";
  session_id: string;
  tool_name: string;
  reason: string;
  policy_rule?: string;
}

// Run events
export interface RunAttemptEvent extends BaseEvent {
  type: "run_attempt";
  session_id: string;
  turn_number: number;
  iteration: number;
}

export interface RunCompleteEvent extends BaseEvent {
  type: "run_complete";
  session_id: string;
  turn_number: number;
  iterations: number;
  tool_uses: number;
  tokens_used: number;
}

export interface RunDoomLoopEvent extends BaseEvent {
  type: "run_doom_loop_detected";
  session_id: string;
  turn_number: number;
  tool_name: string;
  repeat_count: number;
}

// Circuit events
export interface CircuitOpenedEvent extends BaseEvent {
  type: "circuit_opened";
  backend: string;
  failure_count: number;
  recovery_timeout_secs: number;
}

export interface CircuitHalfOpenEvent extends BaseEvent {
  type: "circuit_half_open";
  backend: string;
  probes_remaining: number;
}

export interface CircuitClosedEvent extends BaseEvent {
  type: "circuit_closed";
  backend: string;
  success_probes: number;
}

// Policy events
export interface PolicyDecisionEvent extends BaseEvent {
  type: "policy_decision";
  tool_name: string;
  decision: string;
  rule?: string;
  agent_id?: string;
}

export interface PolicyViolationEvent extends BaseEvent {
  type: "policy_violation";
  tool_name: string;
  violation: string;
  agent_id?: string;
}

// System events
export interface HeartbeatEvent extends BaseEvent {
  type: "heartbeat";
  uptime_secs: number;
  active_sessions: number;
  pending_requests: number;
}

export interface ConfigReloadEvent extends BaseEvent {
  type: "config_reload";
  changed_keys: string[];
}

export interface HealthCheckEvent extends BaseEvent {
  type: "health_check";
  status: string;
  backends: Record<string, string>;
}

// Cost events
export interface CostThresholdWarningEvent extends BaseEvent {
  type: "cost_threshold_warning";
  session_id: string;
  current_cost_usd: number;
  threshold_usd: number;
}

export interface CostBudgetExceededEvent extends BaseEvent {
  type: "cost_budget_exceeded";
  session_id: string;
  budget_usd: number;
  actual_usd: number;
}

/** Union of all event types */
export type Event =
  | ModelUsageEvent
  | ModelErrorEvent
  | ModelStreamStartEvent
  | ModelStreamEndEvent
  | SessionCreatedEvent
  | SessionResumedEvent
  | SessionPausedEvent
  | SessionEndedEvent
  | SessionStuckEvent
  | ToolInvokedEvent
  | ToolCompletedEvent
  | ToolTimeoutEvent
  | ToolDeniedEvent
  | RunAttemptEvent
  | RunCompleteEvent
  | RunDoomLoopEvent
  | CircuitOpenedEvent
  | CircuitHalfOpenEvent
  | CircuitClosedEvent
  | PolicyDecisionEvent
  | PolicyViolationEvent
  | HeartbeatEvent
  | ConfigReloadEvent
  | HealthCheckEvent
  | CostThresholdWarningEvent
  | CostBudgetExceededEvent;
