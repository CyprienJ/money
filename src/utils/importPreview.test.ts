import { describe, expect, it } from "vitest";
import type { ExistingOperation, ImportPreview, PreviewLine } from "../bindings";
import { splitLine } from "./csvMapping";
import {
  ambiguityText,
  defaultSelection,
  groupPreview,
  operationCount,
  periodText,
} from "./importPreview";

function line(overrides: Partial<PreviewLine> = {}): PreviewLine {
  return {
    line: { date: "2026-09-02", valueDate: null, label: "CB X", amountCents: -420 },
    normalizedLabel: "CB X",
    duplicate: { status: "new" },
    beforeOpening: false,
    includedByDefault: true,
    ...overrides,
  };
}

function preview(lines: PreviewLine[]): ImportPreview {
  return {
    fileName: "releve.csv",
    fileHash: "hash",
    lines,
    matchedOperations: [],
    previousImports: [],
    firstDate: "2026-09-01",
    lastDate: "2026-09-30",
  };
}

const manual: ExistingOperation = {
  id: 7,
  date: "2026-09-04",
  amountCents: -3500,
  rawLabel: "Resto",
  isManual: true,
};

describe("groupPreview", () => {
  it("separates lines to choose from duplicates and lines before opening", () => {
    const lines = [
      line(),
      line({ duplicate: { status: "duplicate", operationId: 1 }, includedByDefault: false }),
      line({ beforeOpening: true, includedByDefault: false }),
      line({
        duplicate: { status: "ambiguous", reason: "nearManualEntry", candidateIds: [7] },
        includedByDefault: false,
      }),
    ];
    const groups = groupPreview(preview(lines));
    expect(groups.candidates.map((l) => l.index)).toEqual([0, 3]);
    expect(groups.duplicates.map((l) => l.index)).toEqual([1]);
    expect(groups.beforeOpening.map((l) => l.index)).toEqual([2]);
    expect([...defaultSelection(preview(lines))]).toEqual([0]);
  });
});

describe("ambiguityText", () => {
  it("names the operations the line may duplicate", () => {
    const ambiguous = line({
      duplicate: { status: "ambiguous", reason: "nearManualEntry", candidateIds: [7] },
    });
    expect(ambiguityText(ambiguous, [manual])).toBe("Saisie manuelle proche : « Resto » du 04/09");
    const sameDay = line({
      duplicate: { status: "ambiguous", reason: "sameDayOtherLabel", candidateIds: [7] },
    });
    expect(ambiguityText(sameDay, [manual])).toBe("Même jour et même montant : « Resto » du 04/09");
    const repeated = line({
      duplicate: { status: "ambiguous", reason: "repeatedLine", candidateIds: [7] },
    });
    expect(ambiguityText(repeated, [manual])).toBe(
      "Déjà dans le compte, mais moins de fois que dans le fichier",
    );
    expect(ambiguityText(line(), [manual])).toBe("");
  });
});

describe("texts", () => {
  it("formats the period and counts", () => {
    expect(periodText(preview([]))).toBe("relevé du 01/09 au 30/09");
    expect(periodText({ ...preview([]), firstDate: null, lastDate: null })).toBe("");
    expect(operationCount(1)).toBe("1 opération");
    expect(operationCount(9)).toBe("9 opérations");
  });
});

describe("splitLine", () => {
  it("splits on the delimiter outside double quotes", () => {
    expect(splitLine('01/09/2026;"VIR; SALAIRE";;2 450,00;', ";")).toEqual([
      "01/09/2026",
      "VIR; SALAIRE",
      "",
      "2 450,00",
      "",
    ]);
    expect(splitLine('a,"say ""hi""",b', ",")).toEqual(["a", 'say "hi"', "b"]);
  });
});
