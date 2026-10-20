<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { commands, type Account, type Category, type Operation } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmountInput, formatDate, parseAmount, todayIso } from "../utils/format";
import BaseModal from "./BaseModal.vue";
import CategorySelect from "./CategorySelect.vue";

const props = defineProps<{
  open: boolean;
  account: Account;
  categories: Category[];
  /** Operation to edit; `null` for a new manual entry. */
  operation: Operation | null;
}>();

const emit = defineEmits<{
  close: [];
  /** Emitted after a create, update or delete. */
  changed: [];
}>();

type Direction = "debit" | "credit";

const form = reactive({
  direction: "debit" as Direction,
  amount: "",
  date: todayIso(),
  valueDate: "",
  label: "",
  categoryId: null as number | null,
});
const submitted = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const error = ref("");

const isEdit = computed(() => props.operation !== null);
const isImported = computed(() => props.operation?.importBatchId != null);

/** Unsigned amount typed by the user, in cents. */
const amountCents = computed(() => {
  const cents = parseAmount(form.amount);
  return cents === null ? null : Math.abs(cents);
});
const labelError = computed(() => (form.label.trim() === "" ? "Le libellé est obligatoire." : ""));
const amountError = computed(() =>
  amountCents.value === null || amountCents.value === 0
    ? "Montant invalide, par exemple 64,28."
    : "",
);
const dateError = computed(() => {
  if (form.date === "") return "La date est obligatoire.";
  if (form.date < props.account.openingDate) {
    return `Le compte a été ouvert le ${formatDate(props.account.openingDate)}.`;
  }
  return "";
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const operation = props.operation;
    form.direction = operation && operation.amountCents > 0 ? "credit" : "debit";
    form.amount = operation ? formatAmountInput(Math.abs(operation.amountCents)) : "";
    form.date = operation?.date ?? todayIso();
    form.valueDate = operation?.valueDate ?? "";
    form.label = operation?.rawLabel ?? "";
    form.categoryId = operation?.categoryId ?? null;
    submitted.value = false;
    confirmingDelete.value = false;
    error.value = "";
  },
  { immediate: true },
);

async function run(action: () => Promise<unknown>) {
  saving.value = true;
  error.value = "";
  try {
    await action();
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function save() {
  submitted.value = true;
  if (!isImported.value && (labelError.value || amountError.value || dateError.value)) return;

  const operation = props.operation;
  if (operation && isImported.value) {
    // Bank fields are sent back unchanged.
    await run(() =>
      unwrap(
        commands.updateOperation(operation.id, {
          date: operation.date,
          amountCents: operation.amountCents,
          label: operation.rawLabel,
          valueDate: form.valueDate || null,
          categoryId: form.categoryId,
        }),
      ),
    );
    return;
  }

  const signed = form.direction === "debit" ? -amountCents.value! : amountCents.value!;
  const fields = {
    date: form.date,
    valueDate: form.valueDate || null,
    amountCents: signed,
    label: form.label,
    categoryId: form.categoryId,
  };
  await run(() =>
    operation
      ? unwrap(commands.updateOperation(operation.id, fields))
      : unwrap(commands.createOperation({ ...fields, accountId: props.account.id })),
  );
}

async function remove() {
  const operation = props.operation;
  if (!operation) return;
  if (!confirmingDelete.value) {
    confirmingDelete.value = true;
    return;
  }
  await run(() => unwrap(commands.deleteOperation(operation.id)));
}
</script>

<template>
  <BaseModal
    :open="open"
    :title="isEdit ? 'Modifier l\'opération' : 'Nouvelle opération'"
    @close="emit('close')"
  >
    <form class="form" novalidate @submit.prevent="save">
      <p class="context">
        {{ account.name }}
        <template v-if="isImported"> · opération importée</template>
      </p>
      <p v-if="isImported" class="field-hint notice">
        La date, le montant et le libellé viennent de la banque et ne sont pas modifiables.
      </p>

      <div class="direction" role="radiogroup" aria-label="Sens">
        <label :class="{ active: form.direction === 'debit' }">
          <input v-model="form.direction" type="radio" value="debit" :disabled="isImported" />
          Dépense
        </label>
        <label :class="{ active: form.direction === 'credit' }">
          <input v-model="form.direction" type="radio" value="credit" :disabled="isImported" />
          Recette
        </label>
      </div>

      <div class="row">
        <label class="field">
          Montant (€)
          <input
            v-model="form.amount"
            type="text"
            inputmode="decimal"
            placeholder="64,28"
            :disabled="isImported"
            :autofocus="!isEdit"
          />
          <span v-if="submitted && !isImported && amountError" class="field-error">
            {{ amountError }}
          </span>
        </label>
        <label class="field">
          Date
          <input v-model="form.date" type="date" :disabled="isImported" />
          <span v-if="submitted && !isImported && dateError" class="field-error">
            {{ dateError }}
          </span>
        </label>
      </div>

      <label class="field">
        Libellé
        <input
          v-model="form.label"
          type="text"
          autocomplete="off"
          placeholder="Boulangerie Martin"
          :disabled="isImported"
        />
        <span v-if="submitted && !isImported && labelError" class="field-error">
          {{ labelError }}
        </span>
      </label>

      <div class="row">
        <label class="field">
          Catégorie
          <CategorySelect v-model="form.categoryId" :categories="categories" />
        </label>
        <label class="field">
          Date de valeur
          <input v-model="form.valueDate" type="date" />
          <span class="field-hint">Facultative</span>
        </label>
      </div>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button
          v-if="operation && !isImported"
          type="button"
          class="btn btn-link delete"
          :disabled="saving"
          @click="remove"
        >
          {{ confirmingDelete ? "Confirmer la suppression" : "Supprimer" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">
          {{ isEdit ? "Enregistrer" : "Ajouter" }}
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

.context {
  margin: -10px 0 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-muted);
}

.notice {
  margin: -6px 0 0;
  padding: 10px 12px;
  border-radius: var(--radius-nav);
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
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

.row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 12px;
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
