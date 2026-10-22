<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { commands, type Security } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
  /** Security to edit; `null` to create one. */
  security: Security | null;
}>();

const emit = defineEmits<{
  close: [];
  /** The created or updated security, `null` after a deletion. */
  changed: [security: Security | null];
}>();

const form = reactive({ name: "", isin: "" });
const submitted = ref(false);
const saving = ref(false);
const confirmingDelete = ref(false);
const error = ref("");

const nameError = computed(() => (form.name.trim() === "" ? "Le nom est obligatoire." : ""));

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    form.name = props.security?.name ?? "";
    form.isin = props.security?.isin ?? "";
    submitted.value = false;
    confirmingDelete.value = false;
    error.value = "";
  },
  { immediate: true },
);

async function save() {
  submitted.value = true;
  error.value = "";
  if (nameError.value) return;
  const input = { name: form.name, isin: form.isin.trim() || null };
  saving.value = true;
  try {
    const security = props.security
      ? await unwrap(commands.updateSecurity(props.security.id, input))
      : await unwrap(commands.createSecurity(input));
    emit("changed", security);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!props.security) return;
  if (!confirmingDelete.value) {
    confirmingDelete.value = true;
    return;
  }
  saving.value = true;
  try {
    await unwrap(commands.deleteSecurity(props.security.id));
    emit("changed", null);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" :title="security ? 'Modifier le titre' : 'Nouveau titre'" @close="emit('close')">
    <form class="form" novalidate @submit.prevent="save">
      <label class="field">
        Nom
        <input v-model="form.name" type="text" autocomplete="off" placeholder="iShares Core MSCI World" />
        <span v-if="submitted && nameError" class="field-error">{{ nameError }}</span>
      </label>
      <label class="field">
        Code ISIN (facultatif)
        <input v-model="form.isin" type="text" autocomplete="off" placeholder="IE00B4L5Y983" />
        <span class="field-hint">12 caractères, sur l'avis d'opéré ou la fiche du titre.</span>
      </label>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button
          v-if="security && security.transactionCount === 0"
          type="button"
          class="btn btn-link delete"
          :disabled="saving"
          @click="remove"
        >
          {{ confirmingDelete ? "Confirmer la suppression" : "Supprimer" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">
          {{ security ? "Enregistrer" : "Créer le titre" }}
        </button>
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
