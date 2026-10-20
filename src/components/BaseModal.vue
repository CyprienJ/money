<script setup lang="ts">
import { onMounted, ref, watch } from "vue";

const props = defineProps<{
  open: boolean;
  title: string;
}>();

const emit = defineEmits<{ close: [] }>();

const dialog = ref<HTMLDialogElement>();

function sync(open: boolean) {
  const element = dialog.value;
  if (!element) return;
  if (open && !element.open) element.showModal();
  if (!open && element.open) element.close();
}

onMounted(() => sync(props.open));
watch(() => props.open, sync);
</script>

<template>
  <!-- Escape triggers "cancel": the parent decides whether to close. -->
  <dialog ref="dialog" class="modal" @cancel.prevent="emit('close')">
    <header class="modal-header">
      <h2>{{ title }}</h2>
      <button type="button" class="close" aria-label="Fermer" @click="emit('close')">×</button>
    </header>
    <slot />
  </dialog>
</template>

<style scoped>
.modal {
  width: min(480px, calc(100vw - 32px));
  padding: 24px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-card);
  background: var(--color-card);
  color: var(--color-text);
  box-shadow: 0 24px 60px rgba(30, 24, 48, 0.2);
}

.modal::backdrop {
  background: rgba(30, 24, 48, 0.35);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 18px;
}

h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 800;
}

.close {
  width: 36px;
  height: 36px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--color-text-muted);
  font-size: 22px;
  cursor: pointer;
}

.close:hover {
  background: var(--color-page);
}
</style>
