import { describe, it, expect, beforeEach } from "vitest";
import { useCostStore } from "@/store/costStore";
import type { OperationCost } from "@/types/cost";

describe("costStore", () => {
  beforeEach(() => {
    useCostStore.getState().clear();
  });

  describe("recordOperation", () => {
    it("accumulates total cost and tokens", () => {
      const op: OperationCost = {
        model: "gpt-4",
        input_tokens: 100,
        output_tokens: 50,
        cost_usd: 0.01,
      };

      useCostStore.getState().recordOperation("session-1", op);

      const state = useCostStore.getState();
      expect(state.totalCostUsd).toBe(0.01);
      expect(state.totalInputTokens).toBe(100);
      expect(state.totalOutputTokens).toBe(50);
    });

    it("updates byModel map", () => {
      const op: OperationCost = {
        model: "gpt-4",
        input_tokens: 100,
        output_tokens: 50,
        cost_usd: 0.01,
      };

      useCostStore.getState().recordOperation("session-1", op);

      const state = useCostStore.getState();
      const modelData = state.byModel.get("gpt-4");
      expect(modelData).toEqual({
        inputTokens: 100,
        outputTokens: 50,
        costUsd: 0.01,
      });
    });

    it("updates bySession map", () => {
      const op: OperationCost = {
        model: "gpt-4",
        input_tokens: 100,
        output_tokens: 50,
        cost_usd: 0.01,
      };

      useCostStore.getState().recordOperation("session-1", op);

      const state = useCostStore.getState();
      expect(state.bySession.get("session-1")).toBe(0.01);
    });

    it("aggregates multiple operations correctly", () => {
      const op1: OperationCost = {
        model: "gpt-4",
        input_tokens: 100,
        output_tokens: 50,
        cost_usd: 0.01,
      };
      const op2: OperationCost = {
        model: "gpt-4",
        input_tokens: 200,
        output_tokens: 100,
        cost_usd: 0.02,
      };
      const op3: OperationCost = {
        model: "claude-3",
        input_tokens: 150,
        output_tokens: 75,
        cost_usd: 0.015,
      };

      const store = useCostStore.getState();
      store.recordOperation("session-1", op1);
      store.recordOperation("session-1", op2);
      store.recordOperation("session-2", op3);

      const state = useCostStore.getState();

      // Check totals
      expect(state.totalCostUsd).toBeCloseTo(0.045);
      expect(state.totalInputTokens).toBe(450);
      expect(state.totalOutputTokens).toBe(225);

      // Check byModel aggregation
      expect(state.byModel.get("gpt-4")).toEqual({
        inputTokens: 300,
        outputTokens: 150,
        costUsd: 0.03,
      });
      expect(state.byModel.get("claude-3")).toEqual({
        inputTokens: 150,
        outputTokens: 75,
        costUsd: 0.015,
      });

      // Check bySession aggregation
      expect(state.bySession.get("session-1")).toBeCloseTo(0.03);
      expect(state.bySession.get("session-2")).toBe(0.015);
    });
  });

  describe("clear", () => {
    it("resets all state", () => {
      const op: OperationCost = {
        model: "gpt-4",
        input_tokens: 100,
        output_tokens: 50,
        cost_usd: 0.01,
      };

      useCostStore.getState().recordOperation("session-1", op);
      useCostStore.getState().clear();

      const state = useCostStore.getState();
      expect(state.totalCostUsd).toBe(0);
      expect(state.totalInputTokens).toBe(0);
      expect(state.totalOutputTokens).toBe(0);
      expect(state.byModel.size).toBe(0);
      expect(state.bySession.size).toBe(0);
    });
  });
});
