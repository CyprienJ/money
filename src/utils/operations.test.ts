import { describe, expect, it } from "vitest";
import type { Operation } from "../bindings";
import { matchesSearch, summarize } from "./operations";

function operation(fields: Partial<Operation>): Operation {
  return {
    id: 1,
    accountId: 1,
    date: "2026-10-19",
    valueDate: null,
    amountCents: -6428,
    rawLabel: "CB CARREFOUR MKT 18/10 CARTE 4417",
    normalizedLabel: "CARREFOUR MKT",
    payeeId: 1,
    payeeName: "Carrefour Mkt",
    importBatchId: 1,
    categoryId: 2,
    isInternalTransfer: false,
    ...fields,
  };
}

describe("summarize", () => {
  it("matches the Operations mockup footer", () => {
    const totals = summarize([
      operation({ amountCents: 328000 }),
      operation({ amountCents: -30000, isInternalTransfer: true }),
      operation({ amountCents: -84200 }),
      operation({ amountCents: -840, categoryId: null }),
    ]);
    expect(totals).toEqual({
      count: 4,
      incomeCents: 328000,
      expenseCents: -85040,
      internalTransferCents: -30000,
      uncategorized: 1,
    });
  });
});

describe("matchesSearch", () => {
  const carrefour = operation({});

  it("searches both labels, ignoring case and accents", () => {
    expect(matchesSearch(carrefour, "carrefour")).toBe(true);
    expect(matchesSearch(carrefour, "carte 4417")).toBe(true);
    expect(matchesSearch(operation({ rawLabel: "Crédit immobilier" }), "credit")).toBe(true);
    expect(matchesSearch(carrefour, "sncf")).toBe(false);
  });

  it("searches the formatted amount", () => {
    expect(matchesSearch(carrefour, "64,28")).toBe(true);
    expect(matchesSearch(carrefour, "-64,28")).toBe(true);
    expect(matchesSearch(operation({ amountCents: -84200 }), "842")).toBe(true);
  });

  it("matches everything with an empty query", () => {
    expect(matchesSearch(carrefour, "  ")).toBe(true);
  });
});
