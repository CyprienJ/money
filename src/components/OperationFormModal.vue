<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { commands, type Account, type Category, type Operation, type Suggestion } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { currencySymbol, formatAmount, formatAmountInput, formatDate, parseAmount, todayIso } from "../utils/format";
import AttachmentList from "./AttachmentList.vue";
import BaseModal from "./BaseModal.vue";
import SourceDocumentLink from "./SourceDocumentLink.vue";
import TransferLink from "./TransferLink.vue";
import CategorySelect from "./CategorySelect.vue";
import TagInput from "./TagInput.vue";

const props = defineProps<{
  open: boolean;
  account: Account;
  categories: Category[];
  /** Operation to edit; `null` for a new manual entry. */
  operation: Operation | null;
  /** Category suggested for an operation to categorize, preselected until the form is saved. */
  suggestion?: Suggestion | null;
}>();

const emit = defineEmits<{
  close: [];
  /** Emitted after a create, update or delete. */
  changed: [];
  /** Emitted after a change made at once (attachment, transfer link), the form staying open. */
  refresh: [];
  /** Asks for a rule from the label of the operation, with the category in the form. */
  createRule: [operation: Operation, categoryId: number | null];
}>();

type Direction = "debit" | "credit";

const form = reactive({
  direction: "debit" as Direction,
  amount: "",
  date: todayIso(),
  valueDate: "",
  label: "",
  categoryId: null as number | null,
  note: "",
  tags: [] as string[],
  /** Parts of a split, unsigned amounts typed by the user; empty for a single category. */
  parts: [] as { categoryId: number | null; amount: string }[],
});
/** Names of the existing tags, for the suggestions. */
const tagNames = ref<string[]>([]);
const submitted = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const error = ref("");

const isEdit = computed(() => props.operation !== null);
const isImported = computed(() => props.operation?.importBatchId != null);
/** Attachments of the edited operation, kept up to date by the attachment list. */
const attachmentCount = ref(0);

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
const isSplit = computed(() => form.parts.length > 0);
/** A part cannot be an internal transfer. */
const splitCategories = computed(() => props.categories.filter((c) => !c.isInternalTransfer));
/** Unsigned amounts of the parts, in cents; `null` for an invalid one. */
const partCents = computed(() =>
  form.parts.map((part) => {
    const cents = parseAmount(part.amount);
    return cents === null || cents <= 0 ? null : cents;
  }),
);
/** Amount left to split, in cents; `null` while the amount or a part is invalid. */
const remainingCents = computed(() => {
  if (amountCents.value === null || partCents.value.some((c) => c === null)) return null;
  return amountCents.value - partCents.value.reduce<number>((sum, c) => sum + c!, 0);
});
const splitError = computed(() => {
  if (!isSplit.value) return "";
  if (form.parts.some((part) => part.categoryId === null)) return "Chaque part a une catégorie.";
  if (partCents.value.some((c) => c === null)) return "Chaque part a un montant positif, par exemple 12,50.";
  if (remainingCents.value !== 0) return "La somme des parts doit égaler le montant.";
  return "";
});

function startSplit() {
  const total = amountCents.value;
  form.parts = [
    { categoryId: form.categoryId, amount: total ? formatAmountInput(total) : "" },
    { categoryId: null, amount: "" },
  ];
  // The internal transfer category is not offered to a part.
  if (props.categories.find((c) => c.id === form.categoryId)?.isInternalTransfer) {
    form.parts[0].categoryId = null;
  }
}

function addPart() {
  const left = remainingCents.value;
  form.parts.push({ categoryId: null, amount: left !== null && left > 0 ? formatAmountInput(left) : "" });
}

function removePart(index: number) {
  form.parts.splice(index, 1);
  // A single part left is a single category.
  if (form.parts.length === 1) cancelSplit();
}

function cancelSplit() {
  form.categoryId = form.parts.find((part) => part.categoryId !== null)?.categoryId ?? null;
  form.parts = [];
}

/** Parts sent to the backend, with the sign of the operation. */
function signedParts() {
  const sign = form.direction === "debit" ? -1 : 1;
  return form.parts.map((part, index) => ({
    categoryId: part.categoryId!,
    amountCents: sign * partCents.value[index]!,
  }));
}

const dateError = computed(() => {
  if (form.date === "") return "La date est obligatoire.";
  if (props.account.openingDate !== null && form.date < props.account.openingDate) {
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
    form.categoryId = operation?.categoryId ?? props.suggestion?.categoryId ?? null;
    form.parts = (operation?.splits ?? []).map((part) => ({
      categoryId: part.categoryId,
      amount: formatAmountInput(Math.abs(part.amountCents)),
    }));
    form.note = operation?.note ?? "";
    form.tags = [...(operation?.tags ?? [])];
    attachmentCount.value = operation?.attachmentCount ?? 0;
    submitted.value = false;
    confirmingDelete.value = false;
    error.value = "";
    loadTagNames();
  },
  { immediate: true },
);

/** A linked operation is in the internal transfer category: the form follows. */
function onTransferChanged(linked: boolean) {
  if (linked) {
    // Linking replaces a split by the internal transfer category.
    form.parts = [];
    form.categoryId = props.categories.find((c) => c.isInternalTransfer)?.id ?? form.categoryId;
  }
  emit("refresh");
}

async function loadTagNames() {
  try {
    tagNames.value = (await unwrap(commands.listTags())).map((tag) => tag.name);
  } catch {
    // Suggestions only: tags can still be typed.
    tagNames.value = [];
  }
}

/** Note and tags, editable on every operation. */
function annotations() {
  return { note: form.note.trim() || null, tags: form.tags };
}

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
  if (splitError.value) return;
  const category = isSplit.value
    ? { categoryId: null, splits: signedParts() }
    : { categoryId: form.categoryId, splits: [] };

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
          ...category,
          ...annotations(),
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
    ...category,
    ...annotations(),
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
          Montant ({{ currencySymbol(account.currency) }})
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
        <div v-if="isSplit" class="field">
          <span>Catégorie</span>
          <span class="split-summary">Ventilée en {{ form.parts.length }} parts</span>
          <button type="button" class="btn btn-link split-toggle" @click="cancelSplit">
            Une seule catégorie
          </button>
        </div>
        <label v-else class="field">
          Catégorie
          <CategorySelect v-model="form.categoryId" :categories="categories" />
          <span
            v-if="suggestion && operation?.categoryId === null && form.categoryId === suggestion.categoryId"
            class="field-hint"
          >
            Suggestion ({{ suggestion.confidencePercent }} %), enregistrée seulement si tu valides
          </span>
          <button type="button" class="btn btn-link split-toggle" @click="startSplit">
            Ventiler sur plusieurs catégories
          </button>
        </label>
        <label class="field">
          Date de valeur
          <input v-model="form.valueDate" type="date" />
          <span class="field-hint">Facultative</span>
        </label>
      </div>

      <fieldset v-if="isSplit" class="field parts">
        <legend>Parts</legend>
        <div v-for="(part, index) in form.parts" :key="index" class="part">
          <CategorySelect
            v-model="part.categoryId"
            :categories="splitCategories"
            :aria-label="`Catégorie de la part ${index + 1}`"
          />
          <input
            v-model="part.amount"
            type="text"
            inputmode="decimal"
            placeholder="12,50"
            :aria-label="`Montant de la part ${index + 1} (${currencySymbol(account.currency)})`"
          />
          <button
            type="button"
            class="btn btn-link remove-part"
            :aria-label="`Retirer la part ${index + 1}`"
            @click="removePart(index)"
          >
            ✕
          </button>
        </div>
        <div class="parts-footer">
          <button type="button" class="btn btn-link" @click="addPart">Ajouter une part</button>
          <span v-if="remainingCents !== null && remainingCents !== 0" class="remaining" aria-live="polite">
            {{ remainingCents > 0 ? "Reste à répartir" : "Dépassement" }} :
            {{ formatAmount(Math.abs(remainingCents), { currency: account.currency }) }}
          </span>
        </div>
        <span v-if="submitted && splitError" class="field-error">{{ splitError }}</span>
      </fieldset>

      <div class="field">
        <span aria-hidden="true">Tags</span>
        <TagInput v-model="form.tags" :suggestions="tagNames" label="Tags" />
        <span class="field-hint">Entrée ou virgule pour ajouter un tag</span>
      </div>

      <label class="field">
        Note
        <textarea v-model="form.note" rows="2" placeholder="Facultative"></textarea>
      </label>

      <SourceDocumentLink
        v-if="operation?.hasSourceDocument && operation.importBatchId !== null"
        :import-batch-id="operation.importBatchId"
        :page="operation.sourcePage"
        :line="operation.sourceLine"
      />

      <TransferLink v-if="operation" :operation-id="operation.id" @changed="onTransferChanged" />

      <AttachmentList
        v-if="operation"
        :operation-id="operation.id"
        @changed="(count) => { attachmentCount = count; emit('refresh'); }"
      />
      <p v-else class="field-hint">Les pièces jointes s'ajoutent une fois l'opération enregistrée.</p>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button
          v-if="operation"
          type="button"
          class="btn btn-link"
          title="Classer automatiquement les opérations au libellé semblable"
          @click="emit('createRule', operation, form.categoryId)"
        >
          Créer une règle
        </button>
        <button
          v-if="operation && !isImported"
          type="button"
          class="btn btn-link delete"
          :disabled="saving"
          @click="remove"
        >
          {{
            !confirmingDelete
              ? "Supprimer"
              : attachmentCount > 0
                ? `Confirmer : supprimer avec ${attachmentCount} pièce${attachmentCount > 1 ? "s" : ""} jointe${attachmentCount > 1 ? "s" : ""}`
                : "Confirmer la suppression"
          }}
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

textarea {
  padding: 10px 12px;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: var(--color-card);
  color: var(--color-text);
  font-size: 14px;
  font-weight: 500;
  resize: vertical;
}

textarea:focus {
  outline: 2px solid var(--color-accent);
  outline-offset: -1px;
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

.split-toggle {
  align-self: flex-start;
  font-size: 12px;
}

.split-summary {
  display: flex;
  align-items: center;
  height: 42px;
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text);
}

.parts {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 0;
  padding: 12px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
}

.parts legend {
  padding: 0 4px;
  font-size: 13px;
  font-weight: 600;
}

.part {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 110px auto;
  gap: 8px;
  align-items: center;
}

.remove-part {
  padding: 0 6px;
  color: var(--color-text-muted);
  font-size: 14px;
}

.parts-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}

.remaining {
  font-size: 13px;
  font-weight: 600;
  color: var(--cat-restaurants-fg);
  font-variant-numeric: tabular-nums;
}

.delete {
  margin-right: auto;
  color: var(--cat-restaurants-fg);
}
</style>
