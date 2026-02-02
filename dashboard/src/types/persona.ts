/**
 * Persona and agent types
 */

/** Persona configuration */
export interface Persona {
  name: string;
  display_name: string;
  tools: string[];
  description?: string;
}

/** Subagent configuration */
export interface SubagentConfig {
  name: string;
  tools: string[];
  max_turns: number;
  description?: string;
}

/** Available agents list */
export interface AgentInfo {
  id: string;
  name: string;
  type: "main" | "subagent";
  tools: string[];
  max_turns?: number;
  description?: string;
}
