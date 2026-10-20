<script setup lang="ts">
defineProps<{
  title: string;
  /** Small line above the title (date, account, scope…). */
  subtitle?: string;
}>();
</script>

<template>
  <header class="page-header">
    <div>
      <div v-if="subtitle" class="subtitle">{{ subtitle }}</div>
      <h1>{{ title }}</h1>
    </div>
    <div v-if="$slots.actions" class="actions">
      <slot name="actions" />
    </div>
  </header>
</template>

<style scoped>
.page-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.subtitle {
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-muted);
}

h1 {
  margin: 2px 0 0;
  font-size: 30px;
  font-weight: 800;
  letter-spacing: -0.02em;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}
</style>
