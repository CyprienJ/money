import { describe, expect, it } from "vitest";
import { formatAmount, formatDate } from "./format";

// Intl groups thousands with a narrow no-break space; normalize to plain spaces for readability.
const plain = (text: string) => text.replace(/[  ]/g, " ");

describe("formatAmount", () => {
  it("formats cents with thousands grouping", () => {
    expect(plain(formatAmount(218432))).toBe("2 184,32 €");
    expect(plain(formatAmount(21000000))).toBe("210 000,00 €");
  });

  it("uses a typographic minus for debits", () => {
    expect(plain(formatAmount(-6428))).toBe("−64,28 €");
    expect(plain(formatAmount(-12699000))).toBe("−126 990,00 €");
  });

  it("adds a plus sign only when requested", () => {
    expect(plain(formatAmount(328000, { signed: true }))).toBe("+3 280,00 €");
    expect(plain(formatAmount(328000))).toBe("3 280,00 €");
  });

  it("pads cents", () => {
    expect(plain(formatAmount(5))).toBe("0,05 €");
    expect(plain(formatAmount(-840))).toBe("−8,40 €");
  });

  it("never signs zero", () => {
    expect(plain(formatAmount(0, { signed: true }))).toBe("0,00 €");
    expect(plain(formatAmount(-49, { cents: false }))).toBe("0 €");
  });

  it("rounds to the nearest euro without cents", () => {
    expect(plain(formatAmount(14823999, { cents: false }))).toBe("148 240 €");
    expect(plain(formatAmount(-214560, { cents: false }))).toBe("−2 146 €");
    expect(plain(formatAmount(113449, { cents: false, signed: true }))).toBe("+1 134 €");
  });

  it("rejects non-integer amounts", () => {
    expect(() => formatAmount(12.5)).toThrow(RangeError);
  });
});

describe("formatDate", () => {
  it("formats day and month", () => {
    expect(formatDate("2026-10-05", "day-month")).toBe("05/10");
  });

  it("formats a short date by default", () => {
    expect(formatDate("2026-10-19")).toBe("19/10/2026");
  });

  it("formats a long date with a capital letter", () => {
    expect(formatDate("2026-10-20", "long")).toBe("Mardi 20 octobre 2026");
  });

  it("rejects malformed dates", () => {
    expect(() => formatDate("20/10/2026")).toThrow(RangeError);
  });
});
