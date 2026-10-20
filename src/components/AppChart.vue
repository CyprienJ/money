<script setup lang="ts">
import { BarChart, LineChart, PieChart } from "echarts/charts";
import {
  DataZoomComponent,
  GridComponent,
  LegendComponent,
  TitleComponent,
  TooltipComponent,
} from "echarts/components";
import { type EChartsCoreOption, registerTheme, use } from "echarts/core";
import { SVGRenderer } from "echarts/renderers";
import VChart from "vue-echarts";
import { createTheme, readTokens } from "../charts/theme";

defineProps<{
  /** Option built by one of the builders of `src/charts/options.ts`. */
  option: EChartsCoreOption;
  /** Accessible description of the chart. */
  label: string;
}>();
</script>

<script lang="ts">
// Only the chart types and components the app uses are bundled. SVG rendering stays sharp
// and behaves the same under WebKitGTK and WebView2.
use([
  SVGRenderer,
  BarChart,
  LineChart,
  PieChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
  TitleComponent,
  DataZoomComponent,
]);
registerTheme("money", createTheme(readTokens()));
</script>

<template>
  <VChart class="chart" :option="option" theme="money" autoresize role="img" :aria-label="label" />
</template>

<style scoped>
.chart {
  width: 100%;
  height: 100%;
  min-height: 220px;
}
</style>
