<script setup lang="ts">
import { computed } from "vue";
import type { CategoryColor } from "../bindings";

const props = withDefaults(
  defineProps<{
    label: string;
    color?: CategoryColor;
    /** Dashed "to categorize" style. */
    pending?: boolean;
  }>(),
  { color: "neutral", pending: false },
);

const style = computed(() =>
  props.pending
    ? undefined
    : {
        background: `var(--cat-${props.color}-bg)`,
        color: `var(--cat-${props.color}-fg)`,
      },
);
</script>

<template>
  <span class="pill" :class="{ pending }" :style="style">{{ label }}</span>
</template>

<style scoped>
.pill {
  display: inline-block;
  padding: 4px 10px;
  border-radius: var(--radius-pill);
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
}

.pending {
  padding: 2.5px 8.5px;
  background: var(--color-card);
  color: var(--cat-groceries-fg);
  border: 1.5px dashed var(--cat-groceries);
}
</style>
