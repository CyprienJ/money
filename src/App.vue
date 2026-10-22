<script setup lang="ts">
import { onMounted, ref } from "vue";
import { commands } from "./bindings";
import AppSidebar from "./components/AppSidebar.vue";
import { errorMessage, unwrap } from "./utils/commands";
import UnlockView from "./views/UnlockView.vue";

/** `null` while the status is read: nothing is shown before the data is open. */
const locked = ref<boolean | null>(null);
const error = ref("");

onMounted(async () => {
  try {
    const status = await unwrap(commands.lockStatus());
    locked.value = !status.unlocked;
  } catch (e) {
    error.value = errorMessage(e);
  }
});
</script>

<template>
  <p v-if="error" class="error-banner">{{ error }}</p>
  <UnlockView v-else-if="locked" @unlocked="locked = false" />
  <div v-else-if="locked === false" class="shell">
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
</style>
