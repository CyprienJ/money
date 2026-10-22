<script setup lang="ts">
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  commands,
  type AccountSummary,
  type Category,
  type ImportFormat,
  type ImportPreview,
  type MappingSuggestion,
  type SavedCsvMapping,
  type SupportedBank,
} from "../bindings";
import CategoryPicker from "../components/CategoryPicker.vue";
import CsvMappingModal from "../components/CsvMappingModal.vue";
import { ACCOUNT_TYPES } from "../utils/accounts";
import { errorMessage, unwrap } from "../utils/commands";
import { displayLabel, formatAmount, formatDate, localDateOf, operationCount } from "../utils/format";
import {
  ambiguityText,
  balanceGap,
  categoryCounts,
  defaultSelection,
  groupPreview,
  lineAmount,
  lineCategory,
  pendingSuggestions,
  periodText,
} from "../utils/importPreview";

type Step = "file" | "preview" | "importing";

const route = useRoute();
const router = useRouter();

const step = ref<Step>("file");
const accounts = ref<AccountSummary[]>([]);
const categories = ref<Category[]>([]);
const mappings = ref<SavedCsvMapping[]>([]);
const suggestions = ref<MappingSuggestion[]>([]);
const path = ref<string | null>(null);
const accountId = ref<number | null>(null);
const mappingId = ref<number | null>(null);
/** Banks whose PDF statements are supported. */
const banks = ref<SupportedBank[]>([]);
/** Bank recognized from the chosen PDF; `null` when not recognized. */
const detectedBank = ref<SupportedBank | null>(null);
const preview = ref<ImportPreview | null>(null);
const selected = ref(new Set<number>());
/** Categories chosen by the user in the preview, by line index. */
const chosen = ref(new Map<number, number | null>());
/** Last category chosen for each normalized label, applied to the other lines of the label. */
const spread = ref(new Map<string, number | null>());
const busy = ref(false);
const error = ref("");

const mappingModalOpen = ref(false);
const editedMapping = ref<SavedCsvMapping | null>(null);

const steps: { id: Step; label: string }[] = [
  { id: "file", label: "Fichier" },
  { id: "preview", label: "Vérification et aperçu" },
  { id: "importing", label: "Import" },
];
const stepIndex = computed(() => steps.findIndex((s) => s.id === step.value));

const fileName = computed(() => path.value?.split(/[\\/]/).pop() ?? "");
const account = computed(() => accounts.value.find((s) => s.account.id === accountId.value)?.account);
/** Currency of the amounts of the statement: the one of the account. */
const currency = computed(() => account.value?.currency);
const mapping = computed(() => mappings.value.find((m) => m.id === mappingId.value));
const suggestedLines = computed(
  () => new Map(suggestions.value.map((s) => [s.mapping.id, s.lineCount])),
);
const isPdf = computed(() => /\.pdf$/i.test(path.value ?? ""));
const banksText = computed(() => banks.value.map((b) => b.name).join(", "));
/** How the chosen file is read: by the parser of its bank, or with a saved CSV format. */
const format = computed<ImportFormat | null>(() => {
  if (isPdf.value) return detectedBank.value ? { kind: "pdf" } : null;
  return mappingId.value === null ? null : { kind: "csv", mappingId: mappingId.value };
});
const canPreview = computed(
  () => path.value !== null && accountId.value !== null && format.value !== null && !busy.value,
);

const groups = computed(() => (preview.value ? groupPreview(preview.value) : null));
const ambiguousCount = computed(
  () => groups.value?.candidates.filter((c) => c.line.duplicate.status === "ambiguous").length ?? 0,
);
const categoriesById = computed(() => new Map(categories.value.map((c) => [c.id, c])));
const counts = computed(() =>
  preview.value
    ? categoryCounts(preview.value, selected.value, chosen.value, spread.value)
    : { automatic: 0, toCategorize: 0 },
);
/** No balance mismatch: balances checked, or not available (CSV). */
const balanceOk = computed(() => preview.value?.balanceCheck.status !== "MISMATCH");
const accountNames = computed(() => new Map(accounts.value.map((s) => [s.account.id, s.account.name])));

async function load() {
  try {
    const summaries = await unwrap(commands.listAccountSummaries(false));
    accounts.value = summaries.filter((s) => ACCOUNT_TYPES[s.account.type].hasOperations);
    mappings.value = await unwrap(commands.listCsvMappings());
    categories.value = await unwrap(commands.listCategories());
    banks.value = await commands.supportedBanks();
    const requested = Number(route.query.account);
    accountId.value =
      accounts.value.find((s) => s.account.id === requested)?.account.id ??
      accounts.value[0]?.account.id ??
      null;
  } catch (e) {
    error.value = errorMessage(e);
  }
}

async function chooseFile() {
  error.value = "";
  const chosen = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "Relevé PDF ou CSV", extensions: ["pdf", "PDF", "csv", "CSV", "txt"] }],
  });
  if (typeof chosen !== "string") return;
  path.value = chosen;
  detectedBank.value = null;
  if (isPdf.value) await detectBank();
  else await suggest();
}

/** Recognizes the bank of a PDF statement from its content. */
async function detectBank() {
  if (!path.value) return;
  busy.value = true;
  try {
    detectedBank.value = await unwrap(commands.detectPdfStatement(path.value));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}

/** Picks the saved format reading the most lines of the file. */
async function suggest() {
  if (!path.value) return;
  try {
    suggestions.value = await unwrap(commands.suggestCsvMappings(path.value));
    if (suggestions.value.length > 0 && !suggestedLines.value.has(mappingId.value ?? -1)) {
      mappingId.value = suggestions.value[0].mapping.id;
    }
  } catch (e) {
    error.value = errorMessage(e);
  }
}

function mappingLabel(saved: SavedCsvMapping): string {
  return suggestedLines.value.has(saved.id) ? `${saved.name} ✓` : saved.name;
}

/** Lines of the file read by the selected format, `undefined` when it cannot read it. */
const selectedLines = computed(() =>
  mappingId.value === null ? undefined : suggestedLines.value.get(mappingId.value),
);

function openMapping(edited: SavedCsvMapping | null) {
  editedMapping.value = edited;
  mappingModalOpen.value = true;
}

async function onMappingSaved(saved: SavedCsvMapping) {
  mappingModalOpen.value = false;
  mappings.value = await unwrap(commands.listCsvMappings());
  mappingId.value = saved.id;
  await suggest();
}

async function onMappingDeleted(id: number) {
  mappingModalOpen.value = false;
  mappings.value = mappings.value.filter((m) => m.id !== id);
  if (mappingId.value === id) mappingId.value = null;
  await suggest();
}

async function showPreview() {
  if (!path.value || accountId.value === null || format.value === null) return;
  busy.value = true;
  error.value = "";
  try {
    preview.value = await unwrap(commands.previewImport(path.value, accountId.value, format.value));
    selected.value = defaultSelection(preview.value);
    chosen.value = new Map();
    spread.value = new Map();
    step.value = "preview";
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}

function categoryOf(index: number): number | null {
  const current = preview.value;
  return current ? lineCategory(current.lines[index], index, chosen.value, spread.value) : null;
}

/** Chooses the category of a line, and of the other lines of its label not chosen by hand. */
function choose(index: number, categoryId: number | null) {
  const line = preview.value?.lines[index];
  if (!line) return;
  chosen.value = new Map(chosen.value).set(index, categoryId);
  spread.value = new Map(spread.value).set(line.normalizedLabel, categoryId);
}

/** Hint under the category: where it comes from. */
function categoryHint(index: number): string {
  const line = preview.value?.lines[index];
  if (!line) return "";
  const category = categoryOf(index);
  if (category === null) return line.categoryId === null ? "Libellé inconnu" : "";
  if (category !== line.categoryId) {
    if (category === line.suggestion?.categoryId) return "Suggestion acceptée, retenue pour ce libellé";
    return chosen.value.has(index)
      ? "Choisie à la main, retenue pour ce libellé"
      : "Choisie sur une autre ligne de ce libellé";
  }
  if (line.rulePattern !== null) return `Règle « ${line.rulePattern} »`;
  return `Reprise de « ${displayLabel(line.normalizedLabel)} »`;
}

/** Lines whose suggestion can still be accepted. */
const suggested = computed(() =>
  preview.value ? pendingSuggestions(preview.value, selected.value, chosen.value, spread.value) : [],
);

function acceptSuggestions() {
  for (const index of suggested.value) {
    const suggestion = preview.value?.lines[index].suggestion;
    if (suggestion && categoryOf(index) === null) choose(index, suggestion.categoryId);
  }
}

function isInternalTransfer(index: number): boolean {
  const category = categoryOf(index);
  return category !== null && categoriesById.value.get(category)?.isInternalTransfer === true;
}

function toggle(index: number) {
  const next = new Set(selected.value);
  if (!next.delete(index)) next.add(index);
  selected.value = next;
}

function back() {
  step.value = "file";
  error.value = "";
}

async function runImport() {
  const current = preview.value;
  if (!current || !path.value || accountId.value === null || format.value === null) return;
  step.value = "importing";
  error.value = "";
  try {
    const outcome = await unwrap(
      commands.importStatement(
        path.value,
        accountId.value,
        format.value,
        current.fileHash,
        [...selected.value].map((index) => ({
          index,
          categoryId: lineCategory(current.lines[index], index, chosen.value, spread.value),
        })),
      ),
    );
    const query: Record<string, string> = { account: String(accountId.value) };
    if (outcome.linkedTransfers > 0) query.linkedTransfers = String(outcome.linkedTransfers);
    if (outcome.matchedOccurrences > 0) query.matchedOccurrences = String(outcome.matchedOccurrences);
    await router.push({ name: "operations", query });
  } catch (e) {
    error.value = errorMessage(e);
    step.value = "preview";
  }
}

onMounted(load);
</script>

<template>
  <header class="header">
    <h1>Importer un relevé</h1>
    <ol aria-label="Étapes de l'import" class="steps">
      <template v-for="(s, index) in steps" :key="s.id">
        <li v-if="index > 0" aria-hidden="true" class="step-line" :class="{ done: index <= stepIndex }" />
        <li
          class="step"
          :class="{ done: index < stepIndex, current: index === stepIndex }"
          :aria-current="index === stepIndex ? 'step' : undefined"
        >
          <span class="step-dot">
            <svg v-if="index < stepIndex" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" aria-hidden="true">
              <path d="M5 12l5 5 9-10" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <template v-else>{{ index + 1 }}</template>
          </span>
          {{ s.label }}
        </li>
      </template>
    </ol>
  </header>

  <p v-if="error" class="error-banner">{{ error }}</p>

  <!-- Step 1: file, account and format -->
  <section v-if="step === 'file'" class="card file-step">
    <section v-if="accounts.length === 0" class="empty">
      <h2 class="card-title">Aucun compte</h2>
      <p class="muted">
        Un relevé s'importe dans un compte courant, d'épargne ou une carte.
        <RouterLink to="/accounts">Crée un compte</RouterLink> pour commencer.
      </p>
    </section>

    <template v-else>
      <div class="file-row">
        <span class="file-icon" :class="isPdf ? 'pdf' : 'csv'">{{ isPdf ? "PDF" : "CSV" }}</span>
        <div class="file-info">
          <span class="file-name">{{ fileName || "Aucun fichier choisi" }}</span>
          <span class="muted">Relevé PDF ({{ banksText }}) ou export CSV de ta banque.</span>
        </div>
        <button type="button" class="btn btn-secondary" @click="chooseFile">
          {{ path ? "Changer de fichier" : "Choisir un fichier…" }}
        </button>
      </div>

      <div class="row">
        <label class="field">
          Compte de destination
          <select v-model="accountId">
            <option v-for="s in accounts" :key="s.account.id" :value="s.account.id">{{ s.account.name }}</option>
          </select>
        </label>
        <div v-if="isPdf" class="field">
          Format du relevé
          <span v-if="busy" class="field-hint">Lecture du relevé…</span>
          <span v-else-if="detectedBank" class="recognized-bank">Format reconnu : {{ detectedBank.name }}</span>
          <span v-else class="field-error">
            Ce PDF n'est pas un relevé reconnu. Banques prises en charge : {{ banksText }}.
          </span>
        </div>
        <div v-else class="field">
          <label for="mapping">Format du fichier</label>
          <div class="mapping-row">
            <select id="mapping" v-model="mappingId">
              <option :value="null" disabled>Choisir un format…</option>
              <option v-for="m in mappings" :key="m.id" :value="m.id">{{ mappingLabel(m) }}</option>
            </select>
            <button v-if="mapping" type="button" class="btn btn-secondary" @click="openMapping(mapping)">Modifier</button>
            <button type="button" class="btn btn-secondary" @click="openMapping(null)">Nouveau format</button>
          </div>
          <span v-if="path && selectedLines !== undefined" class="field-hint recognized">
            Format reconnu : il lit {{ operationCount(selectedLines) }} dans ce fichier.
          </span>
          <span v-else-if="path && mappings.length > 0 && suggestions.length === 0" class="field-hint">
            Aucun format enregistré ne lit ce fichier : clique sur « Nouveau format », les réglages seront
            détectés automatiquement.
          </span>
          <span v-else-if="mappings.length === 0" class="field-hint">
            Clique sur « Nouveau format » : les réglages de l'export de ta banque seront détectés
            automatiquement, puis reconnus aux imports suivants.
          </span>
        </div>
      </div>

      <footer class="step-footer">
        <RouterLink to="/operations" class="btn btn-secondary">Annuler</RouterLink>
        <button type="button" class="btn btn-primary" :disabled="!canPreview" @click="showPreview">
          Continuer
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
            <path d="M5 12h14M13 6l6 6-6 6" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </footer>
    </template>
  </section>

  <!-- Step 2: check and preview -->
  <template v-else-if="preview && groups">
    <section class="cards">
      <div class="card file-card">
        <span class="file-icon" :class="preview.bank ? 'pdf' : 'csv'">{{ preview.bank ? "PDF" : "CSV" }}</span>
        <div class="file-info">
          <span class="file-name">{{ preview.fileName }}</span>
          <span class="muted">
            {{ preview.bank ? `Format reconnu : ${preview.bank}` : `Format : ${mapping?.name}` }} ·
            {{ periodText(preview) }}
          </span>
          <span class="muted">Compte de destination : <strong>{{ account?.name }}</strong></span>
        </div>
      </div>
      <div
        v-if="preview.balanceCheck.status !== 'NOT_AVAILABLE'"
        class="card balance-card"
        :class="balanceOk ? 'checked' : 'mismatch'"
      >
        <div class="balance-title">
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" aria-hidden="true">
            <circle cx="12" cy="12" r="9.5" />
            <path v-if="balanceOk" d="M7.5 12l3 3 6-6" stroke-linecap="round" stroke-linejoin="round" />
            <path v-else d="M12 7.5v5M12 16v.5" stroke-linecap="round" />
          </svg>
          {{ balanceOk ? "Soldes vérifiés" : "Écart de solde" }}
        </div>
        <div class="balance-grid">
          <span>Solde initial du relevé</span>
          <strong>{{ formatAmount(preview.startBalanceCents ?? 0, { currency }) }}</strong>
          <span>+ Somme des opérations lues</span>
          <strong>{{ formatAmount(preview.balanceCheck.linesTotalCents, { signed: true, currency }) }}</strong>
          <span class="total">= Solde calculé</span>
          <strong class="total">{{ formatAmount(preview.balanceCheck.computedEndCents ?? 0, { currency }) }}</strong>
          <template v-if="!balanceOk">
            <span>Solde final du relevé</span>
            <strong>{{ formatAmount(preview.endBalanceCents ?? 0, { currency }) }}</strong>
          </template>
        </div>
        <span v-if="balanceOk">Identique au solde final indiqué sur le relevé : aucune ligne manquante.</span>
        <span v-else>
          Écart de {{ formatAmount(balanceGap(preview) ?? 0, { signed: true, currency }) }} : une ligne du relevé n'a pas
          été lue correctement. Tu peux annuler, ou importer quand même et saisir ensuite l'opération manquante.
        </span>
      </div>
      <div v-else class="card balance-card">
        <div class="balance-title">
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" aria-hidden="true">
            <circle cx="12" cy="12" r="9.5" />
            <path d="M12 7.5v5M12 16v.5" stroke-linecap="round" />
          </svg>
          Soldes non vérifiés
        </div>
        <span>Un export CSV n'indique pas les soldes du relevé : le contrôle des soldes se fait sur les relevés PDF.</span>
      </div>
    </section>

    <p v-for="batch in preview.previousImports" :key="batch.id" class="warning-banner">
      Ce fichier a déjà été importé le {{ formatDate(localDateOf(batch.importedAt)) }} dans
      « {{ accountNames.get(batch.accountId) ?? "un autre compte" }} » ({{ operationCount(batch.operationCount) }}).
      Ses opérations déjà présentes sont reconnues comme doublons.
    </p>

    <section aria-label="Résumé de l'import" class="chips">
      <span class="chip"><strong>{{ preview.lines.length }}</strong> lues</span>
      <span class="chip neutral"><strong>{{ groups.duplicates.length }}</strong> doublons ignorés</span>
      <span class="chip housing"><strong>{{ groups.candidates.length - ambiguousCount }}</strong> nouvelles</span>
      <span v-if="ambiguousCount > 0" class="chip groceries"><strong>{{ ambiguousCount }}</strong> à vérifier</span>
      <span v-if="groups.beforeOpening.length > 0" class="chip neutral">
        <strong>{{ groups.beforeOpening.length }}</strong> antérieures à l'ouverture
      </span>
      <span class="chip transport"><strong>{{ counts.automatic }}</strong> catégorisées automatiquement</span>
      <span class="chip groceries"><strong>{{ counts.toCategorize }}</strong> à catégoriser</span>
    </section>

    <section class="table-card">
      <div class="table-title">
        <h2>Nouvelles opérations</h2>
        <span class="muted">Décoche une ligne pour l'exclure</span>
        <button v-if="suggested.length > 0" type="button" class="btn btn-secondary accept-all" @click="acceptSuggestions">
          Accepter {{ suggested.length > 1 ? `les ${suggested.length} suggestions` : "la suggestion" }}
        </button>
      </div>
      <table>
        <thead>
          <tr>
            <th scope="col" class="col-check"><span class="sr-only">Inclure</span></th>
            <th scope="col" class="col-date">Date</th>
            <th scope="col">Libellé</th>
            <th scope="col" class="col-category">Catégorie</th>
            <th scope="col" class="col-amount">Montant</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="{ index, line } in groups.candidates"
            :key="index"
            :class="{
              todo: line.duplicate.status === 'ambiguous' || categoryOf(index) === null,
              excluded: !selected.has(index),
            }"
          >
            <td class="col-check">
              <input
                type="checkbox"
                :checked="selected.has(index)"
                :aria-label="`Inclure ${line.payeeName} du ${formatDate(line.line.date, 'day-month')}`"
                @change="toggle(index)"
              />
            </td>
            <td class="col-date">{{ formatDate(line.line.date, "day-month") }}</td>
            <td>
              <span class="label">{{ line.payeeName }}</span>
              <span class="raw">{{ line.line.label }}</span>
              <span v-if="line.duplicate.status === 'ambiguous'" class="hint">
                {{ ambiguityText(line, preview.matchedOperations) }}
              </span>
            </td>
            <td>
              <CategoryPicker
                :model-value="categoryOf(index)"
                :categories="categories"
                :label="`Catégorie de ${line.payeeName}`"
                @update:model-value="choose(index, $event)"
              />
              <button
                v-if="categoryOf(index) === null && line.suggestion"
                type="button"
                class="suggestion"
                :title="'Accepter la catégorie suggérée'"
                @click="choose(index, line.suggestion.categoryId)"
              >
                <span aria-hidden="true">✓</span>
                Suggestion : {{ categoriesById.get(line.suggestion.categoryId)?.name ?? "?" }}
                ({{ line.suggestion.confidencePercent }} %)
              </button>
              <span v-if="isInternalTransfer(index)" class="category-hint">Exclu des statistiques</span>
              <span v-else-if="categoryHint(index)" class="category-hint">{{ categoryHint(index) }}</span>
            </td>
            <td
              class="col-amount"
              :class="{ positive: line.line.amountCents > 0, internal: isInternalTransfer(index) }"
            >
              {{ formatAmount(line.line.amountCents, { signed: true, currency }) }}
            </td>
          </tr>
          <tr v-if="groups.candidates.length === 0" class="empty-row">
            <td colspan="5">Aucune nouvelle opération dans ce fichier.</td>
          </tr>
        </tbody>
      </table>

      <details v-if="groups.duplicates.length > 0">
        <summary>{{ groups.duplicates.length }} doublon{{ groups.duplicates.length > 1 ? "s" : "" }} ignoré{{ groups.duplicates.length > 1 ? "s" : "" }} (déjà dans le compte)</summary>
        <ul>
          <li v-for="{ index, line } in groups.duplicates" :key="index">
            <span class="list-date">{{ formatDate(line.line.date, "day-month") }}</span>
            <span class="list-label">{{ line.payeeName }}</span>
            <span class="list-amount">{{ lineAmount(line, currency) }}</span>
          </li>
        </ul>
      </details>
      <details v-if="groups.beforeOpening.length > 0">
        <summary>
          {{ groups.beforeOpening.length }} ligne{{ groups.beforeOpening.length > 1 ? "s" : "" }}
          antérieure{{ groups.beforeOpening.length > 1 ? "s" : "" }} à l'ouverture du compte
          ({{ account ? formatDate(account.openingDate) : "" }}), déjà comprise{{ groups.beforeOpening.length > 1 ? "s" : "" }}
          dans le solde d'ouverture
        </summary>
        <ul>
          <li v-for="{ index, line } in groups.beforeOpening" :key="index">
            <span class="list-date">{{ formatDate(line.line.date, "day-month") }}</span>
            <span class="list-label">{{ line.payeeName }}</span>
            <span class="list-amount">{{ lineAmount(line, currency) }}</span>
          </li>
        </ul>
      </details>
    </section>

    <footer class="step-footer preview-footer">
      <span class="backup-note">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <path d="M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6z" stroke-linejoin="round" />
        </svg>
        Une sauvegarde sera faite avant l'import. Tu pourras l'annuler à tout moment.
        <template v-if="!balanceOk">L'import sera signalé par un écart de solde.</template>
      </span>
      <div class="buttons">
        <button type="button" class="btn btn-secondary" :disabled="step === 'importing'" @click="back">Retour</button>
        <button
          type="button"
          class="btn btn-primary"
          :disabled="selected.size === 0 || step === 'importing'"
          @click="runImport"
        >
          {{
            step === "importing"
              ? "Import en cours…"
              : `Importer ${balanceOk ? "" : "quand même "}${operationCount(selected.size)}`
          }}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
            <path d="M5 12h14M13 6l6 6-6 6" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>
    </footer>
  </template>

  <CsvMappingModal
    :open="mappingModalOpen"
    :mapping="editedMapping"
    :path="path"
    @close="mappingModalOpen = false"
    @saved="onMappingSaved"
    @deleted="onMappingDeleted"
  />
</template>

<style scoped>
.header {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

h1 {
  margin: 0;
  font-size: 30px;
  font-weight: 800;
  letter-spacing: -0.02em;
}

.steps {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 14px;
  font-weight: 700;
}

.step {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--color-text-muted);
}

.step.current {
  color: var(--color-text);
}

.step.done {
  color: var(--cat-transport-fg);
}

.step-dot {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  box-sizing: border-box;
  border: 1.5px solid var(--color-control-border);
  border-radius: 14px;
  background: var(--color-card);
  font-size: 13px;
}

.step.current .step-dot {
  border: 0;
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.step.done .step-dot {
  border: 0;
  background: var(--cat-transport);
  color: var(--color-on-primary);
}

.step-line {
  width: 40px;
  height: 2px;
  border-radius: 1px;
  background: var(--color-border);
}

.step-line.done {
  background: var(--cat-transport);
}

.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.file-step {
  gap: 20px;
}

.file-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 16px;
}

.file-icon {
  display: flex;
  flex: none;
  align-items: flex-end;
  justify-content: center;
  width: 48px;
  height: 56px;
  box-sizing: border-box;
  padding-bottom: 8px;
  border-radius: 10px;
  font-size: 11px;
  font-weight: 800;
}

.file-icon.pdf {
  background: var(--cat-restaurants-bg);
  color: var(--cat-restaurants-fg);
}

.recognized-bank {
  display: flex;
  align-items: center;
  height: 42px;
  font-size: 14px;
  font-weight: 700;
  color: var(--cat-transport-fg);
}

.balance-card.checked {
  border-color: var(--color-success-border);
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
}

.balance-card.mismatch {
  border-color: var(--color-danger-border);
  background: var(--cat-restaurants-bg);
  color: var(--cat-restaurants-fg);
}

.balance-grid .total {
  padding-top: 6px;
  border-top: 1px solid currentColor;
}

.balance-grid {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 6px 16px;
  font-size: 14px;
  color: var(--color-text);
}

.balance-grid strong {
  text-align: right;
  white-space: nowrap;
}

.file-icon.csv {
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
}

.file-info {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.file-name {
  font-size: 15px;
  font-weight: 800;
  overflow-wrap: anywhere;
}

.row {
  display: grid;
  grid-template-columns: minmax(220px, 2fr) minmax(360px, 3fr);
  gap: 16px;
}

.recognized {
  color: var(--cat-transport-fg);
}

.mapping-row {
  display: flex;
  gap: 8px;
}

.mapping-row select {
  flex: 1;
  min-width: 0;
}

.mapping-row .btn {
  height: 42px;
  padding: 0 14px;
  font-size: 13px;
}

.step-footer {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 16px;
}

.file-card {
  flex-direction: row;
  align-items: flex-start;
  gap: 16px;
  padding: 20px 22px;
}

.balance-card {
  gap: 10px;
  padding: 20px 22px;
  background: var(--cat-neutral-bg);
  color: var(--cat-neutral-fg);
  font-size: 13px;
}

.balance-title {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 15px;
  font-weight: 800;
}

.warning-banner {
  margin: 0;
  padding: 12px 16px;
  border-radius: 14px;
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
  font-size: 14px;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.chip {
  padding: 10px 16px;
  border-radius: var(--radius-control);
  background: var(--color-card);
  border: 1px solid var(--color-border);
  font-size: 14px;
}

.chip strong {
  font-size: 18px;
}

.chip.neutral {
  border-color: transparent;
  background: var(--cat-neutral-bg);
  color: var(--cat-neutral-fg);
}

.chip.housing {
  border-color: transparent;
  background: var(--cat-housing-bg);
  color: var(--cat-housing-fg);
}

.chip.transport {
  border-color: transparent;
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
}

.suggestion {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  padding: 2px 8px;
  border: 1px dashed var(--color-border);
  border-radius: 999px;
  background: transparent;
  color: var(--cat-housing-fg);
  font-size: 11px;
  font-weight: 700;
  cursor: pointer;
}

.suggestion:hover {
  border-style: solid;
  background: var(--cat-housing-bg);
}

.accept-all {
  margin-left: auto;
}

.category-hint {
  display: block;
  margin-top: 4px;
  font-size: 11px;
  color: var(--color-text-muted);
}

.chip.groceries {
  border-color: transparent;
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
}

.table-card {
  overflow-x: auto;
  background: var(--color-card);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

.table-title {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  padding: 18px 20px 6px;
}

.table-title h2 {
  margin: 0;
  font-size: 17px;
  font-weight: 800;
}

table {
  width: 100%;
  min-width: 760px;
  border-collapse: collapse;
  font-size: 14px;
}

th {
  padding: 12px;
  border-bottom: 1px solid var(--color-border);
  font-size: 12px;
  font-weight: 700;
  color: var(--color-text-muted);
  text-align: left;
}

td {
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-divider);
}

.col-check {
  width: 40px;
  padding-left: 20px;
}

.col-check input {
  width: 18px;
  height: 18px;
  accent-color: var(--color-primary);
}

.col-date {
  width: 64px;
}

td.col-date {
  color: var(--color-text-muted);
  font-weight: 600;
}

.col-category {
  width: 200px;
}

.col-amount {
  width: 120px;
  padding-right: 20px;
  text-align: right;
  white-space: nowrap;
}

td.col-amount {
  font-weight: 800;
}

td.col-amount.positive {
  color: var(--color-positive);
}

td.col-amount.internal {
  color: var(--color-text-muted);
}

tr.todo {
  background: var(--color-pending-row);
}

tr.excluded td:not(.col-check) {
  opacity: 0.5;
}

.label {
  display: block;
  font-weight: 700;
}

.raw {
  display: block;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  color: var(--color-text-muted);
}

.hint {
  display: block;
  margin-top: 4px;
  font-size: 12px;
  font-weight: 600;
  color: var(--cat-groceries-fg);
}

.empty-row td {
  padding: 28px 20px;
  color: var(--color-text-muted);
  text-align: center;
}

details {
  padding: 14px 20px;
  border-top: 1px solid var(--color-border);
  font-size: 14px;
}

summary {
  font-weight: 700;
  color: var(--cat-neutral-fg);
  cursor: pointer;
}

details ul {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 12px 0 0;
  padding: 0;
  list-style: none;
  color: var(--color-text-muted);
}

details li {
  display: flex;
  gap: 16px;
}

.list-date {
  width: 52px;
}

.list-label {
  flex: 1;
}

.list-amount {
  white-space: nowrap;
}

.preview-footer {
  justify-content: space-between;
}

.backup-note {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--color-text-muted);
}

.buttons {
  display: flex;
  gap: 10px;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}
</style>
