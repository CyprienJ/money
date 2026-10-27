<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { TradeLine, TradePreview } from "../bindings";
import { formatAmount, formatDate } from "../utils/format";
import { formatQuantity } from "../utils/portfolio";

const props = defineProps<{
  preview: TradePreview;
  /** Investment account receiving the trades. */
  accountName: string;
  currency: string;
  importing: boolean;
}>();

const emit = defineEmits<{
  back: [];
  import: [indexes: number[]];
}>();

/** Lines that can be imported: new and dated after the opening of the account. */
function importable(line: TradeLine): boolean {
  return !line.duplicate && !line.beforeOpening;
}

const selected = ref(new Set<number>());

watch(
  () => props.preview,
  (preview) => {
    selected.value = new Set(preview.lines.flatMap((line, index) => (importable(line) ? [index] : [])));
  },
  { immediate: true },
);

const duplicateCount = computed(() => props.preview.lines.filter((l) => l.duplicate).length);
/** Securities created at import, one per ISIN. */
const newSecurities = computed(
  () =>
    new Set(
      props.preview.lines
        .filter((line, index) => line.securityId === null && selected.value.has(index))
        .map((line) => line.trade.isin),
    ).size,
);

function toggle(index: number) {
  const next = new Set(selected.value);
  if (!next.delete(index)) next.add(index);
  selected.value = next;
}

function status(line: TradeLine): string {
  if (line.duplicate) return "Déjà dans le portefeuille";
  if (line.beforeOpening) return "Antérieur à l'ouverture du compte";
  return line.securityId === null ? "Nouveau titre" : "";
}

function movementCount(count: number): string {
  return `${count} mouvement${count > 1 ? "s" : ""}`;
}
</script>

<template>
  <section class="card summary">
    <div>
      <h2 class="card-title">Mouvements titres · {{ preview.bank }}</h2>
      <p class="muted">
        {{ preview.fileName }} · vers le portefeuille de <strong>{{ accountName }}</strong>
      </p>
    </div>
    <ul class="chips" aria-label="Résumé de l'import">
      <li class="chip"><strong>{{ preview.lines.length }}</strong> lus</li>
      <li class="chip neutral"><strong>{{ duplicateCount }}</strong> déjà saisis</li>
      <li class="chip new"><strong>{{ newSecurities }}</strong> titres à créer</li>
    </ul>
    <p class="muted">
      Le montant d'un ordre comprend ses frais, que le relevé ne détaille pas. Un titre inconnu est créé avec son
      ISIN et le nom du relevé : renomme-le et cherche son symbole depuis le portefeuille.
    </p>
  </section>

  <section class="card">
    <p v-if="preview.lines.length === 0" class="muted">Ce relevé ne contient aucun achat ni vente de titres.</p>
    <table v-else>
      <thead>
        <tr>
          <th scope="col"><span class="sr-only">Importer</span></th>
          <th scope="col">Date</th>
          <th scope="col">Titre</th>
          <th scope="col">Sens</th>
          <th scope="col" class="number">Quantité</th>
          <th scope="col" class="number">Montant</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(line, index) in preview.lines" :key="index" :class="{ excluded: !importable(line) }">
          <td>
            <input
              type="checkbox"
              :checked="selected.has(index)"
              :disabled="!importable(line)"
              :aria-label="`Importer ${line.trade.name} du ${formatDate(line.trade.date)}`"
              @change="toggle(index)"
            />
          </td>
          <td>{{ formatDate(line.trade.date) }}</td>
          <td>
            <span class="name">{{ line.trade.name }}</span>
            <span class="sub">
              {{ line.trade.isin }}<template v-if="status(line)"> · {{ status(line) }}</template>
            </span>
          </td>
          <td>{{ line.trade.kind === "SELL" ? "Vente" : "Achat" }}</td>
          <td class="number">{{ formatQuantity(line.trade.quantityMicro) }}</td>
          <td class="number">{{ formatAmount(line.trade.amountCents, { currency }) }}</td>
        </tr>
      </tbody>
    </table>
  </section>

  <footer class="footer">
    <span class="muted">Une sauvegarde sera faite avant l'import.</span>
    <div class="buttons">
      <button type="button" class="btn btn-secondary" :disabled="importing" @click="emit('back')">Retour</button>
      <button
        type="button"
        class="btn btn-primary"
        :disabled="selected.size === 0 || importing"
        @click="emit('import', [...selected].sort((a, b) => a - b))"
      >
        {{ importing ? "Import en cours…" : `Importer ${movementCount(selected.size)}` }}
      </button>
    </div>
  </footer>
</template>

<style scoped>
.summary {
  gap: 12px;
}

.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.chip {
  padding: 6px 12px;
  border-radius: 999px;
  background: var(--cat-housing-bg);
  color: var(--cat-housing-fg);
  font-size: 13px;
}

.chip.neutral {
  background: var(--cat-neutral-bg);
  color: var(--cat-neutral-fg);
}

.chip.new {
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 14px;
}

th {
  padding: 8px 10px;
  border-bottom: 1px solid var(--color-border);
  color: var(--color-text-muted);
  font-size: 12px;
  font-weight: 700;
  text-align: left;
}

td {
  padding: 10px;
  border-bottom: 1px solid var(--color-border);
  vertical-align: top;
}

.number {
  text-align: right;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.name {
  display: block;
  font-weight: 700;
}

.sub {
  font-size: 12px;
  color: var(--color-text-muted);
}

tr.excluded td {
  color: var(--color-text-muted);
}

.footer {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.buttons {
  display: flex;
  gap: 12px;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}
</style>
