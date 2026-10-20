<script setup lang="ts">
import type { AccountType } from "../bindings";
import { ACCOUNT_TYPES, accountInitials } from "../utils/accounts";

defineProps<{
  name: string;
  type: AccountType;
}>();
</script>

<template>
  <span
    class="badge"
    aria-hidden="true"
    :style="{
      background: `var(--cat-${ACCOUNT_TYPES[type].color}-bg)`,
      color: `var(--cat-${ACCOUNT_TYPES[type].color}-fg)`,
    }"
  >
    {{ accountInitials(name) }}
  </span>
</template>

<style scoped>
.badge {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border-radius: 10px;
  font-size: 12px;
  font-weight: 800;
}
</style>
