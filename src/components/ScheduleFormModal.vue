<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import {
  commands,
  type Account,
  type Category,
  type Frequency,
  type ScheduledOperation,
  type ScheduledOperationInput,
} from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { currencySymbol, formatAmountInput, parseAmount, todayIso } from "../utils/format";
import { FREQUENCIES, WEEKDAYS } from "../utils/schedule";
import BaseModal from "./BaseModal.vue";
import CategorySelect from "./CategorySelect.vue";

type Direction = "debit" | "credit";

const props = defineProps<{
  open: boolean;
  /** Accounts with operations. */
  accounts: Account[];
  categories: Category[];
  /** Scheduled operation to edit; `null` to create one. */
  schedule: ScheduledOperation | null;
  /** Values of a new scheduled operation, e.g. from a detected series. */
  prefill?: ScheduledOperationInput | null;
}>();

const emit = defineEmits<{
  close: [];
  changed: [];
}>();

const form = reactive({
  label: "",
  accountId: null as number | null,
  direction: "debit" as Direction,
  amount: "",
  categoryId: null as number | null,
  frequency: "MONTHLY" as Frequency,
  day: 1,
  startDate: todayIso(),
  endDate: "",
});
const submitted = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const error = ref("");

const isEdit = computed(() => props.schedule !== null);
const isWeekly = computed(() => form.frequency === "WEEKLY");
const amountCents = computed(() => parseAmount(form.amount));
const labelError = computed(() => (form.label.trim() === "" ? "Le libellé est obligatoire." : ""));
const amountError = computed(() =>
  amountCents.value === null || amountCents.value <= 0 ? "Montant invalide, par exemple 1 120,00." : "",
);
const accountError = computed(() => (form.accountId === null ? "Choisis un compte." : ""));

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const schedule = props.schedule ?? props.prefill ?? null;
    form.label = schedule?.label ?? "";
    form.accountId = schedule?.accountId ?? props.accounts[0]?.id ?? null;
    form.direction = schedule && schedule.amountCents > 0 ? "credit" : "debit";
    form.amount = schedule ? formatAmountInput(Math.abs(schedule.amountCents)) : "";
    form.categoryId = schedule?.categoryId ?? null;
    form.frequency = schedule?.frequency ?? "MONTHLY";
    form.day = schedule?.dayOfPeriod ?? new Date().getDate();
    form.startDate = schedule?.startDate ?? todayIso();
    form.endDate = schedule?.endDate ?? "";
    submitted.value = false;
    confirmingDelete.value = false;
    error.value = "";
  },
  { immediate: true },
);

// A weekly schedule takes a weekday (1 to 7), the others a day of the month.
watch(isWeekly, (weekly) => {
  if (weekly && form.day > 7) form.day = 1;
});

async function save() {
  submitted.value = true;
  error.value = "";
  if (labelError.value || amountError.value || accountError.value) return;

  const cents = amountCents.value!;
  const input = {
    accountId: form.accountId!,
    label: form.label,
    amountCents: form.direction === "debit" ? -cents : cents,
    categoryId: form.categoryId,
    frequency: form.frequency,
    dayOfPeriod: form.day,
    startDate: form.startDate,
    endDate: form.endDate || null,
  };
  saving.value = true;
  try {
    if (props.schedule) {
      await unwrap(commands.updateScheduledOperation(props.schedule.id, input));
    } else {
      await unwrap(commands.createScheduledOperation(input));
    }
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!props.schedule) return;
  if (!confirmingDelete.value) {
    confirmingDelete.value = true;
    return;
  }
  saving.value = true;
  try {
    await unwrap(commands.deleteScheduledOperation(props.schedule.id));
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" :title="isEdit ? 'Modifier l\'échéance' : 'Nouvelle échéance'" @close="emit('close')">
    <form class="form" novalidate @submit.prevent="save">
      <label class="field">
        Libellé
        <input v-model="form.label" type="text" autocomplete="off" placeholder="Loyer" />
        <span v-if="submitted && labelError" class="field-error">{{ labelError }}</span>
      </label>

      <label class="field">
        Compte
        <select v-model="form.accountId">
          <option v-for="account in accounts" :key="account.id" :value="account.id">{{ account.name }}</option>
        </select>
        <span v-if="submitted && accountError" class="field-error">{{ accountError }}</span>
      </label>

      <div class="direction" role="radiogroup" aria-label="Sens">
        <label :class="{ active: form.direction === 'debit' }">
          <input v-model="form.direction" type="radio" value="debit" />
          Dépense
        </label>
        <label :class="{ active: form.direction === 'credit' }">
          <input v-model="form.direction" type="radio" value="credit" />
          Recette
        </label>
      </div>

      <div class="row">
        <label class="field">
          Montant ({{ currencySymbol(accounts.find((a) => a.id === form.accountId)?.currency ?? "EUR") }})
          <input v-model="form.amount" type="text" inputmode="decimal" placeholder="1 120,00" />
          <span v-if="submitted && amountError" class="field-error">{{ amountError }}</span>
        </label>
        <label class="field">
          Catégorie
          <CategorySelect v-model="form.categoryId" :categories="categories" />
        </label>
      </div>

      <div class="row">
        <label class="field">
          Périodicité
          <select v-model="form.frequency">
            <option v-for="f in FREQUENCIES" :key="f.value" :value="f.value">{{ f.label }}</option>
          </select>
        </label>
        <label v-if="isWeekly" class="field">
          Jour
          <select v-model.number="form.day">
            <option v-for="(name, index) in WEEKDAYS" :key="name" :value="index + 1">{{ name }}</option>
          </select>
        </label>
        <label v-else class="field">
          Jour du mois
          <input v-model.number="form.day" type="number" min="1" max="31" step="1" />
          <span class="field-hint">Le 31 tombe le dernier jour des mois plus courts.</span>
        </label>
      </div>

      <div class="row">
        <label class="field">
          À partir du
          <input v-model="form.startDate" type="date" />
          <span v-if="!isWeekly && form.frequency !== 'MONTHLY'" class="field-hint">
            Les échéances suivent le mois de cette date.
          </span>
        </label>
        <label class="field">
          Jusqu'au (facultatif)
          <input v-model="form.endDate" type="date" />
        </label>
      </div>

      <p class="field-hint muted">Les échéances s'affichent sans créer d'opérations : celles-ci viennent de l'import.</p>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button v-if="schedule" type="button" class="btn btn-link delete" :disabled="saving" @click="remove">
          {{ confirmingDelete ? "Confirmer la suppression" : "Supprimer" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">
          {{ isEdit ? "Enregistrer" : "Créer l'échéance" }}
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

.row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 12px;
}

.direction {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
}

.direction label {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 36px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 700;
  color: var(--color-text-muted);
  cursor: pointer;
}

.direction label.active {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.direction input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.muted {
  margin: 0;
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
