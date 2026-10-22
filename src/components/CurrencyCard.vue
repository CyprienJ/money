<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { commands, type CurrencyUse, type ExchangeRate } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { currencyLabel, formatRate, parseRate } from "../utils/currency";
import { formatDate, todayIso } from "../utils/format";

/** Rates shown for the open currency. */
const RATES_SHOWN = 5;

const uses = ref<CurrencyUse[]>([]);
const loaded = ref(false);
const error = ref("");
/** Currency whose rates are shown, `null` when all are closed. */
const openCurrency = ref<string | null>(null);
const rates = ref<ExchangeRate[]>([]);
const form = reactive({ date: todayIso(), rate: "" });
const formError = ref("");

async function load() {
  try {
    uses.value = await unwrap(commands.currenciesInUse());
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

async function loadRates(currency: string) {
  try {
    rates.value = await unwrap(commands.listExchangeRates(currency));
  } catch (e) {
    formError.value = errorMessage(e);
  }
}

async function toggle(currency: string) {
  formError.value = "";
  form.date = todayIso();
  form.rate = "";
  rates.value = [];
  openCurrency.value = openCurrency.value === currency ? null : currency;
  if (openCurrency.value) await loadRates(currency);
}

async function addRate(currency: string) {
  formError.value = "";
  const rateNano = parseRate(form.rate);
  if (rateNano === null) {
    formError.value = "Taux invalide, par exemple 0,9234.";
    return;
  }
  try {
    await unwrap(commands.setExchangeRate(currency, form.date, rateNano));
    form.rate = "";
    await Promise.all([loadRates(currency), load()]);
  } catch (e) {
    formError.value = errorMessage(e);
  }
}

async function removeRate(rate: ExchangeRate) {
  formError.value = "";
  try {
    await unwrap(commands.deleteExchangeRate(rate.currency, rate.date));
    await Promise.all([loadRates(rate.currency), load()]);
  } catch (e) {
    formError.value = errorMessage(e);
  }
}

onMounted(load);
</script>

<template>
  <section class="card">
    <h2 class="card-title">Devises</h2>
    <p class="muted">
      Les statistiques sont en euros. Un montant dans une autre devise y est converti au taux de son jour, un solde
      au taux du jour du solde (le dernier taux connu). Les taux se saisissent ici, et sont récupérés avec les cours
      de bourse quand c'est activé ; un taux saisi à la main n'est jamais remplacé.
    </p>
    <p v-if="error" class="error-banner">{{ error }}</p>
    <p v-else-if="loaded && uses.length === 0" class="muted">
      Tous tes comptes et tes titres sont en euros. La devise d'un compte se choisit dans sa fiche.
    </p>
    <ul v-else class="currencies">
      <li v-for="use in uses" :key="use.currency">
        <div class="row">
          <span class="details">
            <strong>{{ currencyLabel(use.currency) }}</strong>
            <span v-if="use.latest" class="muted">
              1 {{ use.currency }} = {{ formatRate(use.latest.rateNano) }} € au {{ formatDate(use.latest.date) }}
            </span>
            <span v-else class="missing">Aucun taux : ses montants comptent pour zéro dans les statistiques.</span>
          </span>
          <button type="button" class="btn btn-link" @click="toggle(use.currency)">
            {{ openCurrency === use.currency ? "Fermer" : "Taux…" }}
          </button>
        </div>

        <div v-if="openCurrency === use.currency" class="rates">
          <form class="rate-form" novalidate @submit.prevent="addRate(use.currency)">
            <label class="field">
              Date
              <input v-model="form.date" type="date" />
            </label>
            <label class="field">
              Euros pour 1 {{ use.currency }}
              <input v-model="form.rate" type="text" inputmode="decimal" placeholder="0,9234" />
            </label>
            <button type="submit" class="btn btn-secondary" :disabled="form.date === ''">Enregistrer</button>
          </form>
          <p v-if="formError" class="field-error">{{ formError }}</p>
          <ul v-if="rates.length > 0" class="rate-list">
            <li v-for="rate in rates.slice(0, RATES_SHOWN)" :key="rate.date">
              <span>{{ formatDate(rate.date) }}</span>
              <strong>{{ formatRate(rate.rateNano) }} €</strong>
              <span class="source">{{ rate.source === "MANUAL" ? "saisi" : "Yahoo" }}</span>
              <button
                type="button"
                class="btn btn-link"
                :aria-label="`Supprimer le taux du ${formatDate(rate.date)}`"
                @click="removeRate(rate)"
              >
                ×
              </button>
            </li>
          </ul>
          <p v-else class="muted">Aucun taux.</p>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.missing {
  font-size: 13px;
  font-weight: 600;
  color: var(--cat-restaurants-fg);
}

.currencies {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.details {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.rates {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 10px;
  padding: 12px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-nav);
}

.rate-form {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 12px;
}

.rate-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 13px;
}

.rate-list li {
  display: grid;
  grid-template-columns: 90px 1fr auto auto;
  align-items: center;
  gap: 12px;
}

.source {
  color: var(--color-text-muted);
}
</style>
