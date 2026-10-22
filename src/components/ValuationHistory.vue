<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { commands, type Account, type Valuation } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import {
  currencySymbol,
  formatAmount,
  formatAmountInput,
  formatDate,
  parseAmount,
  todayIso,
} from "../utils/format";

const props = defineProps<{
  /** Investment, asset or liability account. */
  account: Account;
}>();

const emit = defineEmits<{ changed: [] }>();

const valuations = ref<Valuation[]>([]);
const form = reactive({ date: todayIso(), value: "" });
/** Valuation being edited; `null` to add one. */
const editingId = ref<number | null>(null);
const confirmingDeleteId = ref<number | null>(null);
const saving = ref(false);
const error = ref("");

const isDebt = computed(() => props.account.type === "LIABILITY");
const valueLabel = computed(() => {
  const symbol = currencySymbol(props.account.currency);
  return isDebt.value ? `Capital restant dû (${symbol})` : `Valeur (${symbol})`;
});

async function load() {
  error.value = "";
  try {
    valuations.value = await unwrap(commands.listValuations(props.account.id));
  } catch (e) {
    error.value = errorMessage(e);
  }
}

function resetForm() {
  editingId.value = null;
  form.date = todayIso();
  form.value = "";
}

function edit(valuation: Valuation) {
  editingId.value = valuation.id;
  form.date = valuation.date;
  form.value = formatAmountInput(valuation.valueCents);
  confirmingDeleteId.value = null;
}

async function save() {
  error.value = "";
  const valueCents = parseAmount(form.value);
  if (valueCents === null || valueCents < 0) {
    error.value = "Montant invalide : un nombre positif, par exemple 41 820,00.";
    return;
  }
  if (form.date === "") {
    error.value = "La date est obligatoire.";
    return;
  }
  saving.value = true;
  try {
    const input = { date: form.date, valueCents };
    if (editingId.value === null) {
      await unwrap(commands.createValuation(props.account.id, input));
    } else {
      await unwrap(commands.updateValuation(editingId.value, input));
    }
    resetForm();
    await load();
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function remove(valuation: Valuation) {
  if (confirmingDeleteId.value !== valuation.id) {
    confirmingDeleteId.value = valuation.id;
    return;
  }
  error.value = "";
  try {
    await unwrap(commands.deleteValuation(valuation.id));
    confirmingDeleteId.value = null;
    if (editingId.value === valuation.id) resetForm();
    await load();
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  }
}

watch(
  () => props.account.id,
  () => {
    resetForm();
    load();
  },
  { immediate: true },
);
</script>

<template>
  <section class="history">
    <h3>{{ isDebt ? "Capital restant dû" : "Valorisations" }}</h3>

    <form class="entry" novalidate @submit.prevent="save">
      <label class="field">
        Date
        <input v-model="form.date" type="date" />
      </label>
      <label class="field">
        {{ valueLabel }}
        <input v-model="form.value" type="text" inputmode="decimal" placeholder="0,00" />
      </label>
      <button type="submit" class="btn btn-secondary" :disabled="saving">
        {{ editingId === null ? "Ajouter" : "Enregistrer" }}
      </button>
      <button v-if="editingId !== null" type="button" class="btn btn-link" @click="resetForm">Annuler</button>
    </form>

    <p v-if="error" class="error-banner">{{ error }}</p>

    <ul v-if="valuations.length > 0" class="list">
      <li v-for="valuation in valuations" :key="valuation.id" :class="{ editing: valuation.id === editingId }">
        <span class="date">{{ formatDate(valuation.date) }}</span>
        <strong class="value">{{
          formatAmount(isDebt ? -valuation.valueCents : valuation.valueCents, {
            currency: account.currency,
          })
        }}</strong>
        <button type="button" class="btn btn-link" @click="edit(valuation)">Modifier</button>
        <button type="button" class="btn btn-link delete" @click="remove(valuation)">
          {{ confirmingDeleteId === valuation.id ? "Confirmer" : "Supprimer" }}
        </button>
      </li>
    </ul>
    <p v-else class="empty">
      Aucune valorisation : saisis {{ isDebt ? "le capital restant dû" : "la valeur" }} à une date pour suivre
      ce compte.
    </p>
  </section>
</template>

<style scoped>
.history {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-top: 16px;
  border-top: 1px solid var(--color-border);
}

h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 800;
}

.entry {
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  align-items: end;
  gap: 10px;
}

.entry .btn {
  height: 42px;
}

.entry .btn-link {
  grid-column: 3;
  height: auto;
}

.list {
  display: flex;
  flex-direction: column;
  max-height: 220px;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
  font-size: 14px;
}

.list li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 4px;
  border-bottom: 1px solid var(--color-divider);
}

.list li.editing {
  background: var(--color-page);
}

.date {
  width: 96px;
  color: var(--color-text-muted);
}

.value {
  flex: 1;
  text-align: right;
  white-space: nowrap;
}

.delete {
  color: var(--cat-restaurants-fg);
}

.empty {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}
</style>
