<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { commands, type CsvMapping, type SavedCsvMapping } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import {
  DATE_FORMATS,
  DECIMAL_SEPARATORS,
  DELIMITERS,
  ENCODINGS,
  defaultMapping,
  splitLine,
} from "../utils/csvMapping";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
  /** Mapping to edit; `null` to create one. */
  mapping: SavedCsvMapping | null;
  /** File shown as columns while defining the mapping. */
  path: string | null;
}>();

const emit = defineEmits<{
  close: [];
  saved: [mapping: SavedCsvMapping];
  deleted: [id: number];
}>();

/** Columns are typed 1-based in the form and stored 0-based. */
const form = reactive({
  name: "",
  delimiter: ";",
  encoding: "WINDOWS-1252" as CsvMapping["encoding"],
  dateFormat: "%d/%m/%Y",
  decimalSeparator: "," as CsvMapping["decimalSeparator"],
  skipLines: 1,
  amountKind: "debitCredit" as CsvMapping["amountColumns"]["kind"],
  columnDate: 1,
  columnLabel: 2,
  columnAmount: 3,
  columnDebit: 3,
  columnCredit: 4,
});
const head = ref<string[]>([]);
const submitted = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const error = ref("");

const isEdit = computed(() => props.mapping !== null);
const nameError = computed(() => (form.name.trim() === "" ? "Le nom est obligatoire." : ""));
const columnsError = computed(() => {
  const columns = [form.columnDate, form.columnLabel];
  if (form.amountKind === "signed") columns.push(form.columnAmount);
  else columns.push(form.columnDebit, form.columnCredit);
  return columns.every((c) => Number.isInteger(c) && c >= 1)
    ? ""
    : "Les numéros de colonne commencent à 1.";
});
const skipError = computed(() =>
  Number.isInteger(form.skipLines) && form.skipLines >= 0 ? "" : "Nombre de lignes invalide.",
);

/** Date formats offered, including a custom one of the edited mapping. */
const dateFormats = computed(() =>
  DATE_FORMATS.some((f) => f.value === form.dateFormat)
    ? DATE_FORMATS
    : [...DATE_FORMATS, { value: form.dateFormat, label: form.dateFormat }],
);

const rows = computed(() => head.value.map((line) => splitLine(line, form.delimiter)));
const columnCount = computed(() => Math.max(0, ...rows.value.map((r) => r.length)));

/** Role of each column (1-based) in the preview header. */
const roles = computed(() => {
  const roles = new Map<number, string>([
    [form.columnDate, "Date"],
    [form.columnLabel, "Libellé"],
  ]);
  if (form.amountKind === "signed") {
    roles.set(form.columnAmount, "Montant");
  } else {
    roles.set(form.columnDebit, "Débit");
    roles.set(form.columnCredit, "Crédit");
  }
  return roles;
});

function fill(mapping: CsvMapping, name: string) {
  form.name = name;
  form.delimiter = mapping.delimiter;
  form.encoding = mapping.encoding;
  form.dateFormat = mapping.dateFormat;
  form.decimalSeparator = mapping.decimalSeparator;
  form.skipLines = mapping.skipLines;
  form.columnDate = mapping.columnDate + 1;
  form.columnLabel = mapping.columnLabel + 1;
  form.amountKind = mapping.amountColumns.kind;
  if (mapping.amountColumns.kind === "signed") {
    form.columnAmount = mapping.amountColumns.amount + 1;
  } else {
    form.columnDebit = mapping.amountColumns.debit + 1;
    form.columnCredit = mapping.amountColumns.credit + 1;
  }
}

function toMapping(): CsvMapping {
  return {
    delimiter: form.delimiter,
    encoding: form.encoding,
    dateFormat: form.dateFormat,
    decimalSeparator: form.decimalSeparator,
    columnDate: form.columnDate - 1,
    columnLabel: form.columnLabel - 1,
    amountColumns:
      form.amountKind === "signed"
        ? { kind: "signed", amount: form.columnAmount - 1 }
        : { kind: "debitCredit", debit: form.columnDebit - 1, credit: form.columnCredit - 1 },
    skipLines: form.skipLines,
  };
}

async function loadHead() {
  if (!props.open || !props.path) {
    head.value = [];
    return;
  }
  try {
    head.value = await unwrap(commands.readFileHead(props.path, form.encoding));
  } catch (e) {
    error.value = errorMessage(e);
  }
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    fill(props.mapping?.mapping ?? defaultMapping(), props.mapping?.name ?? "");
    submitted.value = false;
    confirmingDelete.value = false;
    error.value = "";
    loadHead();
  },
  { immediate: true },
);
watch(() => form.encoding, loadHead);

async function save() {
  submitted.value = true;
  error.value = "";
  if (nameError.value || columnsError.value || skipError.value) return;

  saving.value = true;
  try {
    const fields = { name: form.name, mapping: toMapping() };
    const saved = props.mapping
      ? await unwrap(commands.updateCsvMapping(props.mapping.id, fields))
      : await unwrap(commands.createCsvMapping(fields));
    emit("saved", saved);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!props.mapping) return;
  if (!confirmingDelete.value) {
    confirmingDelete.value = true;
    return;
  }
  saving.value = true;
  error.value = "";
  try {
    await unwrap(commands.deleteCsvMapping(props.mapping.id));
    emit("deleted", props.mapping.id);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal
    :open="open"
    :title="isEdit ? 'Modifier le format CSV' : 'Nouveau format CSV'"
    wide
    @close="emit('close')"
  >
    <form class="form" novalidate @submit.prevent="save">
      <label class="field">
        Nom
        <input v-model="form.name" type="text" autocomplete="off" placeholder="Ma banque – export CSV" />
        <span v-if="submitted && nameError" class="field-error">{{ nameError }}</span>
      </label>

      <div class="row">
        <label class="field">
          Séparateur de colonnes
          <select v-model="form.delimiter">
            <option v-for="d in DELIMITERS" :key="d.value" :value="d.value">{{ d.label }}</option>
          </select>
        </label>
        <label class="field">
          Encodage
          <select v-model="form.encoding">
            <option v-for="e in ENCODINGS" :key="e.value" :value="e.value">{{ e.label }}</option>
          </select>
        </label>
        <label class="field">
          Lignes à ignorer en tête
          <input v-model.number="form.skipLines" type="number" min="0" step="1" />
          <span class="field-hint">Ligne des titres de colonnes comprise.</span>
          <span v-if="submitted && skipError" class="field-error">{{ skipError }}</span>
        </label>
      </div>

      <div class="row">
        <label class="field">
          Format des dates
          <select v-model="form.dateFormat">
            <option v-for="f in dateFormats" :key="f.value" :value="f.value">{{ f.label }}</option>
          </select>
        </label>
        <label class="field">
          Séparateur décimal
          <select v-model="form.decimalSeparator">
            <option v-for="s in DECIMAL_SEPARATORS" :key="s.value" :value="s.value">{{ s.label }}</option>
          </select>
        </label>
        <label class="field">
          Montants
          <select v-model="form.amountKind">
            <option value="debitCredit">Colonnes débit et crédit</option>
            <option value="signed">Une colonne de montant signé</option>
          </select>
        </label>
      </div>

      <fieldset class="columns">
        <legend>Numéros de colonne</legend>
        <label class="field">
          Date
          <input v-model.number="form.columnDate" type="number" min="1" step="1" />
        </label>
        <label class="field">
          Libellé
          <input v-model.number="form.columnLabel" type="number" min="1" step="1" />
        </label>
        <label v-if="form.amountKind === 'signed'" class="field">
          Montant
          <input v-model.number="form.columnAmount" type="number" min="1" step="1" />
        </label>
        <template v-else>
          <label class="field">
            Débit
            <input v-model.number="form.columnDebit" type="number" min="1" step="1" />
          </label>
          <label class="field">
            Crédit
            <input v-model.number="form.columnCredit" type="number" min="1" step="1" />
          </label>
        </template>
      </fieldset>
      <span v-if="submitted && columnsError" class="field-error">{{ columnsError }}</span>

      <div v-if="rows.length > 0" class="file">
        <div class="file-title">Début du fichier</div>
        <div class="file-table">
          <table>
            <thead>
              <tr>
                <th v-for="column in columnCount" :key="column" :class="{ used: roles.has(column) }">
                  {{ column }}<span v-if="roles.has(column)"> · {{ roles.get(column) }}</span>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, index) in rows" :key="index" :class="{ skipped: index < form.skipLines }">
                <td v-for="column in columnCount" :key="column" :class="{ used: roles.has(column) }">
                  {{ row[column - 1] ?? "" }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <span class="field-hint">Les lignes grisées sont ignorées.</span>
      </div>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button v-if="mapping" type="button" class="btn btn-link delete" :disabled="saving" @click="remove">
          {{ confirmingDelete ? "Confirmer la suppression" : "Supprimer" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">
          {{ isEdit ? "Enregistrer" : "Créer le format" }}
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
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 12px;
}

.columns {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(110px, 1fr));
  gap: 12px;
  margin: 0;
  padding: 0;
  border: 0;
}

.columns legend {
  padding: 0;
  margin-bottom: 8px;
  font-size: 13px;
  font-weight: 700;
}

.file {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--color-text-muted);
}

.file-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--color-text);
}

.file-table {
  max-height: 220px;
  overflow: auto;
  border: 1px solid var(--color-border);
  border-radius: 10px;
}

table {
  border-collapse: collapse;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  color: var(--color-text);
}

th,
td {
  padding: 6px 10px;
  border-bottom: 1px solid var(--color-divider);
  text-align: left;
  white-space: nowrap;
}

th {
  position: sticky;
  top: 0;
  background: var(--color-page);
  font-family: var(--font-family);
  font-weight: 700;
  color: var(--color-text-muted);
}

th.used {
  color: var(--cat-housing-fg);
}

td.used {
  background: var(--cat-housing-bg);
}

tr.skipped td {
  background: transparent;
  color: var(--color-text-muted);
  opacity: 0.6;
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
