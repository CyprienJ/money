<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import {
  commands,
  type Account,
  type InvestmentTransaction,
  type Security,
  type TransactionKind,
} from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import {
  currencySymbol,
  formatAmount,
  formatAmountInput,
  formatDate,
  parseAmount,
  todayIso,
} from "../utils/format";
import { TRANSACTION_KINDS, formatQuantityInput, parseQuantity, unitPriceCents } from "../utils/portfolio";
import BaseModal from "./BaseModal.vue";
import SourceDocumentLink from "./SourceDocumentLink.vue";

const props = defineProps<{
  open: boolean;
  account: Account;
  securities: Security[];
  /** Transaction to edit; `null` to record one. */
  transaction: InvestmentTransaction | null;
}>();

const emit = defineEmits<{
  close: [];
  changed: [];
  /** Asks to create a security, to choose it afterwards. */
  newSecurity: [];
}>();

const form = reactive({
  kind: "BUY" as TransactionKind,
  securityId: null as number | null,
  date: todayIso(),
  quantity: "",
  amount: "",
  fees: "",
  note: "",
});
const submitted = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const error = ref("");

const isDividend = computed(() => form.kind === "DIVIDEND");
const quantityMicro = computed(() => parseQuantity(form.quantity));
const amountCents = computed(() => parseAmount(form.amount));
const feesCents = computed(() => (form.fees.trim() === "" ? 0 : parseAmount(form.fees)));

const securityError = computed(() => (form.securityId === null ? "Choisis un titre." : ""));
const quantityError = computed(() =>
  !isDividend.value && quantityMicro.value === null ? "Quantité invalide, par exemple 12,5 (6 décimales au plus)." : "",
);
const amountError = computed(() =>
  amountCents.value === null || amountCents.value <= 0 ? "Montant invalide, par exemple 1 250,00." : "",
);
const feesError = computed(() =>
  feesCents.value === null || feesCents.value < 0 ? "Frais invalides, par exemple 1,99." : "",
);
const dateError = computed(() =>
  form.date < props.account.openingDate ? `Le compte a été ouvert le ${formatDate(props.account.openingDate)}.` : "",
);

/** "Soit 120,00 € par part" under the amount of a buy or a sell. */
const unitPriceText = computed(() => {
  if (isDividend.value || quantityMicro.value === null || amountCents.value === null || amountCents.value <= 0) {
    return "";
  }
  const unitPrice = unitPriceCents(amountCents.value, quantityMicro.value);
  return `Soit ${formatAmount(unitPrice, { currency: props.account.currency })} par part`;
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const transaction = props.transaction;
    form.kind = transaction?.kind ?? "BUY";
    form.securityId = transaction?.securityId ?? props.securities[0]?.id ?? null;
    form.date = transaction?.date ?? todayIso();
    form.quantity = transaction && transaction.quantityMicro > 0 ? formatQuantityInput(transaction.quantityMicro) : "";
    form.amount = transaction ? formatAmountInput(transaction.amountCents) : "";
    form.fees = transaction && transaction.feesCents > 0 ? formatAmountInput(transaction.feesCents) : "";
    form.note = transaction?.note ?? "";
    submitted.value = false;
    confirmingDelete.value = false;
    error.value = "";
  },
  { immediate: true },
);

// A security created from this form is chosen at once.
watch(
  () => props.securities.length,
  (count, previous) => {
    if (props.open && previous !== undefined && count > previous) {
      const newest = props.securities.reduce((a, b) => (b.id > a.id ? b : a));
      form.securityId = newest.id;
    }
  },
);

async function save() {
  submitted.value = true;
  error.value = "";
  if (securityError.value || quantityError.value || amountError.value || feesError.value || dateError.value) return;
  const input = {
    securityId: form.securityId!,
    date: form.date,
    kind: form.kind,
    quantityMicro: isDividend.value ? 0 : quantityMicro.value!,
    amountCents: amountCents.value!,
    feesCents: feesCents.value!,
    note: form.note.trim() || null,
  };
  saving.value = true;
  try {
    if (props.transaction) {
      await unwrap(commands.updateInvestmentTransaction(props.transaction.id, input));
    } else {
      await unwrap(commands.createInvestmentTransaction(props.account.id, input));
    }
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!props.transaction) return;
  if (!confirmingDelete.value) {
    confirmingDelete.value = true;
    return;
  }
  saving.value = true;
  try {
    await unwrap(commands.deleteInvestmentTransaction(props.transaction.id));
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" :title="transaction ? 'Modifier le mouvement' : 'Nouveau mouvement'" @close="emit('close')">
    <form class="form" novalidate @submit.prevent="save">
      <div class="kinds" role="radiogroup" aria-label="Type de mouvement">
        <label v-for="k in TRANSACTION_KINDS" :key="k.value" :class="{ active: form.kind === k.value }">
          <input v-model="form.kind" type="radio" :value="k.value" />
          {{ k.label }}
        </label>
      </div>

      <label class="field">
        Titre
        <span class="security">
          <select v-model="form.securityId">
            <option v-for="security in securities" :key="security.id" :value="security.id">
              {{ security.name }}{{ security.isin ? ` (${security.isin})` : "" }}
            </option>
          </select>
          <button type="button" class="btn btn-link" @click="emit('newSecurity')">+ Nouveau titre</button>
        </span>
        <span v-if="submitted && securityError" class="field-error">{{ securityError }}</span>
      </label>

      <div class="row">
        <label class="field">
          Date
          <input v-model="form.date" type="date" />
          <span v-if="submitted && dateError" class="field-error">{{ dateError }}</span>
        </label>
        <label v-if="!isDividend" class="field">
          Quantité
          <input v-model="form.quantity" type="text" inputmode="decimal" placeholder="12,5" />
          <span v-if="submitted && quantityError" class="field-error">{{ quantityError }}</span>
        </label>
      </div>

      <div class="row">
        <label class="field">
          {{ isDividend ? "Montant reçu" : "Montant hors frais" }} ({{ currencySymbol(account.currency) }})
          <input v-model="form.amount" type="text" inputmode="decimal" placeholder="1 250,00" />
          <span v-if="submitted && amountError" class="field-error">{{ amountError }}</span>
          <span v-else-if="unitPriceText" class="field-hint">{{ unitPriceText }}</span>
        </label>
        <label class="field">
          {{ isDividend ? "Retenue ou frais" : "Frais" }} ({{ currencySymbol(account.currency) }})
          <input v-model="form.fees" type="text" inputmode="decimal" placeholder="0,00" />
          <span v-if="submitted && feesError" class="field-error">{{ feesError }}</span>
          <span v-else-if="form.kind === 'BUY'" class="field-hint">Ajoutés au prix de revient.</span>
        </label>
      </div>

      <label class="field">
        Note (facultative)
        <input v-model="form.note" type="text" autocomplete="off" />
      </label>

      <SourceDocumentLink
        v-if="transaction?.hasSourceDocument && transaction.importBatchId !== null"
        :import-batch-id="transaction.importBatchId"
        :page="transaction.sourcePage"
        :line="null"
      />

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button v-if="transaction" type="button" class="btn btn-link delete" :disabled="saving" @click="remove">
          {{ confirmingDelete ? "Confirmer la suppression" : "Supprimer" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">
          {{ transaction ? "Enregistrer" : "Ajouter le mouvement" }}
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

.kinds {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 4px;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
}

.kinds label {
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

.kinds label.active {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.kinds input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.security {
  display: flex;
  align-items: center;
  gap: 12px;
}

.security select {
  flex: 1;
  min-width: 0;
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
