/** Typographic minus sign, used for negative amounts as in the mockups. */
export const MINUS = "−";
const NBSP = " ";

const integerFormat = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 0 });

export interface AmountFormatOptions {
  /** Prefix positive amounts with "+" (negative amounts always get "−"). */
  signed?: boolean;
  /** Show cents ("1 234,56 €"); otherwise round to the nearest euro ("1 235 €"). */
  cents?: boolean;
}

/**
 * Formats an amount in cents as euros, e.g. `-123456` → `"−1 234,56 €"`.
 * Uses integer arithmetic only: amounts never go through floating point.
 */
export function formatAmount(amountCents: number, options: AmountFormatOptions = {}): string {
  const { signed = false, cents = true } = options;
  if (!Number.isSafeInteger(amountCents)) {
    throw new RangeError(`amount must be an integer number of cents, got ${amountCents}`);
  }

  const abs = Math.abs(amountCents);
  const body = cents
    ? `${integerFormat.format(Math.floor(abs / 100))},${String(abs % 100).padStart(2, "0")}`
    : integerFormat.format(Math.floor((abs + 50) / 100));

  const isZero = cents ? abs === 0 : abs < 50;
  const sign = isZero ? "" : amountCents < 0 ? MINUS : signed ? "+" : "";
  return `${sign}${body}${NBSP}€`;
}

export type DateStyle = "day-month" | "short" | "long";

const longFormat = new Intl.DateTimeFormat("fr-FR", {
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
});

/**
 * Formats an ISO date (`YYYY-MM-DD`):
 * - `day-month`: `19/10`
 * - `short`: `19/10/2026`
 * - `long`: `Mardi 20 octobre 2026`
 */
export function formatDate(isoDate: string, style: DateStyle = "short"): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(isoDate);
  if (!match) {
    throw new RangeError(`expected a YYYY-MM-DD date, got "${isoDate}"`);
  }
  const [, year, month, day] = match;

  switch (style) {
    case "day-month":
      return `${day}/${month}`;
    case "short":
      return `${day}/${month}/${year}`;
    case "long": {
      // Built from local components so the day never shifts with the time zone.
      const date = new Date(Number(year), Number(month) - 1, Number(day));
      const text = longFormat.format(date);
      return text.charAt(0).toUpperCase() + text.slice(1);
    }
  }
}
