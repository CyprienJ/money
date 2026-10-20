import type { CategoryColor } from "../bindings";

/** Design tokens used by the charts, read from the CSS variables of `tokens.css`. */
export interface ChartTokens {
  fontFamily: string;
  text: string;
  textMuted: string;
  border: string;
  divider: string;
  card: string;
  accent: string;
  positive: string;
  /** Full color of each category family. */
  categories: Record<CategoryColor, string>;
}

const CATEGORY_COLORS: CategoryColor[] = [
  "housing",
  "groceries",
  "restaurants",
  "transport",
  "leisure",
  "subscriptions",
  "health",
  "neutral",
];

/** Reads the chart tokens from the CSS variables of the document. */
export function readTokens(element: Element = document.documentElement): ChartTokens {
  const style = getComputedStyle(element);
  const token = (name: string) => style.getPropertyValue(name).trim();
  return {
    fontFamily: token("--font-family"),
    text: token("--color-text"),
    textMuted: token("--color-text-muted"),
    border: token("--color-border"),
    divider: token("--color-divider"),
    card: token("--color-card"),
    accent: token("--color-accent"),
    positive: token("--color-positive"),
    categories: Object.fromEntries(CATEGORY_COLORS.map((c) => [c, token(`--cat-${c}`)])) as Record<
      CategoryColor,
      string
    >,
  };
}

/** ECharts theme matching the mockups: tabular numbers, muted axes, light grid lines. */
export function createTheme(tokens: ChartTokens): Record<string, unknown> {
  const axis = {
    axisLine: { show: false },
    axisTick: { show: false },
    axisLabel: { color: tokens.textMuted, fontSize: 11 },
    splitLine: { lineStyle: { color: tokens.divider } },
  };
  return {
    color: CATEGORY_COLORS.map((c) => tokens.categories[c]),
    backgroundColor: "transparent",
    textStyle: { fontFamily: tokens.fontFamily, color: tokens.text },
    categoryAxis: { ...axis, splitLine: { show: false } },
    valueAxis: axis,
    timeAxis: axis,
    legend: {
      icon: "circle",
      itemWidth: 10,
      itemHeight: 10,
      textStyle: { color: tokens.textMuted, fontSize: 13 },
    },
    tooltip: {
      backgroundColor: tokens.card,
      borderColor: tokens.border,
      textStyle: { color: tokens.text, fontSize: 13 },
      extraCssText: "border-radius: 10px; box-shadow: 0 8px 24px rgba(30, 24, 48, 0.12);",
    },
  };
}
