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
