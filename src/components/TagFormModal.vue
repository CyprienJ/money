<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { commands, type Tag } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { operationCount } from "../utils/format";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
  tag: Tag | null;
}>();

const emit = defineEmits<{
  close: [];
  /** `message` sums up what was done. */
  changed: [message: string];
}>();

const name = ref("");
const submitted = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const error = ref("");

const nameError = computed(() => (name.value.trim() === "" ? "Le nom du tag est obligatoire." : ""));

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    name.value = props.tag?.name ?? "";
    submitted.value = false;
    confirmingDelete.value = false;
    error.value = "";
  },
  { immediate: true },
);

async function run(action: () => Promise<string>) {
  saving.value = true;
  error.value = "";
  try {
    emit("changed", await action());
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function save() {
  submitted.value = true;
  const tag = props.tag;
  if (!tag || nameError.value) return;
  await run(async () => {
    const saved = await unwrap(commands.renameTag(tag.id, name.value));
    return `Tag renommé en « ${saved.name} ».`;
  });
}

async function remove() {
  const tag = props.tag;
  if (!tag) return;
  if (!confirmingDelete.value) {
    confirmingDelete.value = true;
    return;
  }
  await run(async () => {
    await unwrap(commands.deleteTag(tag.id));
    return `Tag « ${tag.name} » supprimé.`;
  });
}
</script>

<template>
  <BaseModal :open="open" title="Modifier le tag" @close="emit('close')">
    <form v-if="tag" class="form" novalidate @submit.prevent="save">
      <label class="field">
        Nom
        <input v-model="name" type="text" autocomplete="off" />
        <span v-if="submitted && nameError" class="field-error">{{ nameError }}</span>
        <span class="field-hint">
          {{ tag.operationCount > 0 ? `Sur ${operationCount(tag.operationCount)}.` : "Sur aucune opération." }}
        </span>
      </label>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button type="button" class="btn btn-link delete" :disabled="saving" @click="remove">
          {{
            confirmingDelete
              ? tag.operationCount > 0
                ? `Confirmer : retirer de ${operationCount(tag.operationCount)}`
                : "Confirmer la suppression"
              : "Supprimer"
          }}
        </button>
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

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}

.delete {
  margin-right: auto;
  color: var(--cat-restaurants-fg);
}
</style>
