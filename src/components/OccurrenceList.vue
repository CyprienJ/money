<script setup lang="ts">
import { ref } from "vue";
import { commands, type Account, type Category, type MatchedOperation, type UpcomingOperation } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate } from "../utils/format";
import CategoryPill from "./CategoryPill.vue";

const props = defineProps<{
  occurrences: UpcomingOperation[];
  accounts: Map<number, Account>;
  categoriesById: Map<number, Category>;
  /** Past occurrences: unrealized ones are "En attente". */
  past: boolean;
}>();

const emit = defineEmits<{
  /** Emitted after a match or an unmatch. */
  changed: [];
}>();

/** Occurrence whose candidates are shown, as `<schedule id>-<date>`. */
const openKey = ref<string | null>(null);
const candidates = ref<MatchedOperation[]>([]);
const busy = ref(false);
const error = ref("");

function key(occurrence: UpcomingOperation): string {
  return `${occurrence.scheduledOperationId}-${occurrence.date}`;
}

async function run(action: () => Promise<unknown>): Promise<boolean> {
  busy.value = true;
  error.value = "";
  try {
    await action();
    return true;
  } catch (e) {
    error.value = errorMessage(e);
    return false;
  } finally {
    busy.value = false;
  }
}

async function toggleCandidates(occurrence: UpcomingOperation) {
  if (openKey.value === key(occurrence)) {
    openKey.value = null;
    return;
  }
  await run(async () => {
    candidates.value = await unwrap(
      commands.scheduleMatchCandidates(occurrence.scheduledOperationId, occurrence.date),
    );
    openKey.value = key(occurrence);
  });
}

async function match(occurrence: UpcomingOperation, operation: MatchedOperation) {
  const done = await run(() =>
    unwrap(commands.matchScheduleOccurrence(occurrence.scheduledOperationId, occurrence.date, operation.operationId)),
  );
  if (done) {
    openKey.value = null;
    emit("changed");
  }
}

async function unmatch(occurrence: UpcomingOperation) {
  if (await run(() => unwrap(commands.unmatchScheduleOccurrence(occurrence.scheduledOperationId, occurrence.date)))) {
    emit("changed");
  }
}

function currencyOf(occurrence: UpcomingOperation): string | undefined {
  return props.accounts.get(occurrence.accountId)?.currency;
}

function operationText(occurrence: UpcomingOperation, operation: MatchedOperation): string {
  const amount = formatAmount(operation.amountCents, { signed: true, currency: currencyOf(occurrence) });
  return `${formatDate(operation.date)} · ${amount}`;
}
</script>

<template>
  <p v-if="error" class="error-banner">{{ error }}</p>
  <ul class="list">
    <li v-for="item in occurrences" :key="key(item)">
      <div class="row">
        <span class="date">{{ formatDate(item.date, "day-month") }}</span>
        <span class="details">
          <span class="label">{{ item.label }}</span>
          <span class="sub">{{ props.accounts.get(item.accountId)?.name }}</span>
          <span v-if="item.realizedBy" class="status realized">
            Réalisée : {{ item.realizedBy.payeeName }} · {{ operationText(item, item.realizedBy) }}
          </span>
          <span v-else-if="past" class="status pending">En attente</span>
        </span>
        <CategoryPill
          v-if="item.categoryId !== null && categoriesById.has(item.categoryId)"
          :label="categoriesById.get(item.categoryId)!.name"
          :color="categoriesById.get(item.categoryId)!.color"
        />
        <strong class="amount" :class="{ positive: item.amountCents > 0 }">
          {{ formatAmount(item.amountCents, { signed: true, currency: currencyOf(item) }) }}
        </strong>
      </div>
      <div class="actions">
        <button v-if="item.realizedBy" type="button" class="btn btn-link" :disabled="busy" @click="unmatch(item)">
          Dissocier
        </button>
        <button v-else type="button" class="btn btn-link" :disabled="busy" @click="toggleCandidates(item)">
          {{ openKey === key(item) ? "Fermer" : "Associer une opération…" }}
        </button>
      </div>
      <div v-if="openKey === key(item)" class="candidates">
        <ul v-if="candidates.length > 0">
          <li v-for="operation in candidates" :key="operation.operationId">
            <span>
              <strong>{{ operation.payeeName }}</strong> · {{ operationText(item, operation) }}
            </span>
            <button type="button" class="btn btn-link" :disabled="busy" @click="match(item, operation)">
              Associer
            </button>
          </li>
        </ul>
        <span v-else class="sub">
          Aucune opération non associée de même sens sur ce compte, à 15 jours près.
        </span>
      </div>
    </li>
  </ul>
</template>

<style scoped>
.list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 14px;
}

.list > li {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 0;
  border-bottom: 1px solid var(--color-divider);
}

.list > li:last-child {
  border-bottom: 0;
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.date {
  width: 44px;
  font-weight: 700;
  color: var(--color-text-muted);
}

.details {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.label {
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--color-text-muted);
}

.status {
  align-self: flex-start;
  margin-top: 4px;
  padding: 2px 8px;
  border-radius: var(--radius-pill);
  font-size: 11px;
  font-weight: 700;
}

.realized {
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
}

.pending {
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
}

.amount {
  white-space: nowrap;
}

.amount.positive {
  color: var(--color-positive);
}

.actions {
  padding-left: 56px;
  font-size: 13px;
}

.candidates {
  margin-left: 56px;
  font-size: 13px;
}

.candidates ul {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.candidates li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 4px 4px 4px 10px;
  border-radius: var(--radius-nav);
  background: var(--color-page);
}
</style>
