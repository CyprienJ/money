import { describe, expect, it } from "vitest";
import { parsePreference, resolveTheme } from "./theme";

describe("resolveTheme", () => {
  it("follows the system by default and the choice otherwise", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
});

describe("parsePreference", () => {
  it("falls back to the system for a missing or unknown value", () => {
    expect(parsePreference("dark")).toBe("dark");
    expect(parsePreference("light")).toBe("light");
    expect(parsePreference(null)).toBe("system");
    expect(parsePreference("blue")).toBe("system");
  });
});
