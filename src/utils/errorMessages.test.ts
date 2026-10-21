import { describe, expect, it } from "vitest";
import { CommandError, errorMessage } from "./commands";
import { businessMessage } from "./errorMessages";

describe("businessMessage", () => {
  it("writes business errors in French with their details", () => {
    expect(businessMessage({ domain: "operation", error: { code: "zeroAmount" } })).toBe(
      "Le montant ne peut pas être nul.",
    );
    expect(
      businessMessage({ domain: "csv", error: { code: "invalidDate", params: { line: 4, value: "32/09/2026" } } }),
    ).toBe("Ligne 4 : date « 32/09/2026 » illisible avec ce format.");
    expect(businessMessage({ domain: "csv", error: { code: "missingColumn", params: { line: 2, column: 3 } } })).toBe(
      "Ligne 2 : la colonne 4 manque.",
    );
    expect(businessMessage({ domain: "operation", error: { code: "dateBeforeOpening", params: "2026-09-01" } })).toContain(
      "(01/09/2026)",
    );
    expect(
      businessMessage({ domain: "csv", error: { code: "wrongDecimalSeparator", params: { line: 2, value: "500.00" } } }),
    ).toContain("change le « Séparateur décimal » du format");
    expect(businessMessage({ domain: "import", error: { code: "duplicateLine", params: 0 } })).toBe(
      "La ligne 1 est déjà dans le compte et ne peut pas être réimportée.",
    );
  });
});

describe("errorMessage", () => {
  it("prefers the French business message", () => {
    const error = new CommandError({
      kind: "validation",
      message: "category name cannot be empty",
      detail: { domain: "category", error: { code: "emptyName" } },
    });
    expect(errorMessage(error)).toBe("Le nom de la catégorie est obligatoire.");
  });

  it("falls back on the kind for technical errors", () => {
    const io = new CommandError({ kind: "io", message: "I/O error: No such file", detail: null });
    expect(errorMessage(io)).toBe("Fichier inaccessible : I/O error: No such file.");
    const db = new CommandError({ kind: "database", message: "database error: locked", detail: null });
    expect(errorMessage(db)).toBe("Erreur inattendue : database error: locked.");
  });
});
