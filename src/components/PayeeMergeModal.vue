<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { commands, type Payee } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { operationCount } from "../utils/format";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
  /** Payees to merge, two at least. */
  payees: Payee[];
}>();

const emit = defineEmits<{
  close: [];
  /** `message` sums up what was done. */
  changed: [message: string];
}>();

/** Payee whose name is kept, or `null` for a new name. */
const targetId = ref<number | null>(null);
const otherName = ref("");
const submitted = ref(false);
const saving = ref(false);
const error = ref("");

const nameError = computed(() =>
  targetId.value === null && otherName.value.trim() === "" ? "Indique le nom du tiers." : "",
);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    // By default, the name of the payee with the most operations.
    const busiest = [...props.payees].sort((a, b) => b.operationCount - a.operationCount)[0];
    targetId.value = busiest?.id ?? null;
    otherName.value = "";
    submitted.value = false;
    error.value = "";
  },
  { immediate: true },
);

async function save() {
  submitted.value = true;
  error.value = "";
  if (nameError.value || props.payees.length < 2) return;

  // With a new name, the busiest payee is kept and renamed.
  const target =
    props.payees.find((p) => p.id === targetId.value) ??
    [...props.payees].sort((a, b) => b.operationCount - a.operationCount)[0];
  const sources = props.payees.filter((p) => p.id !== target.id).map((p) => p.id);
  saving.value = true;
  try {
    let merged = await unwrap(commands.mergePayees(target.id, sources));
    if (targetId.value === null) {
      merged = await unwrap(commands.renamePayee(target.id, otherName.value));
    }
    emit("changed", `${props.payees.length} tiers fusionnés en « ${merged.name} ».`);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" title="Fusionner des tiers" @close="emit('close')">
    <form class="form" novalidate @submit.prevent="save">
      <p class="note">
        Les libellés et les opérations des {{ payees.length }} tiers sont regroupés sous un seul nom. Les
        prochaines opérations de ces libellés y seront rattachées.
      </p>

      <fieldset class="choices">
        <legend>Nom du tiers</legend>
        <label v-for="payee in payees" :key="payee.id" class="choice">
          <input v-model="targetId" type="radio" name="target" :value="payee.id" />
          <span class="choice-name">{{ payee.name }}</span>
          <span class="choice-count">{{ operationCount(payee.operationCount) }}</span>
        </label>
        <label class="choice">
          <input v-model="targetId" type="radio" name="target" :value="null" />
          Autre nom
        </label>
        <input
          v-if="targetId === null"
          v-model="otherName"
          class="other"
          type="text"
          autocomplete="off"
          aria-label="Autre nom"
        />
        <span v-if="submitted && nameError" class="field-error">{{ nameError }}</span>
      </fieldset>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">Fusionner</button>
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

.note {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.choices {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 0;
  padding: 0;
  border: 0;
}

legend {
  margin-bottom: 8px;
  font-size: 13px;
  font-weight: 700;
}

.choice {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.choice-name {
  flex: 1;
}

.choice-count {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-muted);
}

.other {
  height: 42px;
  padding: 0 12px;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: var(--color-card);
  color: var(--color-text);
  font: inherit;
  font-size: 14px;
  font-weight: 500;
}

.other:focus {
  outline: 2px solid var(--color-accent);
  outline-offset: -1px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}
</style>
