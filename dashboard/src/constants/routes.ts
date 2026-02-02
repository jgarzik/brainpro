/**
 * Application route constants
 */

export const ROUTES = {
  /** Main chat interface */
  CHAT: "/",
  /** System overview dashboard */
  OVERVIEW: "/overview",
  /** Session management */
  SESSIONS: "/sessions",
  /** Individual session detail */
  SESSION_DETAIL: "/sessions/:id",
  /** Tool registry */
  TOOLS: "/tools",
  /** Skill browser */
  SKILLS: "/skills",
  /** Subagent configuration */
  AGENTS: "/agents",
  /** Channel status */
  CHANNELS: "/channels",
  /** Backend health dashboard */
  HEALTH: "/health",
  /** Event stream */
  EVENTS: "/events",
  /** Cost tracking */
  COSTS: "/costs",
  /** Configuration viewer */
  CONFIG: "/config",
  /** Debug tools */
  DEBUG: "/debug",
} as const;

export type RoutePath = (typeof ROUTES)[keyof typeof ROUTES];

/** Build session detail path */
export function sessionDetailPath(sessionId: string): string {
  return ROUTES.SESSION_DETAIL.replace(":id", sessionId);
}
