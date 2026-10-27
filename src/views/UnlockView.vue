<script setup lang="ts">
import { ref } from "vue";
import { commands } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";

const emit = defineEmits<{ unlocked: [] }>();

const password = ref("");
const busy = ref(false);
const error = ref("");

async function unlock() {
  if (!password.value) return;
  error.value = "";
  busy.value = true;
  try {
    await unwrap(commands.unlock(password.value));
    password.value = "";
    emit("unlocked");
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <main class="unlock">
    <form class="card" @submit.prevent="unlock">
      <svg width="40" height="40" viewBox="0 0 32 32" aria-hidden="true">
        <circle cx="16" cy="16" r="15" fill="var(--color-accent)" />
        <rect x="10" y="14" width="12" height="9" rx="2" fill="var(--sidebar-bg)" />
        <path d="M12.5 14v-2.5a3.5 3.5 0 0 1 7 0V14" fill="none" stroke="var(--sidebar-bg)" stroke-width="2.2" />
      </svg>
      <h1>Données chiffrées</h1>
      <p class="muted">Saisis ton mot de passe pour ouvrir tes finances.</p>
      <label class="field">
        Mot de passe
        <input v-model="password" type="password" autocomplete="current-password" autofocus />
      </label>
      <p v-if="error" class="error-banner">{{ error }}</p>
      <button type="submit" class="btn btn-primary" :disabled="busy || !password">
        {{ busy ? "Ouverture…" : "Déverrouiller" }}
      </button>
    </form>
  </main>
</template>

<style scoped>
.unlock {
  display: grid;
  place-items: center;
  min-height: 100vh;
  padding: 24px;
  box-sizing: border-box;
}

.card {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 14px;
  width: min(380px, 100%);
}

h1 {
  margin: 0;
  font-size: 22px;
  font-weight: 800;
}

.muted {
  margin: 0;
  font-size: 14px;
  color: var(--color-text-muted);
}
</style>
