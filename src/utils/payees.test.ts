import { describe, expect, it } from "vitest";
import type { Payee } from "../bindings";
import { labelSummary, matchesPayee, sortPayees } from "./payees";

function payee(fields: Partial<Payee>): Payee {
  return {
    id: 1,
    name: "Carrefour Mkt",
    labels: ["CARREFOUR MKT"],
    operationCount: 3,
    totalCents: -12000,
    lastDate: "2026-10-19",
    ...fields,
  };
}

describe("sortPayees", () => {
  it("sorts by name in French order", () => {
    const sorted = sortPayees([payee({ name: "Zara" }), payee({ name: "Épicerie" }), payee({ name: "Amazon" })]);
    expect(sorted.map((p) => p.name)).toEqual(["Amazon", "Épicerie", "Zara"]);
  });
});

describe("matchesPayee", () => {
  it("searches the name and the labels, ignoring case and accents", () => {
    const merged = payee({ name: "Carrefour", labels: ["CARREFOUR CITY", "CARREFOUR MKT"] });
    expect(matchesPayee(merged, "")).toBe(true);
    expect(matchesPayee(merged, "carref")).toBe(true);
    expect(matchesPayee(merged, "city")).toBe(true);
    expect(matchesPayee(merged, "lidl")).toBe(false);
    expect(matchesPayee(payee({ name: "Épicerie" }), "epi")).toBe(true);
  });
});

describe("labelSummary", () => {
  it("shows the labels only when they say more than the name", () => {
    expect(labelSummary(payee({}))).toBe("");
    expect(labelSummary(payee({ name: "Carrefour" }))).toBe("Carrefour Mkt");
    expect(labelSummary(payee({ name: "Carrefour", labels: ["CARREFOUR CITY", "CARREFOUR MKT"] }))).toBe(
      "Carrefour City · Carrefour Mkt",
    );
  });
});
