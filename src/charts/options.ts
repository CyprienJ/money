import type { BalancePoint, CategoryColor, NetWorth, PeriodIncomeExpenses } from "../bindings";
import { variationPercent } from "../utils/analyses";
import { formatAmount, formatDate } from "../utils/format";
import type { ChartTokens } from "./theme";

/**
 * Option builders for the Analyses charts. Amounts stay in cents: only axis labels and
 * tooltips format them, without going through floating point euros.
 */

const MONTHS = ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];

/** "oct." for a month period, "2026" for a year period. */
export function periodLabel(periodStart: string, granularity: "month" | "year"): string {
  const [year, month] = periodStart.split("-");
  return granularity === "year" ? year : MONTHS[Number(month) - 1];
}

const euros = (cents: number) => formatAmount(cents, { cents: false });

interface TooltipParam {
  marker: string;
  seriesName: string;
  value: number;
  name: string;
}

function tooltipLines(params: TooltipParam[]): string {
  const title = params[0]?.name ?? "";
  const lines = params.map((p) => `${p.marker}${p.seriesName} <b>${formatAmount(p.value)}</b>`);
  return [title, ...lines].join("<br>");
}

/** Grouped bars: income and expenses per period, the last period highlighted. */
export function incomeExpensesOption(
  periods: PeriodIncomeExpenses[],
  granularity: "month" | "year",
  tokens: ChartTokens,
) {
  const last = periods.length - 1;
  const bars = (key: "incomeCents" | "expenseCents", color: string) =>
    periods.map((p, i) => ({ value: p[key], itemStyle: { color, opacity: i === last ? 1 : 0.85 } }));
  return {
    grid: { left: 8, right: 8, top: 40, bottom: 8, containLabel: true },
    legend: { top: 0, left: 0, data: ["Revenus", "Dépenses"] },
    tooltip: { trigger: "axis", axisPointer: { type: "shadow" }, formatter: tooltipLines },
    xAxis: {
      type: "category",
      data: periods.map((p) => periodLabel(p.periodStart, granularity)),
      axisLabel: {
        // Every period is named, as in the mockup.
        interval: 0,
        formatter: (value: string, index: number) => (index === last ? `{last|${value}}` : value),
        rich: { last: { color: tokens.text, fontWeight: 800 } },
      },
    },
    yAxis: { type: "value", axisLabel: { formatter: euros } },
    series: [
      {
        name: "Revenus",
        type: "bar",
        data: bars("incomeCents", tokens.categories.housing),
        itemStyle: { color: tokens.categories.housing, borderRadius: [5, 5, 0, 0] },
        barMaxWidth: 18,
        barGap: "15%",
      },
      {
        name: "Dépenses",
        type: "bar",
        data: bars("expenseCents", tokens.accent),
        itemStyle: { color: tokens.accent, borderRadius: [5, 5, 0, 0] },
        barMaxWidth: 18,
      },
    ],
  };
}

/** A category with the amount spent, for the category charts. */
export interface CategorySlice {
  name: string;
  color: CategoryColor;
  amountCents: number;
  /** Amount of the compared period, when comparing. */
  previousCents?: number;
}

/**
 * Horizontal bars, the largest expense on top. When the slices carry a compared amount, a
 * thin grey bar shows it and a badge gives the change in percent, as in the Analyses mockup.
 */
export function categoryBarsOption(slices: CategorySlice[], tokens: ChartTokens) {
  const sorted = [...slices].sort((a, b) => b.amountCents - a.amountCents);
  const comparing = sorted.some((s) => s.previousCents !== undefined);
  const amountLabel = (index: number) => {
    const slice = sorted[index];
    const amount = `{amount|${euros(slice.amountCents)}}`;
    if (!comparing) return amount;
    const change = variationPercent(slice.amountCents, slice.previousCents ?? 0);
    if (change === null) return amount;
    const badge = change > 0 ? "up" : "down";
    const sign = change > 0 ? "+" : change < 0 ? "−" : "";
    return `${amount}  {${badge}|${sign}${Math.abs(change)} %}`;
  };
  const badge = (colors: { background: string; text: string }) => ({
    backgroundColor: colors.background,
    color: colors.text,
    borderRadius: 99,
    padding: [2, 7],
    fontSize: 12,
    fontWeight: 800,
  });
  const previousSeries = comparing
    ? [
        {
          name: "Période comparée",
          type: "bar",
          data: sorted.map((s) => s.previousCents ?? 0),
          barWidth: 4,
          barGap: "60%",
          itemStyle: { color: tokens.categories.neutral, borderRadius: 2 },
        },
      ]
    : [];
  return {
    grid: { left: 8, right: comparing ? 128 : 72, top: 8, bottom: 8, containLabel: true },
    tooltip: comparing
      ? { trigger: "axis", axisPointer: { type: "none" }, formatter: comparisonTooltip(sorted) }
      : { trigger: "item", formatter: (p: TooltipParam) => `${p.name} <b>${formatAmount(p.value)}</b>` },
    xAxis: { type: "value", axisLabel: { show: false }, splitLine: { show: false } },
    yAxis: {
      type: "category",
      inverse: true,
      data: sorted.map((s) => s.name),
      axisLabel: { color: tokens.text, fontSize: 13, fontWeight: 600 },
    },
    series: [
      {
        type: "bar",
        data: sorted.map((s) => ({ value: s.amountCents, itemStyle: { color: tokens.categories[s.color] } })),
        barWidth: 10,
        itemStyle: { borderRadius: 5 },
        // Amount (and change) at the end of each bar, as in the mockup.
        label: {
          show: true,
          position: "right",
          formatter: (p: { dataIndex: number }) => amountLabel(p.dataIndex),
          rich: {
            amount: { color: tokens.text, fontSize: 13, fontWeight: 700 },
            up: badge(tokens.increase),
            down: badge(tokens.decrease),
          },
        },
      },
      ...previousSeries,
    ],
  };
}

/** Tooltip of a compared category: both amounts. */
function comparisonTooltip(sorted: CategorySlice[]) {
  return (params: { dataIndex: number }[]) => {
    const slice = sorted[params[0].dataIndex];
    return `${slice.name}<br>Période : <b>${formatAmount(slice.amountCents)}</b><br>Comparée : <b>${formatAmount(
      slice.previousCents ?? 0,
    )}</b>`;
  };
}

/** Ring of the expenses by category, with the total in the middle. */
export function categoryDonutOption(slices: CategorySlice[], tokens: ChartTokens) {
  const total = slices.reduce((sum, s) => sum + Math.max(0, s.amountCents), 0);
  return {
    tooltip: {
      trigger: "item",
      formatter: (p: TooltipParam & { percent: number }) =>
        `${p.marker}${p.name} <b>${formatAmount(p.value)}</b> (${Math.round(p.percent)} %)`,
    },
    title: {
      text: euros(total),
      subtext: "dépensés",
      left: "center",
      top: "center",
      textStyle: { color: tokens.text, fontSize: 20, fontWeight: 800 },
      subtextStyle: { color: tokens.textMuted, fontSize: 12 },
      itemGap: 4,
    },
    series: [
      {
        type: "pie",
        radius: ["58%", "82%"],
        avoidLabelOverlap: true,
        label: { show: false },
        itemStyle: { borderColor: tokens.card, borderWidth: 2 },
        data: slices
          .filter((s) => s.amountCents > 0)
          .map((s) => ({ name: s.name, value: s.amountCents, itemStyle: { color: tokens.categories[s.color] } })),
      },
    ],
  };
}

/** Axis label of a balance point: "19/10", or "oct. 2026" when the points span over a year. */
export function balanceDateLabel(isoDate: string, multiYear: boolean): string {
  if (!multiYear) return formatDate(isoDate, "day-month");
  const [year, month] = isoDate.split("-");
  return `${MONTHS[Number(month) - 1]} ${year}`;
}

/** Balance over time, with a zoom slider on the period. */
export function balanceHistoryOption(points: BalancePoint[], tokens: ChartTokens) {
  const first = points[0]?.date ?? "";
  const last = points[points.length - 1]?.date ?? "";
  // More than a year between the first and the last point (ISO dates compare as text).
  const multiYear = first !== "" && `${Number(first.slice(0, 4)) + 1}${first.slice(4)}` < last;
  return {
    grid: { left: 8, right: 8, top: 16, bottom: 48, containLabel: true },
    tooltip: {
      trigger: "axis",
      formatter: (params: TooltipParam[]) => {
        const point = points[(params[0] as unknown as { dataIndex: number }).dataIndex];
        return `${formatDate(point.date)}<br><b>${formatAmount(point.balanceCents)}</b>`;
      },
    },
    xAxis: {
      type: "category",
      boundaryGap: false,
      data: points.map((p) => balanceDateLabel(p.date, multiYear)),
    },
    yAxis: { type: "value", scale: true, axisLabel: { formatter: euros } },
    dataZoom: [
      { type: "inside" },
      {
        type: "slider",
        height: 20,
        bottom: 8,
        borderColor: tokens.border,
        fillerColor: "rgba(91, 108, 240, 0.12)",
        handleStyle: { color: tokens.card, borderColor: tokens.categories.housing },
        textStyle: { color: tokens.textMuted },
      },
    ],
    series: [
      {
        type: "line",
        data: points.map((p) => p.balanceCents),
        showSymbol: false,
        smooth: 0.2,
        lineStyle: { color: tokens.categories.housing, width: 2.5 },
        areaStyle: { color: tokens.categories.housing, opacity: 0.08 },
      },
    ],
  };
}

/** Net worth curve of the dark card: no axes, coral line over a light coral area. */
export function netWorthOption(points: NetWorth[], tokens: ChartTokens) {
  return {
    grid: { left: 0, right: 0, top: 6, bottom: 0 },
    tooltip: {
      trigger: "axis",
      formatter: (params: TooltipParam[]) => {
        const point = points[(params[0] as unknown as { dataIndex: number }).dataIndex];
        return `${formatDate(point.date)}<br><b>${formatAmount(point.netWorthCents)}</b>`;
      },
    },
    xAxis: { type: "category", show: false, boundaryGap: false, data: points.map((p) => p.date) },
    yAxis: { type: "value", show: false, scale: true },
    series: [
      {
        type: "line",
        data: points.map((p) => p.netWorthCents),
        showSymbol: false,
        lineStyle: { color: tokens.accent, width: 3, cap: "round", join: "round" },
        areaStyle: { color: tokens.accent, opacity: 0.22 },
      },
    ],
  };
}

/** "24 k" for 2 400 000 cents, as on the dashboard axis. */
export function compactEuros(cents: number): string {
  return `${Math.round(cents / 100_000)} k`;
}

/**
 * "Évolution du disponible" of the dashboard: green line over a light area, month names, the
 * last value in a green badge.
 */
export function availableHistoryOption(points: BalancePoint[], tokens: ChartTokens) {
  const green = tokens.categories.transport;
  const last = points.length - 1;
  return {
    grid: { left: 8, right: 70, top: 16, bottom: 8, containLabel: true },
    tooltip: {
      trigger: "axis",
      formatter: (params: TooltipParam[]) => {
        const point = points[(params[0] as unknown as { dataIndex: number }).dataIndex];
        return `${formatDate(point.date)}<br><b>${formatAmount(point.balanceCents)}</b>`;
      },
    },
    xAxis: {
      type: "category",
      boundaryGap: false,
      data: points.map((p) => periodLabel(p.date, "month")),
    },
    yAxis: { type: "value", scale: true, splitNumber: 2, axisLabel: { formatter: compactEuros } },
    series: [
      {
        type: "line",
        data: points.map((p, index) => ({
          value: p.balanceCents,
          symbol: index === last ? "circle" : "none",
          symbolSize: 12,
          itemStyle: { color: tokens.card, borderColor: green, borderWidth: 3 },
        })),
        lineStyle: { color: green, width: 3 },
        areaStyle: { color: green, opacity: 0.12 },
        endLabel: {
          show: true,
          formatter: (p: TooltipParam) => euros(p.value),
          backgroundColor: tokens.decrease.background,
          color: tokens.decrease.text,
          fontSize: 12,
          fontWeight: 800,
          padding: [3, 8],
          borderRadius: 8,
          offset: [-8, -16],
        },
      },
    ],
  };
}
