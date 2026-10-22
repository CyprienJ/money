<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  commands,
  type DataLocation,
  type PriceSettings,
  type SavedCsvMapping,
  type SupportedBank,
} from "../bindings";
import CsvMappingModal from "../components/CsvMappingModal.vue";
import PageHeader from "../components/PageHeader.vue";
import { errorMessage, unwrap } from "../utils/commands";
import { DATE_FORMATS, DELIMITERS, ENCODINGS } from "../utils/csvMapping";
import { formatDate } from "../utils/format";
import { priceUpdateLines } from "../utils/portfolio";
import { THEME_PREFERENCES, setThemePreference, themePreference } from "../utils/theme";

const version = ref("");
const location = ref<DataLocation | null>(null);
const mappings = ref<SavedCsvMapping[]>([]);
const banks = ref<SupportedBank[]>([]);
const error = ref("");
const copied = ref("");
const priceSettings = ref<PriceSettings | null>(null);
const updatingPrices = ref(false);
const priceLines = ref<string[]>([]);

const mappingModalOpen = ref(false);
const editedMapping = ref<SavedCsvMapping | null>(null);

async function load() {
  error.value = "";
  try {
    version.value = await commands.appVersion();
    banks.value = await commands.supportedBanks();
    location.value = await unwrap(commands.dataLocation());
    mappings.value = await unwrap(commands.listCsvMappings());
    priceSettings.value = await unwrap(commands.priceSettings());
  } catch (e) {
    error.value = errorMessage(e);
  }
}

async function togglePriceUpdates(enabled: boolean) {
  error.value = "";
  priceLines.value = [];
  try {
    priceSettings.value = await unwrap(commands.setPriceUpdatesEnabled(enabled));
  } catch (e) {
    error.value = errorMessage(e);
  }
}

async function updatePrices() {
  error.value = "";
  updatingPrices.value = true;
  try {
    priceLines.value = priceUpdateLines(await unwrap(commands.updateSecurityPrices()));
    priceSettings.value = await unwrap(commands.priceSettings());
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    updatingPrices.value = false;
  }
}

/** "; · Windows-1252 · JJ/MM/AAAA · débit et crédit". */
function describeMapping({ mapping }: SavedCsvMapping): string {
  const label = (options: { value: string; label: string }[], value: string) =>
    options.find((o) => o.value === value)?.label.split(" (")[0] ?? value;
  return [
    label(DELIMITERS, mapping.delimiter),
    label(ENCODINGS, mapping.encoding),
    label(DATE_FORMATS, mapping.dateFormat),
    mapping.amountColumns.kind === "signed" ? "montant signé" : "débit et crédit",
    ...(mapping.columnFee === null ? [] : ["frais"]),
    ...(mapping.stateFilter ? [`état ${mapping.stateFilter.keep}`] : []),
  ].join(" · ");
}

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    copied.value = text;
    setTimeout(() => (copied.value = ""), 2000);
  } catch {
    // The path stays selectable when the clipboard is not available.
  }
}

function openMapping(mapping: SavedCsvMapping | null) {
  editedMapping.value = mapping;
  mappingModalOpen.value = true;
}

async function onMappingsChanged() {
  mappingModalOpen.value = false;
  await load();
}

onMounted(load);
</script>

<template>
  <PageHeader title="Paramètres" />

  <p v-if="error" class="error-banner">{{ error }}</p>

  <div class="grid">
    <section class="card">
      <h2 class="card-title">Apparence</h2>
      <div role="radiogroup" aria-label="Thème" class="segmented">
        <button
          v-for="choice in THEME_PREFERENCES"
          :key="choice.value"
          type="button"
          role="radio"
          :aria-checked="themePreference === choice.value"
          @click="setThemePreference(choice.value)"
        >
          {{ choice.label }}
        </button>
      </div>
      <p class="muted">« Système » suit le mode clair ou sombre réglé dans le système d'exploitation.</p>
    </section>

    <section class="card">
      <h2 class="card-title">Données</h2>
      <p class="muted">
        Toutes tes données restent sur cet ordinateur, dans un seul fichier. Pour les sauvegarder ou les
        transférer, copie ce fichier application fermée.
      </p>
      <template v-if="location">
        <div class="path-block">
          <span class="path-label">Fichier de données</span>
          <code class="path">{{ location.dataFile }}</code>
          <button type="button" class="btn btn-link" @click="copy(location.dataFile)">
            {{ copied === location.dataFile ? "Copié" : "Copier le chemin" }}
          </button>
        </div>
        <div class="path-block">
          <span class="path-label">Sauvegardes automatiques</span>
          <code class="path">{{ location.backupsDir }}</code>
          <span class="muted">
            {{ location.backupCount }} sauvegarde{{ location.backupCount > 1 ? "s" : "" }} : une copie du fichier
            de données est faite avant chaque import.
          </span>
          <button type="button" class="btn btn-link" @click="copy(location.backupsDir)">
            {{ copied === location.backupsDir ? "Copié" : "Copier le chemin" }}
          </button>
        </div>
      </template>
    </section>

    <section class="card">
      <div class="card-header">
        <h2 class="card-title">Formats CSV enregistrés</h2>
        <button type="button" class="btn btn-link" @click="openMapping(null)">+ Nouveau format</button>
      </div>
      <ul v-if="mappings.length > 0" class="list">
        <li v-for="mapping in mappings" :key="mapping.id">
          <button type="button" class="row" @click="openMapping(mapping)">
            <span class="name">{{ mapping.name }}</span>
            <span class="muted">{{ describeMapping(mapping) }}</span>
          </button>
        </li>
      </ul>
      <p v-else class="muted">
        Aucun format : il se crée à la première importation d'un export CSV, depuis l'écran Importer un relevé.
      </p>
    </section>

    <section v-if="priceSettings" class="card">
      <h2 class="card-title">Cours de bourse</h2>
      <label class="check">
        <input
          type="checkbox"
          :checked="priceSettings.updatesEnabled"
          @change="togglePriceUpdates(($event.target as HTMLInputElement).checked)"
        />
        Récupérer les cours sur Yahoo Finance
      </label>
      <p class="muted">
        C'est le seul accès à Internet de l'application. Elle envoie à query1.finance.yahoo.com uniquement les
        symboles de tes titres (par exemple CW8.PA), et leur code ISIN quand tu cherches un symbole : aucun
        montant, aucune quantité. Les cours sont mis à jour au démarrage, une fois par jour, et à la demande
        depuis le portefeuille. Un cours saisi à la main n'est jamais remplacé.
      </p>
      <template v-if="priceSettings.updatesEnabled">
        <p class="muted">
          {{ priceSettings.updatedOn ? `Dernière mise à jour le ${formatDate(priceSettings.updatedOn)}.` : "Pas encore de mise à jour." }}
        </p>
        <div>
          <button type="button" class="btn btn-secondary" :disabled="updatingPrices" @click="updatePrices">
            {{ updatingPrices ? "Mise à jour…" : "Mettre à jour maintenant" }}
          </button>
        </div>
        <ul v-if="priceLines.length > 0" class="price-lines">
          <li v-for="line in priceLines" :key="line">{{ line }}</li>
        </ul>
      </template>
    </section>

    <section class="card">
      <h2 class="card-title">Relevés PDF pris en charge</h2>
      <ul class="banks">
        <li v-for="bank in banks" :key="bank.id">{{ bank.name }}</li>
      </ul>
      <p class="muted">La banque est reconnue automatiquement à l'import. D'autres banques s'ajouteront au fil des versions.</p>
    </section>

    <section class="card">
      <h2 class="card-title">À propos</h2>
      <p class="muted">Finances perso{{ version ? ` · version ${version}` : "" }}</p>
      <p class="muted">Application locale : aucune donnée ne quitte cet ordinateur.</p>
    </section>
  </div>

  <CsvMappingModal
    :open="mappingModalOpen"
    :mapping="editedMapping"
    :path="null"
    @close="mappingModalOpen = false"
    @saved="onMappingsChanged"
    @deleted="onMappingsChanged"
  />
</template>

<style scoped>
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
  align-items: start;
  gap: 16px;
}

.segmented {
  display: flex;
  align-self: flex-start;
  gap: 4px;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-control);
  background: var(--color-card);
}

.segmented button {
  height: 36px;
  padding: 0 16px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
}

.segmented button[aria-checked="true"] {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.segmented button:hover:not([aria-checked="true"]) {
  background: var(--color-page);
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
}

.check input {
  width: 18px;
  height: 18px;
  accent-color: var(--color-primary);
}

.price-lines {
  margin: 0;
  padding-left: 18px;
  font-size: 13px;
}

.card-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}

.muted {
  margin: 0;
  font-size: 13px;
  color: var(--color-text-muted);
}

.path-block {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
}

.path-label {
  font-size: 13px;
  font-weight: 700;
}

.path {
  width: 100%;
  padding: 8px 10px;
  box-sizing: border-box;
  border-radius: 8px;
  background: var(--color-page);
  font-size: 12px;
  overflow-wrap: anywhere;
  user-select: all;
}

.list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.row {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  margin: 0 -8px;
  padding: 8px;
  box-sizing: content-box;
  border: 0;
  border-radius: var(--radius-nav);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.row:hover {
  background: var(--color-page);
}

.name {
  font-size: 14px;
  font-weight: 700;
}

.banks {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.banks li {
  padding: 4px 12px;
  border-radius: var(--radius-pill);
  background: var(--cat-restaurants-bg);
  color: var(--cat-restaurants-fg);
  font-size: 13px;
  font-weight: 700;
}
</style>
