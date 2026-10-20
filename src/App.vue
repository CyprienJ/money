<script setup lang="ts">
import AppSidebar from "./components/AppSidebar.vue";
</script>

<template>
  <div class="shell">
    <AppSidebar />
    <main class="content">
      <RouterView />
    </main>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  min-height: 100vh;
}

.content {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding: 32px 40px 48px;
}

/* Placeholder text of screens not built yet. */
.content :deep(.placeholder) {
  margin: 0;
  color: var(--color-text-muted);
}
</style>
