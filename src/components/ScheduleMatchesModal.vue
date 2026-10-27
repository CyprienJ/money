<script setup lang="ts">
import { ref, watch } from "vue";
import { commands, type Account, type ScheduleMatchProposal } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate } from "../utils/format";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
  accounts: Map<number, Account>;
}>();

const emit = defineEmits<{
  close: [];
  /** Emitted after each match, dismissal or detection, with the proposals left. */
  changed: [remaining: number];
}>();

const proposals = ref<ScheduleMatchProposal[]>([]);
const busy = ref(false);
const error = ref("");
const notice = ref("");

async function load() {
  proposals.value = await unwrap(commands.listScheduleMatchProposals());
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

async function match(proposal: ScheduleMatchProposal) {
  notice.value = "";
  const { occurrence, operation } = proposal;
  await run(async () => {
    await unwrap(
      commands.matchScheduleOccurrence(occurrence.scheduledOperationId, occurrence.date, operation.operationId),
    );
    await load();
  });
}

async function dismiss(proposal: ScheduleMatchProposal) {
  notice.value = "";
  const { occurrence, operation } = proposal;
  await run(async () => {
    await unwrap(
      commands.dismissScheduleMatch(occurrence.scheduledOperationId, occurrence.date, operation.operationId),
    );
    await load();
  });
}

async function detect() {
  await run(async () => {
    const count = await unwrap(commands.detectScheduleMatches());
    notice.value =
      count > 0
        ? `${count} échéance${count > 1 ? "s" : ""} réalisée${count > 1 ? "s" : ""} automatiquement.`
        : "Aucune nouvelle association certaine dans l'historique.";
    await load();
  });
}

function proposalKey(proposal: ScheduleMatchProposal): string {
  const { occurrence, operation } = proposal;
  return `${occurrence.scheduledOperationId}-${occurrence.date}-${operation.operationId}`;
}
</script>

<template>
  <BaseModal :open="open" title="Échéances à valider" @close="emit('close')">
    <div class="content">
      <p class="hint">
        Une opération du même compte et du même sens, à 10 % près du montant et à 5 jours près de la
        date, réalise sans doute une échéance. Celles qui ont plusieurs opérations possibles
        attendent ton choix.
      </p>
      <p v-if="notice" class="notice">{{ notice }}</p>

      <ul v-if="proposals.length > 0" class="proposals">
        <li v-for="proposal in proposals" :key="proposalKey(proposal)">
          <div class="sides">
            <span class="side">
              <span class="caption">Échéance · {{ accounts.get(proposal.occurrence.accountId)?.name }}</span>
              <strong>{{ proposal.occurrence.label }}</strong>
              <span>
                {{ formatDate(proposal.occurrence.date) }} ·
                {{
                  formatAmount(proposal.occurrence.amountCents, {
                    signed: true,
                    currency: accounts.get(proposal.occurrence.accountId)?.currency,
                  })
                }}
              </span>
            </span>
            <span class="arrow" aria-hidden="true">←</span>
            <span class="side">
              <span class="caption">Opération</span>
              <strong>{{ proposal.operation.payeeName }}</strong>
              <span>
                {{ formatDate(proposal.operation.date) }} ·
                {{
                  formatAmount(proposal.operation.amountCents, {
                    signed: true,
                    currency: accounts.get(proposal.occurrence.accountId)?.currency,
                  })
                }}
              </span>
            </span>
          </div>
          <div class="buttons">
            <button type="button" class="btn btn-secondary" :disabled="busy" @click="dismiss(proposal)">Ignorer</button>
            <button type="button" class="btn btn-primary" :disabled="busy" @click="match(proposal)">Associer</button>
          </div>
        </li>
      </ul>
      <p v-else class="hint">Aucune échéance à valider.</p>

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

.caption {
  font-size: 11px;
  font-weight: 700;
  color: var(--color-text-muted);
  text-transform: uppercase;
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
