import { describe, expect, it } from "vitest";
import {
  formatBasisPoints,
  formatQuantity,
  formatQuantityInput,
  gainBasisPoints,
  parseQuantity,
  priceUpdateLines,
  unitPriceCents,
} from "./portfolio";

describe("parseQuantity", () => {
  it("reads up to 6 decimals into millionths", () => {
    expect(parseQuantity("12,5")).toBe(12_500_000);
    expect(parseQuantity("0.123456")).toBe(123_456);
    expect(parseQuantity("1 000")).toBe(1_000_000_000);
    expect(parseQuantity("3")).toBe(3_000_000);
  });

  it("refuses zero, signs, and more than 6 decimals", () => {
    expect(parseQuantity("0")).toBeNull();
    expect(parseQuantity("-1")).toBeNull();
    expect(parseQuantity("0,1234567")).toBeNull();
    expect(parseQuantity("abc")).toBeNull();
  });
});

describe("formatQuantity", () => {
  it("drops trailing zeros and groups thousands", () => {
    expect(formatQuantity(12_500_000)).toBe("12,5");
    expect(formatQuantity(123_456)).toBe("0,123456");
    expect(formatQuantity(1_500_000_000)).toBe("1 500");
    expect(formatQuantityInput(1_500_250_000)).toBe("1500,25");
  });
});

describe("unitPriceCents", () => {
  it("divides the amount by the quantity, rounded half up", () => {
    expect(unitPriceCents(30_000, 2_500_000)).toBe(12_000);
    expect(unitPriceCents(100, 3_000_000)).toBe(33);
    expect(unitPriceCents(5, 2_000_000)).toBe(3);
  });
});

describe("priceUpdateLines", () => {
  it("sums up the update and explains each failure", () => {
    expect(priceUpdateLines({ updated: 1, failures: [] })).toEqual(["Cours mis à jour pour 1 titre."]);
    const lines = priceUpdateLines({
      updated: 0,
      failures: [{ securityName: "Vieux titre", error: { code: "unknownSymbol", params: "OLD.PA" } }],
    });
    expect(lines).toEqual(["Aucun cours mis à jour.", "Vieux titre : Le symbole OLD.PA est inconnu de Yahoo Finance."]);
  });
});

describe("formatBasisPoints", () => {
  it("writes hundredths of a percent with their sign", () => {
    expect(formatBasisPoints(1_645)).toBe("+16,45\u00a0%");
    expect(formatBasisPoints(-5)).toBe("−0,05\u00a0%");
    expect(formatBasisPoints(0)).toBe("0,00\u00a0%");
  });
});

describe("gainBasisPoints", () => {
  it("divides the gain by the cost, rounded", () => {
    expect(gainBasisPoints(20_000, 100_000)).toBe(2_000);
    expect(gainBasisPoints(-1, 3_000)).toBe(-3);
    expect(gainBasisPoints(100, 0)).toBeNull();
  });
});
