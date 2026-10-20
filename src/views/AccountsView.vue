<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { commands, type Account, type AccountSummary } from "../bindings";
import AccountBadge from "../components/AccountBadge.vue";
import AccountFormModal from "../components/AccountFormModal.vue";
import PageHeader from "../components/PageHeader.vue";
import { ACCOUNT_GROUPS, ACCOUNT_TYPES, valuationSubtitle } from "../utils/accounts";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount } from "../utils/format";

const summaries = ref<AccountSummary[]>([]);
const loaded = ref(false);
const error = ref("");
const showArchived = ref(false);

const modalOpen = ref(false);
const editedAccount = ref<Account | null>(null);

const groups = computed(() =>
  ACCOUNT_GROUPS.map((group) => {
    const items = summaries.value.filter((s) => group.types.includes(s.account.type));
    const balances = items.map((s) => s.balanceCents).filter((b): b is number => b !== null);
    return {
      title: group.title,
      items,
      totalCents: balances.length > 0 ? balances.reduce((sum, b) => sum + b, 0) : null,
    };
  }).filter((group) => group.items.length > 0),
);

async function load() {
  error.value = "";
  try {
    summaries.value = await unwrap(commands.listAccountSummaries(showArchived.value));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

function subtitle({ account, valuedOn }: AccountSummary): string {
  if (!ACCOUNT_TYPES[account.type].hasOperations) return valuationSubtitle(account.type, valuedOn);
  return [ACCOUNT_TYPES[account.type].label, account.institution].filter(Boolean).join(" · ");
}

function openCreate() {
  editedAccount.value = null;
  modalOpen.value = true;
}

function openEdit(account: Account) {
  editedAccount.value = account;
  modalOpen.value = true;
}

async function onSaved() {
  modalOpen.value = false;
  await load();
}

onMounted(load);
watch(showArchived, load);
</script>

<template>
  <PageHeader title="Comptes" subtitle="Patrimoine">
    <template #actions>
      <label class="toggle">
        <input v-model="showArchived" type="checkbox" />
        Afficher les comptes archivés
      </label>
      <button type="button" class="btn btn-primary" @click="openCreate">+ Ajouter un compte</button>
    </template>
  </PageHeader>

  <p v-if="error" class="error-banner">{{ error }}</p>

  <section v-else-if="loaded && groups.length === 0" class="card empty">
    <h2 class="card-title">Aucun compte</h2>
    <p>Crée ton premier compte pour commencer à importer tes relevés.</p>
    <div>
      <button type="button" class="btn btn-primary" @click="openCreate">+ Ajouter un compte</button>
    </div>
  </section>

  <div v-else class="groups">
    <section v-for="group in groups" :key="group.title" class="card">
      <div class="group-header">
        <h2 class="card-title">{{ group.title }}</h2>
        <span v-if="group.totalCents !== null" class="total">
          {{ formatAmount(group.totalCents) }}
        </span>
      </div>
      <ul class="accounts">
        <li v-for="summary in group.items" :key="summary.account.id">
          <button type="button" class="account" @click="openEdit(summary.account)">
            <AccountBadge :name="summary.account.name" :type="summary.account.type" />
            <span class="details">
              <span class="name">
                {{ summary.account.name }}
                <span v-if="summary.account.archived" class="archived">Archivé</span>
              </span>
              <span class="subtitle">{{ subtitle(summary) }}</span>
            </span>
            <strong
              v-if="summary.balanceCents !== null"
              class="amount"
              :class="{ negative: summary.balanceCents < 0 }"
            >
              {{ formatAmount(summary.balanceCents) }}
            </strong>
            <span v-else class="amount pending" title="Ajoute une valorisation depuis la fiche du compte">—</span>
          </button>
        </li>
      </ul>
    </section>
  </div>

  <AccountFormModal
    :open="modalOpen"
    :account="editedAccount"
    @close="modalOpen = false"
    @saved="onSaved"
    @valuations-changed="load"
  />
</template>

<style scoped>
.toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-muted);
  cursor: pointer;
}

.groups {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
  align-items: start;
  gap: 16px;
}

.group-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.total {
  font-size: 14px;
  font-weight: 700;
  color: var(--color-text-muted);
}

.accounts {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 14px;
}

.account {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  margin: 0 -8px;
  padding: 8px;
  box-sizing: content-box;
  border: 0;
  border-radius: var(--radius-nav);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.account:hover {
  background: var(--color-page);
}

.details {
  flex: 1;
  min-width: 0;
}

.name {
  display: block;
  font-weight: 700;
}

.archived {
  margin-left: 6px;
  padding: 1px 7px;
  border-radius: var(--radius-pill);
  background: var(--cat-neutral-bg);
  color: var(--cat-neutral-fg);
  font-size: 11px;
  font-weight: 700;
}

.subtitle {
  font-size: 12px;
  color: var(--color-text-muted);
}

.amount {
  white-space: nowrap;
}

.amount.negative {
  color: var(--cat-restaurants-fg);
}

.amount.pending {
  color: var(--color-text-muted);
}

.empty p {
  margin: 0;
  color: var(--color-text-muted);
}
</style>
