import type { TransactionKind } from "../bindings";
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
