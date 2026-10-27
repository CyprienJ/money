import type { CategoryFilter, Direction, OperationFilter } from "../bindings";
import { parseAmount } from "./format";

/** Filters of the "Plus de filtres" panel of the Operations screen. */
export interface AdvancedFilters {
  direction: Direction | "all";
  /** Amount bounds as typed, in euros, in absolute value; empty for no bound. */
  minAmount: string;
  maxAmount: string;
  payeeId: number | null;
  tagId: number | null;
}

export function emptyAdvancedFilters(): AdvancedFilters {
  return { direction: "all", minAmount: "", maxAmount: "", payeeId: null, tagId: null };
}

/** Count of the advanced filters in use, shown on the panel button. */
export function activeFilterCount(filters: AdvancedFilters): number {
  return [
    filters.direction !== "all",
    filters.minAmount.trim() !== "" || filters.maxAmount.trim() !== "",
    filters.payeeId !== null,
    filters.tagId !== null,
  ].filter(Boolean).length;
}

/** An amount bound in cents, `null` when empty, `undefined` when invalid. */
function bound(text: string): number | null | undefined {
  if (text.trim() === "") return null;
  const cents = parseAmount(text);
  return cents === null ? undefined : Math.abs(cents);
}

/** Why the amount bounds cannot be used, or an empty string. */
export function amountBoundsError(filters: AdvancedFilters): string {
  const min = bound(filters.minAmount);
  const max = bound(filters.maxAmount);
  if (min === undefined || max === undefined) return "Montant invalide, par exemple 64,28.";
  if (min !== null && max !== null && min > max) return "Le minimum dépasse le maximum.";
  return "";
}

/** "all", "none" (to categorize) or a category id, as in the category select. */
export type CategoryChoice = "all" | "none" | number;

/**
 * Backend filter of the Operations screen. Invalid amount bounds are left out, since the form
 * shows their error. `accountId` is `null` for every account, archived ones excluded.
 */
export function toOperationFilter(
  accountId: number | null,
  period: { from: string | null; to: string | null },
  category: CategoryChoice,
  advanced: AdvancedFilters,
): OperationFilter {
  const valid = amountBoundsError(advanced) === "";
  let categoryFilter: CategoryFilter | null = null;
  if (category === "none") categoryFilter = { kind: "uncategorized" };
  else if (typeof category === "number") categoryFilter = { kind: "category", id: category };
  return {
    accountId,
    includeArchived: false,
    from: period.from,
    to: period.to,
    direction: advanced.direction === "all" ? null : advanced.direction,
    minCents: valid ? (bound(advanced.minAmount) ?? null) : null,
    maxCents: valid ? (bound(advanced.maxAmount) ?? null) : null,
    payeeId: advanced.payeeId,
    category: categoryFilter,
    tagId: advanced.tagId,
  };
}
