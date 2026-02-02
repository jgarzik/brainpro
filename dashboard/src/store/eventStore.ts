/**
 * Event store for streaming events
 */

import { create } from "zustand";
import type { Event } from "@/types/event";
import type { Subsystem } from "@/constants/events";
import { EVENT_BUFFER_LIMIT } from "@/constants/api";

interface EventStore {
  // All events (capped at limit)
  events: Event[];

  // Filter state
  filters: Set<Subsystem>;
  searchQuery: string;

  // Paused state (stop adding new events)
  paused: boolean;

  // Actions
  addEvent: (event: Event) => void;
  addEvents: (events: Event[]) => void;
  clearEvents: () => void;

  setFilters: (filters: Set<Subsystem>) => void;
  toggleFilter: (subsystem: Subsystem) => void;
  clearFilters: () => void;

  setSearchQuery: (query: string) => void;
  setPaused: (paused: boolean) => void;
}

export const useEventStore = create<EventStore>((set, get) => ({
  events: [],
  filters: new Set(),
  searchQuery: "",
  paused: false,

  addEvent: (event) => {
    if (get().paused) return;
    set((state) => {
      const events = [...state.events, event];
      if (events.length > EVENT_BUFFER_LIMIT) {
        return { events: events.slice(-EVENT_BUFFER_LIMIT) };
      }
      return { events };
    });
  },

  addEvents: (newEvents) => {
    if (get().paused) return;
    set((state) => {
      const events = [...state.events, ...newEvents];
      if (events.length > EVENT_BUFFER_LIMIT) {
        return { events: events.slice(-EVENT_BUFFER_LIMIT) };
      }
      return { events };
    });
  },

  clearEvents: () => set({ events: [] }),

  setFilters: (filters) => set({ filters }),

  toggleFilter: (subsystem) =>
    set((state) => {
      const filters = new Set(state.filters);
      if (filters.has(subsystem)) {
        filters.delete(subsystem);
      } else {
        filters.add(subsystem);
      }
      return { filters };
    }),

  clearFilters: () => set({ filters: new Set() }),

  setSearchQuery: (query) => set({ searchQuery: query }),

  setPaused: (paused) => set({ paused }),
}));

/** Selector: get filtered events */
export function useFilteredEvents(): Event[] {
  return useEventStore((state) => {
    let filtered = state.events;

    // Apply subsystem filters
    if (state.filters.size > 0) {
      filtered = filtered.filter((e) => state.filters.has(e.subsystem));
    }

    // Apply search query
    if (state.searchQuery) {
      const query = state.searchQuery.toLowerCase();
      filtered = filtered.filter((e) =>
        JSON.stringify(e).toLowerCase().includes(query),
      );
    }

    return filtered;
  });
}

/** Selector: get recent events (last N) */
export function useRecentEvents(count: number = 10): Event[] {
  return useEventStore((state) => state.events.slice(-count));
}
