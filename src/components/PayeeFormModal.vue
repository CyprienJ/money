<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { commands, type Payee } from "../bindings";
import { CommandError, errorMessage, unwrap } from "../utils/commands";
import { displayLabel } from "../utils/format";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
  payee: Payee | null;
}>();

const emit = defineEmits<{
  close: [];
  /** `message` sums up what was done. */
  changed: [message: string];
}>();

const name = ref("");
const submitted = ref(false);
const saving = ref(false);
const error = ref("");
/** Payee already named like the typed name, offered as a merge target. */
const taken = ref<{ id: number; name: string } | null>(null);

const nameError = computed(() => (name.value.trim() === "" ? "Le nom du tiers est obligatoire." : ""));

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    name.value = props.payee?.name ?? "";
    submitted.value = false;
    error.value = "";
    taken.value = null;
  },
  { immediate: true },
);

async function save() {
  submitted.value = true;
  error.value = "";
  taken.value = null;
  if (!props.payee || nameError.value) return;

  saving.value = true;
  try {
    const saved = await unwrap(commands.renamePayee(props.payee.id, name.value));
    emit("changed", `Tiers renommé en « ${saved.name} ».`);
  } catch (e) {
    const detail = e instanceof CommandError ? e.appError.detail : null;
    if (detail?.domain === "payee" && detail.error.code === "nameTaken") {
      taken.value = { id: detail.error.params.payeeId, name: detail.error.params.name };
    }
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function mergeIntoTaken() {
  if (!props.payee || !taken.value) return;
  saving.value = true;
  try {
    const merged = await unwrap(commands.mergePayees(taken.value.id, [props.payee.id]));
    emit("changed", `« ${props.payee.name} » fusionné avec « ${merged.name} ».`);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function detach(label: string) {
  saving.value = true;
  error.value = "";
  try {
    const detached = await unwrap(commands.detachPayeeLabel(label));
    emit("changed", `« ${displayLabel(label)} » est maintenant le tiers « ${detached.name} ».`);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" title="Modifier le tiers" @close="emit('close')">
    <form v-if="payee" class="form" novalidate @submit.prevent="save">
      <label class="field">
        Nom
        <input v-model="name" type="text" autocomplete="off" />
        <span v-if="submitted && nameError" class="field-error">{{ nameError }}</span>
      </label>

      <div class="labels">
        <span class="labels-title">Libellés de ce tiers</span>
        <ul>
          <li v-for="label in payee.labels" :key="label">
            <span class="label">{{ label }}</span>
            <button
              v-if="payee.labels.length > 1"
              type="button"
              class="btn btn-link"
              :disabled="saving"
              @click="detach(label)"
            >
              Détacher
            </button>
          </li>
        </ul>
        <span class="field-hint">
          Les opérations de ces libellés, passées et à venir, sont rattachées à ce tiers. Un libellé
          détaché devient un tiers à part.
        </span>
      </div>

      <p v-if="error" class="error-banner">
        {{ error }}
        <button v-if="taken" type="button" class="btn btn-link merge" :disabled="saving" @click="mergeIntoTaken">
          Fusionner avec « {{ taken.name }} »
        </button>
      </p>

      <footer class="actions">
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">Enregistrer</button>
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

.labels {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.labels-title {
  font-size: 13px;
  font-weight: 700;
}

.labels ul {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.labels li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 10px;
  border-radius: var(--radius-nav);
  background: var(--color-page);
  font-size: 13px;
}

.label {
  font-weight: 600;
  overflow-wrap: anywhere;
}

.merge {
  display: block;
  margin-top: 4px;
  padding: 0;
}

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}
</style>
