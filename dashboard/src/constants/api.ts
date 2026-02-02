/**
 * API and WebSocket configuration constants
 */

/** Gateway WebSocket port */
export const GATEWAY_PORT = 18789;

/** Gateway WebSocket host */
export const GATEWAY_HOST = "localhost";

/** Default WebSocket URL */
export const WS_URL = `ws://${GATEWAY_HOST}:${GATEWAY_PORT}/ws`;

/** Request timeout in milliseconds */
export const REQUEST_TIMEOUT_MS = 30000;

/** Reconnect base delay in milliseconds */
export const RECONNECT_BASE_DELAY_MS = 1000;

/** Maximum reconnect delay in milliseconds */
export const RECONNECT_MAX_DELAY_MS = 30000;

/** Reconnect backoff multiplier */
export const RECONNECT_BACKOFF_MULTIPLIER = 2;

/** Maximum number of reconnect attempts */
export const MAX_RECONNECT_ATTEMPTS = 10;

/** Health check interval in milliseconds */
export const HEALTH_CHECK_INTERVAL_MS = 5000;

/** Event buffer limit (max events to retain) */
export const EVENT_BUFFER_LIMIT = 1000;

/** Message history limit per session */
export const MESSAGE_HISTORY_LIMIT = 500;

/** Protocol version */
export const PROTOCOL_VERSION = 1;
