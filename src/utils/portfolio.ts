import type { Position, PriceUpdate, TransactionKind } from "../bindings";
import { priceMessage } from "./errorMessages";
import { groupThousands } from "./format";

/** Millionths of a unit in a unit, as on the backend. */
export const QUANTITY_SCALE = 1_000_000;

export const TRANSACTION_KINDS: { value: TransactionKind; label: string }[] = [
  { value: "BUY", label: "Achat" },
  { value: "SELL", label: "Vente" },
  { value: "DIVIDEND", label: "Dividende" },
];

export function kindLabel(kind: TransactionKind): string {
  return TRANSACTION_KINDS.find((k) => k.value === kind)?.label ?? kind;
}

/**
 * Parses a positive quantity with at most 6 decimals ("12,5", "0.123456") into millionths of a
 * unit, without floating point; `null` when unreadable or zero.
 */
export function parseQuantity(text: string): number | null {
  const compact = text.replace(/[\s  ]/g, "");
  const match = /^(\d+)(?:[.,](\d{1,6}))?$/.exec(compact);
  if (!match) return null;
  const [, units, decimals = ""] = match;
  const micro = Number(units) * QUANTITY_SCALE + Number(decimals.padEnd(6, "0"));
  return Number.isSafeInteger(micro) && micro > 0 ? micro : null;
}

/** Quantity in millionths of a unit, without trailing zeros: `12500000` → `"12,5"`. */
export function formatQuantity(micro: number): string {
  const units = Math.floor(micro / QUANTITY_SCALE);
  const decimals = String(micro % QUANTITY_SCALE).padStart(6, "0").replace(/0+$/, "");
  return decimals ? `${groupThousands(units)},${decimals}` : groupThousands(units);
}

/** Quantity for an editable input: `12500000` → `"12,5"` without thousands separators. */
export function formatQuantityInput(micro: number): string {
  return formatQuantity(micro).replace(/[\s  ]/g, "");
}

/** Price of one unit in cents, rounded half up: amount ÷ quantity, in integers. */
export function unitPriceCents(amountCents: number, quantityMicro: number): number {
  const [amount, quantity] = [BigInt(amountCents), BigInt(quantityMicro)];
  return Number((amount * BigInt(QUANTITY_SCALE) * 2n + quantity) / (2n * quantity));
}

/** Summary of a price update, then one line per security or exchange rate ("USD/EUR") that failed. */
export function priceUpdateLines(update: PriceUpdate): string[] {
  const { updated, failures } = update;
  const summary =
    updated === 0
      ? "Aucun cours mis à jour."
      : `Cours mis à jour pour ${updated} titre${updated > 1 ? "s" : ""} ou devise${updated > 1 ? "s" : ""}.`;
  return [summary, ...failures.map((f) => `${f.name} : ${priceMessage(f.error)}`)];
}

/** Hundredths of a percent as a signed percentage: `1645` → `"+16,45 %"`. */
export function formatBasisPoints(basisPoints: number): string {
  const abs = Math.abs(basisPoints);
  const sign = basisPoints > 0 ? "+" : basisPoints < 0 ? "−" : "";
  return `${sign}${Math.floor(abs / 100)},${String(abs % 100).padStart(2, "0")} %`;
}

/** Gain over cost in hundredths of a percent, rounded; `null` without cost. */
export function gainBasisPoints(gainCents: number, costCents: number): number | null {
  if (costCents <= 0) return null;
  const [gain, cost] = [BigInt(gainCents), BigInt(costCents)];
  const doubled = gain * 20_000n;
  const rounded = doubled >= 0n ? (doubled + cost) / (2n * cost) : (doubled - cost) / (2n * cost);
  return Number(rounded);
}

/** Column the positions table can be sorted on. */
export type PositionSortKey =
  | "name"
  | "quantity"
  | "unitCost"
  | "cost"
  | "lastPrice"
  | "marketValue"
  | "unrealized"
  | "dividends";

export interface PositionSort {
  key: PositionSortKey;
  descending: boolean;
}

/** Value compared for a column; `null` for a position without a price, kept last. */
function positionSortValue(position: Position, key: PositionSortKey): number | string | null {
  switch (key) {
    case "name":
      return position.securityName;
    case "quantity":
      return position.quantityMicro;
    case "unitCost":
      return position.unitCostCents;
    case "cost":
      return position.costCents;
    case "lastPrice":
      return position.lastPrice?.priceCents ?? null;
    case "marketValue":
      return position.marketValueCents;
    case "unrealized":
      return position.lastPrice ? position.unrealizedCents : null;
    case "dividends":
      return position.dividendsCents;
  }
}

/** Positions sorted on a column, names in French order; ties keep the order by name. */
export function sortPositions(positions: Position[], sort: PositionSort): Position[] {
  const byName = (a: Position, b: Position) => a.securityName.localeCompare(b.securityName, "fr");
  return [...positions].sort((a, b) => {
    const [x, y] = [positionSortValue(a, sort.key), positionSortValue(b, sort.key)];
    if (x === null || y === null) return x === y ? byName(a, b) : x === null ? 1 : -1;
    const order = typeof x === "string" ? byName(a, b) : (x as number) - (y as number);
    return (sort.descending ? -order : order) || byName(a, b);
  });
}

/** Sort after a click on a column: the other direction on the same column, else its natural one. */
export function toggledPositionSort(current: PositionSort, key: PositionSortKey): PositionSort {
  if (current.key === key) return { key, descending: !current.descending };
  return { key, descending: key !== "name" };
}
