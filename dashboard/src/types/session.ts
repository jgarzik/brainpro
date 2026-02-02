/**
 * Session and message types
 */

/** Message role */
export type MessageRole = "user" | "assistant" | "system";

/** Tool call status */
export type ToolCallStatus =
  | "pending"
  | "running"
  | "completed"
  | "failed"
  | "denied";

/** Tool call within a message */
export interface ToolCall {
  id: string;
  name: string;
  args: unknown;
  result?: unknown;
  status: ToolCallStatus;
  duration_ms?: number;
  error?: string;
}

/** A single message in a session */
export interface Message {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: number;
  tool_calls?: ToolCall[];
  /** True if message is still streaming */
  streaming?: boolean;
}

/** Session status */
export type SessionStatus =
  | "active"
  | "paused"
  | "ended"
  | "stuck"
  | "awaiting_approval"
  | "awaiting_input";

/** Turn result summary */
export interface TurnResult {
  turn_number: number;
  iterations: number;
  tool_uses: number;
  tokens_used: number;
  cost_usd: number;
  duration_ms: number;
}

/** Session summary */
export interface Session {
  id: string;
  agent_id?: string;
  status: SessionStatus;
  created_at: number;
  updated_at: number;
  turn_count: number;
  total_tokens: number;
  total_cost_usd: number;
  turns: TurnResult[];
}

/** Session with full message history */
export interface SessionWithMessages extends Session {
  messages: Message[];
}

/** Pending approval state */
export interface PendingApproval {
  session_id: string;
  turn_id: string;
  tool_call_id: string;
  tool_name: string;
  args: unknown;
}

/** Question option for ask user */
export interface QuestionOption {
  label: string;
  description?: string;
}

/** Single question from ask user */
export interface Question {
  question: string;
  header?: string;
  options: QuestionOption[];
  multi_select: boolean;
}

/** Pending user question state */
export interface PendingQuestion {
  session_id: string;
  turn_id: string;
  tool_call_id: string;
  questions: Question[];
}
