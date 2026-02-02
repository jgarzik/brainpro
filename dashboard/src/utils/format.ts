/**
 * String formatting utilities
 */

/**
 * Truncate an ID string to a specified length with ellipsis
 * @param id - The ID string to truncate
 * @param length - Maximum length before truncation (default: 12)
 * @returns Truncated string with "..." appended if truncated
 */
export function truncateId(id: string, length = 12): string {
  if (id.length <= length) {
    return id;
  }
  return `${id.slice(0, length)}...`;
}
