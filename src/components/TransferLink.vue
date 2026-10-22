<script setup lang="ts">
import { ref, watch } from "vue";
import { commands, type TransferSide } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate } from "../utils/format";

const props = defineProps<{
  operationId: number;
}>();

const emit = defineEmits<{
  /** Emitted after a link or an unlink; a link puts the operation in the internal transfers. */
  changed: [linked: boolean];
}>();

const partner = ref<TransferSide | null>(null);
/** Operations to link by hand, once asked for. */
const candidates = ref<TransferSide[] | null>(null);
const busy = ref(false);
const error = ref("");

async function load() {
  error.value = "";
  candidates.value = null;
  try {
    partner.value = await unwrap(commands.transferPartner(props.operationId));
  } catch (e) {
    error.value = errorMessage(e);
  }
}

watch(() => props.operationId, load, { immediate: true });

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

async function showCandidates() {
  await run(async () => {
    candidates.value = await unwrap(commands.transferLinkCandidates(props.operationId));
  });
}

async function link(other: TransferSide) {
  if (await run(() => unwrap(commands.linkTransfer(props.operationId, other.operationId)))) {
    await load();
    emit("changed", true);
  }
}

async function unlink() {
  if (await run(() => unwrap(commands.unlinkTransfer(props.operationId)))) {
    await load();
    emit("changed", false);
  }
}

function sideText(side: TransferSide): string {
  return `${side.accountName} · ${formatDate(side.date)} · ${formatAmount(side.amountCents, { signed: true, currency: side.currency })}`;
}
</script>

<template>
  <div class="transfer">
    <div class="header">
      <span class="title">Virement interne</span>
      <button v-if="partner" type="button" class="btn btn-link" :disabled="busy" @click="unlink">Délier</button>
      <button
        v-else-if="candidates === null"
        type="button"
        class="btn btn-link"
        :disabled="busy"
        @click="showCandidates"
      >
        Lier à une opération…
      </button>
      <button v-else type="button" class="btn btn-link" @click="candidates = null">Fermer</button>
    </div>

    <p v-if="partner" class="linked">
      Lié à <strong>{{ partner.payeeName }}</strong> · {{ sideText(partner) }}
    </p>
    <template v-else-if="candidates !== null">
      <ul v-if="candidates.length > 0">
        <li v-for="candidate in candidates" :key="candidate.operationId">
          <span class="candidate">
            <strong>{{ candidate.payeeName }}</strong> · {{ sideText(candidate) }}
          </span>
          <button type="button" class="btn btn-link" :disabled="busy" @click="link(candidate)">Lier</button>
        </li>
      </ul>
      <span v-else class="field-hint">
        Aucune opération non liée de montant opposé sur un autre compte, à 15 jours près.
      </span>
    </template>
    <span v-else class="field-hint">
      Une opération liée à son pendant sur un autre compte passe en Virement interne.
    </span>
    <p v-if="error" class="error-banner">{{ error }}</p>
  </div>
</template>

<style scoped>
.transfer {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--color-text-muted);
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.title {
  font-size: 13px;
  font-weight: 600;
}

.linked,
li {
  margin: 0;
  padding: 8px 10px;
  border-radius: var(--radius-nav);
  background: var(--color-page);
  color: var(--color-text);
  font-size: 13px;
}

ul {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 4px 4px 4px 10px;
}

.candidate {
  min-width: 0;
  overflow-wrap: anywhere;
}
</style>
