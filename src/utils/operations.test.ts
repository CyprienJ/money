import { describe, expect, it } from "vitest";
import type { Operation } from "../bindings";
import { matchesSearch, paginate, summarize } from "./operations";

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
    hasSourceDocument: false,
    sourcePage: null,
    sourceLine: null,
    categoryId: 2,
    splits: [],
    isInternalTransfer: false,
    transferPairId: null,
    note: null,
    tags: [],
    attachmentCount: 0,
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
      // A split operation is categorized by its parts.
      operation({
        amountCents: -1000,
        categoryId: null,
        splits: [
          { categoryId: 2, amountCents: -600 },
          { categoryId: 3, amountCents: -400 },
        ],
      }),
    ]);
    expect(totals).toEqual({
      count: 5,
      incomeCents: 328000,
      expenseCents: -86040,
      internalTransferCents: -30000,
      uncategorized: 1,
    });
  });
});

describe("matchesSearch", () => {
  const carrefour = operation({});

  it("searches the labels, the note and the tags, ignoring case and accents", () => {
    expect(matchesSearch(carrefour, "carrefour")).toBe(true);
    expect(matchesSearch(carrefour, "carte 4417")).toBe(true);
    expect(matchesSearch(operation({ rawLabel: "Crédit immobilier" }), "credit")).toBe(true);
    expect(matchesSearch(operation({ note: "Anniversaire de Léa" }), "lea")).toBe(true);
    expect(matchesSearch(operation({ tags: ["Vacances d'été"] }), "ETE")).toBe(true);
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

describe("paginate", () => {
  const items = Array.from({ length: 120 }, (_, i) => i);

  it("cuts the items into pages", () => {
    expect(paginate(items, 1, 50)).toMatchObject({ page: 1, pageCount: 3 });
    expect(paginate(items, 3, 50).items).toEqual(items.slice(100));
  });

  it("stays on an existing page", () => {
    expect(paginate(items, 9, 50)).toMatchObject({ page: 3, pageCount: 3 });
    expect(paginate([], 2, 50)).toEqual({ items: [], page: 1, pageCount: 1 });
  });
});
