<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { commands, type Account, type AccountType } from "../bindings";
import { ACCOUNT_TYPES } from "../utils/accounts";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmountInput, parseAmount, todayIso } from "../utils/format";
import BaseModal from "./BaseModal.vue";
import ValuationHistory from "./ValuationHistory.vue";

const props = defineProps<{
  open: boolean;
  /** Account to edit; `null` to create a new one. */
  account: Account | null;
}>();

const emit = defineEmits<{
  close: [];
  saved: [account: Account];
  /** Valuations changed: the account value may have changed. */
  valuationsChanged: [];
}>();

const form = reactive({
  name: "",
  type: "CHECKING" as AccountType,
  institution: "",
  openingBalance: "0",
  openingDate: todayIso(),
});
const submitted = ref(false);
const saving = ref(false);
const error = ref("");

const isEdit = computed(() => props.account !== null);
const hasOperations = computed(() => ACCOUNT_TYPES[form.type].hasOperations);
const nameError = computed(() => (form.name.trim() === "" ? "Le nom est obligatoire." : ""));
const openingBalanceCents = computed(() =>
  hasOperations.value ? parseAmount(form.openingBalance) : 0,
);
const balanceError = computed(() =>
  openingBalanceCents.value === null ? "Montant invalide, par exemple 1 234,56." : "",
);
const dateError = computed(() => (form.openingDate === "" ? "La date est obligatoire." : ""));

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const account = props.account;
    form.name = account?.name ?? "";
    form.type = account?.type ?? "CHECKING";
    form.institution = account?.institution ?? "";
    form.openingBalance = formatAmountInput(account?.openingBalanceCents ?? 0);
    form.openingDate = account?.openingDate ?? todayIso();
    submitted.value = false;
    error.value = "";
  },
  { immediate: true },
);

async function save() {
  submitted.value = true;
  error.value = "";
  const cents = openingBalanceCents.value;
  if (nameError.value || dateError.value || cents === null) return;

  saving.value = true;
  try {
    const fields = {
      name: form.name,
      institution: form.institution || null,
      openingBalanceCents: cents,
      openingDate: form.openingDate,
    };
    const saved = props.account
      ? await unwrap(commands.updateAccount(props.account.id, fields))
      : await unwrap(commands.createAccount({ ...fields, type: form.type }));
    emit("saved", saved);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    saving.value = false;
  }
}

async function toggleArchived() {
  if (!props.account) return;
  saving.value = true;
  error.value = "";
  try {
    const saved = await unwrap(
      commands.setAccountArchived(props.account.id, !props.account.archived),
    );
    emit("saved", saved);
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
    :title="isEdit ? 'Modifier le compte' : 'Nouveau compte'"
    :wide="isEdit && !hasOperations"
    @close="emit('close')"
  >
    <form class="form" novalidate @submit.prevent="save">
      <label class="field">
        Nom
        <input v-model="form.name" type="text" autocomplete="off" placeholder="Compte courant" autofocus />
        <span v-if="submitted && nameError" class="field-error">{{ nameError }}</span>
      </label>

      <label class="field">
        Type
        <select v-model="form.type" :disabled="isEdit">
          <option v-for="(info, type) in ACCOUNT_TYPES" :key="type" :value="type">
            {{ info.label }}
          </option>
        </select>
        <span v-if="isEdit" class="field-hint">Le type ne peut plus être modifié.</span>
      </label>

      <label class="field">
        Établissement
        <input v-model="form.institution" type="text" autocomplete="off" placeholder="Facultatif" />
      </label>

      <div class="row">
        <label v-if="hasOperations" class="field">
          Solde d'ouverture (€)
          <input v-model="form.openingBalance" type="text" inputmode="decimal" />
          <span v-if="submitted && balanceError" class="field-error">{{ balanceError }}</span>
        </label>
        <label class="field">
          Date d'ouverture
          <input v-model="form.openingDate" type="date" />
          <span v-if="submitted && dateError" class="field-error">{{ dateError }}</span>
        </label>
      </div>
      <p v-if="!hasOperations" class="field-hint muted">
        La valeur de ce compte sera suivie par des valorisations saisies à la date voulue.
      </p>

      <p v-if="error" class="error-banner">{{ error }}</p>

      <footer class="actions">
        <button
          v-if="account"
          type="button"
          class="btn btn-link archive"
          :disabled="saving"
          @click="toggleArchived"
        >
          {{ account.archived ? "Désarchiver" : "Archiver" }}
        </button>
        <button type="button" class="btn btn-secondary" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="saving">
          {{ isEdit ? "Enregistrer" : "Créer le compte" }}
        </button>
      </footer>
    </form>

    <ValuationHistory v-if="account && !hasOperations" :account="account" @changed="emit('valuationsChanged')" />
  </BaseModal>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 12px;
}

.muted {
  margin: -6px 0 0;
  color: var(--color-text-muted);
}

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}

.archive {
  margin-right: auto;
}
</style>
