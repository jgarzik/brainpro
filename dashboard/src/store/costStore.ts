/**
 * Cost tracking store
 */

import { create } from "zustand";
import type { CostSummary, OperationCost } from "@/types/cost";

interface CostStore {
  // Total cost
  totalCostUsd: number;
  totalInputTokens: number;
  totalOutputTokens: number;

  // Cost by model
  byModel: Map<
    string,
    { inputTokens: number; outputTokens: number; costUsd: number }
  >;

  // Cost by session
  bySession: Map<string, number>;

  // Actions
  recordOperation: (sessionId: string, op: OperationCost) => void;
  setCostSummary: (summary: CostSummary) => void;
  clear: () => void;
}

export const useCostStore = create<CostStore>((set) => ({
  totalCostUsd: 0,
  totalInputTokens: 0,
  totalOutputTokens: 0,
  byModel: new Map(),
  bySession: new Map(),

  recordOperation: (sessionId, op) =>
    set((state) => {
      // Update totals
      const totalCostUsd = state.totalCostUsd + op.cost_usd;
      const totalInputTokens = state.totalInputTokens + op.input_tokens;
      const totalOutputTokens = state.totalOutputTokens + op.output_tokens;

      // Update by model
      const byModel = new Map(state.byModel);
      const existing = byModel.get(op.model) ?? {
        inputTokens: 0,
        outputTokens: 0,
        costUsd: 0,
      };
      byModel.set(op.model, {
        inputTokens: existing.inputTokens + op.input_tokens,
        outputTokens: existing.outputTokens + op.output_tokens,
        costUsd: existing.costUsd + op.cost_usd,
      });

      // Update by session
      const bySession = new Map(state.bySession);
      bySession.set(sessionId, (bySession.get(sessionId) ?? 0) + op.cost_usd);

      return {
        totalCostUsd,
        totalInputTokens,
        totalOutputTokens,
        byModel,
        bySession,
      };
    }),

  setCostSummary: (summary) =>
    set({
      totalCostUsd: summary.total_cost_usd,
      totalInputTokens: summary.total_input_tokens,
      totalOutputTokens: summary.total_output_tokens,
      byModel: new Map(
        summary.by_model.map((m) => [
          m.model,
          {
            inputTokens: m.input_tokens,
            outputTokens: m.output_tokens,
            costUsd: m.cost_usd,
          },
        ]),
      ),
      bySession: new Map(
        summary.by_session.map((s) => [s.session_id, s.cost_usd]),
      ),
    }),

  clear: () =>
    set({
      totalCostUsd: 0,
      totalInputTokens: 0,
      totalOutputTokens: 0,
      byModel: new Map(),
      bySession: new Map(),
    }),
}));

/** Selector: get cost by model as array */
export function useCostByModel(): Array<{
  model: string;
  inputTokens: number;
  outputTokens: number;
  costUsd: number;
}> {
  return useCostStore((state) =>
    Array.from(state.byModel.entries()).map(([model, data]) => ({
      model,
      ...data,
    })),
  );
}

/** Selector: get cost by session as array */
export function useCostBySession(): Array<{
  sessionId: string;
  costUsd: number;
}> {
  return useCostStore((state) =>
    Array.from(state.bySession.entries()).map(([sessionId, costUsd]) => ({
      sessionId,
      costUsd,
    })),
  );
}
