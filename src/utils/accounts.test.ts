import { describe, expect, it } from "vitest";
import { accountInitials } from "./accounts";

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
