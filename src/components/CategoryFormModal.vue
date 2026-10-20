<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { commands, type Category, type CategoryColor } from "../bindings";
import { CATEGORY_COLORS, categoryTree } from "../utils/categories";
import { errorMessage, unwrap } from "../utils/commands";
import { operationCount } from "../utils/format";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  open: boolean;
  categories: Category[];
  /** Category to edit; `null` to create one. */
  category: Category | null;
  /** Parent preselected when creating a subcategory. */
  parentId?: number | null;
}>();

const emit = defineEmits<{
  close: [];
  changed: [];
}>();

const form = reactive({
  name: "",
  parentId: null as number | null,
  color: "neutral" as CategoryColor,
  isIncome: false,
});
const submitted = ref(false);
const saving = ref(false);
const deleting = ref(false);
/** Category receiving the operations of the deleted one; `null` for "to categorize". */
const reassignTo = ref<number | null>(null);
const error = ref("");

const isEdit = computed(() => props.category !== null);
const hasChildren = computed(
  () => props.category !== null && props.categories.some((c) => c.parentId === props.category?.id),
);
const nameError = computed(() => (form.name.trim() === "" ? "Le nom est obligatoire." : ""));

/** Top-level categories that can receive a subcategory. */
const parents = computed(() =>
  props.categories
    .filter((c) => c.parentId === null && !c.isInternalTransfer && c.id !== props.category?.id)
    .sort((a, b) => a.name.localeCompare(b.name, "fr")),
);
const parent = computed(() => props.categories.find((c) => c.id === form.parentId) ?? null);

/** Categories that can take over the operations of the deleted one. */
const reassignTree = computed(() =>
  categoryTree(props.categories.filter((c) => c.id !== props.category?.id)),
);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const category = props.category;
    form.name = category?.name ?? "";
    form.parentId = category ? category.parentId : (props.parentId ?? null);
    form.color = category?.color ?? "neutral";
    form.isIncome = category?.isIncome ?? false;
    submitted.value = false;
    deleting.value = false;
    reassignTo.value = null;
    error.value = "";
  },
  { immediate: true },
);

async function save() {
  submitted.value = true;
  error.value = "";
  if (nameError.value) return;

  saving.value = true;
  try {
    const input = {
      name: form.name,
      parentId: form.parentId,
      color: form.color,
      isIncome: form.isIncome,
    };
    if (props.category) {
      await unwrap(commands.updateCategory(props.category.id, input));
    } else {
      await unwrap(commands.createCategory(input));
    }
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!props.category) return;
  if (!deleting.value) {
    deleting.value = true;
    return;
  }
  saving.value = true;
  error.value = "";
  try {
    await unwrap(commands.deleteCategory(props.category.id, reassignTo.value));
    emit("changed");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <BaseModal
    :open="open"
    :title="isEdit ? 'Modifier la catégorie' : form.parentId === null ? 'Nouvelle catégorie' : 'Nouvelle sous-catégorie'"
    @close="emit('close')"
  >
    <form class="form" novalidate @submit.prevent="save">
      <label class="field">
        Nom
        <input v-model="form.name" type="text" autocomplete="off" placeholder="Animaux" />
        <span v-if="submitted && nameError" class="field-error">{{ nameError }}</span>
      </label>

      <label class="field">
        Catégorie principale
        <select v-model="form.parentId" :disabled="hasChildren">
          <option :value="null">Aucune (catégorie principale)</option>
          <option v-for="p in parents" :key="p.id" :value="p.id">{{ p.name }}</option>
        </select>
        <span v-if="hasChildren" class="field-hint">
          Elle a des sous-catégories : elle reste une catégorie principale.
        </span>
      </label>

      <template v-if="form.parentId === null">
        <fieldset class="field colors">
          <legend>Couleur</legend>
          <label v-for="c in CATEGORY_COLORS" :key="c.value" class="swatch" :title="c.label">
            <input v-model="form.color" type="radio" name="color" :value="c.value" :aria-label="c.label" />
            <span class="dot" :style="{ background: `var(--cat-${c.value})` }" />
          </label>
        </fieldset>
        <label class="check">
          <input v-model="form.isIncome" type="checkbox" />
          Catégorie de revenus
        </label>
      </template>
      <p v-else class="field-hint inherited">
        Couleur et type hérités de « {{ parent?.name }} ».
      </p>

      <div v-if="deleting && category" class="delete-box">
        <template v-if="category.operationCount > 0">
          <label class="field">
            {{
              category.operationCount === 1
                ? "Son opération passe à"
                : `Ses ${operationCount(category.operationCount)} passent à`
            }}
            <select v-model="reassignTo">
              <option :value="null">À catégoriser</option>
              <template v-for="{ parent: p, children } in reassignTree" :key="p.id">
                <option :value="p.id">{{ p.name }}</option>
                <option v-for="child in children" :key="child.id" :value="child.id">
                  &nbsp;&nbsp;&nbsp;{{ child.name }}
                </option>
              </template>
            </select>
          </label>
        </template>
        <span v-else class="field-hint">Aucune opération dans cette catégorie.</span>
      </div>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button
          v-if="category"
          type="button"
          class="btn btn-link delete"
          :disabled="saving || hasChildren"
          :title="hasChildren ? 'Supprime ou déplace d\'abord ses sous-catégories.' : undefined"
          @click="remove"
        >
          {{ deleting ? "Confirmer la suppression" : "Supprimer" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving || deleting">
          {{ isEdit ? "Enregistrer" : "Créer" }}
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

.colors {
  flex-direction: row;
  flex-wrap: wrap;
  gap: 10px;
  margin: 0;
  padding: 0;
  border: 0;
}

.colors legend {
  margin-bottom: 8px;
  padding: 0;
}

.swatch {
  position: relative;
  cursor: pointer;
}

.swatch input {
  position: absolute;
  opacity: 0;
}

.dot {
  display: block;
  width: 28px;
  height: 28px;
  border-radius: 14px;
  box-shadow: inset 0 0 0 2px rgba(255, 255, 255, 0.9);
  outline: 2px solid transparent;
  outline-offset: 1px;
}

.swatch input:checked + .dot {
  outline-color: var(--color-primary);
}

.swatch input:focus-visible + .dot {
  outline-color: var(--color-accent);
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.inherited {
  margin: -4px 0 0;
  color: var(--color-text-muted);
}

.delete-box {
  padding: 14px;
  border-radius: var(--radius-nav);
  background: var(--cat-restaurants-bg);
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
