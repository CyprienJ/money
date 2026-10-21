<script setup lang="ts">
import { ref, watch } from "vue";
import { commands, type TransferProposal, type TransferSide } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate } from "../utils/format";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  close: [];
  /** Emitted after each link, dismissal or detection, with the proposals left. */
  changed: [remaining: number];
}>();

const proposals = ref<TransferProposal[]>([]);
const busy = ref(false);
const error = ref("");
const notice = ref("");

async function load() {
  proposals.value = await unwrap(commands.listTransferProposals());
  emit("changed", proposals.value.length);
}

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    error.value = "";
    notice.value = "";
    await run(load);
  },
  { immediate: true },
);

async function run(action: () => Promise<unknown>) {
  busy.value = true;
  error.value = "";
  try {
    await action();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}

async function link(proposal: TransferProposal) {
  notice.value = "";
  await run(async () => {
    await unwrap(commands.linkTransfer(proposal.debit.operationId, proposal.credit.operationId));
    await load();
  });
}

async function dismiss(proposal: TransferProposal) {
  notice.value = "";
  await run(async () => {
    await unwrap(commands.dismissTransfer(proposal.debit.operationId, proposal.credit.operationId));
    await load();
  });
}

async function detect() {
  await run(async () => {
    const count = await unwrap(commands.detectTransfers());
    notice.value =
      count > 0
        ? `${count} virement${count > 1 ? "s" : ""} interne${count > 1 ? "s" : ""} lié${count > 1 ? "s" : ""} automatiquement.`
        : "Aucun nouveau virement interne certain dans l'historique.";
    await load();
  });
}

function sideText(side: TransferSide): string {
  return `${formatDate(side.date)} · ${formatAmount(side.amountCents, { signed: true })}`;
}
</script>

<template>
  <BaseModal :open="open" title="Virements internes à valider" @close="emit('close')">
    <div class="content">
      <p class="hint">
        Un débit et un crédit de même montant sur deux comptes, à 3 jours près, sont sans doute un
        virement interne. Ceux qui ont plusieurs pendants possibles attendent ton choix ; les liés
        passent en Virement interne, exclus des statistiques.
      </p>
      <p v-if="notice" class="notice">{{ notice }}</p>

      <ul v-if="proposals.length > 0" class="proposals">
        <li v-for="proposal in proposals" :key="`${proposal.debit.operationId}-${proposal.credit.operationId}`">
          <div class="sides">
            <span class="side">
              <span class="account">{{ proposal.debit.accountName }}</span>
              <strong>{{ proposal.debit.payeeName }}</strong>
              <span>{{ sideText(proposal.debit) }}</span>
            </span>
            <span class="arrow" aria-label="vers">→</span>
            <span class="side">
              <span class="account">{{ proposal.credit.accountName }}</span>
              <strong>{{ proposal.credit.payeeName }}</strong>
              <span class="positive">{{ sideText(proposal.credit) }}</span>
            </span>
          </div>
          <div class="buttons">
            <button type="button" class="btn btn-secondary" :disabled="busy" @click="dismiss(proposal)">Ignorer</button>
            <button type="button" class="btn btn-primary" :disabled="busy" @click="link(proposal)">Lier</button>
          </div>
        </li>
      </ul>
      <p v-else class="hint">Aucun virement interne à valider.</p>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button type="button" class="btn btn-link detect" :disabled="busy" @click="detect">
          Rechercher dans tout l'historique
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Fermer</button>
      </footer>
    </div>
  </BaseModal>
</template>

<style scoped>
.content {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.hint {
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

.proposals {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 50vh;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.proposals li {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 10px 16px;
  padding: 10px 12px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-nav);
}

.sides {
  display: flex;
  flex: 1 1 340px;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.side {
  display: flex;
  flex: 1 1 0;
  flex-direction: column;
  min-width: 0;
  font-size: 13px;
  overflow-wrap: anywhere;
}

.account {
  font-size: 11px;
  font-weight: 700;
  color: var(--color-text-muted);
  text-transform: uppercase;
}

.positive {
  color: var(--color-positive);
}

.arrow {
  color: var(--color-text-muted);
}

.buttons {
  display: flex;
  gap: 8px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
}

.detect {
  margin-right: auto;
}
</style>
