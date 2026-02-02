/**
 * Health state store
 */

import { create } from "zustand";
import type { ProviderHealth, HealthStatus } from "@/types/health";
import type { HealthState } from "@/constants/health";

interface HealthStore {
  // Overall status
  status: HealthState;
  uptimeSecs: number;
  activeSessions: number;
  pendingRequests: number;

  // Backend health by name
  backends: Map<string, ProviderHealth>;

  // Last check timestamp
  lastCheck: number | null;

  // Loading state
  loading: boolean;
  error: string | null;

  // Actions
  setHealthStatus: (status: HealthStatus) => void;
  updateBackend: (name: string, health: ProviderHealth) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  clear: () => void;
}

export const useHealthStore = create<HealthStore>((set) => ({
  status: "healthy",
  uptimeSecs: 0,
  activeSessions: 0,
  pendingRequests: 0,
  backends: new Map(),
  lastCheck: null,
  loading: false,
  error: null,

  setHealthStatus: (healthStatus) =>
    set({
      status: healthStatus.status,
      uptimeSecs: healthStatus.uptime_secs,
      activeSessions: healthStatus.active_sessions,
      pendingRequests: healthStatus.pending_requests,
      backends: new Map(Object.entries(healthStatus.backends)),
      lastCheck: Date.now(),
      error: null,
    }),

  updateBackend: (name, health) =>
    set((state) => {
      const backends = new Map(state.backends);
      backends.set(name, health);
      return { backends };
    }),

  setLoading: (loading) => set({ loading }),

  setError: (error) => set({ error, loading: false }),

  clear: () =>
    set({
      status: "healthy",
      uptimeSecs: 0,
      activeSessions: 0,
      pendingRequests: 0,
      backends: new Map(),
      lastCheck: null,
      loading: false,
      error: null,
    }),
}));

/** Selector: get all backends as array */
export function useAllBackends(): ProviderHealth[] {
  return useHealthStore((state) => Array.from(state.backends.values()));
}

/** Selector: get specific backend */
export function useBackend(name: string): ProviderHealth | undefined {
  return useHealthStore((state) => state.backends.get(name));
}
