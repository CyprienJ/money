import { describe, expect, it } from "vitest";
import {
  availableHistoryOption,
  balanceHistoryOption,
  categoryBarsOption,
  categoryDonutOption,
  compactEuros,
  forecastOption,
  incomeExpensesOption,
  netWorthOption,
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
  increase: { background: "#fde7de", text: "#9a3412" },
  decrease: { background: "#ddf3ef", text: "#0f5e52" },
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

describe("netWorthOption", () => {
  it("draws the net worth in cents with the accent color and no axes", () => {
    const point = (date: string, netWorthCents: number) => ({
      date,
      netWorthCents,
      cashCents: 0,
      investmentsCents: 0,
      assetsCents: 0,
      liabilitiesCents: 0,
    });
    const option = netWorthOption([point("2026-09-30", 13910000), point("2026-10-31", 14824000)], tokens);
    expect(option.series[0].data).toEqual([13910000, 14824000]);
    expect(option.series[0].lineStyle.color).toBe("#f26b3a");
    expect([option.xAxis.show, option.yAxis.show]).toEqual([false, false]);
  });
});

/** The main series of the category bars, which carries the amount labels. */
type LabelledSeries = Extract<ReturnType<typeof categoryBarsOption>["series"][number], { label: unknown }>;

describe("categoryBarsOption with a comparison", () => {
  it("adds the thin compared bars and the change badges", () => {
    const option = categoryBarsOption(
      [
        { name: "Courses", color: "groceries", amountCents: 41200, previousCents: 36800 },
        { name: "Logement", color: "housing", amountCents: 93800, previousCents: 92100 },
        { name: "Restaurants", color: "restaurants", amountCents: 21400, previousCents: 25100 },
        { name: "Cadeaux", color: "leisure", amountCents: 5000, previousCents: 0 },
      ],
      tokens,
    );
    const current = option.series[0] as LabelledSeries;
    const previous = option.series[1];
    expect(previous.data).toEqual([92100, 36800, 25100, 0]);
    expect(previous.barWidth).toBe(4);
    const label = (index: number) => current.label.formatter({ dataIndex: index });
    expect(label(0)).toBe("{amount|938\u00a0€}  {up|+2 %}");
    expect(label(2)).toBe("{amount|214\u00a0€}  {down|−15 %}");
    // No badge without a compared amount.
    expect(label(3)).toBe("{amount|50\u00a0€}");
  });

  it("shows amounts only without a comparison", () => {
    const option = categoryBarsOption([{ name: "Courses", color: "groceries", amountCents: 41200 }], tokens);
    expect(option.series).toHaveLength(1);
    expect((option.series[0] as LabelledSeries).label.formatter({ dataIndex: 0 })).toBe("{amount|412\u00a0€}");
  });
});

describe("availableHistoryOption", () => {
  it("names months, marks the last point and labels it", () => {
    const option = availableHistoryOption(
      [
        { date: "2026-09-30", balanceCents: 2_200_000 },
        { date: "2026-10-31", balanceCents: 2_341_000 },
      ],
      tokens,
    );
    expect(option.xAxis.data).toEqual(["sept.", "oct."]);
    expect(option.series[0].data.map((d) => d.symbol)).toEqual(["none", "circle"]);
    expect(option.series[0].endLabel.formatter({ value: 2_341_000 } as never)).toBe("23\u00a0410\u00a0€");
    expect(compactEuros(2_400_000)).toBe("24 k");
  });
});

describe("forecastOption", () => {
  const forecast = {
    currentBalanceCents: 30_000,
    points: [
      { date: "2026-10-20", balanceCents: 30_000 },
      { date: "2026-10-21", balanceCents: -20_000 },
      { date: "2026-10-22", balanceCents: 180_000 },
    ],
    lowest: { date: "2026-10-21", balanceCents: -20_000 },
    occurrences: [
      {
        scheduledOperationId: 1,
        occurrenceDate: "2026-10-21",
        date: "2026-10-21",
        accountId: 1,
        label: "Loyer",
        amountCents: -50_000,
        categoryId: null,
      },
    ],
  };

  it("marks the lowest point and the zero line of a negative forecast", () => {
    const option = forecastOption(forecast, tokens);
    const series = option.series[0];
    expect(series.step).toBe("end");
    expect(series.markPoint.data).toEqual([{ coord: [1, -20_000] }]);
    expect(series.markLine?.data).toEqual([{ yAxis: 0 }]);
    const tooltip = option.tooltip.formatter([{ dataIndex: 1 } as never]);
    expect(tooltip).toContain("Loyer");
  });

  it("draws no zero line while the balance stays positive", () => {
    const option = forecastOption({ ...forecast, lowest: { date: "2026-10-20", balanceCents: 30_000 } }, tokens);
    expect(option.series[0].markLine).toBeUndefined();
    expect(option.tooltip.formatter([{ dataIndex: 0 } as never])).not.toContain("Loyer");
  });
});
