import type { Payee } from "../bindings";
import { displayLabel } from "./format";
import { simplify } from "./operations";

/** Payees by name, in French alphabetical order. */
export function sortPayees(payees: Payee[]): Payee[] {
  return [...payees].sort((a, b) => a.name.localeCompare(b.name, "fr"));
}

/** Search on the payee name and its labels, ignoring case and accents. */
export function matchesPayee(payee: Payee, query: string): boolean {
  const needle = simplify(query);
  if (needle === "") return true;
  return [payee.name, ...payee.labels].some((text) => simplify(text).includes(needle));
}

/**
 * Labels of a payee as shown under its name, when they say more than the name:
 * nothing for a single label matching the name, the labels otherwise.
 */
export function labelSummary(payee: Payee): string {
  if (payee.labels.length === 1 && displayLabel(payee.labels[0]) === payee.name) return "";
  return payee.labels.map(displayLabel).join(" · ");
}
