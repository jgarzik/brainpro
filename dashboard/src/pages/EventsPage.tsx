import { useCallback, useMemo, useState } from "react";
import { Pause, Play, Trash2, Search } from "lucide-react";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Modal } from "@/components/ui/Modal";
import { useAllEvents } from "@/api/hooks";
import { useEventStore, useFilteredEvents } from "@/store/eventStore";
import { SUBSYSTEMS, type Subsystem } from "@/constants/events";
import { truncateId } from "@/utils";
import type { Event } from "@/types/event";
import type { ClientEvent } from "@/types/protocol";

const subsystemColors: Record<Subsystem, string> = {
  model: "info",
  message: "neutral",
  session: "success",
  tool: "warning",
  queue: "neutral",
  run: "info",
  system: "neutral",
  circuit: "error",
  policy: "warning",
  webhook: "info",
  plugin: "neutral",
  cost: "warning",
} as const;

function EventCard({ event, onClick }: { event: Event; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="flex w-full items-center gap-3 rounded-lg border border-gray-200 p-3 text-left transition-colors hover:bg-gray-50 dark:border-gray-700 dark:hover:bg-gray-800"
    >
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <Badge
            variant={
              subsystemColors[event.subsystem] as
                | "info"
                | "neutral"
                | "success"
                | "warning"
                | "error"
            }
          >
            {event.subsystem}
          </Badge>
          <span className="font-mono text-sm">{event.type}</span>
        </div>
        {event.run_context?.session_id && (
          <p className="mt-1 text-xs text-gray-400">
            Session: {truncateId(event.run_context.session_id)}
          </p>
        )}
      </div>
      <div className="text-right">
        <span className="text-xs text-gray-400">#{event.seq}</span>
        <p className="text-xs text-gray-400">
          {new Date(event.timestamp_ms).toLocaleTimeString()}
        </p>
      </div>
    </button>
  );
}

export default function EventsPage() {
  const addEvent = useEventStore((s) => s.addEvent);
  const clearEvents = useEventStore((s) => s.clearEvents);
  const {
    filters,
    toggleFilter,
    searchQuery,
    setSearchQuery,
    paused,
    setPaused,
  } = useEventStore();
  const filteredEvents = useFilteredEvents();

  const [selectedEvent, setSelectedEvent] = useState<Event | null>(null);

  /**
   * Safely parse a ClientEvent into an Event, with validation
   */
  const parseClientEvent = useCallback((event: ClientEvent): Event | null => {
    try {
      const data = event.data as Record<string, unknown> | undefined;

      // Validate required fields
      if (!event.event || typeof event.event !== "string") {
        console.warn("Invalid event: missing or invalid event type", event);
        return null;
      }

      // Extract subsystem with fallback
      let subsystem: Subsystem = "system";
      if (data && typeof data["subsystem"] === "string") {
        const rawSubsystem = data["subsystem"];
        if (Object.values(SUBSYSTEMS).includes(rawSubsystem as Subsystem)) {
          subsystem = rawSubsystem as Subsystem;
        }
      }

      // Construct event with safe defaults
      const parsed: Event = {
        seq: typeof data?.["seq"] === "number" ? data["seq"] : Date.now(),
        timestamp_ms:
          typeof data?.["timestamp_ms"] === "number"
            ? data["timestamp_ms"]
            : Date.now(),
        subsystem,
        type: event.event.replace(".", "_") as Event["type"],
        ...(data ?? {}),
      } as Event;

      return parsed;
    } catch (err) {
      console.warn("Failed to parse client event:", err, event);
      return null;
    }
  }, []);

  // Subscribe to all incoming events
  const handleEvent = useCallback(
    (event: ClientEvent) => {
      const parsed = parseClientEvent(event);
      if (parsed) {
        addEvent(parsed);
      }
    },
    [addEvent, parseClientEvent],
  );

  useAllEvents(handleEvent);

  // Memoize reversed events to avoid re-computation on each render
  const reversedEvents = useMemo(
    () => [...filteredEvents].reverse(),
    [filteredEvents],
  );

  // Memoize event click handler factory
  const handleEventClick = useCallback((event: Event) => {
    setSelectedEvent(event);
  }, []);

  const subsystemList = Object.values(SUBSYSTEMS);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">
          Events
        </h1>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => setPaused(!paused)}>
            {paused ? (
              <Play className="h-4 w-4" />
            ) : (
              <Pause className="h-4 w-4" />
            )}
            {paused ? "Resume" : "Pause"}
          </Button>
          <Button variant="ghost" onClick={clearEvents}>
            <Trash2 className="h-4 w-4" />
            Clear
          </Button>
        </div>
      </div>

      {/* Filters */}
      <Card padding="sm">
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-2">
            <Search className="h-4 w-4 text-gray-400" />
            <Input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search events..."
              className="w-64"
            />
          </div>

          <div className="h-6 w-px bg-gray-200 dark:bg-gray-700" />

          <div className="flex flex-wrap gap-1" role="group" aria-label="Filter by subsystem">
            {subsystemList.map((sub) => (
              <button
                key={sub}
                type="button"
                onClick={() => toggleFilter(sub)}
                aria-pressed={filters.has(sub)}
                className={`rounded-full px-2.5 py-1 text-xs font-medium transition-colors ${
                  filters.has(sub)
                    ? "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400"
                    : "bg-gray-100 text-gray-600 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-400 dark:hover:bg-gray-700"
                }`}
              >
                {sub}
              </button>
            ))}
          </div>
        </div>
      </Card>

      {/* Event list */}
      <div className="space-y-2">
        {reversedEvents.length > 0 ? (
          reversedEvents.map((event) => (
            <EventCard
              key={event.seq}
              event={event}
              onClick={() => handleEventClick(event)}
            />
          ))
        ) : (
          <Card>
            <p className="text-center text-gray-500 dark:text-gray-400">
              {paused ? "Event stream paused" : "No events matching filters"}
            </p>
          </Card>
        )}
      </div>

      {/* Event detail modal */}
      <Modal
        open={!!selectedEvent}
        onClose={() => setSelectedEvent(null)}
        title={`Event #${selectedEvent?.seq}`}
        size="lg"
      >
        {selectedEvent && (
          <div className="space-y-4">
            <div className="flex items-center gap-2">
              <Badge
                variant={subsystemColors[selectedEvent.subsystem] as "info"}
              >
                {selectedEvent.subsystem}
              </Badge>
              <span className="font-mono">{selectedEvent.type}</span>
            </div>

            <div className="text-sm text-gray-500 dark:text-gray-400">
              {new Date(selectedEvent.timestamp_ms).toLocaleString()}
            </div>

            <pre className="max-h-96 overflow-auto rounded-lg bg-gray-50 p-4 text-sm dark:bg-gray-900">
              {JSON.stringify(selectedEvent, null, 2)}
            </pre>
          </div>
        )}
      </Modal>
    </div>
  );
}
