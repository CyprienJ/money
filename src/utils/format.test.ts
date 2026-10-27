import { describe, expect, it } from "vitest";
import {
  displayLabel,
  formatAmount,
  formatAmountInput,
  formatDate,
  formatFileSize,
  localDateOf,
  parseAmount,
} from "./format";

// Intl groups thousands with a narrow no-break space; normalize to plain spaces for readability.
const plain = (text: string) => text.replace(/[  ]/g, " ");

describe("formatAmount", () => {
  it("formats cents with thousands grouping", () => {
    expect(plain(formatAmount(218432))).toBe("2 184,32 €");
    expect(plain(formatAmount(21000000))).toBe("210 000,00 €");
  });

  it("writes the symbol or the code of other currencies", () => {
    expect(plain(formatAmount(-6428, { currency: "USD" }))).toBe("−64,28 $");
    expect(plain(formatAmount(6428, { currency: "GBP" }))).toBe("64,28 £");
    expect(plain(formatAmount(6428, { currency: "CHF" }))).toBe("64,28 CHF");
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

describe("parseAmount", () => {
  it("parses euros and cents with a comma or a dot", () => {
    expect(parseAmount("1234,56")).toBe(123456);
    expect(parseAmount("8.4")).toBe(840);
    expect(parseAmount("12")).toBe(1200);
    expect(parseAmount("0,05")).toBe(5);
  });

  it("accepts spaces, the euro sign and both minus signs", () => {
    expect(parseAmount(" 1 234,56 € ")).toBe(123456);
    expect(parseAmount("-842")).toBe(-84200);
    expect(parseAmount("−64,28")).toBe(-6428);
    expect(parseAmount("+3280")).toBe(328000);
  });

  it("does not lose precision on large amounts", () => {
    expect(parseAmount("210000,01")).toBe(21000001);
    expect(parseAmount("0,29")).toBe(29);
  });

  it("rejects invalid input", () => {
    for (const text of ["", "abc", "1,234", "12,", ",5", "1,2,3", "--5"]) {
      expect(parseAmount(text)).toBeNull();
    }
  });
});

describe("formatAmountInput", () => {
  it("round-trips with parseAmount", () => {
    for (const cents of [0, 5, 840, -6428, 21000001]) {
      expect(parseAmount(formatAmountInput(cents))).toBe(cents);
    }
    expect(formatAmountInput(-123456)).toBe("-1234,56");
  });
});

describe("localDateOf", () => {
  it("returns the local date of a UTC timestamp", () => {
    // Noon UTC is the same day in every time zone between UTC−11 and UTC+11.
    expect(localDateOf("2026-10-20T12:00:00Z")).toBe("2026-10-20");
  });
});

describe("displayLabel", () => {
  it("capitalizes words and keeps small words in lowercase", () => {
    expect(displayLabel("CARREFOUR MKT")).toBe("Carrefour Mkt");
    expect(displayLabel("PHARMACIE DU CENTRE")).toBe("Pharmacie du Centre");
    expect(displayLabel("VERS LIVRET A")).toBe("Vers Livret A");
    expect(displayLabel("LE PETIT BISTROT")).toBe("Le Petit Bistrot");
    expect(displayLabel("LOYER SCI DES ÉRABLES")).toBe("Loyer Sci des Érables");
  });

  it("handles hyphens, apostrophes and digits", () => {
    expect(displayLabel("BOULANGERIE SAINT-MARTIN")).toBe("Boulangerie Saint-Martin");
    expect(displayLabel("CAFE L'ARBRE")).toBe("Cafe L'Arbre");
    expect(displayLabel("RETRAIT DAB PARIS 11")).toBe("Retrait Dab Paris 11");
    expect(displayLabel("FNAC.COM")).toBe("Fnac.com");
  });
});

describe("formatFileSize", () => {
  it("picks a readable unit", () => {
    expect(formatFileSize(820)).toBe("820 o");
    expect(formatFileSize(12 * 1024)).toBe("12 Ko");
    expect(formatFileSize(3.4 * 1024 * 1024)).toBe("3,4 Mo");
    expect(formatFileSize(25 * 1024 * 1024)).toBe("25 Mo");
  });
});
