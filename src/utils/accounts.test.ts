import { describe, expect, it } from "vitest";
import { accountInitials, valuationSubtitle } from "./accounts";

describe("accountInitials", () => {
  it("matches the Dashboard mockup", () => {
    expect(accountInitials("Compte courant")).toBe("CC");
    expect(accountInitials("Livret A")).toBe("LA");
    expect(accountInitials("LDDS")).toBe("LD");
    expect(accountInitials("PEA")).toBe("PEA");
    expect(accountInitials("Appartement")).toBe("AP");
    expect(accountInitials("Prêt immobilier")).toBe("PI");
  });

  it("handles extra spaces and empty names", () => {
    expect(accountInitials("  livret   jeune ")).toBe("LJ");
    expect(accountInitials("   ")).toBe("?");
  });
});

describe("valuationSubtitle", () => {
  const today = new Date(2026, 9, 20);

  it("matches the Dashboard mockup", () => {
    expect(valuationSubtitle("INVESTMENT", "2026-09-30", today)).toBe("Valorisé au 30/09");
    expect(valuationSubtitle("ASSET", "2026-01-15", today)).toBe("Actif · estimé en janvier");
    expect(valuationSubtitle("LIABILITY", "2026-09-05", today)).toBe("Dette · capital restant dû");
  });

  it("adds the year of an older valuation and flags missing ones", () => {
    expect(valuationSubtitle("INVESTMENT", "2025-12-31", today)).toBe("Valorisé au 31/12/2025");
    expect(valuationSubtitle("ASSET", "2025-06-01", today)).toBe("Actif · estimé en juin 2025");
    expect(valuationSubtitle("ASSET", null, today)).toBe("Actif · à valoriser");
  });
});
