<script setup lang="ts">
import { computed } from "vue";
import type { Category } from "../bindings";
import { categoryTree } from "../utils/categories";

const props = defineProps<{
  categories: Category[];
  /** Accessible name, e.g. "Catégorie de Carrefour Mkt". */
  label: string;
}>();

/** Selected category id, `null` for "to categorize". */
const model = defineModel<number | null>({ required: true });

const tree = computed(() => categoryTree(props.categories));
const selected = computed(() => props.categories.find((c) => c.id === model.value) ?? null);
const style = computed(() =>
  selected.value
    ? {
        backgroundColor: `var(--cat-${selected.value.color}-bg)`,
        color: `var(--cat-${selected.value.color}-fg)`,
      }
    : undefined,
);
</script>

<template>
  <!-- A select styled as a category pill: click to change the category. -->
  <select
    v-model="model"
    class="picker"
    :class="{ pending: selected === null }"
    :style="style"
    :aria-label="label"
    @click.stop
    @keydown.enter.stop
  >
    <option :value="null">À catégoriser</option>
    <template v-for="{ parent, children } in tree" :key="parent.id">
      <option :value="parent.id">{{ parent.name }}</option>
      <option v-for="child in children" :key="child.id" :value="child.id">
        &nbsp;&nbsp;&nbsp;{{ child.name }}
      </option>
    </template>
  </select>
</template>

<style scoped>
.picker {
  /* Width of the selected option where supported, instead of the longest one. */
  field-sizing: content;
  max-width: 100%;
  padding: 4px 22px 4px 10px;
  border: 0;
  border-radius: var(--radius-pill);
  background-repeat: no-repeat;
  font-size: 12px;
  font-weight: 700;
  appearance: none;
  cursor: pointer;
  /* Small chevron drawn with the current text color. */
  background-image: linear-gradient(45deg, transparent 50%, currentColor 50%),
    linear-gradient(135deg, currentColor 50%, transparent 50%);
  background-position:
    calc(100% - 12px) 52%,
    calc(100% - 8px) 52%;
  background-size:
    4px 4px,
    4px 4px;
}

.picker:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}

.picker.pending {
  padding: 2.5px 20.5px 2.5px 8.5px;
  border: 1.5px dashed var(--cat-groceries);
  background-color: var(--color-card);
  color: var(--cat-groceries-fg);
}

.picker option {
  background: var(--color-card);
  color: var(--color-text);
  font-weight: 500;
}
</style>
