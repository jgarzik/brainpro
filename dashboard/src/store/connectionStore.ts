/**
 * Connection state store
 */

import { create } from "zustand";
import { getWebSocket, type ConnectionState } from "@/api/websocket";
import type { PolicyInfo } from "@/types/protocol";

interface ConnectionStore {
  state: ConnectionState;
  sessionId: string | null;
  policy: PolicyInfo | null;
  password: string | null;
  error: string | null;

  setPassword: (password: string | null) => void;
  connect: (password?: string) => Promise<void>;
  disconnect: () => void;
  syncState: () => void;
}

export const useConnectionStore = create<ConnectionStore>((set, get) => {
  const ws = getWebSocket();

  // Sync state when WebSocket state changes
  ws.onStateChange((state) => {
    set({
      state,
      sessionId: ws.getSessionId(),
      policy: ws.getPolicy(),
    });
  });

  // Sync initial state after subscription is set up to avoid race condition
  // where state changes before the handler is registered
  const initialState = ws.getState();
  const initialSessionId = ws.getSessionId();
  const initialPolicy = ws.getPolicy();

  return {
    state: initialState,
    sessionId: initialSessionId,
    policy: initialPolicy,
    password: null,
    error: null,

    setPassword: (password) => set({ password }),

    connect: async (password) => {
      const pw = password ?? get().password;
      set({ error: null });
      try {
        await ws.connect(pw ?? undefined);
      } catch (err) {
        set({
          error: err instanceof Error ? err.message : "Connection failed",
        });
        throw err;
      }
    },

    disconnect: () => {
      ws.disconnect();
    },

    syncState: () => {
      set({
        state: ws.getState(),
        sessionId: ws.getSessionId(),
        policy: ws.getPolicy(),
      });
    },
  };
});
