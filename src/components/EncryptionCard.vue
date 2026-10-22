<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { commands } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";

/** Shortest password, as on the backend. */
const MIN_LENGTH = 8;

type Action = "encrypt" | "change" | "decrypt";

const encrypted = ref<boolean | null>(null);
const action = ref<Action | null>(null);
const form = reactive({ current: "", password: "", confirmation: "", understood: false });
const busy = ref(false);
const error = ref("");

const passwordError = computed(() => {
  if (action.value === "decrypt") return "";
  if ([...form.password].length < MIN_LENGTH) return `Au moins ${MIN_LENGTH} caractères.`;
  if (form.password !== form.confirmation) return "Les deux saisies diffèrent.";
  return "";
});
const canSubmit = computed(() => {
  if (busy.value) return false;
  if (action.value !== "encrypt" && !form.current) return false;
  if (action.value === "encrypt" && !form.understood) return false;
  return !passwordError.value;
});

onMounted(async () => {
  try {
    encrypted.value = (await unwrap(commands.lockStatus())).encrypted;
  } catch (e) {
    error.value = errorMessage(e);
  }
});

function start(next: Action) {
  action.value = next;
  Object.assign(form, { current: "", password: "", confirmation: "", understood: false });
  error.value = "";
}

async function submit() {
  if (!canSubmit.value) return;
  error.value = "";
  busy.value = true;
  try {
    // The application restarts on success.
    if (action.value === "encrypt") await unwrap(commands.encryptData(form.password));
    if (action.value === "change") await unwrap(commands.changeDataPassword(form.current, form.password));
    if (action.value === "decrypt") await unwrap(commands.decryptData(form.current));
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section v-if="encrypted !== null" class="card">
    <h2 class="card-title">Chiffrement</h2>
    <p class="muted">
      <template v-if="encrypted">
        Le fichier de données est chiffré : un mot de passe est demandé à chaque ouverture.
      </template>
      <template v-else>
        Le fichier de données est en clair. Le chiffrer protège tes données si l'ordinateur ou une sauvegarde est
        perdu ou volé : un mot de passe sera demandé à chaque ouverture.
      </template>
      Les pièces jointes restent des fichiers en clair.
    </p>

    <div v-if="action === null" class="buttons">
      <button v-if="!encrypted" type="button" class="btn btn-secondary" @click="start('encrypt')">
        Chiffrer les données…
      </button>
      <template v-else>
        <button type="button" class="btn btn-secondary" @click="start('change')">Changer le mot de passe…</button>
        <button type="button" class="btn btn-link" @click="start('decrypt')">Retirer le chiffrement…</button>
      </template>
    </div>

    <form v-else class="form" @submit.prevent="submit">
      <label v-if="action !== 'encrypt'" class="field">
        Mot de passe actuel
        <input v-model="form.current" type="password" autocomplete="current-password" />
      </label>
      <template v-if="action !== 'decrypt'">
        <label class="field">
          {{ action === "change" ? "Nouveau mot de passe" : "Mot de passe" }}
          <input v-model="form.password" type="password" autocomplete="new-password" />
        </label>
        <label class="field">
          Confirmation
          <input v-model="form.confirmation" type="password" autocomplete="new-password" />
          <span v-if="form.password && passwordError" class="field-error">{{ passwordError }}</span>
        </label>
      </template>

      <p v-if="action === 'encrypt'" class="warning">
        Aucun moyen de récupérer un mot de passe oublié : sans lui, tes données et leurs sauvegardes futures sont
        illisibles. Les sauvegardes déjà faites restent en clair : supprime-les de leur dossier si besoin.
      </p>
      <p v-else-if="action === 'change'" class="warning">
        Les sauvegardes déjà faites gardent l'ancien mot de passe.
      </p>
      <p v-else class="warning">Le fichier de données sera de nouveau en clair.</p>
      <label v-if="action === 'encrypt'" class="check">
        <input v-model="form.understood" type="checkbox" />
        J'ai compris : sans ce mot de passe, mes données sont perdues.
      </label>

      <p v-if="error" class="error-banner">{{ error }}</p>
      <div class="buttons">
        <button type="button" class="btn btn-secondary" @click="action = null">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="!canSubmit">
          {{
            busy
              ? "En cours…"
              : action === "encrypt"
                ? "Chiffrer et redémarrer"
                : action === "change"
                  ? "Changer et redémarrer"
                  : "Déchiffrer et redémarrer"
          }}
        </button>
      </div>
    </form>
  </section>
</template>

<style scoped>
.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.buttons {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
}

.warning {
  margin: 0;
  padding: 10px 12px;
  border-radius: var(--radius-nav);
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
  font-size: 13px;
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}

.check input {
  width: 18px;
  height: 18px;
  accent-color: var(--color-primary);
}
</style>
