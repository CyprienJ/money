<script setup lang="ts">
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { ref, watch } from "vue";
import { commands, type Attachment } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { formatFileSize } from "../utils/format";

const props = defineProps<{
  operationId: number;
}>();

const emit = defineEmits<{
  /** Emitted after an attachment is added or deleted. */
  changed: [count: number];
}>();

const attachments = ref<Attachment[]>([]);
const busy = ref(false);
/** Attachment waiting for a second click to be deleted. */
const confirmingId = ref<number | null>(null);
const error = ref("");

async function load() {
  error.value = "";
  confirmingId.value = null;
  try {
    attachments.value = await unwrap(commands.listAttachments(props.operationId));
  } catch (e) {
    error.value = errorMessage(e);
  }
}

watch(() => props.operationId, load, { immediate: true });

async function run(action: () => Promise<unknown>) {
  busy.value = true;
  error.value = "";
  try {
    await action();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    busy.value = false;
  }
}

async function add() {
  const chosen = await openDialog({
    multiple: true,
    directory: false,
    filters: [
      { name: "Justificatifs", extensions: ["pdf", "PDF", "png", "PNG", "jpg", "JPG", "jpeg", "JPEG", "webp", "heic"] },
      { name: "Tous les fichiers", extensions: ["*"] },
    ],
  });
  if (!Array.isArray(chosen) || chosen.length === 0) return;
  await run(async () => {
    for (const path of chosen) {
      await unwrap(commands.addAttachment(props.operationId, path));
    }
  });
  await load();
  emit("changed", attachments.value.length);
}

async function open(attachment: Attachment) {
  await run(() => unwrap(commands.openAttachment(attachment.id)));
}

async function remove(attachment: Attachment) {
  if (confirmingId.value !== attachment.id) {
    confirmingId.value = attachment.id;
    return;
  }
  await run(() => unwrap(commands.deleteAttachment(attachment.id)));
  await load();
  emit("changed", attachments.value.length);
}
</script>

<template>
  <div class="attachments">
    <div class="header">
      <span class="title">Pièces jointes</span>
      <button type="button" class="btn btn-link" :disabled="busy" @click="add">+ Joindre un fichier</button>
    </div>
    <ul v-if="attachments.length > 0">
      <li v-for="attachment in attachments" :key="attachment.id">
        <button type="button" class="file" title="Ouvrir le fichier" :disabled="busy" @click="open(attachment)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z M14 3v5h5" stroke-linejoin="round" />
          </svg>
          <span class="name">{{ attachment.fileName }}</span>
          <span class="size">{{ formatFileSize(attachment.sizeBytes) }}</span>
        </button>
        <button type="button" class="btn btn-link delete" :disabled="busy" @click="remove(attachment)">
          {{ confirmingId === attachment.id ? "Confirmer" : "Supprimer" }}
        </button>
      </li>
    </ul>
    <span v-else class="field-hint">Factures, tickets… copiés à côté du fichier de données, 25 Mo au plus.</span>
    <p v-if="error" class="error-banner">{{ error }}</p>
  </div>
</template>

<style scoped>
.attachments {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--color-text-muted);
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.title {
  font-size: 13px;
  font-weight: 600;
}

ul {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 2px 4px 2px 0;
  border-radius: var(--radius-nav);
  background: var(--color-page);
}

.file {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 6px 10px;
  border: 0;
  border-radius: var(--radius-nav);
  background: transparent;
  color: var(--color-text);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}

.file:hover:not(:disabled) {
  text-decoration: underline;
}

.name {
  overflow: hidden;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.size {
  flex: none;
  color: var(--color-text-muted);
  font-size: 12px;
}

.delete {
  color: var(--cat-restaurants-fg);
  font-size: 13px;
}
</style>
