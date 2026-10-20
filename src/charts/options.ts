import type { BalancePoint, CategoryColor, PeriodIncomeExpenses } from "../bindings";
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
}

/** Horizontal bars, the largest expense on top. */
export function categoryBarsOption(slices: CategorySlice[], tokens: ChartTokens) {
  const sorted = [...slices].sort((a, b) => b.amountCents - a.amountCents);
  return {
    grid: { left: 8, right: 72, top: 8, bottom: 8, containLabel: true },
    tooltip: { trigger: "item", formatter: (p: TooltipParam) => `${p.name} <b>${formatAmount(p.value)}</b>` },
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
        // Amount at the end of each bar, as in the mockup.
        label: {
          show: true,
          position: "right",
          formatter: (p: TooltipParam) => euros(p.value),
          color: tokens.text,
          fontSize: 13,
          fontWeight: 700,
        },
      },
    ],
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
