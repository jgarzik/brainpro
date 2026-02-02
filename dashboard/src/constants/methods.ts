/**
 * RPC method constants matching src/protocol/client.rs
 */

export const METHODS = {
  /** Send a chat message */
  CHAT_SEND: "chat.send",
  /** Create a new session */
  SESSION_CREATE: "session.create",
  /** List all sessions */
  SESSION_LIST: "session.list",
  /** Get session details */
  SESSION_GET: "session.get",
  /** Approve a tool call */
  TOOL_APPROVE: "tool.approve",
  /** Resume a paused turn */
  TURN_RESUME: "turn.resume",
  /** Add a cron job */
  CRON_ADD: "cron.add",
  /** Remove a cron job */
  CRON_REMOVE: "cron.remove",
  /** List cron jobs */
  CRON_LIST: "cron.list",
  /** Pair a device */
  DEVICE_PAIR: "device.pair",
  /** Get health status */
  HEALTH_STATUS: "health.status",
} as const;

export type Method = (typeof METHODS)[keyof typeof METHODS];
