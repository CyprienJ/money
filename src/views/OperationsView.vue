<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  commands,
  type AccountSummary,
  type Category,
  type ImportBatch,
  type Operation,
  type OperationRow,
  type SameLabel,
} from "../bindings";
import CategoryPicker from "../components/CategoryPicker.vue";
import OperationFormModal from "../components/OperationFormModal.vue";
import RuleFormModal from "../components/RuleFormModal.vue";
import { ACCOUNT_TYPES } from "../utils/accounts";
import { categoryTree } from "../utils/categories";
import { errorMessage, unwrap } from "../utils/commands";
import { displayLabel, formatAmount, formatDate, localDateOf, operationCount, todayIso } from "../utils/format";
import { matchesSearch, summarize } from "../utils/operations";
import { periodOptions } from "../utils/periods";

const route = useRoute();
const router = useRouter();

const accounts = ref<AccountSummary[]>([]);
const categories = ref<Category[]>([]);
const rows = ref<OperationRow[]>([]);
/** Most recent import of the selected account, which can be cancelled. */
const lastImport = ref<ImportBatch | null>(null);
const confirmingCancel = ref(false);
const loaded = ref(false);
const error = ref("");

const periods = periodOptions();
const periodId = ref(periods[0].id);
/** "all", "none" (to categorize) or a category id. */
const categoryFilter = ref<"all" | "none" | number>("all");
const search = ref("");

/** Offer to give the category just chosen to the other operations of the same label. */
const spread = ref<{ operation: Operation; categoryId: number; sameLabel: SameLabel } | null>(null);
/** Result of the last spread, shown until the next categorization. */
const spreadDone = ref("");

const modalOpen = ref(false);
const editedOperation = ref<Operation | null>(null);

/** Rule created from an operation: its label and category prefill the form. */
const ruleModalOpen = ref(false);
const rulePrefill = ref<{ pattern: string; categoryId: number | null }>({ pattern: "", categoryId: null });

const selected = computed(() => {
  const id = Number(route.query.account);
  return accounts.value.find((s) => s.account.id === id) ?? accounts.value[0] ?? null;
});

const tree = computed(() => categoryTree(categories.value));

/** Ids matched by the category filter: a parent includes its children. */
const filterIds = computed(() => {
  const filter = categoryFilter.value;
  if (typeof filter !== "number") return null;
  return new Set([filter, ...categories.value.filter((c) => c.parentId === filter).map((c) => c.id)]);
});

const visibleRows = computed(() =>
  rows.value.filter(({ operation }) => {
    if (categoryFilter.value === "none" && operation.categoryId !== null) return false;
    if (filterIds.value && (operation.categoryId === null || !filterIds.value.has(operation.categoryId))) {
      return false;
    }
    return matchesSearch(operation, search.value);
  }),
);

const totals = computed(() => summarize(visibleRows.value.map((r) => r.operation)));

async function loadAccounts() {
  const summaries = await unwrap(commands.listAccountSummaries(false));
  accounts.value = summaries.filter((s) => ACCOUNT_TYPES[s.account.type].hasOperations);
  categories.value = await unwrap(commands.listCategories());
}

async function loadOperations() {
  const account = selected.value;
  confirmingCancel.value = false;
  if (!account) {
    rows.value = [];
    lastImport.value = null;
    return;
  }
  const period = periods.find((p) => p.id === periodId.value) ?? periods[0];
  rows.value = await unwrap(commands.listOperations(account.account.id, period.from, period.to));
  const batches = await unwrap(commands.listImportBatches(account.account.id));
  lastImport.value = batches[0] ?? null;
}

const BALANCE_CHECK_TEXT: Record<ImportBatch["balanceCheckStatus"], string> = {
  OK: "soldes vérifiés",
  MISMATCH: "écart de solde",
  NOT_AVAILABLE: "soldes non vérifiés",
};

async function cancelLastImport() {
  const batch = lastImport.value;
  if (!batch) return;
  if (!confirmingCancel.value) {
    confirmingCancel.value = true;
    return;
  }
  error.value = "";
  try {
    await unwrap(commands.cancelImport(batch.id));
  } catch (e) {
    error.value = errorMessage(e);
  }
  await reload(true);
}

async function reload(withAccounts = false) {
  error.value = "";
  try {
    if (withAccounts) await loadAccounts();
    await loadOperations();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

function selectAccount(id: number) {
  router.replace({ query: { ...route.query, account: String(id) } });
}

function label(operation: Operation): string {
  return operation.payeeName;
}

function amountClass(operation: Operation): string {
  if (operation.isInternalTransfer) return "internal";
  return operation.amountCents > 0 ? "positive" : "";
}

/**
 * Categorizes an operation from the table; the choice is remembered for its label. Then offers
 * to categorize the other operations of the label, and warns about those in another category.
 */
async function setCategory(operation: Operation, categoryId: number | null) {
  error.value = "";
  spread.value = null;
  spreadDone.value = "";
  try {
    await unwrap(commands.setOperationCategory(operation.id, categoryId));
    if (categoryId !== null) {
      const sameLabel = await unwrap(commands.sameLabelOperations(operation.id));
      if (sameLabel.uncategorized > 0 || sameLabel.otherCategories.length > 0) {
        spread.value = { operation, categoryId, sameLabel };
      }
    }
  } catch (e) {
    error.value = errorMessage(e);
  }
  await reload();
}

async function spreadCategory() {
  const current = spread.value;
  if (!current) return;
  error.value = "";
  try {
    const count = await unwrap(commands.categorizeSameLabel(current.operation.id));
    spreadDone.value = `${operationCount(count)} « ${displayLabel(current.sameLabel.normalizedLabel)} » classée${count > 1 ? "s" : ""} dans ${categoryName(current.categoryId)}.`;
    spread.value = null;
  } catch (e) {
    error.value = errorMessage(e);
  }
  await reload();
}

function categoryName(id: number): string {
  return categories.value.find((c) => c.id === id)?.name ?? "une catégorie supprimée";
}

/** "Restaurants (2), Loisirs (1)". */
function otherCategoriesText(sameLabel: SameLabel): string {
  return sameLabel.otherCategories.map((c) => `${categoryName(c.categoryId)} (${c.count})`).join(", ");
}

function otherCount(sameLabel: SameLabel): number {
  return sameLabel.otherCategories.reduce((sum, c) => sum + c.count, 0);
}

function openCreate() {
  editedOperation.value = null;
  modalOpen.value = true;
}

function openEdit(operation: Operation) {
  editedOperation.value = operation;
  modalOpen.value = true;
}

async function onChanged() {
  modalOpen.value = false;
  await reload(true);
}

function openRule(operation: Operation, categoryId: number | null) {
  modalOpen.value = false;
  rulePrefill.value = { pattern: displayLabel(operation.normalizedLabel), categoryId };
  ruleModalOpen.value = true;
}

async function onRuleChanged(message: string) {
  ruleModalOpen.value = false;
  spread.value = null;
  spreadDone.value = message;
  await reload();
}

onMounted(() => reload(true));
watch([() => selected.value?.account.id, periodId], () => reload());
</script>

<template>
  <header class="header">
    <div>
      <div class="subtitle">
        <template v-if="selected">{{ selected.account.name }} · solde au {{ formatDate(todayIso(), "day-month") }}</template>
        <template v-else>Comptes</template>
      </div>
      <h1>
        Opérations
        <span
          v-if="selected && selected.balanceCents !== null"
          class="balance"
          :class="{ negative: selected.balanceCents < 0 }"
        >
          {{ formatAmount(selected.balanceCents) }}
        </span>
      </h1>
    </div>
    <div class="actions">
      <RouterLink
        :to="{ name: 'import', query: selected ? { account: String(selected.account.id) } : {} }"
        class="btn btn-secondary"
      >
        Importer un relevé
      </RouterLink>
      <button type="button" class="btn btn-primary" :disabled="!selected" @click="openCreate">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
          <path d="M12 5v14M5 12h14" stroke-linecap="round" />
        </svg>
        Nouvelle opération
      </button>
    </div>
  </header>

  <p v-if="error" class="error-banner">{{ error }}</p>

  <section v-if="loaded && accounts.length === 0" class="card">
    <h2 class="card-title">Aucun compte</h2>
    <p class="muted">
      Les opérations se rattachent à un compte courant, d'épargne ou à une carte.
      <RouterLink to="/accounts">Crée un compte</RouterLink> pour commencer.
    </p>
  </section>

  <template v-else-if="selected">
    <div role="tablist" aria-label="Comptes" class="tabs">
      <button
        v-for="{ account } in accounts"
        :key="account.id"
        type="button"
        role="tab"
        class="tab"
        :aria-selected="account.id === selected.account.id"
        @click="selectAccount(account.id)"
      >
        {{ account.name }}
      </button>
    </div>

    <div v-if="lastImport" class="banner" :class="{ mismatch: lastImport.balanceCheckStatus === 'MISMATCH' }">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z M14 3v5h5" stroke-linejoin="round" />
      </svg>
      <span class="banner-text">
        Dernier import : <strong>{{ lastImport.sourceFileName }}</strong>
        le {{ formatDate(localDateOf(lastImport.importedAt), "day-month") }}
        · {{ operationCount(lastImport.operationCount) }}
        · {{ BALANCE_CHECK_TEXT[lastImport.balanceCheckStatus] }}
        <template v-if="lastImport.balanceCheckStatus === 'MISMATCH'">
          : saisis l'opération manquante, ou annule l'import.
        </template>
      </span>
      <button type="button" class="banner-action" @click="cancelLastImport">
        {{ confirmingCancel ? `Confirmer : supprimer ${operationCount(lastImport.operationCount)}` : "Annuler cet import" }}
      </button>
    </div>

    <template v-if="spread">
      <div v-if="spread.sameLabel.uncategorized > 0" class="banner">
        <span class="banner-text">
          {{ operationCount(spread.sameLabel.uncategorized) }}
          « {{ displayLabel(spread.sameLabel.normalizedLabel) }} »
          {{ spread.sameLabel.uncategorized > 1 ? "sont" : "est" }} encore à catégoriser.
        </span>
        <button type="button" class="banner-action" @click="spreadCategory">
          Les classer dans {{ categoryName(spread.categoryId) }}
        </button>
        <button type="button" class="banner-action" @click="spread = null">Ignorer</button>
      </div>
      <div v-if="spread.sameLabel.otherCategories.length > 0" class="banner mismatch">
        <span class="banner-text">
          Attention : {{ operationCount(otherCount(spread.sameLabel)) }}
          « {{ displayLabel(spread.sameLabel.normalizedLabel) }} »
          {{ otherCount(spread.sameLabel) > 1 ? "sont" : "est" }} dans une autre catégorie :
          {{ otherCategoriesText(spread.sameLabel) }}.
        </span>
        <button
          v-if="spread.sameLabel.uncategorized === 0"
          type="button"
          class="banner-action"
          @click="spread = null"
        >
          Fermer
        </button>
      </div>
    </template>
    <div v-else-if="spreadDone" class="banner done">
      <span class="banner-text">{{ spreadDone }}</span>
      <button type="button" class="banner-action" @click="spreadDone = ''">Fermer</button>
    </div>

    <div class="filters">
      <label class="field search">
        Rechercher
        <input v-model="search" type="search" placeholder="Libellé, tiers, montant…" />
      </label>
      <label class="field">
        Période
        <select v-model="periodId">
          <option v-for="period in periods" :key="period.id" :value="period.id">
            {{ period.label }}
          </option>
        </select>
      </label>
      <label class="field">
        Catégorie
        <select v-model="categoryFilter">
          <option value="all">Toutes</option>
          <option value="none">À catégoriser</option>
          <template v-for="{ parent, children } in tree" :key="parent.id">
            <option :value="parent.id">{{ parent.name }}</option>
            <option v-for="child in children" :key="child.id" :value="child.id">
              &nbsp;&nbsp;&nbsp;{{ child.name }}
            </option>
          </template>
        </select>
      </label>
    </div>

    <div class="table-card">
      <table>
        <thead>
          <tr>
            <th scope="col" class="col-date">Date</th>
            <th scope="col">Libellé</th>
            <th scope="col" class="col-category">Catégorie</th>
            <th scope="col" class="col-amount">Montant</th>
            <th scope="col" class="col-balance">Solde</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="{ operation, balanceCents } in visibleRows"
            :key="operation.id"
            :class="{ pending: operation.categoryId === null }"
            tabindex="0"
            @click="openEdit(operation)"
            @keydown.enter="openEdit(operation)"
          >
            <td class="col-date">{{ formatDate(operation.date, "day-month") }}</td>
            <td>
              <span class="label">{{ label(operation) }}</span>
              <span v-if="operation.rawLabel !== operation.payeeName" class="raw">{{ operation.rawLabel }}</span>
            </td>
            <td>
              <CategoryPicker
                :model-value="operation.categoryId"
                :categories="categories"
                :label="`Catégorie de ${label(operation)}`"
                @update:model-value="setCategory(operation, $event)"
              />
              <span v-if="operation.isInternalTransfer" class="excluded">Exclu des statistiques</span>
            </td>
            <td class="col-amount" :class="amountClass(operation)">
              {{ formatAmount(operation.amountCents, { signed: true }) }}
            </td>
            <td class="col-balance">{{ formatAmount(balanceCents) }}</td>
          </tr>
          <tr v-if="loaded && visibleRows.length === 0" class="empty">
            <td colspan="5">Aucune opération sur cette période.</td>
          </tr>
        </tbody>
      </table>
    </div>

    <div class="footer">
      <span>{{ totals.count }} opération{{ totals.count > 1 ? "s" : "" }}</span>
      <span>Entrées <strong class="positive">{{ formatAmount(totals.incomeCents, { signed: true }) }}</strong></span>
      <span>Sorties <strong>{{ formatAmount(totals.expenseCents, { signed: true }) }}</strong></span>
      <span>
        Virements internes
        <strong>{{ formatAmount(totals.internalTransferCents, { signed: true }) }}</strong>
      </span>
      <span v-if="totals.uncategorized > 0" class="to-categorize">
        <strong>
          {{ totals.uncategorized }} opération{{ totals.uncategorized > 1 ? "s" : "" }} à catégoriser
        </strong>
      </span>
    </div>

    <OperationFormModal
      :open="modalOpen"
      :account="selected.account"
      :categories="categories"
      :operation="editedOperation"
      @close="modalOpen = false"
      @changed="onChanged"
      @create-rule="openRule"
    />
    <RuleFormModal
      :open="ruleModalOpen"
      :categories="categories"
      :rule="null"
      :initial-pattern="rulePrefill.pattern"
      :initial-category-id="rulePrefill.categoryId"
      @close="ruleModalOpen = false"
      @changed="onRuleChanged"
    />
  </template>
</template>

<style scoped>
.header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
}

.subtitle {
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-muted);
}

h1 {
  margin: 2px 0 0;
  font-size: 30px;
  font-weight: 800;
  letter-spacing: -0.02em;
}

.balance {
  color: var(--color-positive);
  white-space: nowrap;
}

.balance.negative {
  color: var(--cat-restaurants-fg);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.muted {
  margin: 0;
  color: var(--color-text-muted);
}

.tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.tab {
  height: 40px;
  padding: 0 16px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  background: var(--color-card);
  color: var(--color-text);
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
}

.tab[aria-selected="true"] {
  border-color: var(--color-primary);
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.banner {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-radius: 14px;
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
  font-size: 14px;
}

.banner.mismatch {
  background: var(--cat-restaurants-bg);
  color: var(--cat-restaurants-fg);
}

.banner.done {
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
}

.banner-text {
  flex: 1 1 320px;
}

.banner-action {
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  font-size: 14px;
  font-weight: 700;
  text-decoration: underline;
  cursor: pointer;
}

.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 10px;
}

.filters .field {
  flex: 1 1 160px;
  font-size: 12px;
  font-weight: 700;
}

.filters .search {
  flex: 2 1 260px;
}

.filters input,
.filters select {
  height: 44px;
  border-radius: var(--radius-control);
}

.table-card {
  overflow-x: auto;
  background: var(--color-card);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
}

table {
  width: 100%;
  min-width: 760px;
  border-collapse: collapse;
  font-size: 14px;
}

th {
  padding: 14px 12px;
  border-bottom: 1px solid var(--color-border);
  font-size: 12px;
  font-weight: 700;
  color: var(--color-text-muted);
  text-align: left;
}

td {
  padding: 12px;
  border-bottom: 1px solid var(--color-divider);
}

tbody tr:not(.empty) {
  cursor: pointer;
}

tbody tr:not(.empty):hover,
tbody tr:focus-visible {
  background: var(--color-page);
  outline: none;
}

tbody tr.pending {
  background: var(--color-pending-row);
}

th.col-date,
td.col-date {
  width: 70px;
  padding-left: 20px;
}

td.col-date {
  color: var(--color-text-muted);
  font-weight: 600;
  white-space: nowrap;
}

.col-category {
  width: 190px;
}

.col-amount,
.col-balance {
  width: 120px;
  text-align: right;
  white-space: nowrap;
}

th.col-balance,
td.col-balance {
  padding-right: 20px;
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

td.col-balance {
  color: var(--color-text-muted);
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

.excluded {
  display: block;
  margin-top: 4px;
  font-size: 11px;
  color: var(--color-text-muted);
}

.empty td {
  padding: 28px 20px;
  color: var(--color-text-muted);
  text-align: center;
}

.footer {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 28px;
  font-size: 14px;
  color: var(--color-text-muted);
}

.footer strong {
  color: var(--color-text);
}

.footer strong.positive {
  color: var(--color-positive);
}

.to-categorize {
  margin-left: auto;
}

.footer .to-categorize strong {
  color: var(--cat-groceries-fg);
}
</style>
