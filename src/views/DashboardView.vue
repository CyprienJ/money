<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  commands,
  type Account,
  type AccountSummary,
  type Alert,
  type BalanceForecast,
  type BalancePoint,
  type Category,
  type CategoryExpense,
  type NetWorth,
  type PayeeExpense,
  type PeriodIncomeExpenses,
  type UpcomingOperation,
} from "../bindings";
import AccountBadge from "../components/AccountBadge.vue";
import AccountFormModal from "../components/AccountFormModal.vue";
import AppChart from "../components/AppChart.vue";
import {
  availableHistoryOption,
  categoryBarsOption,
  categoryDonutOption,
  periodLabel,
} from "../charts/options";
import { useChartTokens } from "../charts/theme";
import { ACCOUNT_TYPES, valuationSubtitle } from "../utils/accounts";
import { alertCount, describeAlert } from "../utils/alerts";
import { categorySlices, legendSlices, variationPercent } from "../utils/analyses";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate, localDateOf, todayIso } from "../utils/format";
import { analysisPeriod } from "../utils/periods";
import { FORECAST_DAYS } from "../utils/schedule";

/** Days of upcoming scheduled operations shown. */
const UPCOMING_DAYS = 15;

const tokens = useChartTokens();
/** 0 for the current month, -1 for the previous one… */
const offset = ref(0);
const month = computed(() => analysisPeriod("month", offset.value));
const previousMonth = computed(() => analysisPeriod("month", offset.value - 1));

const summaries = ref<AccountSummary[]>([]);
/** Date of the last import of each account. */
const lastImports = ref(new Map<number, string>());
const categories = ref<Category[]>([]);
const netWorth = ref<NetWorth[]>([]);
const months = ref<PeriodIncomeExpenses[]>([]);
const expenses = ref<CategoryExpense[]>([]);
const available = ref<BalancePoint[]>([]);
const upcoming = ref<UpcomingOperation[]>([]);
const forecast = ref<BalanceForecast | null>(null);
const alerts = ref<Alert[]>([]);
const alertError = ref("");
const payees = ref<PayeeExpense[]>([]);
const loaded = ref(false);
const error = ref("");
const accountModalOpen = ref(false);

const categoriesById = computed(() => new Map(categories.value.map((c) => [c.id, c])));
const lowerFirst = (text: string) => text.charAt(0).toLowerCase() + text.slice(1);
const previousName = computed(() => lowerFirst(previousMonth.value.label.split(" ")[0]));

const currentNetWorth = computed(() => netWorth.value[netWorth.value.length - 1] ?? null);
/** Net worth change since the end of the previous month, in percent with one decimal. */
const netWorthChange = computed(() => {
  const [before, after] = netWorth.value;
  if (!before || !after || before.netWorthCents === 0) return null;
  const tenths = Math.round(((after.netWorthCents - before.netWorthCents) * 1000) / Math.abs(before.netWorthCents));
  const sign = tenths > 0 ? "+" : tenths < 0 ? "−" : "";
  return `${sign}${(Math.abs(tenths) / 10).toFixed(1).replace(".", ",")} %`;
});
const alertViews = computed(() => {
  const names = {
    accounts: new Map(summaries.value.map((s) => [s.account.id, s.account.name])),
    categories: new Map(categories.value.map((c) => [c.id, c.name])),
  };
  return alerts.value.map((alert) => ({ alert, view: describeAlert(alert, names) }));
});
const projected = computed(() => forecast.value?.points[forecast.value.points.length - 1] ?? null);
const thisMonth = computed(() => months.value[months.value.length - 1] ?? null);
const lastMonth = computed(() => (months.value.length > 1 ? months.value[0] : null));
const expenseChange = computed(() =>
  thisMonth.value && lastMonth.value
    ? variationPercent(thisMonth.value.expenseCents, lastMonth.value.expenseCents)
    : null,
);
const remaining = computed(() =>
  thisMonth.value ? thisMonth.value.incomeCents - thisMonth.value.expenseCents : 0,
);

const slices = computed(() => legendSlices(categorySlices(expenses.value, categories.value)));
const totalExpenses = computed(() => slices.value.reduce((sum, s) => sum + s.amountCents, 0));
const donutOption = computed(() => categoryDonutOption(slices.value, tokens.value));
const availableOption = computed(() => availableHistoryOption(available.value, tokens.value));
const payeesOption = computed(() =>
  categoryBarsOption(
    payees.value.map((p) => {
      const category = p.categoryId === null ? undefined : categoriesById.value.get(p.categoryId);
      return {
        name: p.payeeName,
        color: category?.color ?? "neutral",
        amountCents: p.expenseCents,
      };
    }),
    tokens.value,
  ),
);

function accountSubtitle(summary: AccountSummary): string {
  const { account } = summary;
  if (!ACCOUNT_TYPES[account.type].hasOperations) return valuationSubtitle(account.type, summary.valuedOn);
  const imported = lastImports.value.get(account.id);
  return imported
    ? `Importé le ${formatDate(localDateOf(imported), "day-month")}`
    : ACCOUNT_TYPES[account.type].label;
}

async function loadAccounts() {
  summaries.value = await unwrap(commands.listAccountSummaries(false));
  const withOperations = summaries.value.filter((s) => ACCOUNT_TYPES[s.account.type].hasOperations);
  const batches = await Promise.all(
    withOperations.map((s) => unwrap(commands.listImportBatches(s.account.id))),
  );
  lastImports.value = new Map(
    withOperations.flatMap((s, index) => {
      const last = batches[index][0];
      return last ? [[s.account.id, last.importedAt] as [number, string]] : [];
    }),
  );
}

async function load() {
  error.value = "";
  const { from, to, trendFrom } = month.value;
  const previous = previousMonth.value;
  try {
    [
      categories.value,
      netWorth.value,
      months.value,
      expenses.value,
      available.value,
      upcoming.value,
      payees.value,
      forecast.value,
      alerts.value,
    ] = await Promise.all([
        unwrap(commands.listCategories()),
        unwrap(commands.netWorthHistory(previous.to, to, "month")),
        unwrap(commands.incomeVsExpenses(previous.from, to, null, "month")),
        unwrap(commands.expensesByCategory(from, to, null, "topLevel")),
        unwrap(commands.balanceHistory(trendFrom, to, null, "month")),
        unwrap(commands.upcomingOperations(UPCOMING_DAYS)),
        unwrap(commands.topPayees(from, to, null, 5)),
        unwrap(commands.balanceForecast(FORECAST_DAYS[0], null)),
        unwrap(commands.listAlerts()),
      ]);
    alertCount.value = alerts.value.length;
    await loadAccounts();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

async function dismissAlert(alert: Alert) {
  alertError.value = "";
  try {
    if (alert.kind === "unusualExpense") {
      await unwrap(commands.dismissUnusualExpense(alert.categoryId, alert.month));
    } else if (alert.kind === "missingOccurrence") {
      const { scheduledOperationId, date } = alert.occurrence;
      await unwrap(commands.dismissMissingOccurrence(scheduledOperationId, date));
    }
    alerts.value = await unwrap(commands.listAlerts());
    alertCount.value = alerts.value.length;
  } catch (e) {
    alertError.value = errorMessage(e);
  }
}

async function onAccountSaved(_: Account) {
  accountModalOpen.value = false;
  await load();
}

onMounted(load);
watch(offset, load);
</script>

<template>
  <header class="header">
    <div>
      <div class="subtitle">{{ formatDate(todayIso(), "long") }}</div>
      <h1>Tableau de bord</h1>
    </div>
    <div class="controls">
      <div class="month" aria-live="polite">
        <button type="button" aria-label="Mois précédent" @click="offset--">‹</button>
        <span>{{ month.label }}</span>
        <button type="button" aria-label="Mois suivant" :disabled="offset >= 0" @click="offset++">›</button>
      </div>
      <RouterLink to="/import" class="btn btn-primary">Importer un relevé</RouterLink>
    </div>
  </header>

  <p v-if="error" class="error-banner">{{ error }}</p>

  <section v-if="loaded && summaries.length === 0" class="card welcome">
    <h2 class="card-title">Bienvenue</h2>
    <p class="muted">
      Commence par créer tes comptes (courant, épargne, placements…), puis importe un relevé de ta banque :
      le tableau de bord se remplira avec tes soldes, tes dépenses et ton patrimoine.
    </p>
    <div class="welcome-actions">
      <button type="button" class="btn btn-primary" @click="accountModalOpen = true">+ Créer un compte</button>
      <RouterLink to="/import" class="btn btn-secondary">Importer un relevé</RouterLink>
    </div>
  </section>

  <template v-else-if="loaded">
    <section v-if="alertViews.length > 0" class="card alerts" aria-label="Alertes">
      <div class="card-header">
        <h2 class="card-title">Alertes</h2>
        <span class="muted">{{ alertViews.length }}</span>
      </div>
      <p v-if="alertError" class="error-banner">{{ alertError }}</p>
      <ul class="alert-list">
        <li v-for="{ alert, view } in alertViews" :key="view.key" :class="{ danger: alert.kind === 'negativeForecast' }">
          <span class="alert-mark" aria-hidden="true">!</span>
          <span class="details">
            <RouterLink :to="view.to" class="name alert-title">{{ view.title }}</RouterLink>
            <span class="sub">{{ view.detail }}</span>
          </span>
          <button v-if="view.dismissible" type="button" class="btn btn-link" @click="dismissAlert(alert)">Masquer</button>
        </li>
      </ul>
    </section>

    <section aria-label="Indicateurs clés" class="kpis">
      <div class="kpi dark">
        <div class="kpi-head"><span>Patrimoine net</span><span class="kpi-icon accent" /></div>
        <div class="kpi-value">{{ formatAmount(currentNetWorth?.netWorthCents ?? 0, { cents: false }) }}</div>
        <div v-if="netWorthChange" class="kpi-note">
          <span class="change light">{{ netWorthChange }}</span> vs fin {{ previousName }}
        </div>
      </div>
      <div class="kpi">
        <div class="kpi-head"><span>Disponible</span><span class="kpi-icon transport" /></div>
        <div class="kpi-value">{{ formatAmount(currentNetWorth?.cashCents ?? 0, { cents: false }) }}</div>
        <div class="kpi-note">Comptes courants et épargne, cartes déduites</div>
      </div>
      <div class="kpi">
        <div class="kpi-head"><span>Dépenses du mois</span><span class="kpi-icon restaurants" /></div>
        <div class="kpi-value">{{ formatAmount(thisMonth?.expenseCents ?? 0, { cents: false }) }}</div>
        <div v-if="expenseChange !== null" class="kpi-note">
          <span class="change" :class="expenseChange > 0 ? 'up' : 'down'">
            {{ expenseChange > 0 ? "+" : expenseChange < 0 ? "−" : "" }}{{ Math.abs(expenseChange) }} %
          </span>
          vs {{ previousName }}
        </div>
      </div>
      <div class="kpi">
        <div class="kpi-head"><span>Revenus du mois</span><span class="kpi-icon housing" /></div>
        <div class="kpi-value">{{ formatAmount(thisMonth?.incomeCents ?? 0, { cents: false }) }}</div>
        <div class="kpi-note">
          Reste
          <strong :class="remaining >= 0 ? 'positive' : 'negative'">
            {{ formatAmount(remaining, { cents: false, signed: true }) }}
          </strong>
          ce mois-ci
        </div>
      </div>
      <RouterLink v-if="forecast && projected" to="/schedule" class="kpi kpi-link">
        <div class="kpi-head"><span>Solde projeté à {{ FORECAST_DAYS[0] }} j</span><span class="kpi-icon subscriptions" /></div>
        <div class="kpi-value" :class="{ negative: projected.balanceCents < 0 }">
          {{ formatAmount(projected.balanceCents, { cents: false }) }}
        </div>
        <div class="kpi-note">
          Point bas
          <strong :class="{ negative: forecast.lowest.balanceCents < 0 }">
            {{ formatAmount(forecast.lowest.balanceCents, { cents: false }) }}
          </strong>
          le {{ formatDate(forecast.lowest.date, "day-month") }}
        </div>
      </RouterLink>
    </section>

    <section class="row two">
      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Dépenses par catégorie</h2>
          <RouterLink to="/analyses" class="link">Détail</RouterLink>
        </div>
        <div v-if="slices.length > 0" class="split">
          <div class="donut">
            <AppChart :option="donutOption" :label="`Dépenses de ${lowerFirst(month.label)} par catégorie`" />
          </div>
          <ul class="legend">
            <li v-for="slice in slices" :key="slice.name">
              <span class="swatch" :style="{ background: `var(--cat-${slice.color})` }" />
              <span class="legend-name" :class="{ muted: slice.name.endsWith(', autres') }">{{ slice.name }}</span>
              <strong>{{ formatAmount(slice.amountCents, { cents: false }) }}</strong>
            </li>
          </ul>
        </div>
        <p v-else class="muted">Aucune dépense ce mois-ci.</p>
        <span v-if="totalExpenses > 0" class="sr-only">Total {{ formatAmount(totalExpenses) }}</span>
      </div>

      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Évolution du disponible</h2>
          <span class="muted">12 derniers mois</span>
        </div>
        <div class="chart available">
          <AppChart :option="availableOption" label="Évolution du disponible sur 12 mois" />
        </div>
      </div>
    </section>

    <section class="row three">
      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Comptes</h2>
          <button type="button" class="link" @click="accountModalOpen = true">+ Ajouter</button>
        </div>
        <ul class="list">
          <li v-for="summary in summaries" :key="summary.account.id">
            <RouterLink
              :to="ACCOUNT_TYPES[summary.account.type].hasOperations
                ? { name: 'operations', query: { account: String(summary.account.id) } }
                : '/accounts'"
              class="list-row"
            >
              <AccountBadge :name="summary.account.name" :type="summary.account.type" />
              <span class="details">
                <span class="name">{{ summary.account.name }}</span>
                <span class="sub">{{ accountSubtitle(summary) }}</span>
              </span>
              <strong v-if="summary.balanceCents !== null" :class="{ negative: summary.balanceCents < 0 }">
                {{ formatAmount(summary.balanceCents) }}
              </strong>
              <span v-else class="muted">—</span>
            </RouterLink>
          </li>
        </ul>
      </div>

      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Prochaines échéances</h2>
          <span class="muted">{{ UPCOMING_DAYS }} jours</span>
        </div>
        <ul v-if="upcoming.length > 0" class="list">
          <li v-for="item in upcoming" :key="`${item.scheduledOperationId}-${item.date}`" class="list-row">
            <span class="day" :class="{ income: item.amountCents > 0 }">
              <span class="day-number">{{ Number(item.date.slice(8, 10)) }}</span>
              <span class="day-month">{{ periodLabel(item.date, "month") }}</span>
            </span>
            <span class="details">
              <span class="name">{{ item.label }}</span>
              <span v-if="item.realizedBy" class="sub realized">Réalisée le {{ formatDate(item.realizedBy.date, "day-month") }}</span>
              <span v-else class="sub">{{ item.categoryId !== null ? categoriesById.get(item.categoryId)?.name : "" }}</span>
            </span>
            <strong :class="{ positive: item.amountCents > 0 }">
              {{ formatAmount(item.amountCents, { signed: true }) }}
            </strong>
          </li>
        </ul>
        <p v-else class="muted">
          Aucune échéance dans les {{ UPCOMING_DAYS }} jours.
          <RouterLink to="/schedule">Ajoute tes opérations récurrentes</RouterLink>.
        </p>
      </div>

      <div class="card">
        <div class="card-header">
          <h2 class="card-title">Top tiers du mois</h2>
          <RouterLink to="/analyses" class="link">Tout voir</RouterLink>
        </div>
        <div v-if="payees.length > 0" class="chart payees" :style="{ height: `${payees.length * 40}px` }">
          <AppChart :option="payeesOption" :label="`Tiers avec le plus de dépenses en ${lowerFirst(month.label)}`" />
        </div>
        <p v-else class="muted">Aucune dépense ce mois-ci.</p>
      </div>
    </section>
  </template>

  <AccountFormModal
    :open="accountModalOpen"
    :account="null"
    @close="accountModalOpen = false"
    @saved="onAccountSaved"
  />
</template>

<style scoped>
.header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
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

.month {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
  background: var(--color-card);
}

.month button {
  width: 36px;
  height: 36px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--color-text);
  font-size: 18px;
  cursor: pointer;
}

.month button:hover:not(:disabled) {
  background: var(--color-page);
}

.month button:disabled {
  color: var(--color-border);
  cursor: default;
}

.month span {
  min-width: 120px;
  padding: 0 8px;
  font-size: 14px;
  font-weight: 700;
  text-align: center;
}

.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.welcome-actions {
  display: flex;
  gap: 10px;
}

.kpis {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
  gap: 16px;
}

.kpi {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 20px 22px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
  background: var(--color-card);
}

.kpi.dark {
  border-color: transparent;
  background: var(--color-feature);
  color: var(--color-on-feature);
}

.kpi-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-muted);
}

.kpi.dark .kpi-head,
.kpi.dark .kpi-note {
  color: var(--color-on-feature-muted);
}

.kpi-icon {
  width: 34px;
  height: 34px;
  border-radius: 10px;
}

.kpi-icon.accent {
  background: var(--color-accent);
}

.kpi-icon.transport {
  background: var(--cat-transport-bg);
}

.kpi-icon.restaurants {
  background: var(--cat-restaurants-bg);
}

.kpi-icon.housing {
  background: var(--cat-housing-bg);
}

.alert-list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.alert-list > li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid var(--color-divider);
}

.alert-list > li:last-child {
  border-bottom: 0;
}

.alert-mark {
  flex: 0 0 28px;
  height: 28px;
  border-radius: 14px;
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
  font-weight: 800;
  line-height: 28px;
  text-align: center;
}

.danger .alert-mark {
  background: var(--cat-restaurants-bg);
  color: var(--cat-restaurants-fg);
}

.alert-title {
  color: inherit;
  text-decoration: none;
}

.alert-title:hover {
  text-decoration: underline;
}

.kpi-icon.subscriptions {
  background: var(--cat-subscriptions-bg);
}

.kpi-link {
  color: inherit;
  text-decoration: none;
}

.kpi-link:hover {
  border-color: var(--cat-housing);
}

.kpi-value {
  font-size: 30px;
  font-weight: 800;
  letter-spacing: -0.02em;
  white-space: nowrap;
}

.kpi-note {
  font-size: 13px;
  color: var(--color-text-muted);
}

.change {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-pill);
  font-weight: 700;
}

.change.light {
  background: rgba(255, 255, 255, 0.14);
  color: var(--color-on-feature);
}

.change.up {
  background: var(--cat-restaurants-bg);
  color: var(--cat-restaurants-fg);
}

.change.down {
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
}

.positive {
  color: var(--color-positive);
}

.negative {
  color: var(--cat-restaurants-fg);
}

.row {
  display: grid;
  gap: 16px;
  align-items: start;
}

.row.two {
  grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
}

.row.three {
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
}

.card-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.link {
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--cat-housing-fg);
  font-size: 13px;
  font-weight: 600;
  text-decoration: none;
  cursor: pointer;
}

.split {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 28px;
}

.donut {
  flex: none;
  width: 168px;
  height: 168px;
}

.legend {
  display: flex;
  flex: 1 1 200px;
  flex-direction: column;
  gap: 9px;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 14px;
}

.legend li {
  display: flex;
  align-items: center;
  gap: 10px;
}

.swatch {
  width: 10px;
  height: 10px;
  border-radius: 3px;
}

.legend-name {
  flex: 1;
}

.legend-name.muted {
  font-size: 14px;
}

.chart.available {
  height: 230px;
}

.list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 14px;
}

.list-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 0;
  color: inherit;
  text-decoration: none;
}

a.list-row {
  margin: 0 -8px;
  padding: 8px;
  border-radius: var(--radius-nav);
}

a.list-row:hover {
  background: var(--color-page);
}

.details {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.name {
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--color-text-muted);
}

.sub.realized {
  color: var(--cat-transport-fg);
  font-weight: 600;
}

.list-row strong {
  white-space: nowrap;
}

.day {
  display: flex;
  flex: none;
  flex-direction: column;
  align-items: center;
  width: 44px;
  padding: 4px 0;
  border-radius: 10px;
  background: var(--color-page);
  line-height: 1.15;
}

.day.income {
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
}

.day-number {
  font-size: 16px;
  font-weight: 800;
}

.day-month {
  font-size: 11px;
  color: var(--color-text-muted);
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}
</style>
