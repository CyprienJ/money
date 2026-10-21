import { describe, expect, it } from "vitest";
import {
  activeFilterCount,
  amountBoundsError,
  emptyAdvancedFilters,
  toOperationFilter,
} from "./operationFilters";

const period = { from: "2026-10-01", to: "2026-10-31" };

describe("toOperationFilter", () => {
  it("leaves out unused filters", () => {
    expect(toOperationFilter(3, period, "all", emptyAdvancedFilters())).toEqual({
      accountId: 3,
      includeArchived: false,
      from: "2026-10-01",
      to: "2026-10-31",
      direction: null,
      minCents: null,
      maxCents: null,
      payeeId: null,
      category: null,
      tagId: null,
    });
  });

  it("converts the category choice and the amounts", () => {
    const advanced = {
      direction: "debit" as const,
      minAmount: "50",
      maxAmount: "-199,9",
      payeeId: 4,
      tagId: 2,
    };
    const filter = toOperationFilter(null, period, 7, advanced);
    expect(filter.category).toEqual({ kind: "category", id: 7 });
    expect(filter).toMatchObject({
      accountId: null,
      direction: "debit",
      minCents: 5000,
      maxCents: 19990,
      payeeId: 4,
      tagId: 2,
    });
    expect(toOperationFilter(null, period, "none", advanced).category).toEqual({ kind: "uncategorized" });
  });

  it("ignores invalid amount bounds", () => {
    const advanced = { ...emptyAdvancedFilters(), minAmount: "abc", maxAmount: "20" };
    expect(amountBoundsError(advanced)).not.toBe("");
    expect(toOperationFilter(null, period, "all", advanced)).toMatchObject({ minCents: null, maxCents: null });
  });
});

describe("amountBoundsError", () => {
  it("rejects a minimum above the maximum", () => {
    expect(amountBoundsError({ ...emptyAdvancedFilters(), minAmount: "30", maxAmount: "20" })).toBe(
      "Le minimum dépasse le maximum.",
    );
    expect(amountBoundsError({ ...emptyAdvancedFilters(), minAmount: "20", maxAmount: "20" })).toBe("");
  });
});

describe("activeFilterCount", () => {
  it("counts the amount bounds as one filter", () => {
    expect(activeFilterCount(emptyAdvancedFilters())).toBe(0);
    expect(
      activeFilterCount({ direction: "credit", minAmount: "10", maxAmount: "20", payeeId: null, tagId: 1 }),
    ).toBe(3);
  });
});
