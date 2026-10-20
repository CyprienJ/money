import type { ExistingOperation, ImportPreview, PreviewLine } from "../bindings";
import { formatAmount, formatDate } from "./format";

/** A preview line with its index in the file, the reference used by the import. */
export interface IndexedLine {
  index: number;
  line: PreviewLine;
}

export interface PreviewGroups {
  /** New and ambiguous lines: the user chooses which ones to import. */
  candidates: IndexedLine[];
  /** Exact duplicates, never imported. */
  duplicates: IndexedLine[];
  /** Dated before the account opening, never imported. */
  beforeOpening: IndexedLine[];
}

/** Splits the preview lines by what the user can do with them. */
export function groupPreview(preview: ImportPreview): PreviewGroups {
  const groups: PreviewGroups = { candidates: [], duplicates: [], beforeOpening: [] };
  preview.lines.forEach((line, index) => {
    if (line.beforeOpening) {
      groups.beforeOpening.push({ index, line });
    } else if (line.duplicate.status === "duplicate") {
      groups.duplicates.push({ index, line });
    } else {
      groups.candidates.push({ index, line });
    }
  });
  return groups;
}

/** Indexes checked when the preview opens. */
export function defaultSelection(preview: ImportPreview): Set<number> {
  const selected = new Set<number>();
  preview.lines.forEach((line, index) => {
    if (line.includedByDefault) selected.add(index);
  });
  return selected;
}

function describe(operation: ExistingOperation): string {
  return `« ${operation.rawLabel} » du ${formatDate(operation.date, "day-month")}`;
}

/**
 * Why a line may already be in the account, e.g.
 * "Saisie manuelle proche : « Resto » du 04/09". Empty for a line that is not ambiguous.
 */
export function ambiguityText(line: PreviewLine, operations: ExistingOperation[]): string {
  const duplicate = line.duplicate;
  if (duplicate.status !== "ambiguous") return "";
  const candidates = duplicate.candidateIds
    .map((id) => operations.find((operation) => operation.id === id))
    .filter((operation): operation is ExistingOperation => operation !== undefined);
  const list = candidates.map(describe).join(", ");
  switch (duplicate.reason) {
    case "repeatedLine":
      return `Déjà dans le compte, mais moins de fois que dans le fichier`;
    case "sameDayOtherLabel":
      return `Même jour et même montant : ${list}`;
    case "nearManualEntry":
      return `Saisie manuelle proche : ${list}`;
  }
}

/** "relevé du 01/09 au 30/09", or an empty string for an empty file. */
export function periodText(preview: ImportPreview): string {
  if (preview.firstDate === null || preview.lastDate === null) return "";
  return `relevé du ${formatDate(preview.firstDate, "day-month")} au ${formatDate(preview.lastDate, "day-month")}`;
}

/** Short amount for lists, e.g. "−97,15 €". */
export function lineAmount(line: PreviewLine): string {
  return formatAmount(line.line.amountCents, { signed: true });
}

/** Checked lines that receive a remembered category, and those left to categorize. */
export function categoryCounts(
  preview: ImportPreview,
  selected: Set<number>,
): { automatic: number; toCategorize: number } {
  const counts = { automatic: 0, toCategorize: 0 };
  preview.lines.forEach((line, index) => {
    if (!selected.has(index)) return;
    if (line.categoryId === null) counts.toCategorize++;
    else counts.automatic++;
  });
  return counts;
}
