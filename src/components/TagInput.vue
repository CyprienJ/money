<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { addTag } from "../utils/tags";

const props = defineProps<{
  /** Names of the existing tags, offered as suggestions. */
  suggestions: string[];
  label: string;
}>();

const tags = defineModel<string[]>({ required: true });

const typed = ref("");
const listId = useId();

/** Suggestions not on the operation yet. */
const available = computed(() => {
  const taken = new Set(tags.value.map((tag) => tag.toLocaleLowerCase()));
  return props.suggestions.filter((tag) => !taken.has(tag.toLocaleLowerCase()));
});

/** Adds the typed tag; returns whether there was one, so Enter does not submit the form then. */
function commit(): boolean {
  if (typed.value.trim() === "") return false;
  tags.value = addTag(tags.value, typed.value, props.suggestions);
  typed.value = "";
  return true;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" || event.key === ",") {
    if (commit() || event.key === ",") event.preventDefault();
  } else if (event.key === "Backspace" && typed.value === "" && tags.value.length > 0) {
    tags.value = tags.value.slice(0, -1);
  }
}

/** A suggestion picked in the list fills the input entirely: add it at once. */
function onInput(event: Event) {
  if (!(event instanceof InputEvent) || event.inputType === "insertReplacementText") {
    if (props.suggestions.includes(typed.value)) commit();
  }
}

function remove(index: number) {
  tags.value = tags.value.filter((_, i) => i !== index);
}
</script>

<template>
  <div class="tag-input">
    <span v-for="(tag, index) in tags" :key="tag" class="tag">
      {{ tag }}
      <button type="button" class="remove" :aria-label="`Retirer le tag ${tag}`" @click="remove(index)">
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" aria-hidden="true">
          <path d="M6 6l12 12M18 6L6 18" stroke-linecap="round" />
        </svg>
      </button>
    </span>
    <input
      v-model="typed"
      type="text"
      autocomplete="off"
      :list="listId"
      :aria-label="label"
      :placeholder="tags.length === 0 ? 'Vacances, à rembourser…' : ''"
      @keydown="onKeydown"
      @input="onInput"
      @blur="commit"
    />
    <datalist :id="listId">
      <option v-for="tag in available" :key="tag" :value="tag" />
    </datalist>
  </div>
</template>

<style scoped>
.tag-input {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  min-height: 42px;
  padding: 5px 8px;
  border: 1px solid var(--color-border);
  border-radius: 10px;
  background: var(--color-card);
}

.tag-input:focus-within {
  outline: 2px solid var(--color-accent);
  outline-offset: -1px;
}

.tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 6px 4px 10px;
  border-radius: var(--radius-pill);
  background: var(--cat-neutral-bg);
  color: var(--cat-neutral-fg);
  font-size: 12px;
  font-weight: 700;
}

.remove {
  display: inline-flex;
  padding: 3px;
  border: 0;
  border-radius: var(--radius-pill);
  background: transparent;
  color: inherit;
  cursor: pointer;
}

.remove:hover {
  background: var(--color-border);
}

.tag-input input {
  flex: 1 1 120px;
  min-width: 0;
  height: 30px;
  padding: 0 4px;
  border: 0;
  background: transparent;
  color: var(--color-text);
  font-size: 14px;
  font-weight: 500;
}

.tag-input input:focus {
  outline: none;
}
</style>
