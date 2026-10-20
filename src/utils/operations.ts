import type { Operation } from "../bindings";
import { formatAmount } from "./format";

export interface OperationTotals {
  count: number;
  /** Credits, internal transfers excluded. */
  incomeCents: number;
  /** Debits (negative), internal transfers excluded. */
  expenseCents: number;
  /** Net amount of internal transfers. */
  internalTransferCents: number;
  uncategorized: number;
}

/** Footer totals of the operations table. Internal transfers are kept apart. */
export function summarize(operations: Operation[]): OperationTotals {
  const totals: OperationTotals = {
    count: operations.length,
    incomeCents: 0,
    expenseCents: 0,
    internalTransferCents: 0,
    uncategorized: 0,
  };
  for (const operation of operations) {
    if (operation.isInternalTransfer) {
      totals.internalTransferCents += operation.amountCents;
    } else if (operation.amountCents > 0) {
      totals.incomeCents += operation.amountCents;
    } else {
      totals.expenseCents += operation.amountCents;
    }
    if (operation.categoryId === null) {
      totals.uncategorized += 1;
    }
  }
  return totals;
}

function simplify(text: string): string {
  return text
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[\s  ]+/g, " ")
    .toLowerCase()
    .trim();
}

/**
 * Free-text search on the labels and the amount, ignoring case and accents.
 * `"64,28"` and `"-64,28"` both match an amount of −64,28 €.
 */
export function matchesSearch(operation: Operation, query: string): boolean {
  const needle = simplify(query).replace("−", "-");
  if (needle === "") {
    return true;
  }
  const amount = simplify(formatAmount(operation.amountCents)).replace("−", "-");
  return [operation.rawLabel, operation.normalizedLabel, amount].some((text) =>
    simplify(text).includes(needle),
  );
}
