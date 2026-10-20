import { describe, expect, it } from "vitest";
import type { Category } from "../bindings";
import { categorySlices, comparisonSlices, trendTotals, variationPercent } from "./analyses";

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

describe("comparisons", () => {
  const categories: Category[] = [
    { id: 2, name: "Logement", parentId: null, color: "housing", isInternalTransfer: false, isIncome: false, operationCount: 0 },
  ];

  it("keeps categories spent in either period", () => {
    expect(
      comparisonSlices(
        [
          { categoryId: 2, currentCents: 93800, previousCents: 92100 },
          { categoryId: null, currentCents: 0, previousCents: 1500 },
          { categoryId: 3, currentCents: -200, previousCents: 0 },
        ],
        categories,
      ),
    ).toEqual([
      { name: "Logement", color: "housing", amountCents: 93800, previousCents: 92100 },
      { name: "À catégoriser", color: "neutral", amountCents: 0, previousCents: 1500 },
    ]);
  });

  it("computes the change in percent", () => {
    expect(variationPercent(93800, 92100)).toBe(2);
    expect(variationPercent(21400, 25100)).toBe(-15);
    expect(variationPercent(500, 0)).toBe(null);
  });
});
