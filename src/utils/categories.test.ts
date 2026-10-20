import { describe, expect, it } from "vitest";
import type { Category } from "../bindings";
import { categoryTree, managementTree } from "./categories";

function category(id: number, name: string, parentId: number | null = null, isInternalTransfer = false): Category {
  return {
    id,
    name,
    parentId,
    color: "neutral",
    isInternalTransfer,
    isIncome: false,
    operationCount: 0,
  };
}

const categories = [
  category(1, "Virement interne", null, true),
  category(2, "Santé"),
  category(3, "Pharmacie", 2),
  category(4, "Abonnements"),
  category(5, "Médecin", 2),
];

describe("categoryTree", () => {
  it("groups subcategories under their parent, sorted by name", () => {
    expect(categoryTree(categories).map((g) => [g.parent.name, g.children.map((c) => c.name)])).toEqual([
      ["Abonnements", []],
      ["Santé", ["Médecin", "Pharmacie"]],
      ["Virement interne", []],
    ]);
  });
});

describe("managementTree", () => {
  it("puts the internal transfer category last", () => {
    expect(managementTree([category(9, "Voyages"), ...categories]).map((g) => g.parent.name)).toEqual([
      "Abonnements",
      "Santé",
      "Voyages",
      "Virement interne",
    ]);
  });
});
