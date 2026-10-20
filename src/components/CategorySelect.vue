<script setup lang="ts">
import { computed } from "vue";
import type { Category } from "../bindings";
import { categoryTree } from "../utils/categories";

const props = defineProps<{
  categories: Category[];
  disabled?: boolean;
}>();

/** Selected category id, `null` for "to categorize". */
const model = defineModel<number | null>({ required: true });

const tree = computed(() => categoryTree(props.categories));
</script>

<template>
  <select v-model="model" :disabled="disabled">
    <option :value="null">À catégoriser</option>
    <template v-for="{ parent, children } in tree" :key="parent.id">
      <option :value="parent.id">{{ parent.name }}</option>
      <option v-for="child in children" :key="child.id" :value="child.id">
        &nbsp;&nbsp;&nbsp;{{ child.name }}
      </option>
    </template>
  </select>
</template>
