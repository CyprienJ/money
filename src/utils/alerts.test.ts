import { describe, expect, it } from "vitest";
import { describeAlert, type AlertNames } from "./alerts";

const names: AlertNames = {
  accounts: new Map([[1, { name: "Compte courant", currency: "EUR" }]]),
  categories: new Map([[7, "Restaurants"]]),
};

describe("describeAlert", () => {
  it("sends a currency without rate to the settings", () => {
    const view = describeAlert({ kind: "missingRate", currency: "USD" }, names);
    expect(view.title).toBe("Taux de change manquant : USD");
    expect(view.to).toBe("/settings");
    expect(view.dismissible).toBe(false);
  });

  it("describes a negative forecast, which cannot be dismissed", () => {
    const view = describeAlert(
      {
        kind: "negativeForecast",
        accountId: 1,
        firstNegativeDate: "2026-10-25",
        lowest: { date: "2026-10-27", balanceCents: -40_000 },
      },
      names,
    );
    expect(view.title).toBe("Compte courant : solde projeté négatif");
    expect(view.detail).toContain("25/10/2026");
    expect(view.detail).toContain("27/10");
    expect(view.dismissible).toBe(false);
    expect(view.to).toBe("/schedule");
  });

  it("describes a missing occurrence", () => {
    const view = describeAlert(
      {
        kind: "missingOccurrence",
        occurrence: {
          scheduledOperationId: 3,
          date: "2026-10-05",
          accountId: 1,
          label: "Loyer",
          amountCents: -80_000,
          categoryId: null,
          realizedBy: null,
        },
      },
      names,
    );
    expect(view.key).toBe("missing-3-2026-10-05");
    expect(view.title).toBe("Échéance manquante : Loyer");
    expect(view.detail).toContain("Compte courant");
    expect(view.dismissible).toBe(true);
  });

  it("describes an unusual expense", () => {
    const view = describeAlert(
      { kind: "unusualExpense", categoryId: 7, month: "2026-10-01", expenseCents: 20_000, averageCents: 10_000 },
      names,
    );
    expect(view.title).toBe("Dépense inhabituelle : Restaurants");
    expect(view.detail).toMatch(/^200,00\s€ ce mois-ci, contre 100,00\s€/);
    expect(view.to).toBe("/analyses");
  });
});
