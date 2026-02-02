/**
 * UI constants for styling and animation
 */

/** Status colors for badges and indicators */
export const STATUS_COLORS = {
  SUCCESS: {
    bg: "bg-emerald-100 dark:bg-emerald-900/30",
    text: "text-emerald-700 dark:text-emerald-400",
    border: "border-emerald-200 dark:border-emerald-800",
    dot: "bg-emerald-500",
  },
  WARNING: {
    bg: "bg-amber-100 dark:bg-amber-900/30",
    text: "text-amber-700 dark:text-amber-400",
    border: "border-amber-200 dark:border-amber-800",
    dot: "bg-amber-500",
  },
  ERROR: {
    bg: "bg-red-100 dark:bg-red-900/30",
    text: "text-red-700 dark:text-red-400",
    border: "border-red-200 dark:border-red-800",
    dot: "bg-red-500",
  },
  INFO: {
    bg: "bg-blue-100 dark:bg-blue-900/30",
    text: "text-blue-700 dark:text-blue-400",
    border: "border-blue-200 dark:border-blue-800",
    dot: "bg-blue-500",
  },
  NEUTRAL: {
    bg: "bg-gray-100 dark:bg-gray-800",
    text: "text-gray-700 dark:text-gray-300",
    border: "border-gray-200 dark:border-gray-700",
    dot: "bg-gray-500",
  },
} as const;

export type StatusColorKey = keyof typeof STATUS_COLORS;

/** Button sizes */
export const BUTTON_SIZES = {
  SM: "px-2.5 py-1.5 text-xs",
  MD: "px-4 py-2 text-sm",
  LG: "px-6 py-3 text-base",
} as const;

export type ButtonSize = keyof typeof BUTTON_SIZES;

/** Card padding variants */
export const CARD_PADDING = {
  SM: "p-3",
  MD: "p-4",
  LG: "p-6",
} as const;

export type CardPadding = keyof typeof CARD_PADDING;

/** Animation durations in milliseconds */
export const ANIMATION_DURATIONS = {
  FAST: 150,
  NORMAL: 200,
  SLOW: 300,
} as const;

/** Toast auto-dismiss duration in milliseconds */
export const TOAST_DURATION_MS = 5000;

/** Sidebar width in pixels */
export const SIDEBAR_WIDTH_PX = 240;

/** Header height in pixels */
export const HEADER_HEIGHT_PX = 56;

/** Breakpoints for responsive design */
export const BREAKPOINTS = {
  SM: 640,
  MD: 768,
  LG: 1024,
  XL: 1280,
  XXL: 1536,
} as const;
