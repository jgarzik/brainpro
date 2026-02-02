/**
 * Tool constants matching src/tools/mod.rs
 */

export const TOOLS = {
  READ: "Read",
  WRITE: "Write",
  EDIT: "Edit",
  PATCH: "Patch",
  GLOB: "Glob",
  GREP: "Grep",
  SEARCH: "Search",
  BASH: "Bash",
  TASK: "Task",
  ACTIVATE_SKILL: "ActivateSkill",
  TODO: "Todo",
  ASK_USER: "AskUser",
  ENTER_PLAN_MODE: "EnterPlanMode",
  EXIT_PLAN_MODE: "ExitPlanMode",
} as const;

export type ToolName = (typeof TOOLS)[keyof typeof TOOLS];

/** Tool categories for UI grouping */
export const TOOL_CATEGORIES = {
  FILE_OPS: "File Operations",
  SEARCH: "Search",
  EXECUTION: "Execution",
  AGENT: "Agent Control",
  USER_INTERACTION: "User Interaction",
} as const;

export type ToolCategory =
  (typeof TOOL_CATEGORIES)[keyof typeof TOOL_CATEGORIES];

/** Map tools to categories */
export const TOOL_CATEGORY_MAP: Record<ToolName, ToolCategory> = {
  [TOOLS.READ]: TOOL_CATEGORIES.FILE_OPS,
  [TOOLS.WRITE]: TOOL_CATEGORIES.FILE_OPS,
  [TOOLS.EDIT]: TOOL_CATEGORIES.FILE_OPS,
  [TOOLS.PATCH]: TOOL_CATEGORIES.FILE_OPS,
  [TOOLS.GLOB]: TOOL_CATEGORIES.SEARCH,
  [TOOLS.GREP]: TOOL_CATEGORIES.SEARCH,
  [TOOLS.SEARCH]: TOOL_CATEGORIES.SEARCH,
  [TOOLS.BASH]: TOOL_CATEGORIES.EXECUTION,
  [TOOLS.TASK]: TOOL_CATEGORIES.AGENT,
  [TOOLS.ACTIVATE_SKILL]: TOOL_CATEGORIES.AGENT,
  [TOOLS.TODO]: TOOL_CATEGORIES.AGENT,
  [TOOLS.ASK_USER]: TOOL_CATEGORIES.USER_INTERACTION,
  [TOOLS.ENTER_PLAN_MODE]: TOOL_CATEGORIES.AGENT,
  [TOOLS.EXIT_PLAN_MODE]: TOOL_CATEGORIES.AGENT,
};

/** Base tools available to subagents (no Task) */
export const BASE_TOOLS: ToolName[] = [
  TOOLS.READ,
  TOOLS.WRITE,
  TOOLS.EDIT,
  TOOLS.PATCH,
  TOOLS.GLOB,
  TOOLS.SEARCH,
  TOOLS.BASH,
];

/** Full tools available to main agent */
export const MAIN_AGENT_TOOLS: ToolName[] = [
  ...BASE_TOOLS,
  TOOLS.TASK,
  TOOLS.ACTIVATE_SKILL,
  TOOLS.TODO,
  TOOLS.ASK_USER,
  TOOLS.ENTER_PLAN_MODE,
  TOOLS.EXIT_PLAN_MODE,
];
