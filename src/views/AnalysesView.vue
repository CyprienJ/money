<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  commands,
  type AccountSummary,
  type BalancePoint,
  type Category,
  type CategoryComparison,
  type CategoryExpense,
  type LargestExpense,
  type NetWorth,
  type PayeeExpense,
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
import { categorySlices, comparisonSlices, trendTotals } from "../utils/analyses";
import { errorMessage, unwrap } from "../utils/commands";
import CategoryPill from "../components/CategoryPill.vue";
import { displayLabel, formatAmount, formatDate } from "../utils/format";
import {
  type AnalysisGranularity,
  type ComparisonMode,
  analysisPeriod,
  comparedPeriod,
} from "../utils/periods";

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
/** Compare the expenses by category with another period, as in the mockup (on by default). */
const comparing = ref(true);
const comparisonMode = ref<ComparisonMode>("previousYear");
const comparison = ref<CategoryComparison[]>([]);
/** Payees with the largest expenses over the 12 months of the trend. */
const topPayees = ref<PayeeExpense[]>([]);
/** Largest single expenses of the period. */
const largest = ref<LargestExpense[]>([]);
const balance = ref<BalancePoint[]>([]);
/** Net worth at the end of each month, from the end of the month before the trend. */
const netWorth = ref<NetWorth[]>([]);
const error = ref("");

const period = computed(() => analysisPeriod(granularity.value, offset.value));
const totals = computed(() => trendTotals(trend.value));
const slices = computed(() =>
  comparing.value
    ? comparisonSlices(comparison.value, categories.value)
    : categorySlices(expenses.value, categories.value),
);
/** Current expenses only, for the ring. */
const currentSlices = computed(() => slices.value.filter((s) => s.amountCents > 0));
const lowerFirst = (text: string) => text.charAt(0).toLowerCase() + text.slice(1);
const categoriesById = computed(() => new Map(categories.value.map((c) => [c.id, c])));
const categoryOf = (id: number | null) => (id === null ? undefined : categoriesById.value.get(id));
/** The periods offered for the comparison: a year earlier, or the period before. */
const comparisonOptions = computed(() =>
  (["previousYear", "previousPeriod"] as ComparisonMode[]).map((mode) => ({
    mode,
    label: lowerFirst(comparedPeriod(granularity.value, offset.value, mode).label),
  })),
);
const comparedLabel = computed(
  () => comparisonOptions.value.find((o) => o.mode === comparisonMode.value)?.label ?? "",
);
const lastBalance = computed(() => balance.value[balance.value.length - 1]?.balanceCents ?? null);

const trendOption = computed(() => incomeExpensesOption(trend.value, "month", tokens));
const barsOption = computed(() => categoryBarsOption(slices.value, tokens));
const donutOption = computed(() => categoryDonutOption(currentSlices.value, tokens));
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
    const compared = comparedPeriod(granularity.value, offset.value, comparisonMode.value);
    [trend.value, expenses.value, comparison.value, balance.value, netWorth.value] = await Promise.all([
      unwrap(commands.incomeVsExpenses(trendFrom, to, null, "month")),
      unwrap(commands.expensesByCategory(from, to, null, "topLevel")),
      comparing.value
        ? unwrap(
            commands.compareExpensesByCategory(
              { from, to },
              { from: compared.from, to: compared.to },
              null,
              "topLevel",
            ),
          )
        : Promise.resolve([]),
      unwrap(commands.balanceHistory(trendFrom, to, balanceAccounts, "day")),
      unwrap(commands.netWorthHistory(trendBase, to, "month")),
    ]);
    [topPayees.value, largest.value] = await Promise.all([
      unwrap(commands.topPayees(trendFrom, to, null, 5)),
      unwrap(commands.largestExpenses(from, to, null, 4)),
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
watch([period, balanceAccount, comparing, comparisonMode], loadStats);
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
      <div class="compare">
        <label>
          <input v-model="comparing" type="checkbox" />
          Comparer à
        </label>
        <select v-model="comparisonMode" :disabled="!comparing" aria-label="Période comparée">
          <option v-for="o in comparisonOptions" :key="o.mode" :value="o.mode">{{ o.label }}</option>
        </select>
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
          <span class="muted">
            {{ period.label }}<template v-if="comparing"> vs {{ comparedLabel }} (barre fine)</template>
          </span>
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
          <h2 class="card-title">Top tiers sur 12 mois</h2>
          <span class="muted">jusqu'à {{ lowerFirst(period.label) }}</span>
        </div>
        <ol v-if="topPayees.length > 0" class="payees">
          <li v-for="payee in topPayees" :key="payee.normalizedLabel">
            <span class="payee-row">
              <span class="payee-name">{{ displayLabel(payee.normalizedLabel) }}</span>
              <CategoryPill
                :label="categoryOf(payee.categoryId)?.name ?? 'À catégoriser'"
                :color="categoryOf(payee.categoryId)?.color"
                :pending="payee.categoryId === null"
              />
              <strong class="payee-amount">{{ formatAmount(payee.expenseCents, { cents: false }) }}</strong>
            </span>
          </li>
        </ol>
        <p v-else class="muted empty">Aucune dépense sur la période.</p>
      </div>

      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Répartition des dépenses</h2>
          <span class="muted">{{ period.label }}</span>
        </div>
        <div v-if="currentSlices.length > 0" class="chart donut">
          <AppChart :option="donutOption" :label="`Répartition des dépenses, ${period.label}`" />
        </div>
        <p v-else class="muted empty">Aucune dépense sur la période.</p>
      </div>
    </div>
  </section>

  <section class="card">
    <div class="card-header">
      <h2 class="card-title">Plus grosses dépenses</h2>
      <span class="muted">{{ period.label }}</span>
    </div>
    <div v-if="largest.length > 0" class="tiles">
      <div
        v-for="expense in largest"
        :key="expense.operationId"
        class="tile"
        :style="{
          background: `var(--cat-${categoryOf(expense.categoryId)?.color ?? 'neutral'}-bg)`,
          '--tile-accent': `var(--cat-${categoryOf(expense.categoryId)?.color ?? 'neutral'}-fg)`,
        }"
      >
        <span class="tile-meta">
          {{ formatDate(expense.date, "day-month") }} · {{ categoryOf(expense.categoryId)?.name ?? "À catégoriser" }}
        </span>
        <span class="tile-label">{{ displayLabel(expense.normalizedLabel) }}</span>
        <strong class="tile-amount">{{ formatAmount(expense.expenseCents) }}</strong>
      </div>
    </div>
    <p v-else class="muted empty">Aucune dépense sur la période.</p>
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

.compare {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 46px;
  padding: 0 6px 0 14px;
  box-sizing: border-box;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
  background: var(--color-card);
  font-size: 14px;
  font-weight: 600;
}

.compare label {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}

.compare input {
  width: 18px;
  height: 18px;
  accent-color: var(--color-primary);
}

.compare select {
  height: 34px;
  padding: 0 8px;
  border: 0;
  border-radius: 8px;
  background: var(--color-page);
  color: var(--color-text);
  font-size: 14px;
  font-weight: 700;
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

.payees {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin: 0;
  padding: 0 0 0 22px;
  font-size: 14px;
}

.payees li::marker {
  color: var(--color-text-muted);
  font-weight: 700;
}

.payee-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.payee-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-weight: 700;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.payee-amount {
  width: 84px;
  text-align: right;
  white-space: nowrap;
}

.tiles {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
}

.tile {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px 16px;
  border-radius: 14px;
}

.tile-meta {
  font-size: 12px;
  font-weight: 700;
  color: var(--tile-accent);
}

.tile-label {
  font-weight: 700;
}

.tile-amount {
  font-size: 20px;
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
