/**
 * Cost types matching src/cost.rs
 */

/** Pricing for a single model (per 1M tokens in USD) */
export interface ModelPricing {
  input: number;
  output: number;
}

/** Cost for a single LLM operation */
export interface OperationCost {
  model: string;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
}

/** Aggregated costs for a single turn */
export interface TurnCost {
  turn_number: number;
  operations: OperationCost[];
  total_tokens: number;
  total_cost: number;
}

/** Session-level cost summary */
export interface SessionCost {
  session_id: string;
  turns: TurnCost[];
  total_tokens: number;
  total_cost_usd: number;
  cost_by_model: Record<string, { tokens: number; cost: number }>;
}

/** Cost configuration */
export interface CostConfig {
  enabled: boolean;
  warn_threshold_usd?: number;
  display_in_stats: boolean;
}

/** Cost summary for display */
export interface CostSummary {
  total_cost_usd: number;
  total_input_tokens: number;
  total_output_tokens: number;
  by_model: Array<{
    model: string;
    input_tokens: number;
    output_tokens: number;
    cost_usd: number;
  }>;
  by_session: Array<{
    session_id: string;
    cost_usd: number;
  }>;
}

/** Format cost for display */
export function formatCost(cost: number): string {
  if (cost < 0.01) {
    return `$${cost.toFixed(4)}`;
  } else if (cost < 1.0) {
    return `$${cost.toFixed(3)}`;
  } else {
    return `$${cost.toFixed(2)}`;
  }
}

/** Format token count for display */
export function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) {
    return `${(tokens / 1_000_000).toFixed(1)}M`;
  } else if (tokens >= 1_000) {
    return `${(tokens / 1_000).toFixed(1)}k`;
  } else {
    return tokens.toString();
  }
}
