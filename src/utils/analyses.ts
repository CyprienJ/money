import type { Category, CategoryComparison, CategoryExpense, PeriodIncomeExpenses } from "../bindings";
import type { CategorySlice } from "../charts/options";

export interface TrendTotals {
  incomeCents: number;
  expenseCents: number;
  /** Income minus expenses. */
  savedCents: number;
  /** Share of income saved, rounded; `null` without income. */
  savedPercent: number | null;
}

/** Totals of the income and expenses trend. */
export function trendTotals(periods: PeriodIncomeExpenses[]): TrendTotals {
  const incomeCents = periods.reduce((sum, p) => sum + p.incomeCents, 0);
  const expenseCents = periods.reduce((sum, p) => sum + p.expenseCents, 0);
  const savedCents = incomeCents - expenseCents;
  return {
    incomeCents,
    expenseCents,
    savedCents,
    savedPercent: incomeCents > 0 ? Math.round((savedCents * 100) / incomeCents) : null,
  };
}

/**
 * Chart slices of the expenses by category: positive expenses only, uncategorized ones
 * under "À catégoriser".
 */
export function categorySlices(rows: CategoryExpense[], categories: Category[]): CategorySlice[] {
  const byId = new Map(categories.map((c) => [c.id, c]));
  return rows
    .filter((row) => row.expenseCents > 0)
    .map((row) => {
      const category = row.categoryId === null ? undefined : byId.get(row.categoryId);
      return {
        name: category?.name ?? "À catégoriser",
        color: category?.color ?? "neutral",
        amountCents: row.expenseCents,
      };
    });
}

/**
 * Chart slices comparing two periods: categories with expenses in either period, the
 * compared amount in `previousCents`.
 */
export function comparisonSlices(rows: CategoryComparison[], categories: Category[]): CategorySlice[] {
  const byId = new Map(categories.map((c) => [c.id, c]));
  return rows
    .filter((row) => row.currentCents > 0 || row.previousCents > 0)
    .map((row) => {
      const category = row.categoryId === null ? undefined : byId.get(row.categoryId);
      return {
        name: category?.name ?? "À catégoriser",
        color: category?.color ?? "neutral",
        amountCents: Math.max(0, row.currentCents),
        previousCents: Math.max(0, row.previousCents),
      };
    });
}

/** Change from `previous` to `current` in percent, rounded; `null` without a previous amount. */
export function variationPercent(current: number, previous: number): number | null {
  if (previous <= 0) return null;
  return Math.round(((current - previous) * 100) / previous);
}
