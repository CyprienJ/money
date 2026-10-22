import { describe, expect, it } from "vitest";
import { currencyLabel, formatRate, parseRate } from "./currency";

describe("parseRate", () => {
  it("reads euros for one unit in billionths", () => {
    expect(parseRate("0,8924")).toBe(892_400_000);
    expect(parseRate("1.15")).toBe(1_150_000_000);
    expect(parseRate(" 2 ")).toBe(2_000_000_000);
    expect(parseRate("0,000000001")).toBe(1);
  });

  it("refuses zero, negative and too precise rates", () => {
    expect(parseRate("0")).toBeNull();
    expect(parseRate("-1")).toBeNull();
    expect(parseRate("0,0000000001")).toBeNull();
    expect(parseRate("abc")).toBeNull();
  });
});

describe("formatRate", () => {
  it("keeps four decimals at least", () => {
    expect(formatRate(892_400_000)).toBe("0,8924");
    expect(formatRate(1_150_000_000)).toBe("1,1500");
    expect(formatRate(1_234_567_891)).toBe("1,234567891");
  });
});

describe("currencyLabel", () => {
  it("names the known currencies", () => {
    expect(currencyLabel("USD")).toBe("USD · Dollar américain");
    expect(currencyLabel("XYZ")).toBe("XYZ");
  });
});
