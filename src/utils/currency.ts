import { commands } from "../bindings";

/** Currency of the statistics: every other amount is converted to it. */
export const REFERENCE_CURRENCY = "EUR";

/** Billionths in a unit: a rate is stored as euros for one unit of the currency × this. */
const RATE_DECIMALS = 9;

const CURRENCY_NAMES: Record<string, string> = {
  EUR: "Euro",
  USD: "Dollar américain",
  GBP: "Livre sterling",
  CHF: "Franc suisse",
  CAD: "Dollar canadien",
  AUD: "Dollar australien",
  NZD: "Dollar néo-zélandais",
  SEK: "Couronne suédoise",
  NOK: "Couronne norvégienne",
  DKK: "Couronne danoise",
  PLN: "Zloty",
  CZK: "Couronne tchèque",
  HKD: "Dollar de Hong Kong",
  SGD: "Dollar de Singapour",
};

/** "USD · Dollar américain", for the currency selects. */
export function currencyLabel(code: string): string {
  const name = CURRENCY_NAMES[code];
  return name ? `${code} · ${name}` : code;
}

/**
 * Parses a rate typed as euros for one unit (`"0,8924"`, `"1.15"`) into billionths.
 * Returns `null` when the text is not a positive number with at most 9 decimals.
 * Parsing is done on the digits, without floating point.
 */
export function parseRate(text: string): number | null {
  const match = /^(\d+)(?:[.,](\d{1,9}))?$/.exec(text.replace(/\s/g, ""));
  if (!match) return null;
  const [, units, decimals = ""] = match;
  const nano = BigInt(units) * 10n ** BigInt(RATE_DECIMALS) + BigInt(decimals.padEnd(RATE_DECIMALS, "0"));
  if (nano <= 0n || nano > BigInt(Number.MAX_SAFE_INTEGER)) return null;
  return Number(nano);
}

/** Formats a rate in billionths without trailing zeros, at least 4 decimals: `"0,8924"`. */
export function formatRate(rateNano: number): string {
  const text = String(rateNano).padStart(RATE_DECIMALS + 1, "0");
  const units = text.slice(0, -RATE_DECIMALS);
  const decimals = text.slice(-RATE_DECIMALS).replace(/0+$/, "").padEnd(4, "0");
  return `${units},${decimals}`;
}

let supported: Promise<string[]> | null = null;

/** Supported currencies, euros first; asked to the backend once. */
export function supportedCurrencies(): Promise<string[]> {
  supported ??= commands.supportedCurrencies();
  return supported;
}
