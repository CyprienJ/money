import { describe, expect, it } from "vitest";
import { describeSchedule } from "./schedule";

describe("describeSchedule", () => {
  it("describes each frequency in plain French", () => {
    const at = (frequency: Parameters<typeof describeSchedule>[0]["frequency"], dayOfPeriod: number) =>
      describeSchedule({ frequency, dayOfPeriod, startDate: "2026-01-15" });
    expect(at("MONTHLY", 5)).toBe("Tous les mois, le 5");
    expect(at("MONTHLY", 1)).toBe("Tous les mois, le 1er");
    expect(at("WEEKLY", 5)).toBe("Chaque semaine, le vendredi");
    expect(at("QUARTERLY", 15)).toBe("Tous les trimestres, le 15");
    expect(at("SEMIANNUAL", 31)).toBe("Tous les six mois, le 31");
    expect(at("YEARLY", 1)).toBe("Tous les ans, le 1er janvier");
  });
});
