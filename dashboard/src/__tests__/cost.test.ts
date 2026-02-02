import { describe, it, expect } from "vitest";
import { formatCost, formatTokens } from "@/types/cost";

describe("formatCost", () => {
  it("formats sub-cent costs with 4 decimal places", () => {
    expect(formatCost(0.001)).toBe("$0.0010");
    expect(formatCost(0.0005)).toBe("$0.0005");
    expect(formatCost(0.0099)).toBe("$0.0099");
  });

  it("formats cents with 3 decimal places", () => {
    expect(formatCost(0.01)).toBe("$0.010");
    expect(formatCost(0.5)).toBe("$0.500");
    expect(formatCost(0.99)).toBe("$0.990");
  });

  it("formats dollars with 2 decimal places", () => {
    expect(formatCost(1.5)).toBe("$1.50");
    expect(formatCost(10.0)).toBe("$10.00");
    expect(formatCost(123.45)).toBe("$123.45");
  });

  it("formats zero", () => {
    expect(formatCost(0)).toBe("$0.0000");
  });
});

describe("formatTokens", () => {
  it("formats small numbers as-is", () => {
    expect(formatTokens(0)).toBe("0");
    expect(formatTokens(500)).toBe("500");
    expect(formatTokens(999)).toBe("999");
  });

  it("formats thousands with k suffix", () => {
    expect(formatTokens(1000)).toBe("1.0k");
    expect(formatTokens(1500)).toBe("1.5k");
    expect(formatTokens(10000)).toBe("10.0k");
    expect(formatTokens(999999)).toBe("1000.0k");
  });

  it("formats millions with M suffix", () => {
    expect(formatTokens(1000000)).toBe("1.0M");
    expect(formatTokens(1500000)).toBe("1.5M");
    expect(formatTokens(10000000)).toBe("10.0M");
  });
});
