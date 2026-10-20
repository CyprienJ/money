/** Typographic minus sign, used for negative amounts as in the mockups. */
export const MINUS = "−";
const NBSP = " ";

/**
 * No-break space: same width as the plain space of the mockups, but keeps amounts on one line.
 * (The narrow no-break space U+202F is nearly invisible in Plus Jakarta Sans.)
 */
const THOUSANDS_SEPARATOR = "\u00a0";

/** Groups digits by three. Done by hand: Intl locale data varies between web engines. */
function groupThousands(value: number): string {
  return String(value).replace(/\B(?=(\d{3})+(?!\d))/g, THOUSANDS_SEPARATOR);
}

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
    ? `${groupThousands(Math.floor(abs / 100))},${String(abs % 100).padStart(2, "0")}`
    : groupThousands(Math.floor((abs + 50) / 100));

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

/**
 * Parses an amount typed in euros (`"1 234,56"`, `"-8.4"`, `"−12"`) into cents.
 * Returns `null` when the text is not a valid amount with at most two decimals.
 * Parsing is done on the digits, without floating point.
 */
export function parseAmount(text: string): number | null {
  const compact = text.replace(/[\s  €]/g, "").replace(MINUS, "-");
  const match = /^([+-]?)(\d+)(?:[.,](\d{1,2}))?$/.exec(compact);
  if (!match) {
    return null;
  }
  const [, sign, euros, decimals = ""] = match;
  const cents = Number(euros) * 100 + Number(decimals.padEnd(2, "0"));
  if (!Number.isSafeInteger(cents)) {
    return null;
  }
  return sign === "-" ? -cents : cents;
}

/** Formats cents for an editable input: `-123456` → `"-1234,56"`. */
export function formatAmountInput(amountCents: number): string {
  const abs = Math.abs(amountCents);
  const text = `${Math.floor(abs / 100)},${String(abs % 100).padStart(2, "0")}`;
  return amountCents < 0 ? `-${text}` : text;
}

/** Today's local date as `YYYY-MM-DD`. */
export function todayIso(): string {
  const now = new Date();
  return [
    now.getFullYear(),
    String(now.getMonth() + 1).padStart(2, "0"),
    String(now.getDate()).padStart(2, "0"),
  ].join("-");
}

/** Local date (`YYYY-MM-DD`) of a UTC timestamp such as `2026-10-20T21:30:00Z`. */
export function localDateOf(timestamp: string): string {
  const date = new Date(timestamp);
  return [
    date.getFullYear(),
    String(date.getMonth() + 1).padStart(2, "0"),
    String(date.getDate()).padStart(2, "0"),
  ].join("-");
}

/** "12 opérations" / "1 opération". */
export function operationCount(count: number): string {
  return `${count} opération${count > 1 ? "s" : ""}`;
}

/** Small French words kept in lowercase inside a displayed label. */
const SMALL_WORDS = new Set(["à", "au", "aux", "d", "de", "des", "du", "en", "et", "l", "la", "le", "les", "par", "pour", "sur"]);

/**
 * Readable form of a normalized label: a capital at the start of each word, small words in
 * lowercase, words with digits unchanged. `PHARMACIE DU CENTRE` → `Pharmacie du Centre`.
 */
export function displayLabel(normalizedLabel: string): string {
  return normalizedLabel
    .split(" ")
    .map((word, index) => {
      if (/\d/.test(word)) return word;
      const lower = word.toLocaleLowerCase("fr");
      if (index > 0 && SMALL_WORDS.has(lower)) return lower;
      // Capitalize each part of "SAINT-MARTIN" or "L'ARBRE".
      return lower.replace(/(^|[-'’])(\p{L})/gu, (_, separator: string, letter: string) =>
        separator + letter.toLocaleUpperCase("fr"),
      );
    })
    .join(" ");
}
