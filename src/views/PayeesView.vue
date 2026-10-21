<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { commands, type Payee } from "../bindings";
import PageHeader from "../components/PageHeader.vue";
import PayeeFormModal from "../components/PayeeFormModal.vue";
import PayeeMergeModal from "../components/PayeeMergeModal.vue";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate, operationCount } from "../utils/format";
import { labelSummary, matchesPayee, sortPayees } from "../utils/payees";

const payees = ref<Payee[]>([]);
const loaded = ref(false);
const error = ref("");
const notice = ref("");
const search = ref("");
/** Payees checked for a merge. */
const selected = ref(new Set<number>());

const editedPayee = ref<Payee | null>(null);
const editOpen = ref(false);
const mergeOpen = ref(false);

const visible = computed(() => sortPayees(payees.value).filter((p) => matchesPayee(p, search.value)));
const selectedPayees = computed(() => payees.value.filter((p) => selected.value.has(p.id)));

async function load() {
  error.value = "";
  try {
    payees.value = await unwrap(commands.listPayees());
    const ids = new Set(payees.value.map((p) => p.id));
    selected.value = new Set([...selected.value].filter((id) => ids.has(id)));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

function toggle(id: number) {
  const next = new Set(selected.value);
  if (!next.delete(id)) next.add(id);
  selected.value = next;
}

function openEdit(payee: Payee) {
  editedPayee.value = payee;
  editOpen.value = true;
}

async function onChanged(message: string) {
  editOpen.value = false;
  mergeOpen.value = false;
  selected.value = new Set();
  notice.value = message;
  await load();
}

onMounted(load);
</script>

<template>
  <PageHeader title="Tiers" subtitle="Patrimoine">
    <template #actions>
      <span class="field search">
        <input v-model="search" type="search" placeholder="Rechercher un tiers…" aria-label="Rechercher un tiers" />
      </span>
      <button
        type="button"
        class="btn btn-primary"
        :disabled="selectedPayees.length < 2"
        @click="mergeOpen = true"
      >
        Fusionner{{ selectedPayees.length >= 2 ? ` ${selectedPayees.length} tiers` : "" }}
      </button>
    </template>
  </PageHeader>

  <p v-if="error" class="error-banner">{{ error }}</p>

  <section v-else-if="loaded" class="card">
    <p class="hint">
      Chaque libellé d'opération a son tiers. Coche plusieurs tiers pour les fusionner, ou clique sur un tiers
      pour le renommer ou en détacher un libellé.
    </p>
    <p v-if="notice" class="notice">{{ notice }}</p>

    <p v-if="payees.length === 0" class="muted">Aucun tiers : importe un relevé ou saisis une opération.</p>
    <p v-else-if="visible.length === 0" class="muted">Aucun tiers ne correspond à « {{ search }} ».</p>
    <ul v-else class="payees">
      <li v-for="payee in visible" :key="payee.id" class="payee" :class="{ checked: selected.has(payee.id) }">
        <input
          type="checkbox"
          :checked="selected.has(payee.id)"
          :aria-label="`Sélectionner ${payee.name}`"
          @change="toggle(payee.id)"
        />
        <button type="button" class="open" @click="openEdit(payee)">
          <span class="details">
            <span class="name">{{ payee.name }}</span>
            <span v-if="labelSummary(payee)" class="labels">{{ labelSummary(payee) }}</span>
          </span>
          <span class="meta">
            {{ operationCount(payee.operationCount) }}
            <template v-if="payee.lastDate"> · dernière le {{ formatDate(payee.lastDate) }}</template>
          </span>
          <strong v-if="payee.totalCents !== null" class="amount" :class="{ positive: payee.totalCents > 0 }">
            {{ formatAmount(payee.totalCents, { signed: true }) }}
          </strong>
          <span v-else class="amount transfer">Virement interne</span>
        </button>
      </li>
    </ul>
  </section>

  <PayeeFormModal :open="editOpen" :payee="editedPayee" @close="editOpen = false" @changed="onChanged" />
  <PayeeMergeModal :open="mergeOpen" :payees="selectedPayees" @close="mergeOpen = false" @changed="onChanged" />
</template>

<style scoped>
.search input {
  width: 240px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.hint,
.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.notice {
  margin: 0;
  padding: 8px 12px;
  border-radius: var(--radius-nav);
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
  font-size: 13px;
  font-weight: 600;
}

.payees {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 14px;
}

.payee {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 8px;
  border-radius: var(--radius-nav);
}

.payee:hover,
.payee.checked {
  background: var(--color-page);
}

.payee input {
  width: 16px;
  height: 16px;
  margin: 0;
  accent-color: var(--color-primary);
  cursor: pointer;
}

.open {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 16px;
  min-width: 0;
  padding: 10px 0;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.details {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.name {
  font-weight: 700;
}

.labels,
.meta {
  font-size: 12px;
  color: var(--color-text-muted);
}

.labels {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  white-space: nowrap;
}

.amount {
  min-width: 110px;
  text-align: right;
  white-space: nowrap;
}

.amount.transfer {
  font-size: 12px;
  color: var(--color-text-muted);
}

.amount.positive {
  color: var(--color-positive);
}
</style>
