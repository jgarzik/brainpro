/**
 * Event type constants matching src/events.rs and src/protocol/client.rs
 */

// Agent streaming events (from protocol/client.rs)
export const AGENT_EVENTS = {
  THINKING: "agent.thinking",
  TOKEN_DELTA: "agent.token_delta",
  TOOL_CALL: "agent.tool_call",
  TOOL_RESULT: "agent.tool_result",
  MESSAGE: "agent.message",
  DONE: "agent.done",
  ERROR: "agent.error",
  AWAITING_APPROVAL: "agent.awaiting_approval",
  AWAITING_INPUT: "agent.awaiting_input",
} as const;

// System events
export const SYSTEM_EVENTS = {
  PRESENCE_UPDATE: "presence.update",
  HEALTH_TICK: "health.tick",
  CRON_FIRED: "cron.fired",
} as const;

// Event subsystems (from events.rs)
export const SUBSYSTEMS = {
  MODEL: "model",
  MESSAGE: "message",
  SESSION: "session",
  TOOL: "tool",
  QUEUE: "queue",
  RUN: "run",
  SYSTEM: "system",
  CIRCUIT: "circuit",
  POLICY: "policy",
  WEBHOOK: "webhook",
  PLUGIN: "plugin",
  COST: "cost",
} as const;

export type Subsystem = (typeof SUBSYSTEMS)[keyof typeof SUBSYSTEMS];

// Model event types
export const MODEL_EVENT_TYPES = {
  MODEL_USAGE: "model_usage",
  MODEL_ERROR: "model_error",
  MODEL_STREAM_START: "model_stream_start",
  MODEL_STREAM_END: "model_stream_end",
} as const;

// Message event types
export const MESSAGE_EVENT_TYPES = {
  MESSAGE_QUEUED: "message_queued",
  MESSAGE_PROCESSING: "message_processing",
  MESSAGE_COMPLETE: "message_complete",
} as const;

// Session event types
export const SESSION_EVENT_TYPES = {
  SESSION_CREATED: "session_created",
  SESSION_RESUMED: "session_resumed",
  SESSION_PAUSED: "session_paused",
  SESSION_ENDED: "session_ended",
  SESSION_STUCK: "session_stuck",
} as const;

// Tool event types
export const TOOL_EVENT_TYPES = {
  TOOL_INVOKED: "tool_invoked",
  TOOL_COMPLETED: "tool_completed",
  TOOL_TIMEOUT: "tool_timeout",
  TOOL_DENIED: "tool_denied",
} as const;

// Queue event types
export const QUEUE_EVENT_TYPES = {
  QUEUE_LANE_CREATED: "queue_lane_created",
  QUEUE_LANE_ACTIVE: "queue_lane_active",
  QUEUE_REQUEST_ENQUEUED: "queue_request_enqueued",
  QUEUE_REQUEST_DEQUEUED: "queue_request_dequeued",
} as const;

// Run event types
export const RUN_EVENT_TYPES = {
  RUN_ATTEMPT: "run_attempt",
  RUN_COMPLETE: "run_complete",
  RUN_DOOM_LOOP_DETECTED: "run_doom_loop_detected",
} as const;

// System event types
export const SYSTEM_EVENT_TYPES = {
  HEARTBEAT: "heartbeat",
  CONFIG_RELOAD: "config_reload",
  HEALTH_CHECK: "health_check",
} as const;

// Circuit breaker event types
export const CIRCUIT_EVENT_TYPES = {
  CIRCUIT_OPENED: "circuit_opened",
  CIRCUIT_HALF_OPEN: "circuit_half_open",
  CIRCUIT_CLOSED: "circuit_closed",
} as const;

// Policy event types
export const POLICY_EVENT_TYPES = {
  POLICY_DECISION: "policy_decision",
  POLICY_VIOLATION: "policy_violation",
} as const;

// Webhook event types
export const WEBHOOK_EVENT_TYPES = {
  WEBHOOK_DELIVERY_STARTED: "webhook_delivery_started",
  WEBHOOK_DELIVERY_COMPLETED: "webhook_delivery_completed",
  WEBHOOK_DELIVERY_FAILED: "webhook_delivery_failed",
} as const;

// Cost event types
export const COST_EVENT_TYPES = {
  COST_THRESHOLD_WARNING: "cost_threshold_warning",
  COST_BUDGET_EXCEEDED: "cost_budget_exceeded",
} as const;

// All event types combined
export const EVENT_TYPES = {
  ...MODEL_EVENT_TYPES,
  ...MESSAGE_EVENT_TYPES,
  ...SESSION_EVENT_TYPES,
  ...TOOL_EVENT_TYPES,
  ...QUEUE_EVENT_TYPES,
  ...RUN_EVENT_TYPES,
  ...SYSTEM_EVENT_TYPES,
  ...CIRCUIT_EVENT_TYPES,
  ...POLICY_EVENT_TYPES,
  ...WEBHOOK_EVENT_TYPES,
  ...COST_EVENT_TYPES,
} as const;

export type EventType = (typeof EVENT_TYPES)[keyof typeof EVENT_TYPES];
