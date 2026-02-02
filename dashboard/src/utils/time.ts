/**
 * Time formatting utilities
 */

/**
 * Format uptime in seconds to a human-readable string
 * @param secs - Uptime in seconds
 * @param includeSeconds - Whether to include seconds in the output (default: false)
 * @returns Formatted string like "2h 30m" or "2h 30m 15s"
 */
export function formatUptime(secs: number, includeSeconds = false): string {
  const hours = Math.floor(secs / 3600);
  const mins = Math.floor((secs % 3600) / 60);

  if (includeSeconds) {
    const s = secs % 60;
    return `${hours}h ${mins}m ${s}s`;
  }

  return `${hours}h ${mins}m`;
}
