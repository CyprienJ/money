import { describe, expect, it } from "vitest";
import {
  balanceHistoryOption,
  categoryBarsOption,
  categoryDonutOption,
  incomeExpensesOption,
  periodLabel,
} from "./options";
import { createTheme, type ChartTokens } from "./theme";

const tokens: ChartTokens = {
  fontFamily: "Plus Jakarta Sans",
  text: "#1e1830",
  textMuted: "#6b6480",
  border: "#e7e3ef",
  divider: "#f0eef5",
  card: "#ffffff",
  accent: "#f26b3a",
  positive: "#0f7a5f",
  categories: {
    housing: "#5b6cf0",
    groceries: "#f2a93b",
    restaurants: "#f26b3a",
    transport: "#1f9e89",
    leisure: "#e8547a",
    subscriptions: "#3ba3d9",
    health: "#8c5ad8",
    neutral: "#b9b4c6",
  },
};

describe("periodLabel", () => {
  it("names months like the mockup and years by their number", () => {
    expect(periodLabel("2026-01-01", "month")).toBe("janv.");
    expect(periodLabel("2026-10-01", "month")).toBe("oct.");
    expect(periodLabel("2026-01-01", "year")).toBe("2026");
  });
});

describe("incomeExpensesOption", () => {
  it("keeps cents and highlights the last period", () => {
    const option = incomeExpensesOption(
      [
        { periodStart: "2026-09-01", incomeCents: 328000, expenseCents: 226000 },
        { periodStart: "2026-10-01", incomeCents: 328000, expenseCents: 214600 },
      ],
      "month",
      tokens,
    );
    expect(option.xAxis.data).toEqual(["sept.", "oct."]);
    expect(option.xAxis.axisLabel.interval).toBe(0);
    const [income, expenses] = option.series;
    expect(income.data.map((d) => d.value)).toEqual([328000, 328000]);
    expect(expenses.data.map((d) => [d.value, d.itemStyle.opacity])).toEqual([
      [226000, 0.85],
      [214600, 1],
    ]);
    expect(income.itemStyle.color).toBe("#5b6cf0");
    expect(expenses.itemStyle.color).toBe("#f26b3a");
    expect(option.yAxis.axisLabel.formatter(123456)).toBe("1 235 €");
  });
});

describe("category options", () => {
  const slices = [
    { name: "Courses", color: "groceries" as const, amountCents: 41200 },
    { name: "Logement", color: "housing" as const, amountCents: 93800 },
    { name: "Remboursés", color: "neutral" as const, amountCents: -500 },
  ];

  it("sorts the bars by amount with the category colors", () => {
    const option = categoryBarsOption(slices, tokens);
    expect(option.yAxis.data).toEqual(["Logement", "Courses", "Remboursés"]);
    expect(option.series[0].data[0]).toEqual({ value: 93800, itemStyle: { color: "#5b6cf0" } });
  });

  it("shows positive expenses in the ring with their total", () => {
    const option = categoryDonutOption(slices, tokens);
    expect(option.series[0].data.map((d) => d.name)).toEqual(["Courses", "Logement"]);
    expect(option.title.text).toBe("1 350 €");
  });
});

describe("balanceHistoryOption", () => {
  it("plots the balance in cents with a zoom", () => {
    const option = balanceHistoryOption(
      [
        { date: "2026-09-30", balanceCents: 150000 },
        { date: "2026-10-31", balanceCents: 218432 },
      ],
      tokens,
    );
    expect(option.xAxis.data).toEqual(["30/09", "31/10"]);
    const long = balanceHistoryOption(
      [
        { date: "2016-10-21", balanceCents: 0 },
        { date: "2026-10-20", balanceCents: 1 },
      ],
      tokens,
    );
    expect(long.xAxis.data).toEqual(["oct. 2016", "oct. 2026"]);
    expect(option.series[0].data).toEqual([150000, 218432]);
    expect(option.dataZoom.map((z) => z.type)).toEqual(["inside", "slider"]);
  });
});

describe("createTheme", () => {
  it("uses the category colors as palette and the design font", () => {
    const theme = createTheme(tokens) as { color: string[]; textStyle: { fontFamily: string } };
    expect(theme.color[0]).toBe("#5b6cf0");
    expect(theme.color).toHaveLength(8);
    expect(theme.textStyle.fontFamily).toBe("Plus Jakarta Sans");
  });
});
