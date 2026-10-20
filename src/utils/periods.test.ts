import { describe, expect, it } from "vitest";
import { periodOptions } from "./periods";

describe("periodOptions", () => {
  const options = periodOptions(new Date(2026, 9, 20)); // 20 October 2026

  it("starts with the current month", () => {
    expect(options[0]).toEqual({
      id: "2026-10",
      label: "Octobre 2026",
      from: "2026-10-01",
      to: "2026-10-31",
    });
  });

  it("lists the 12 last months across years, with month lengths", () => {
    expect(options[1]).toMatchObject({ label: "Septembre 2026", to: "2026-09-30" });
    expect(options[8]).toMatchObject({ label: "Février 2026", from: "2026-02-01", to: "2026-02-28" });
    expect(options[11]).toMatchObject({ id: "2025-11", from: "2025-11-01", to: "2025-11-30" });
  });

  it("ends with rolling ranges and the whole history", () => {
    expect(options.slice(12)).toEqual([
      { id: "last-3-months", label: "3 derniers mois", from: "2026-08-01", to: "2026-10-20" },
      { id: "last-12-months", label: "12 derniers mois", from: "2025-11-01", to: "2026-10-20" },
      { id: "all", label: "Tout l'historique", from: null, to: null },
    ]);
  });
});
