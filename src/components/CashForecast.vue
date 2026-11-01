<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { commands, type Account, type BalanceForecast } from "../bindings";
import { forecastOption } from "../charts/options";
import { useChartTokens } from "../charts/theme";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate } from "../utils/format";
import { FORECAST_DAYS } from "../utils/schedule";
import AppChart from "./AppChart.vue";

/** Projected balance of one account or of all of them, over 30, 60 or 90 days. */
const props = defineProps<{
  /** Accounts with operations; reassigning the list reloads the forecast. */
  accounts: Account[];
}>();

const tokens = useChartTokens();
const days = ref<number>(FORECAST_DAYS[0]);
/** `null` for every account. */
const accountId = ref<number | null>(null);
const forecast = ref<BalanceForecast | null>(null);
const showOccurrences = ref(false);
const error = ref("");

const accountNames = computed(() => new Map(props.accounts.map((a) => [a.id, a.name])));
const projected = computed(() => forecast.value?.points[forecast.value.points.length - 1] ?? null);
const option = computed(() => (forecast.value ? forecastOption(forecast.value, tokens.value) : null));

let lastLoad = 0;
async function load() {
  const current = ++lastLoad;
  error.value = "";
  if (accountId.value !== null && !accountNames.value.has(accountId.value)) accountId.value = null;
  try {
    const result = await unwrap(
      commands.balanceForecast(days.value, accountId.value === null ? null : [accountId.value]),
    );
    if (current === lastLoad) forecast.value = result;
  } catch (e) {
    if (current === lastLoad) error.value = errorMessage(e);
  }
}

watch([days, accountId, () => props.accounts], load, { immediate: true });
</script>

<template>
  <section class="card forecast">
    <div class="card-header">
      <h2 class="card-title">Prévision de trésorerie</h2>
      <div class="controls">
        <select v-model="accountId" class="account-select" aria-label="Compte">
          <option :value="null">Tous les comptes</option>
          <option v-for="account in accounts" :key="account.id" :value="account.id">{{ account.name }}</option>
        </select>
        <div role="group" aria-label="Horizon" class="segmented">
          <button
            v-for="d in FORECAST_DAYS"
            :key="d"
            type="button"
            :aria-pressed="days === d"
            @click="days = d"
          >
            {{ d }} j
          </button>
        </div>
      </div>
    </div>

    <p v-if="error" class="error-banner">{{ error }}</p>

    <template v-if="forecast && projected">
      <dl class="figures">
        <div>
          <dt>Aujourd'hui</dt>
          <dd>{{ formatAmount(forecast.currentBalanceCents) }}</dd>
        </div>
        <div>
          <dt>Dans {{ days }} jours</dt>
          <dd :class="{ negative: projected.balanceCents < 0 }">{{ formatAmount(projected.balanceCents) }}</dd>
        </div>
        <div>
          <dt>Point bas</dt>
          <dd :class="{ negative: forecast.lowest.balanceCents < 0 }">
            {{ formatAmount(forecast.lowest.balanceCents) }}
            <span class="sub">le {{ formatDate(forecast.lowest.date, "day-month") }}</span>
          </dd>
        </div>
      </dl>
      <p v-if="forecast.lowest.balanceCents < 0" class="warning">
        Le solde passerait sous zéro le {{ formatDate(forecast.points.find((p) => p.balanceCents < 0)!.date) }}.
      </p>

      <div class="chart">
        <AppChart v-if="option" :option="option" :label="`Solde projeté sur ${days} jours`" />
      </div>

      <p class="muted">
        Solde actuel et échéances non réalisées ; une échéance en retard de moins de 5 jours compte
        aujourd'hui. Part personnelle : solo à 100 %, joint à 50 % ; les échéances ci-dessous restent à 100 %.<template v-if="accountId === null"> Les virements internes déclarés dans
        l'échéancier ne changent pas le total.</template>
      </p>

      <button
        v-if="forecast.occurrences.length > 0"
        type="button"
        class="btn btn-link toggle"
        :aria-expanded="showOccurrences"
        @click="showOccurrences = !showOccurrences"
      >
        {{ showOccurrences ? "Masquer" : "Voir" }} les {{ forecast.occurrences.length }} échéance{{
          forecast.occurrences.length > 1 ? "s" : ""
        }} prise{{ forecast.occurrences.length > 1 ? "s" : "" }} en compte
      </button>
      <p v-else class="muted">Aucune échéance à venir sur la période.</p>
      <ul v-if="showOccurrences" class="list">
        <li v-for="o in forecast.occurrences" :key="`${o.scheduledOperationId}-${o.occurrenceDate}`">
          <span class="date">{{ formatDate(o.date, "day-month") }}</span>
          <span class="details">
            <span class="label">{{ o.label }}</span>
            <span class="sub">
              {{ accountNames.get(o.accountId) }}
              <template v-if="o.date !== o.occurrenceDate"> · prévue le {{ formatDate(o.occurrenceDate) }}, en retard</template>
            </span>
          </span>
          <strong class="amount" :class="{ positive: o.amountCents > 0 }">
            {{ formatAmount(o.amountCents, { signed: true, currency: o.currency }) }}
          </strong>
        </li>
      </ul>
    </template>
  </section>
</template>

<style scoped>
.card-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.account-select {
  height: 34px;
  padding: 0 10px;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: var(--color-card);
  color: var(--color-text);
  font-size: 14px;
  font-weight: 600;
}

.segmented {
  display: flex;
  gap: 4px;
  padding: 3px;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: var(--color-card);
}

.segmented button {
  height: 28px;
  padding: 0 10px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--color-text);
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}

.segmented button[aria-pressed="true"] {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.segmented button:hover:not([aria-pressed="true"]) {
  background: var(--color-page);
}

.figures {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 32px;
  margin: 0;
}

.figures dt {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-muted);
}

.figures dd {
  margin: 2px 0 0;
  font-size: 20px;
  font-weight: 800;
  font-variant-numeric: tabular-nums;
}

.warning {
  margin: 0;
  padding: 8px 12px;
  border-radius: var(--radius-nav);
  background: var(--cat-restaurants-bg);
  color: var(--cat-restaurants-fg);
  font-size: 13px;
  font-weight: 600;
}

.chart {
  height: 240px;
}

.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.toggle {
  align-self: flex-start;
}

.list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 14px;
}

.list > li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid var(--color-divider);
}

.list > li:last-child {
  border-bottom: 0;
}

.date {
  width: 44px;
  font-weight: 700;
  color: var(--color-text-muted);
}

.details {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.label {
  font-weight: 700;
}

.sub {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-muted);
}

.amount {
  white-space: nowrap;
}

.amount.positive {
  color: var(--color-positive);
}

.negative {
  color: var(--cat-restaurants-fg);
}
</style>
