<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { commands, type Category, type Rule } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { operationCount } from "../utils/format";
import BaseModal from "./BaseModal.vue";
import CategorySelect from "./CategorySelect.vue";

const props = defineProps<{
  open: boolean;
  categories: Category[];
  /** Rule to edit; `null` to create one. */
  rule: Rule | null;
  /** Prefilled text and category when creating a rule from an operation. */
  initialPattern?: string;
  initialCategoryId?: number | null;
}>();

const emit = defineEmits<{
  close: [];
  /** `message` sums up what was done, e.g. the operations categorized. */
  changed: [message: string];
}>();

const form = reactive({
  pattern: "",
  categoryId: null as number | null,
  applyNow: true,
});
/** Operations to categorize containing the text; `null` while unknown. */
const pendingCount = ref<number | null>(null);
const submitted = ref(false);
const saving = ref(false);
const deleting = ref(false);
const error = ref("");

const isEdit = computed(() => props.rule !== null);
const patternError = computed(() =>
  form.pattern.trim() === "" ? "Indique le texte que le libellé doit contenir." : "",
);
const categoryError = computed(() => (form.categoryId === null ? "Choisis une catégorie." : ""));

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    form.pattern = props.rule?.pattern ?? props.initialPattern ?? "";
    form.categoryId = props.rule?.categoryId ?? props.initialCategoryId ?? null;
    form.applyNow = true;
    submitted.value = false;
    deleting.value = false;
    error.value = "";
  },
  { immediate: true },
);

/** Counts the operations the text matches, a moment after typing stops. */
let countTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => [props.open, form.pattern] as const,
  ([open, pattern]) => {
    clearTimeout(countTimer);
    pendingCount.value = null;
    if (!open || pattern.trim() === "") return;
    countTimer = setTimeout(async () => {
      try {
        pendingCount.value = await unwrap(commands.countRuleMatches(pattern));
      } catch {
        // The count is a hint only.
      }
    }, 250);
  },
  { immediate: true },
);

async function save() {
  submitted.value = true;
  error.value = "";
  if (patternError.value || form.categoryId === null) return;

  saving.value = true;
  try {
    const input = { pattern: form.pattern, categoryId: form.categoryId };
    const saved = props.rule
      ? await unwrap(commands.updateRule(props.rule.id, input))
      : await unwrap(commands.createRule(input));
    let message = isEdit.value ? "Règle enregistrée." : "Règle créée.";
    if (form.applyNow && saved.pendingCount > 0) {
      const count = await unwrap(commands.applyRule(saved.id));
      message += ` ${operationCount(count)} classée${count > 1 ? "s" : ""}.`;
    }
    emit("changed", message);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!props.rule) return;
  if (!deleting.value) {
    deleting.value = true;
    return;
  }
  saving.value = true;
  error.value = "";
  try {
    await unwrap(commands.deleteRule(props.rule.id));
    emit("changed", "Règle supprimée.");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" :title="isEdit ? 'Modifier la règle' : 'Nouvelle règle'" @close="emit('close')">
    <form class="form" novalidate @submit.prevent="save">
      <label class="field">
        Le libellé contient
        <input v-model="form.pattern" type="text" autocomplete="off" placeholder="Intérêts nets" />
        <span class="field-hint">Sans tenir compte des majuscules ni des accents.</span>
        <span v-if="submitted && patternError" class="field-error">{{ patternError }}</span>
      </label>

      <label class="field">
        Catégorie
        <CategorySelect v-model="form.categoryId" :categories="categories" />
        <span v-if="submitted && categoryError" class="field-error">{{ categoryError }}</span>
      </label>

      <label v-if="pendingCount !== null && pendingCount > 0" class="check">
        <input v-model="form.applyNow" type="checkbox" />
        Classer aussi {{ pendingCount > 1 ? `les ${operationCount(pendingCount)}` : "l'opération" }}
        à catégoriser qui {{ pendingCount > 1 ? "contiennent" : "contient" }} ce texte
      </label>
      <p v-else-if="pendingCount === 0" class="note">
        Aucune opération à catégoriser ne contient ce texte : la règle servira aux prochains imports.
      </p>

      <p class="note">
        La règle s'applique aux prochains imports, avant la reprise de catégorie. Les opérations déjà
        catégorisées ne changent pas.
      </p>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button v-if="rule" type="button" class="btn btn-link delete" :disabled="saving" @click="remove">
          {{ deleting ? "Confirmer la suppression" : "Supprimer" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving || deleting">
          {{ isEdit ? "Enregistrer" : "Créer la règle" }}
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

.note {
  margin: 0;
  font-size: 12px;
  color: var(--color-text-muted);
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
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
