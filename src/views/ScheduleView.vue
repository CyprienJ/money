<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import {
  commands,
  type Account,
  type Category,
  type ScheduledOperation,
  type UpcomingOperation,
} from "../bindings";
import OccurrenceList from "../components/OccurrenceList.vue";
import PageHeader from "../components/PageHeader.vue";
import ScheduleFormModal from "../components/ScheduleFormModal.vue";
import ScheduleMatchesModal from "../components/ScheduleMatchesModal.vue";
import { ACCOUNT_TYPES } from "../utils/accounts";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate, todayIso } from "../utils/format";
import { RECENT_DAYS, describeSchedule, recentRange } from "../utils/schedule";

/** Days of upcoming occurrences shown. */
const UPCOMING_DAYS = 30;

const accounts = ref<Account[]>([]);
const categories = ref<Category[]>([]);
const schedules = ref<ScheduledOperation[]>([]);
const upcoming = ref<UpcomingOperation[]>([]);
/** Past occurrences, the most recent first. */
const recent = ref<UpcomingOperation[]>([]);
/** Detected matches waiting for a validation. */
const proposalCount = ref(0);
const matchesOpen = ref(false);
const loaded = ref(false);
const error = ref("");

const modalOpen = ref(false);
const edited = ref<ScheduledOperation | null>(null);

const accountNames = computed(() => new Map(accounts.value.map((a) => [a.id, a.name])));
const categoriesById = computed(() => new Map(categories.value.map((c) => [c.id, c])));
/** Next occurrence of each schedule within the upcoming window. */
const nextDates = computed(() => {
  const next = new Map<number, string>();
  for (const occurrence of upcoming.value) {
    if (!next.has(occurrence.scheduledOperationId)) next.set(occurrence.scheduledOperationId, occurrence.date);
  }
  return next;
});

async function load() {
  error.value = "";
  try {
    const all = await unwrap(commands.listAccounts(false));
    accounts.value = all.filter((a) => ACCOUNT_TYPES[a.type].hasOperations);
    categories.value = await unwrap(commands.listCategories());
    schedules.value = await unwrap(commands.listScheduledOperations());
    upcoming.value = await unwrap(commands.upcomingOperations(UPCOMING_DAYS));
    const range = recentRange(todayIso());
    recent.value = (await unwrap(commands.scheduleOccurrences(range.from, range.to))).reverse();
    proposalCount.value = (await unwrap(commands.listScheduleMatchProposals())).length;
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

function openCreate() {
  edited.value = null;
  modalOpen.value = true;
}

function openEdit(schedule: ScheduledOperation) {
  edited.value = schedule;
  modalOpen.value = true;
}

/** Past occurrences not realized yet. */
const pendingCount = computed(() => recent.value.filter((o) => o.realizedBy === null).length);

async function onMatchesChanged(remaining: number) {
  proposalCount.value = remaining;
  await load();
}

async function onChanged() {
  modalOpen.value = false;
  await load();
}

onMounted(load);
</script>

<template>
  <PageHeader title="Échéancier" subtitle="Patrimoine">
    <template #actions>
      <button type="button" class="btn btn-secondary" :disabled="accounts.length === 0" @click="matchesOpen = true">
        Rapprocher<template v-if="proposalCount > 0"> ({{ proposalCount }})</template>
      </button>
      <button type="button" class="btn btn-primary" :disabled="accounts.length === 0" @click="openCreate">
        + Nouvelle échéance
      </button>
    </template>
  </PageHeader>

  <p v-if="error" class="error-banner">{{ error }}</p>

  <section v-if="loaded && accounts.length === 0" class="card">
    <h2 class="card-title">Aucun compte</h2>
    <p class="muted">
      Une échéance se rattache à un compte courant, d'épargne ou à une carte.
      <RouterLink to="/accounts">Crée un compte</RouterLink> pour commencer.
    </p>
  </section>

  <div v-else-if="loaded && proposalCount > 0" class="banner">
    <span class="banner-text">
      {{ proposalCount }} échéance{{ proposalCount > 1 ? "s" : "" }} à valider : plusieurs opérations
      importées peuvent {{ proposalCount > 1 ? "les" : "la" }} réaliser.
    </span>
    <button type="button" class="banner-action" @click="matchesOpen = true">Valider</button>
  </div>

  <div v-if="loaded && accounts.length > 0" class="grid">
    <section class="card">
      <div class="card-header">
        <h2 class="card-title">Prochaines échéances</h2>
        <span class="muted">{{ UPCOMING_DAYS }} jours</span>
      </div>
      <OccurrenceList
        v-if="upcoming.length > 0"
        :occurrences="upcoming"
        :account-names="accountNames"
        :categories-by-id="categoriesById"
        :past="false"
        @changed="load"
      />
      <p v-else class="muted">Aucune échéance dans les {{ UPCOMING_DAYS }} prochains jours.</p>
    </section>

    <section class="card">
      <div class="card-header">
        <h2 class="card-title">Échéances récentes</h2>
        <span class="muted">
          {{ RECENT_DAYS }} jours<template v-if="pendingCount > 0"> · {{ pendingCount }} en attente</template>
        </span>
      </div>
      <OccurrenceList
        v-if="recent.length > 0"
        :occurrences="recent"
        :account-names="accountNames"
        :categories-by-id="categoriesById"
        :past="true"
        @changed="load"
      />
      <p v-else class="muted">Aucune échéance dans les {{ RECENT_DAYS }} derniers jours.</p>
    </section>

    <section class="card">
      <div class="card-header">
        <h2 class="card-title">Opérations récurrentes</h2>
        <span class="muted">{{ schedules.length }}</span>
      </div>
      <ul v-if="schedules.length > 0" class="list">
        <li v-for="schedule in schedules" :key="schedule.id">
          <button type="button" class="row-button" @click="openEdit(schedule)">
            <span class="details">
              <span class="label">{{ schedule.label }}</span>
              <span class="sub">
                {{ describeSchedule(schedule) }} · {{ accountNames.get(schedule.accountId) }}
                <template v-if="schedule.endDate"> · jusqu'au {{ formatDate(schedule.endDate) }}</template>
              </span>
              <span v-if="nextDates.has(schedule.id)" class="sub">
                Prochaine : {{ formatDate(nextDates.get(schedule.id)!) }}
              </span>
            </span>
            <strong class="amount" :class="{ positive: schedule.amountCents > 0 }">
              {{ formatAmount(schedule.amountCents, { signed: true }) }}
            </strong>
          </button>
        </li>
      </ul>
      <p v-else class="muted">
        Ajoute ton loyer, ton salaire ou tes abonnements pour voir les prochaines échéances. Elles
        s'affichent sans créer d'opérations : celles-ci viennent de l'import.
      </p>
    </section>
  </div>

  <ScheduleMatchesModal
    :open="matchesOpen"
    :account-names="accountNames"
    @close="matchesOpen = false"
    @changed="onMatchesChanged"
  />

  <ScheduleFormModal
    :open="modalOpen"
    :accounts="accounts"
    :categories="categories"
    :schedule="edited"
    @close="modalOpen = false"
    @changed="onChanged"
  />
</template>

<style scoped>
.banner {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  padding: 12px 16px;
  border-radius: 14px;
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
  font-size: 14px;
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

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
  align-items: start;
  gap: 16px;
}

.card-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

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
  align-items: center;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid var(--color-divider);
}

.list > li:last-child {
  border-bottom: 0;
}

.row-button {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 12px;
  margin: -6px -8px;
  padding: 6px 8px;
  border: 0;
  border-radius: var(--radius-nav);
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.row-button:hover {
  background: var(--color-page);
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

.amount {
  white-space: nowrap;
}

.amount.positive {
  color: var(--color-positive);
}
</style>
