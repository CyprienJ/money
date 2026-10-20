import { describe, expect, it } from "vitest";
import { analysisPeriod, periodOptions } from "./periods";

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

describe("analysisPeriod", () => {
  const today = new Date(2026, 9, 20);

  it("gives the current month, quarter and year", () => {
    expect(analysisPeriod("month", 0, today)).toEqual({
      label: "Octobre 2026",
      from: "2026-10-01",
      to: "2026-10-31",
      trendFrom: "2025-11-01",
      trendBase: "2025-10-31",
    });
    expect(analysisPeriod("quarter", 0, today)).toEqual({
      label: "4e trimestre 2026",
      from: "2026-10-01",
      to: "2026-12-31",
      trendFrom: "2026-01-01",
      trendBase: "2025-12-31",
    });
    expect(analysisPeriod("year", 0, today)).toEqual({
      label: "2026",
      from: "2026-01-01",
      to: "2026-12-31",
      trendFrom: "2026-01-01",
      trendBase: "2025-12-31",
    });
  });

  it("goes back across years", () => {
    expect(analysisPeriod("month", -10, today).label).toBe("Décembre 2025");
    expect(analysisPeriod("quarter", -3, today)).toMatchObject({
      label: "1er trimestre 2026",
      from: "2026-01-01",
      to: "2026-03-31",
    });
    expect(analysisPeriod("quarter", -4, today).label).toBe("4e trimestre 2025");
    expect(analysisPeriod("year", -1, today).from).toBe("2025-01-01");
  });
});
