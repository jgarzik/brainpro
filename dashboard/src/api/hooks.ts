/**
 * React hooks for WebSocket and API
 */

import { useState, useEffect, useCallback } from "react";
import { getWebSocket, type ConnectionState } from "./websocket";
import type { ClientEvent } from "@/types/protocol";

/** Hook for WebSocket connection state */
export function useWebSocket() {
  const ws = getWebSocket();
  const [state, setState] = useState<ConnectionState>(ws.getState());
  const [sessionId, setSessionId] = useState<string | null>(ws.getSessionId());

  useEffect(() => {
    const unsubscribe = ws.onStateChange((newState) => {
      setState(newState);
      setSessionId(ws.getSessionId());
    });
    return unsubscribe;
  }, [ws]);

  const connect = useCallback(
    async (password?: string) => {
      await ws.connect(password);
    },
    [ws],
  );

  const disconnect = useCallback(() => {
    ws.disconnect();
  }, [ws]);

  return {
    state,
    sessionId,
    policy: ws.getPolicy(),
    connect,
    disconnect,
    isConnected: state === "connected",
    isConnecting: state === "connecting" || state === "authenticating",
  };
}

/** Hook for subscribing to specific events */
export function useEvent(
  eventType: string,
  handler: (event: ClientEvent) => void,
) {
  const ws = getWebSocket();

  useEffect(() => {
    const unsubscribe = ws.on(eventType, handler);
    return unsubscribe;
  }, [ws, eventType, handler]);
}

/** Hook for subscribing to multiple event types */
export function useEvents(
  eventTypes: string[],
  handler: (event: ClientEvent) => void,
) {
  const ws = getWebSocket();

  useEffect(() => {
    const unsubscribes = eventTypes.map((type) => ws.on(type, handler));
    return () => {
      for (const unsub of unsubscribes) {
        unsub();
      }
    };
  }, [ws, eventTypes, handler]);
}

/** Hook for subscribing to all events */
export function useAllEvents(handler: (event: ClientEvent) => void) {
  const ws = getWebSocket();

  useEffect(() => {
    const unsubscribe = ws.onAny(handler);
    return unsubscribe;
  }, [ws, handler]);
}

/** Hook for subscribing to events and accumulating them */
export function useEventBuffer(
  eventTypes?: string[],
  maxSize: number = 100,
): ClientEvent[] {
  const [events, setEvents] = useState<ClientEvent[]>([]);
  const ws = getWebSocket();

  useEffect(() => {
    const handler = (event: ClientEvent) => {
      setEvents((prev) => {
        const next = [...prev, event];
        if (next.length > maxSize) {
          return next.slice(-maxSize);
        }
        return next;
      });
    };

    if (eventTypes && eventTypes.length > 0) {
      const unsubscribes = eventTypes.map((type) => ws.on(type, handler));
      return () => {
        for (const unsub of unsubscribes) {
          unsub();
        }
      };
    } else {
      return ws.onAny(handler);
    }
  }, [ws, eventTypes, maxSize]);

  return events;
}
