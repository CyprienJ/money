import type { Frequency, ScheduledOperation } from "../bindings";

export const FREQUENCIES: { value: Frequency; label: string }[] = [
  { value: "WEEKLY", label: "Chaque semaine" },
  { value: "MONTHLY", label: "Tous les mois" },
  { value: "QUARTERLY", label: "Tous les trimestres" },
  { value: "SEMIANNUAL", label: "Tous les six mois" },
  { value: "YEARLY", label: "Tous les ans" },
];

/** Weekdays numbered as `day_of_period` of a weekly schedule (1 = Monday). */
export const WEEKDAYS = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

const MONTHS = [
  "janvier",
  "février",
  "mars",
  "avril",
  "mai",
  "juin",
  "juillet",
  "août",
  "septembre",
  "octobre",
  "novembre",
  "décembre",
];

const dayText = (day: number) => (day === 1 ? "1er" : String(day));

/**
 * Plain description of a schedule: "Tous les mois, le 5", "Chaque semaine, le vendredi",
 * "Tous les ans, le 1er janvier".
 */
export function describeSchedule(schedule: Pick<ScheduledOperation, "frequency" | "dayOfPeriod" | "startDate">): string {
  const label = FREQUENCIES.find((f) => f.value === schedule.frequency)?.label ?? "";
  const day = schedule.dayOfPeriod;
  switch (schedule.frequency) {
    case "WEEKLY":
      return `${label}, le ${WEEKDAYS[day - 1]}`;
    case "YEARLY":
      return `${label}, le ${dayText(day)} ${MONTHS[Number(schedule.startDate.slice(5, 7)) - 1]}`;
    default:
      return `${label}, le ${dayText(day)}`;
  }
}
