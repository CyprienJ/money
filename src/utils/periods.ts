export interface Period {
  id: string;
  label: string;
  /** First day included (`YYYY-MM-DD`), `null` for no lower bound. */
  from: string | null;
  /** Last day included (`YYYY-MM-DD`), `null` for no upper bound. */
  to: string | null;
}

const monthFormat = new Intl.DateTimeFormat("fr-FR", { month: "long", year: "numeric" });

function iso(year: number, monthIndex: number, day: number): string {
  // Date normalizes overflowing months and days (e.g. month -1, day 0).
  const date = new Date(year, monthIndex, day);
  return [
    date.getFullYear(),
    String(date.getMonth() + 1).padStart(2, "0"),
    String(date.getDate()).padStart(2, "0"),
  ].join("-");
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/**
 * Periods offered in the operations filter: the last 12 calendar months (current first),
 * then rolling ranges ending today and the whole history.
 */
export function periodOptions(today: Date = new Date()): Period[] {
  const year = today.getFullYear();
  const month = today.getMonth();
  const todayIso = iso(year, month, today.getDate());

  const months: Period[] = Array.from({ length: 12 }, (_, offset) => {
    const first = new Date(year, month - offset, 1);
    return {
      id: iso(first.getFullYear(), first.getMonth(), 1).slice(0, 7),
      label: capitalize(monthFormat.format(first)),
      from: iso(first.getFullYear(), first.getMonth(), 1),
      to: iso(first.getFullYear(), first.getMonth() + 1, 0),
    };
  });

  return [
    ...months,
    { id: "last-3-months", label: "3 derniers mois", from: iso(year, month - 2, 1), to: todayIso },
    { id: "last-12-months", label: "12 derniers mois", from: iso(year, month - 11, 1), to: todayIso },
    { id: "all", label: "Tout l'historique", from: null, to: null },
  ];
}

export type AnalysisGranularity = "month" | "quarter" | "year";

export interface AnalysisPeriod {
  /** "Octobre 2026", "3e trimestre 2026" or "2026". */
  label: string;
  /** First and last days of the period (`YYYY-MM-DD`), both included. */
  from: string;
  to: string;
  /** The 12 months ending with the period, for the income and expenses trend. */
  trendFrom: string;
  /** Last day before `trendFrom`: the reference of the 12-month variations. */
  trendBase: string;
}

/**
 * Period of the Analyses screen: the current month, quarter or year for `offset` 0, the
 * previous one for -1, and so on.
 */
export function analysisPeriod(
  granularity: AnalysisGranularity,
  offset: number,
  today: Date = new Date(),
): AnalysisPeriod {
  const length = { month: 1, quarter: 3, year: 12 }[granularity];
  const year = today.getFullYear();
  // First month (0-based) of the current period.
  const month = today.getMonth();
  const currentStart = { month, quarter: month - (month % 3), year: 0 }[granularity];
  const start = new Date(year, currentStart + offset * length, 1);
  const startYear = start.getFullYear();
  const startMonth = start.getMonth();

  let label: string;
  if (granularity === "month") label = capitalize(monthFormat.format(start));
  else if (granularity === "quarter") label = `${startMonth / 3 + 1}e trimestre ${startYear}`.replace(/^1e/, "1er");
  else label = String(startYear);

  return {
    label,
    from: iso(startYear, startMonth, 1),
    to: iso(startYear, startMonth + length, 0),
    trendFrom: iso(startYear, startMonth + length - 12, 1),
    trendBase: iso(startYear, startMonth + length - 12, 0),
  };
}

/** Period compared with the selected one in the Analyses screen. */
export type ComparisonMode = "previousYear" | "previousPeriod";

/** The same period a year earlier, or the period just before. */
export function comparedPeriod(
  granularity: AnalysisGranularity,
  offset: number,
  mode: ComparisonMode,
  today: Date = new Date(),
): AnalysisPeriod {
  const periodsPerYear = { month: 12, quarter: 4, year: 1 }[granularity];
  const shift = mode === "previousYear" ? periodsPerYear : 1;
  return analysisPeriod(granularity, offset - shift, today);
}
