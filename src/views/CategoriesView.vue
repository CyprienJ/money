<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { commands, type Category, type Rule } from "../bindings";
import CategoryFormModal from "../components/CategoryFormModal.vue";
import CategoryPill from "../components/CategoryPill.vue";
import PageHeader from "../components/PageHeader.vue";
import RuleFormModal from "../components/RuleFormModal.vue";
import { managementTree } from "../utils/categories";
import { errorMessage, unwrap } from "../utils/commands";
import { operationCount } from "../utils/format";

const categories = ref<Category[]>([]);
const rules = ref<Rule[]>([]);
/** Result of the last change of the rules. */
const notice = ref("");
const loaded = ref(false);
const error = ref("");

const modalOpen = ref(false);
const editedCategory = ref<Category | null>(null);
const newParentId = ref<number | null>(null);

const ruleModalOpen = ref(false);
const editedRule = ref<Rule | null>(null);

const categoriesById = computed(() => new Map(categories.value.map((c) => [c.id, c])));

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
    rules.value = await unwrap(commands.listRules());
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

function openRule(rule: Rule | null) {
  editedRule.value = rule;
  ruleModalOpen.value = true;
}

async function onRuleChanged(message: string) {
  ruleModalOpen.value = false;
  notice.value = message;
  await load();
}

/** Moves a rule one place up (`-1`) or down (`1`): the first matching rule wins. */
async function moveRule(index: number, offset: -1 | 1) {
  const ids = rules.value.map((r) => r.id);
  const target = index + offset;
  if (target < 0 || target >= ids.length) return;
  [ids[index], ids[target]] = [ids[target], ids[index]];
  error.value = "";
  try {
    await unwrap(commands.reorderRules(ids));
  } catch (e) {
    error.value = errorMessage(e);
  }
  await load();
}

async function applyRule(rule: Rule) {
  error.value = "";
  try {
    const count = await unwrap(commands.applyRule(rule.id));
    notice.value = `${operationCount(count)} classée${count > 1 ? "s" : ""} avec la règle « ${rule.pattern} ».`;
  } catch (e) {
    error.value = errorMessage(e);
  }
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

  <section v-if="loaded" class="card rules">
    <div class="group-header">
      <h2 class="card-title">Règles de catégorisation</h2>
      <button type="button" class="btn btn-link add-rule" @click="openRule(null)">+ Nouvelle règle</button>
    </div>
    <p class="hint">
      À l'import, une opération dont le libellé contient le texte d'une règle reçoit sa catégorie. La
      première règle de la liste qui correspond l'emporte, avant la reprise de catégorie.
    </p>
    <p v-if="notice" class="notice">{{ notice }}</p>
    <ol v-if="rules.length > 0" class="rule-list">
      <li v-for="(rule, index) in rules" :key="rule.id" class="rule">
        <span class="rule-order">
          <button
            type="button"
            class="move"
            :disabled="index === 0"
            :aria-label="`Monter la règle « ${rule.pattern} »`"
            @click="moveRule(index, -1)"
          >
            ▲
          </button>
          <button
            type="button"
            class="move"
            :disabled="index === rules.length - 1"
            :aria-label="`Descendre la règle « ${rule.pattern} »`"
            @click="moveRule(index, 1)"
          >
            ▼
          </button>
        </span>
        <button type="button" class="rule-main" title="Modifier la règle" @click="openRule(rule)">
          <span class="rule-text">contient <strong>« {{ rule.pattern }} »</strong></span>
          <span aria-hidden="true">→</span>
          <CategoryPill
            :label="categoriesById.get(rule.categoryId)?.name ?? '?'"
            :color="categoriesById.get(rule.categoryId)?.color"
          />
        </button>
        <span v-if="rule.pendingCount > 0" class="rule-pending">
          {{ operationCount(rule.pendingCount) }} à catégoriser
          <button type="button" class="btn btn-link" @click="applyRule(rule)">Classer</button>
        </span>
      </li>
    </ol>
    <p v-else class="hint">Aucune règle. Exemple : « Intérêts nets » → Intérêts.</p>
  </section>

  <div v-if="!error && loaded" class="groups">
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

  <RuleFormModal
    :open="ruleModalOpen"
    :categories="categories"
    :rule="editedRule"
    @close="ruleModalOpen = false"
    @changed="onRuleChanged"
  />

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
.rules {
  gap: 12px;
  margin-bottom: 16px;
}

.add-rule {
  margin-left: auto;
}

.hint {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.notice {
  margin: 0;
  padding: 8px 12px;
  border-radius: var(--radius-nav);
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
  font-size: 13px;
  font-weight: 600;
}

.rule-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.rule {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 12px;
  padding: 4px 0;
  border-bottom: 1px solid var(--color-divider);
}

.rule:last-child {
  border-bottom: 0;
}

.rule-order {
  display: flex;
  flex-direction: column;
}

.move {
  padding: 0 4px;
  border: 0;
  background: transparent;
  color: var(--color-text-muted);
  font-size: 10px;
  line-height: 1.4;
  cursor: pointer;
}

.move:disabled {
  opacity: 0.3;
  cursor: default;
}

.rule-main {
  display: flex;
  flex: 1 1 260px;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 0;
  border-radius: var(--radius-nav);
  background: transparent;
  color: inherit;
  font-size: 14px;
  text-align: left;
  cursor: pointer;
}

.rule-main:hover {
  background: var(--color-page);
}

.rule-text {
  overflow-wrap: anywhere;
}

.rule-pending {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--color-text-muted);
}

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
