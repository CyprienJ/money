<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  commands,
  type AccountSummary,
  type BalancePoint,
  type Category,
  type CategoryExpense,
  type NetWorth,
  type PeriodIncomeExpenses,
} from "../bindings";
import AppChart from "../components/AppChart.vue";
import {
  balanceHistoryOption,
  categoryBarsOption,
  categoryDonutOption,
  incomeExpensesOption,
  netWorthOption,
  periodLabel,
} from "../charts/options";
import { readTokens } from "../charts/theme";
import { ACCOUNT_TYPES } from "../utils/accounts";
import { categorySlices, trendTotals } from "../utils/analyses";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount } from "../utils/format";
import { type AnalysisGranularity, analysisPeriod } from "../utils/periods";

const GRANULARITIES: { id: AnalysisGranularity; label: string }[] = [
  { id: "month", label: "Mois" },
  { id: "quarter", label: "Trimestre" },
  { id: "year", label: "Année" },
];

const tokens = readTokens();
const granularity = ref<AnalysisGranularity>("month");
/** 0 for the current period, -1 for the previous one… */
const offset = ref(0);
/** Account of the balance chart; `null` for every account. */
const balanceAccount = ref<number | null>(null);

const accounts = ref<AccountSummary[]>([]);
const categories = ref<Category[]>([]);
const trend = ref<PeriodIncomeExpenses[]>([]);
const expenses = ref<CategoryExpense[]>([]);
const balance = ref<BalancePoint[]>([]);
/** Net worth at the end of each month, from the end of the month before the trend. */
const netWorth = ref<NetWorth[]>([]);
const error = ref("");

const period = computed(() => analysisPeriod(granularity.value, offset.value));
const totals = computed(() => trendTotals(trend.value));
const slices = computed(() => categorySlices(expenses.value, categories.value));
const lastBalance = computed(() => balance.value[balance.value.length - 1]?.balanceCents ?? null);

const trendOption = computed(() => incomeExpensesOption(trend.value, "month", tokens));
const barsOption = computed(() => categoryBarsOption(slices.value, tokens));
const donutOption = computed(() => categoryDonutOption(slices.value, tokens));
const balanceOption = computed(() => balanceHistoryOption(balance.value, tokens));
const netWorthChart = computed(() => netWorthOption(netWorth.value, tokens));
const currentNetWorth = computed(() => netWorth.value[netWorth.value.length - 1]?.netWorthCents ?? 0);
/** Change over the 12 months of the trend. */
const netWorthChange = computed(() => currentNetWorth.value - (netWorth.value[0]?.netWorthCents ?? 0));
/** "nov. 2025" … "oct. 2026" under the curve. */
const netWorthRange = computed(() => {
  const label = (iso: string) => `${periodLabel(iso, "month")} ${iso.slice(0, 4)}`;
  return [label(period.value.trendFrom), label(period.value.to)];
});

async function loadReferences() {
  const summaries = await unwrap(commands.listAccountSummaries(false));
  accounts.value = summaries.filter((s) => ACCOUNT_TYPES[s.account.type].hasOperations);
  categories.value = await unwrap(commands.listCategories());
}

async function loadStats() {
  error.value = "";
  const { from, to, trendFrom, trendBase } = period.value;
  const balanceAccounts = balanceAccount.value === null ? null : [balanceAccount.value];
  try {
    [trend.value, expenses.value, balance.value, netWorth.value] = await Promise.all([
      unwrap(commands.incomeVsExpenses(trendFrom, to, null, "month")),
      unwrap(commands.expensesByCategory(from, to, null, "topLevel")),
      unwrap(commands.balanceHistory(trendFrom, to, balanceAccounts, "day")),
      unwrap(commands.netWorthHistory(trendBase, to, "month")),
    ]);
  } catch (e) {
    error.value = errorMessage(e);
  }
}

function selectGranularity(id: AnalysisGranularity) {
  granularity.value = id;
  offset.value = 0;
}

onMounted(async () => {
  try {
    await loadReferences();
  } catch (e) {
    error.value = errorMessage(e);
  }
  await loadStats();
});
watch([period, balanceAccount], loadStats);
</script>

<template>
  <header class="header">
    <div>
      <div class="subtitle">Tous comptes · virements internes exclus</div>
      <h1>Analyses</h1>
    </div>
    <div class="controls">
      <div role="group" aria-label="Granularité" class="segmented">
        <button
          v-for="g in GRANULARITIES"
          :key="g.id"
          type="button"
          :aria-pressed="granularity === g.id"
          @click="selectGranularity(g.id)"
        >
          {{ g.label }}
        </button>
      </div>
      <div class="period" aria-live="polite">
        <button type="button" aria-label="Période précédente" @click="offset--">‹</button>
        <span>{{ period.label }}</span>
        <button type="button" aria-label="Période suivante" :disabled="offset >= 0" @click="offset++">›</button>
      </div>
    </div>
  </header>

  <p v-if="error" class="error-banner">{{ error }}</p>

  <section class="card">
    <div class="card-header">
      <h2 class="card-title">Revenus et dépenses</h2>
      <span class="muted">12 mois jusqu'à {{ period.label.toLowerCase() }}</span>
    </div>
    <div class="chart trend">
      <AppChart :option="trendOption" label="Revenus et dépenses mensuels sur 12 mois" />
    </div>
    <div class="totals">
      <span>Revenus sur 12 mois <strong>{{ formatAmount(totals.incomeCents, { cents: false }) }}</strong></span>
      <span>Dépenses sur 12 mois <strong>{{ formatAmount(totals.expenseCents, { cents: false }) }}</strong></span>
      <span>
        Épargné
        <strong :class="totals.savedCents >= 0 ? 'positive' : 'negative'">
          {{ formatAmount(totals.savedCents, { cents: false, signed: true }) }}
        </strong>
        <template v-if="totals.savedPercent !== null"> ({{ totals.savedPercent }} %)</template>
      </span>
    </div>
  </section>

  <section class="grid">
    <div class="column">
      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Par catégorie</h2>
          <span class="muted">{{ period.label }}</span>
        </div>
        <div v-if="slices.length > 0" class="chart" :style="{ height: `${Math.max(160, slices.length * 40)}px` }">
          <AppChart :option="barsOption" :label="`Dépenses par catégorie, ${period.label}`" />
        </div>
        <p v-else class="muted empty">Aucune dépense sur la période.</p>
      </div>

      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Évolution du solde</h2>
          <select v-model="balanceAccount" class="account-select" aria-label="Compte">
            <option :value="null">Tous les comptes</option>
            <option v-for="{ account } in accounts" :key="account.id" :value="account.id">{{ account.name }}</option>
          </select>
        </div>
        <div v-if="lastBalance !== null" class="balance">{{ formatAmount(lastBalance) }}</div>
        <div class="chart balance-chart">
          <AppChart :option="balanceOption" label="Évolution du solde sur 12 mois" />
        </div>
      </div>
    </div>

    <div class="column">
      <div class="card net-worth">
        <div class="card-header">
          <h2 class="card-title">Patrimoine net</h2>
          <span class="change">{{ formatAmount(netWorthChange, { cents: false, signed: true }) }} sur 12 mois</span>
        </div>
        <div class="net-worth-value">{{ formatAmount(currentNetWorth, { cents: false }) }}</div>
        <div class="chart net-worth-chart">
          <AppChart :option="netWorthChart" label="Patrimoine net en fin de mois sur 12 mois" />
        </div>
        <div class="range">
          <span>{{ netWorthRange[0] }}</span>
          <span>{{ netWorthRange[1] }}</span>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Répartition des dépenses</h2>
          <span class="muted">{{ period.label }}</span>
        </div>
        <div v-if="slices.length > 0" class="chart donut">
          <AppChart :option="donutOption" :label="`Répartition des dépenses, ${period.label}`" />
        </div>
        <p v-else class="muted empty">Aucune dépense sur la période.</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
}

.subtitle {
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-muted);
}

h1 {
  margin: 2px 0 0;
  font-size: 30px;
  font-weight: 800;
  letter-spacing: -0.02em;
}

.controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.segmented,
.period {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
  background: var(--color-card);
}

.segmented button,
.period button {
  height: 38px;
  padding: 0 14px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--color-text);
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
}

.segmented button[aria-pressed="true"] {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.period span {
  min-width: 150px;
  font-size: 14px;
  font-weight: 700;
  text-align: center;
}

.period button {
  padding: 0 12px;
  font-size: 18px;
}

.period button:hover:not(:disabled),
.segmented button:hover:not([aria-pressed="true"]) {
  background: var(--color-page);
}

.period button:disabled {
  color: var(--color-border);
  cursor: default;
}

.card-header {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.muted {
  font-size: 13px;
  color: var(--color-text-muted);
}

.empty {
  margin: 0;
}

.chart.trend {
  height: 260px;
}

.chart.donut {
  height: 240px;
}

.chart.balance-chart {
  height: 220px;
}

.totals {
  display: flex;
  flex-wrap: wrap;
  gap: 10px 32px;
  font-size: 14px;
  color: var(--color-text-muted);
}

.totals strong {
  color: var(--color-text);
}

.totals strong.positive {
  color: var(--color-positive);
}

.totals strong.negative {
  color: var(--cat-restaurants-fg);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
  align-items: start;
  gap: 16px;
}

.column {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.net-worth {
  gap: 12px;
  border-color: transparent;
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.net-worth .card-title {
  color: var(--color-on-primary);
}

.net-worth .change,
.net-worth .range {
  font-size: 13px;
  color: var(--color-on-primary-muted);
}

.net-worth .range {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
}

.net-worth-value {
  font-size: 28px;
  font-weight: 800;
  letter-spacing: -0.02em;
}

.chart.net-worth-chart {
  height: 120px;
}

.account-select {
  height: 34px;
  padding: 0 10px;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: var(--color-card);
  color: var(--color-text);
  font-size: 13px;
  font-weight: 600;
}

.balance {
  font-size: 28px;
  font-weight: 800;
  letter-spacing: -0.02em;
}
</style>
