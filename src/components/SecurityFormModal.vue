<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { commands, type Security, type SecurityPrice, type SymbolCandidate } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { currencyLabel, REFERENCE_CURRENCY, supportedCurrencies } from "../utils/currency";
import { formatAmount, formatDate, parseAmount, todayIso } from "../utils/format";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
  /** Security to edit; `null` to create one. */
  security: Security | null;
  /** Fetching prices is allowed: the symbol can be searched by ISIN. */
  priceUpdatesEnabled: boolean;
}>();

const emit = defineEmits<{
  close: [];
  /** The created or updated security, `null` after a deletion. */
  changed: [security: Security | null];
}>();

const form = reactive({ name: "", isin: "", symbol: "", currency: REFERENCE_CURRENCY });
const currencies = ref<string[]>([REFERENCE_CURRENCY]);

onMounted(async () => {
  currencies.value = await supportedCurrencies();
});
const candidates = ref<SymbolCandidate[] | null>(null);
const searching = ref(false);
/** Prices of the edited security, the most recent first. */
const prices = ref<SecurityPrice[]>([]);
const priceForm = reactive({ date: todayIso(), price: "" });
const priceError = ref("");

/** Prices shown in the form. */
const PRICES_SHOWN = 5;
const submitted = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const error = ref("");

const nameError = computed(() => (form.name.trim() === "" ? "Le nom est obligatoire." : ""));

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    form.name = props.security?.name ?? "";
    form.isin = props.security?.isin ?? "";
    form.symbol = props.security?.symbol ?? "";
    form.currency = props.security?.currency ?? REFERENCE_CURRENCY;
    candidates.value = null;
    priceForm.date = todayIso();
    priceForm.price = "";
    priceError.value = "";
    prices.value = [];
    if (props.security) loadPrices(props.security.id);
    submitted.value = false;
    confirmingDelete.value = false;
    error.value = "";
  },
  { immediate: true },
);

async function save() {
  submitted.value = true;
  error.value = "";
  if (nameError.value) return;
  const input = {
    name: form.name,
    isin: form.isin.trim() || null,
    symbol: form.symbol.trim() || null,
    currency: form.currency,
  };
  saving.value = true;
  try {
    const security = props.security
      ? await unwrap(commands.updateSecurity(props.security.id, input))
      : await unwrap(commands.createSecurity(input));
    emit("changed", security);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function loadPrices(securityId: number) {
  try {
    prices.value = await unwrap(commands.listSecurityPrices(securityId));
  } catch (e) {
    priceError.value = errorMessage(e);
  }
}

async function searchSymbol() {
  error.value = "";
  searching.value = true;
  try {
    candidates.value = await unwrap(commands.searchSecuritySymbols(form.isin));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    searching.value = false;
  }
}

function chooseSymbol(candidate: SymbolCandidate) {
  form.symbol = candidate.symbol;
  if (!form.name.trim()) form.name = candidate.name;
  candidates.value = null;
}

async function addPrice() {
  const security = props.security;
  if (!security) return;
  priceError.value = "";
  const cents = parseAmount(priceForm.price);
  if (cents === null || cents <= 0) {
    priceError.value = "Cours invalide, par exemple 512,25.";
    return;
  }
  try {
    await unwrap(commands.setSecurityPrice(security.id, priceForm.date, cents));
    priceForm.price = "";
    await loadPrices(security.id);
  } catch (e) {
    priceError.value = errorMessage(e);
  }
}

async function removePrice(price: SecurityPrice) {
  priceError.value = "";
  try {
    await unwrap(commands.deleteSecurityPrice(price.securityId, price.date));
    await loadPrices(price.securityId);
  } catch (e) {
    priceError.value = errorMessage(e);
  }
}

async function remove() {
  if (!props.security) return;
  if (!confirmingDelete.value) {
    confirmingDelete.value = true;
    return;
  }
  saving.value = true;
  try {
    await unwrap(commands.deleteSecurity(props.security.id));
    emit("changed", null);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" :title="security ? 'Modifier le titre' : 'Nouveau titre'" @close="emit('close')">
    <form class="form" novalidate @submit.prevent="save">
      <label class="field">
        Nom
        <input v-model="form.name" type="text" autocomplete="off" placeholder="iShares Core MSCI World" />
        <span v-if="submitted && nameError" class="field-error">{{ nameError }}</span>
      </label>
      <label class="field">
        Code ISIN (facultatif)
        <input v-model="form.isin" type="text" autocomplete="off" placeholder="IE00B4L5Y983" />
        <span class="field-hint">12 caractères, sur l'avis d'opéré ou la fiche du titre.</span>
      </label>
      <label class="field">
        Symbole Yahoo Finance (facultatif)
        <span class="inline">
          <input v-model="form.symbol" type="text" autocomplete="off" placeholder="CW8.PA" />
          <button
            v-if="priceUpdatesEnabled"
            type="button"
            class="btn btn-secondary"
            :disabled="searching || form.isin.trim().length < 12"
            @click="searchSymbol"
          >
            {{ searching ? "Recherche…" : "Trouver par l'ISIN" }}
          </button>
        </span>
        <span class="field-hint">
          Cotation dont les cours sont récupérés ; sa devise est reprise des premiers cours.
          <template v-if="!priceUpdatesEnabled">La récupération des cours s'active dans les Paramètres.</template>
        </span>
      </label>
      <div v-if="candidates" class="candidates">
        <p v-if="candidates.length === 0" class="field-hint">Aucune cotation trouvée pour cet ISIN.</p>
        <button
          v-for="candidate in candidates"
          :key="candidate.symbol"
          type="button"
          class="candidate"
          @click="chooseSymbol(candidate)"
        >
          <strong>{{ candidate.symbol }}</strong>
          <span>{{ candidate.exchange }} · {{ candidate.name }}</span>
        </button>
      </div>

      <label class="field">
        Devise des cours
        <select v-model="form.currency" :disabled="security?.hasPrices">
          <option v-for="code in currencies" :key="code" :value="code">
            {{ currencyLabel(code) }}
          </option>
        </select>
        <span v-if="security?.hasPrices" class="field-hint">
          Le titre a des cours : sa devise ne peut plus être modifiée.
        </span>
      </label>
      <fieldset v-if="security" class="prices">
        <legend>Cours</legend>
        <ul v-if="prices.length > 0" class="price-list">
          <li v-for="price in prices.slice(0, PRICES_SHOWN)" :key="price.date">
            <span>{{ formatDate(price.date) }}</span>
            <strong>{{ formatAmount(price.priceCents, { currency: form.currency }) }}</strong>
            <span class="source">{{ price.source === "MANUAL" ? "saisi" : "Yahoo" }}</span>
            <button type="button" class="btn btn-link" :aria-label="`Supprimer le cours du ${formatDate(price.date)}`" @click="removePrice(price)">×</button>
          </li>
        </ul>
        <p v-else class="field-hint">Aucun cours.</p>
        <span class="inline">
          <input v-model="priceForm.date" type="date" aria-label="Date du cours" />
          <input v-model="priceForm.price" type="text" inputmode="decimal" placeholder="512,25" aria-label="Cours en euros" />
          <button type="button" class="btn btn-secondary" @click="addPrice">Saisir</button>
        </span>
        <span v-if="priceError" class="field-error">{{ priceError }}</span>
      </fieldset>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button
          v-if="security && security.transactionCount === 0"
          type="button"
          class="btn btn-link delete"
          :disabled="saving"
          @click="remove"
        >
          {{ confirmingDelete ? "Confirmer la suppression" : "Supprimer" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">
          {{ security ? "Enregistrer" : "Créer le titre" }}
        </button>
      </footer>
    </form>
  </BaseModal>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.inline {
  display: flex;
  align-items: center;
  gap: 8px;
}

.inline input {
  flex: 1;
  min-width: 0;
}

.candidates {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: -8px;
}

.candidate {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-nav);
  background: var(--color-card);
  text-align: left;
  font-size: 13px;
  cursor: pointer;
}

.candidate:hover {
  background: var(--color-page);
}

.prices {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 0;
  padding: 12px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
}

.prices legend {
  padding: 0 4px;
  font-size: 13px;
  font-weight: 700;
}

.price-list {
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 13px;
}

.price-list li {
  display: grid;
  grid-template-columns: 1fr auto 56px 24px;
  align-items: center;
  gap: 8px;
  padding: 2px 0;
}

.source {
  font-size: 11px;
  color: var(--color-text-muted);
}

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}

.delete {
  margin-right: auto;
  color: var(--cat-restaurants-fg);
}
</style>
