/**
 * WebSocket client for Gateway communication
 */

import {
  WS_URL,
  REQUEST_TIMEOUT_MS,
  RECONNECT_BASE_DELAY_MS,
  RECONNECT_MAX_DELAY_MS,
  RECONNECT_BACKOFF_MULTIPLIER,
  MAX_RECONNECT_ATTEMPTS,
  PROTOCOL_VERSION,
} from "@/constants/api";
import type {
  ClientRequest,
  ClientResponse,
  ClientEvent,
  ChallengeMessage,
  WelcomeMessage,
  PolicyInfo,
} from "@/types/protocol";

/** Connection state */
export type ConnectionState =
  | "disconnected"
  | "connecting"
  | "authenticating"
  | "connected"
  | "error";

/** Event handler type */
export type EventHandler = (event: ClientEvent) => void;

/** Pending request tracking */
interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timeout: ReturnType<typeof setTimeout>;
}

/** WebSocket client class */
export class BrainproWebSocket {
  private ws: WebSocket | null = null;
  private url: string;
  private password: string | null = null;
  private deviceId: string;

  private state: ConnectionState = "disconnected";
  private sessionId: string | null = null;
  private policy: PolicyInfo | null = null;

  private requestId = 0;
  private pendingRequests = new Map<string, PendingRequest>();
  private eventHandlers = new Map<string, Set<EventHandler>>();
  private globalEventHandlers = new Set<EventHandler>();

  private reconnectAttempts = 0;
  private reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
  private shouldReconnect = true;

  private stateChangeHandlers = new Set<(state: ConnectionState) => void>();

  constructor(url: string = WS_URL) {
    this.url = url;
    this.deviceId = this.generateDeviceId();
  }

  /** Get current connection state */
  getState(): ConnectionState {
    return this.state;
  }

  /** Get session ID after connected */
  getSessionId(): string | null {
    return this.sessionId;
  }

  /** Get policy info after connected */
  getPolicy(): PolicyInfo | null {
    return this.policy;
  }

  /** Subscribe to state changes */
  onStateChange(handler: (state: ConnectionState) => void): () => void {
    this.stateChangeHandlers.add(handler);
    return () => this.stateChangeHandlers.delete(handler);
  }

  /** Connect to the gateway */
  async connect(password?: string): Promise<void> {
    if (
      this.ws &&
      (this.state === "connected" || this.state === "connecting")
    ) {
      return;
    }

    this.password = password ?? null;
    this.shouldReconnect = true;

    return new Promise((resolve, reject) => {
      this.setState("connecting");

      try {
        this.ws = new WebSocket(this.url);

        this.ws.onopen = () => {
          this.reconnectAttempts = 0;
          this.sendHello();
        };

        this.ws.onmessage = (event) => {
          this.handleMessage(event.data as string, resolve);
        };

        this.ws.onerror = () => {
          this.setState("error");
          reject(new Error("WebSocket error"));
        };

        this.ws.onclose = () => {
          this.handleClose();
        };
      } catch (error) {
        this.setState("error");
        reject(error);
      }
    });
  }

  /** Disconnect from the gateway */
  disconnect(): void {
    this.shouldReconnect = false;
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.setState("disconnected");
    this.clearPendingRequests();
  }

  /** Send a request and wait for response */
  async send<T>(method: string, params: unknown = {}): Promise<T> {
    if (this.state !== "connected" || !this.ws) {
      throw new Error("Not connected");
    }

    const id = this.nextRequestId();
    const request: ClientRequest = {
      type: "req",
      id,
      method,
      params,
    };

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`Request timeout: ${method}`));
      }, REQUEST_TIMEOUT_MS);

      this.pendingRequests.set(id, {
        resolve: resolve as (value: unknown) => void,
        reject,
        timeout,
      });
      this.ws!.send(JSON.stringify(request));
    });
  }

  /** Subscribe to a specific event type */
  on(event: string, handler: EventHandler): () => void {
    let handlers = this.eventHandlers.get(event);
    if (!handlers) {
      handlers = new Set();
      this.eventHandlers.set(event, handlers);
    }
    handlers.add(handler);
    return () => handlers.delete(handler);
  }

  /** Subscribe to all events */
  onAny(handler: EventHandler): () => void {
    this.globalEventHandlers.add(handler);
    return () => this.globalEventHandlers.delete(handler);
  }

  private setState(state: ConnectionState): void {
    this.state = state;
    for (const handler of this.stateChangeHandlers) {
      handler(state);
    }
  }

  private generateDeviceId(): string {
    const stored = localStorage.getItem("brainpro_device_id");
    if (stored) {
      return stored;
    }
    const id = `dashboard-${crypto.randomUUID()}`;
    localStorage.setItem("brainpro_device_id", id);
    return id;
  }

  private nextRequestId(): string {
    return `${++this.requestId}`;
  }

  private sendHello(): void {
    this.setState("authenticating");
    const hello = {
      type: "hello",
      role: "operator",
      device_id: this.deviceId,
      caps: {
        tools: [],
        protocol_version: PROTOCOL_VERSION,
      },
    };
    this.ws?.send(JSON.stringify(hello));
  }

  private handleMessage(
    data: string,
    connectResolve?: (value: void | PromiseLike<void>) => void,
  ): void {
    let message: Record<string, unknown>;
    try {
      message = JSON.parse(data) as Record<string, unknown>;
    } catch {
      console.error("Failed to parse WebSocket message:", data);
      return;
    }

    const type = message["type"] as string | undefined;

    switch (type) {
      case "challenge":
        this.handleChallenge(message as unknown as ChallengeMessage);
        break;
      case "welcome":
        this.handleWelcome(
          message as unknown as WelcomeMessage,
          connectResolve,
        );
        break;
      case "res":
        this.handleResponse(message as unknown as ClientResponse);
        break;
      case "event":
        this.handleEvent(message as unknown as ClientEvent);
        break;
      default:
        console.warn("Unknown message type:", type);
    }
  }

  private handleChallenge(message: ChallengeMessage): void {
    // Sign the challenge with password if provided
    const signature = this.signChallenge(message.nonce);
    const auth = {
      type: "auth",
      signature,
    };
    this.ws?.send(JSON.stringify(auth));
  }

  private signChallenge(nonce: string): string {
    // Simple signature: hash of nonce + password
    // In production, use proper HMAC
    if (!this.password) {
      return nonce;
    }
    return btoa(`${nonce}:${this.password}`);
  }

  private handleWelcome(
    message: WelcomeMessage,
    resolve?: (value: void | PromiseLike<void>) => void,
  ): void {
    this.sessionId = message.session_id;
    this.policy = message.policy;
    this.setState("connected");
    resolve?.();
  }

  private handleResponse(message: ClientResponse): void {
    const pending = this.pendingRequests.get(message.id);
    if (!pending) {
      console.warn("Received response for unknown request:", message.id);
      return;
    }

    this.pendingRequests.delete(message.id);
    clearTimeout(pending.timeout);

    if (message.ok) {
      pending.resolve(message.payload);
    } else {
      pending.reject(new Error(message.error?.message ?? "Request failed"));
    }
  }

  private handleEvent(event: ClientEvent): void {
    // Call specific event handlers
    const handlers = this.eventHandlers.get(event.event);
    if (handlers) {
      for (const handler of handlers) {
        handler(event);
      }
    }

    // Call global event handlers
    for (const handler of this.globalEventHandlers) {
      handler(event);
    }
  }

  private handleClose(): void {
    this.ws = null;
    this.clearPendingRequests();

    if (this.state === "connected") {
      this.setState("disconnected");
    }

    if (
      this.shouldReconnect &&
      this.reconnectAttempts < MAX_RECONNECT_ATTEMPTS
    ) {
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect(): void {
    const delay = Math.min(
      RECONNECT_BASE_DELAY_MS *
        Math.pow(RECONNECT_BACKOFF_MULTIPLIER, this.reconnectAttempts),
      RECONNECT_MAX_DELAY_MS,
    );

    this.reconnectAttempts++;
    console.log(
      `Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS})`,
    );

    this.reconnectTimeout = setTimeout(() => {
      this.connect(this.password ?? undefined).catch(() => {
        // Error handled in connect
      });
    }, delay);
  }

  private clearPendingRequests(): void {
    for (const [id, pending] of this.pendingRequests) {
      clearTimeout(pending.timeout);
      pending.reject(new Error("Connection closed"));
      this.pendingRequests.delete(id);
    }
  }
}

/** Global WebSocket instance */
let globalWs: BrainproWebSocket | null = null;

/** Get or create global WebSocket instance */
export function getWebSocket(): BrainproWebSocket {
  if (!globalWs) {
    globalWs = new BrainproWebSocket();
  }
  return globalWs;
}

/**
 * Destroy the global WebSocket instance.
 * Useful for cleanup during dev HMR or when unmounting the app.
 */
export function destroyWebSocket(): void {
  if (globalWs) {
    globalWs.disconnect();
    globalWs = null;
  }
}
