import { describe, it, expect, beforeEach } from "vitest";
import { useEventStore } from "@/store/eventStore";
import { EVENT_BUFFER_LIMIT } from "@/constants/api";
import type { HeartbeatEvent, ToolInvokedEvent } from "@/types/event";

function makeEvent(
  seq: number,
  subsystem: "system" | "tool" = "system",
): HeartbeatEvent | ToolInvokedEvent {
  if (subsystem === "system") {
    return {
      seq,
      timestamp_ms: Date.now(),
      subsystem: "system",
      type: "heartbeat",
      uptime_secs: seq,
      active_sessions: 1,
      pending_requests: 0,
    };
  }
  return {
    seq,
    timestamp_ms: Date.now(),
    subsystem: "tool",
    type: "tool_invoked",
    session_id: "session-1",
    tool_name: `tool-${seq}`,
    tool_call_id: `call-${seq}`,
    args_preview: "{}",
  };
}

describe("eventStore", () => {
  beforeEach(() => {
    useEventStore.setState({
      events: [],
      filters: new Set(),
      searchQuery: "",
      paused: false,
    });
  });

  describe("buffer limit", () => {
    it("enforces MAX_EVENTS limit", () => {
      const store = useEventStore.getState();

      // Add more events than the limit
      for (let i = 0; i < EVENT_BUFFER_LIMIT + 100; i++) {
        store.addEvent(makeEvent(i));
      }

      const state = useEventStore.getState();
      expect(state.events.length).toBe(EVENT_BUFFER_LIMIT);
    });

    it("uses FIFO when full (keeps newest)", () => {
      const store = useEventStore.getState();

      // Add exactly limit + 10 events
      for (let i = 0; i < EVENT_BUFFER_LIMIT + 10; i++) {
        store.addEvent(makeEvent(i));
      }

      const state = useEventStore.getState();
      // First event should be seq 10 (oldest 10 were dropped)
      expect(state.events[0]?.seq).toBe(10);
      // Last event should be seq 1009
      expect(state.events[state.events.length - 1]?.seq).toBe(
        EVENT_BUFFER_LIMIT + 9,
      );
    });
  });

  describe("clearEvents", () => {
    it("resets events array", () => {
      const store = useEventStore.getState();
      store.addEvent(makeEvent(1));
      store.addEvent(makeEvent(2));

      expect(useEventStore.getState().events.length).toBe(2);

      store.clearEvents();

      expect(useEventStore.getState().events.length).toBe(0);
    });
  });

  describe("filtering", () => {
    it("filters by subsystem", () => {
      const store = useEventStore.getState();
      store.addEvent(makeEvent(1, "system"));
      store.addEvent(makeEvent(2, "tool"));
      store.addEvent(makeEvent(3, "system"));

      // Set filter to only show tool events
      store.setFilters(new Set(["tool"]));

      const state = useEventStore.getState();
      // Filter applied via selector, check raw state still has all
      expect(state.events.length).toBe(3);
      expect(state.filters.has("tool")).toBe(true);
    });

    it("toggleFilter adds and removes subsystems", () => {
      const store = useEventStore.getState();

      store.toggleFilter("tool");
      expect(useEventStore.getState().filters.has("tool")).toBe(true);

      store.toggleFilter("tool");
      expect(useEventStore.getState().filters.has("tool")).toBe(false);
    });

    it("clearFilters removes all filters", () => {
      const store = useEventStore.getState();
      store.setFilters(new Set(["tool", "system", "model"]));

      expect(useEventStore.getState().filters.size).toBe(3);

      store.clearFilters();

      expect(useEventStore.getState().filters.size).toBe(0);
    });
  });

  describe("search", () => {
    it("sets search query", () => {
      const store = useEventStore.getState();
      store.setSearchQuery("error");

      expect(useEventStore.getState().searchQuery).toBe("error");
    });
  });

  describe("paused state", () => {
    it("does not add events when paused", () => {
      const store = useEventStore.getState();
      store.addEvent(makeEvent(1));
      expect(useEventStore.getState().events.length).toBe(1);

      store.setPaused(true);
      store.addEvent(makeEvent(2));

      expect(useEventStore.getState().events.length).toBe(1);
    });

    it("resumes adding events when unpaused", () => {
      const store = useEventStore.getState();
      store.setPaused(true);
      store.addEvent(makeEvent(1));
      expect(useEventStore.getState().events.length).toBe(0);

      store.setPaused(false);
      store.addEvent(makeEvent(2));

      expect(useEventStore.getState().events.length).toBe(1);
    });
  });

  describe("addEvents batch", () => {
    it("adds multiple events at once", () => {
      const store = useEventStore.getState();
      store.addEvents([makeEvent(1), makeEvent(2), makeEvent(3)]);

      expect(useEventStore.getState().events.length).toBe(3);
    });

    it("enforces limit with batch add", () => {
      const store = useEventStore.getState();
      const events = Array.from({ length: EVENT_BUFFER_LIMIT + 50 }, (_, i) =>
        makeEvent(i),
      );
      store.addEvents(events);

      expect(useEventStore.getState().events.length).toBe(EVENT_BUFFER_LIMIT);
    });
  });
});
