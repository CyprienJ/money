<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { commands, type SourceDocument } from "../bindings";
import { errorMessage, unwrap } from "../utils/commands";
import { formatDate, localDateOf } from "../utils/format";
import BaseModal from "./BaseModal.vue";

const props = defineProps<{
  importBatchId: number;
  /** Page of the line in a PDF statement. */
  page: number | null;
  /** Line in a text file (CSV, OFX, QIF). */
  line: number | null;
}>();

const document = ref<SourceDocument | null>(null);
const error = ref("");
/** Lines of a text file shown in the viewer, `null` while it is closed. */
const lines = ref<string[] | null>(null);
const viewer = ref<HTMLElement>();

async function load() {
  error.value = "";
  document.value = null;
  try {
    document.value = await unwrap(commands.sourceDocument(props.importBatchId));
  } catch (e) {
    error.value = errorMessage(e);
  }
}

/** "page 3", "ligne 12", or nothing when the position is unknown. */
function position(): string {
  if (props.page !== null) return `page ${props.page}`;
  if (props.line !== null) return `ligne ${props.line}`;
  return "";
}

async function show() {
  const current = document.value;
  if (!current) return;
  error.value = "";
  try {
    if (current.kind === "pdf") {
      await unwrap(commands.openSourceDocument(props.importBatchId));
      return;
    }
    const text = await unwrap(commands.sourceDocumentText(props.importBatchId));
    lines.value = text.split(/\r?\n/);
    await nextTick();
    viewer.value?.querySelector(".current")?.scrollIntoView({ block: "center" });
  } catch (e) {
    error.value = errorMessage(e);
  }
}

onMounted(load);
watch(() => props.importBatchId, load);
</script>

<template>
  <div class="source">
    <span class="details">
      <span class="title">Relevé source</span>
      <span v-if="document" class="muted">
        {{ document.fileName }}<template v-if="position()"> · {{ position() }}</template>
        · importé le {{ formatDate(localDateOf(document.importedAt)) }}
      </span>
    </span>
    <button v-if="document" type="button" class="btn btn-secondary" @click="show">
      {{ document.kind === "pdf" ? "Ouvrir le PDF" : "Voir le relevé" }}
    </button>
  </div>
  <p v-if="error" class="field-error">{{ error }}</p>

  <BaseModal :open="lines !== null" :title="document?.fileName ?? 'Relevé'" wide @close="lines = null">
    <p v-if="line !== null" class="muted">La ligne de l'opération est surlignée.</p>
    <div ref="viewer" class="viewer" role="document">
      <div
        v-for="(text, index) in lines ?? []"
        :key="index"
        class="text-line"
        :class="{ current: index + 1 === line }"
      >
        <span class="number">{{ index + 1 }}</span>
        <span class="content">{{ text }}</span>
      </div>
    </div>
  </BaseModal>
</template>

<style scoped>
.source {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-nav);
}

.details {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.title {
  font-size: 13px;
  font-weight: 700;
}

.muted {
  margin: 0;
  font-size: 12px;
  color: var(--color-text-muted);
  overflow-wrap: anywhere;
}

.viewer {
  max-height: min(60vh, 560px);
  margin-top: 10px;
  overflow: auto;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-nav);
  font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
}

.text-line {
  display: flex;
  gap: 12px;
  padding: 0 10px;
  white-space: pre;
}

.text-line.current {
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
  font-weight: 700;
}

.number {
  flex: none;
  width: 4ch;
  color: var(--color-text-muted);
  text-align: right;
  user-select: none;
}
</style>
