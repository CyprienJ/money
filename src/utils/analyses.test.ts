import { describe, expect, it } from "vitest";
import type { Category } from "../bindings";
import { categorySlices, trendTotals } from "./analyses";

describe("trendTotals", () => {
  it("sums income and expenses and computes the savings rate", () => {
    const totals = trendTotals([
      { periodStart: "2026-09-01", incomeCents: 328000, expenseCents: 226000 },
      { periodStart: "2026-10-01", incomeCents: 328000, expenseCents: 214600 },
    ]);
    expect(totals).toEqual({
      incomeCents: 656000,
      expenseCents: 440600,
      savedCents: 215400,
      savedPercent: 33,
    });
  });

  it("has no savings rate without income", () => {
    expect(trendTotals([{ periodStart: "2026-10-01", incomeCents: 0, expenseCents: 5000 }])).toMatchObject({
      savedCents: -5000,
      savedPercent: null,
    });
  });
});

describe("categorySlices", () => {
  const categories: Category[] = [
    { id: 2, name: "Courses", parentId: null, color: "groceries", isInternalTransfer: false, isIncome: false, operationCount: 0 },
  ];

  it("names categories and keeps positive expenses only", () => {
    expect(
      categorySlices(
        [
          { categoryId: 2, expenseCents: 41200 },
          { categoryId: null, expenseCents: 4000 },
          { categoryId: 3, expenseCents: -500 },
        ],
        categories,
      ),
    ).toEqual([
      { name: "Courses", color: "groceries", amountCents: 41200 },
      { name: "À catégoriser", color: "neutral", amountCents: 4000 },
    ]);
  });
});
