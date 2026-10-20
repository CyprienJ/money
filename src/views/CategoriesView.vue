<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { commands, type Category } from "../bindings";
import CategoryFormModal from "../components/CategoryFormModal.vue";
import PageHeader from "../components/PageHeader.vue";
import { managementTree } from "../utils/categories";
import { errorMessage, unwrap } from "../utils/commands";
import { operationCount } from "../utils/format";

const categories = ref<Category[]>([]);
const loaded = ref(false);
const error = ref("");

const modalOpen = ref(false);
const editedCategory = ref<Category | null>(null);
const newParentId = ref<number | null>(null);

const groups = computed(() =>
  managementTree(categories.value).map((group) => ({
    ...group,
    // Operations of the category and its subcategories.
    totalCount: [group.parent, ...group.children].reduce((sum, c) => sum + c.operationCount, 0),
  })),
);

async function load() {
  error.value = "";
  try {
    categories.value = await unwrap(commands.listCategories());
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

function openCreate(parentId: number | null) {
  editedCategory.value = null;
  newParentId.value = parentId;
  modalOpen.value = true;
}

function openEdit(category: Category) {
  if (category.isInternalTransfer) return;
  editedCategory.value = category;
  modalOpen.value = true;
}

async function onChanged() {
  modalOpen.value = false;
  await load();
}

onMounted(load);
</script>

<template>
  <PageHeader title="Catégories" subtitle="Patrimoine">
    <template #actions>
      <button type="button" class="btn btn-primary" @click="openCreate(null)">+ Nouvelle catégorie</button>
    </template>
  </PageHeader>

  <p v-if="error" class="error-banner">{{ error }}</p>

  <div v-else-if="loaded" class="groups">
    <section
      v-for="{ parent, children, totalCount } in groups"
      :key="parent.id"
      class="card"
      :class="{ system: parent.isInternalTransfer }"
    >
      <div class="group-header">
        <button
          type="button"
          class="title"
          :disabled="parent.isInternalTransfer"
          :title="parent.isInternalTransfer ? undefined : 'Modifier la catégorie'"
          @click="openEdit(parent)"
        >
          <span class="dot" :style="{ background: `var(--cat-${parent.color})` }" />
          <h2 class="card-title">{{ parent.name }}</h2>
        </button>
        <span v-if="parent.isIncome" class="tag income">Revenu</span>
        <span v-if="parent.isInternalTransfer" class="tag">Exclue des statistiques</span>
        <span class="count">{{ operationCount(totalCount) }}</span>
      </div>

      <p v-if="parent.isInternalTransfer" class="muted">
        Catégorie système pour les virements entre tes comptes : comptés dans les soldes, mais pas
        dans les revenus et les dépenses. Elle ne peut être ni modifiée ni supprimée.
      </p>

      <template v-else>
        <ul class="children">
          <li v-for="child in children" :key="child.id">
            <button type="button" class="child" @click="openEdit(child)">
              <span
                class="pill"
                :style="{ background: `var(--cat-${child.color}-bg)`, color: `var(--cat-${child.color}-fg)` }"
              >
                {{ child.name }}
              </span>
              <span class="child-count">{{ child.operationCount > 0 ? operationCount(child.operationCount) : "" }}</span>
            </button>
          </li>
        </ul>
        <div>
          <button type="button" class="btn btn-link" @click="openCreate(parent.id)">+ Sous-catégorie</button>
        </div>
      </template>
    </section>
  </div>

  <CategoryFormModal
    :open="modalOpen"
    :categories="categories"
    :category="editedCategory"
    :parent-id="newParentId"
    @close="modalOpen = false"
    @changed="onChanged"
  />
</template>

<style scoped>
.groups {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  align-items: start;
  gap: 16px;
}

.card {
  gap: 12px;
}

.card.system {
  background: var(--cat-neutral-bg);
  border-color: transparent;
}

.group-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.title {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: -4px -6px;
  padding: 4px 6px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.title:hover:not(:disabled) {
  background: var(--color-page);
}

.title:disabled {
  cursor: default;
}

.dot {
  width: 14px;
  height: 14px;
  flex: none;
  border-radius: 7px;
}

.tag {
  padding: 2px 8px;
  border-radius: var(--radius-pill);
  background: var(--color-card);
  color: var(--cat-neutral-fg);
  font-size: 11px;
  font-weight: 700;
}

.tag.income {
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
}

.count {
  margin-left: auto;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-text-muted);
}

.muted {
  margin: 0;
  font-size: 13px;
  color: var(--cat-neutral-fg);
}

.children {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.child {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  margin: 0 -8px;
  padding: 6px 8px;
  box-sizing: content-box;
  border: 0;
  border-radius: var(--radius-nav);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.child:hover {
  background: var(--color-page);
}

.pill {
  padding: 4px 10px;
  border-radius: var(--radius-pill);
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
}

.child-count {
  font-size: 12px;
  color: var(--color-text-muted);
  white-space: nowrap;
}
</style>
