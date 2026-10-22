<script setup lang="ts">
import { onMounted, ref } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  commands,
  type Backup,
  type BackupSettings,
  type DataLocation,
  type PriceSettings,
  type SavedCsvMapping,
  type SupportedBank,
} from "../bindings";
import CsvMappingModal from "../components/CsvMappingModal.vue";
import PageHeader from "../components/PageHeader.vue";
import { errorMessage, unwrap } from "../utils/commands";
import { DATE_FORMATS, DELIMITERS, ENCODINGS } from "../utils/csvMapping";
import { formatDate, formatFileSize } from "../utils/format";
import { priceUpdateLines } from "../utils/portfolio";
import { THEME_PREFERENCES, setThemePreference, themePreference } from "../utils/theme";

const version = ref("");
const location = ref<DataLocation | null>(null);
const mappings = ref<SavedCsvMapping[]>([]);
const banks = ref<SupportedBank[]>([]);
const error = ref("");
const copied = ref("");
const priceSettings = ref<PriceSettings | null>(null);
const backupSettings = ref<BackupSettings | null>(null);
const backups = ref<Backup[]>([]);
const backingUp = ref(false);
const backupNotice = ref("");
/** Backup waiting for the confirmation of its restore. */
const restoring = ref<Backup | null>(null);
const showAllBackups = ref(false);
/** Backups listed before "Tout afficher". */
const BACKUPS_SHOWN = 5;
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
    await loadBackups();
  } catch (e) {
    error.value = errorMessage(e);
  }
}

async function loadBackups() {
  [backupSettings.value, backups.value] = await Promise.all([
    unwrap(commands.backupSettings()),
    unwrap(commands.listBackups()),
  ]);
}

/** "22/10/2026 à 9 h 30". */
function backupDate(backup: Backup): string {
  const [date, time] = backup.createdAt.split("T");
  const [hours, minutes] = time.split(":");
  return `${formatDate(date)} à ${Number(hours)} h ${minutes}`;
}

function backupDescription(backup: Backup): string {
  const size = formatFileSize(backup.sizeBytes);
  if (backup.kind === "IMPORT") return `Avant un import, données seules · ${size}`;
  const files = backup.attachmentCount;
  return `Complète, ${files} pièce${files > 1 ? "s" : ""} jointe${files > 1 ? "s" : ""} · ${size}`;
}

async function runBackup(action: () => Promise<unknown>) {
  error.value = "";
  backupNotice.value = "";
  try {
    await action();
    await loadBackups();
  } catch (e) {
    error.value = errorMessage(e);
  }
}

async function backupNow() {
  backingUp.value = true;
  await runBackup(async () => {
    await unwrap(commands.createFullBackup());
    backupNotice.value = "Sauvegarde complète faite.";
  });
  backingUp.value = false;
}

async function setDaily(daily: boolean) {
  await runBackup(() => unwrap(commands.setDailyBackups(daily)));
}

async function resetBackupFolder() {
  await runBackup(() => unwrap(commands.setBackupFolder(null)));
}

async function chooseBackupFolder() {
  const folder = await openDialog({ directory: true, title: "Dossier des sauvegardes complètes" });
  if (typeof folder !== "string") return;
  await runBackup(() => unwrap(commands.setBackupFolder(folder)));
}

async function restore(backup: Backup) {
  error.value = "";
  try {
    // The application restarts on success.
    await unwrap(commands.restoreBackup(backup.path));
  } catch (e) {
    error.value = errorMessage(e);
    restoring.value = null;
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
      </template>
    </section>

    <section v-if="backupSettings" class="card">
      <h2 class="card-title">Sauvegardes</h2>
      <label class="check">
        <input
          type="checkbox"
          :checked="backupSettings.daily"
          @change="setDaily(($event.target as HTMLInputElement).checked)"
        />
        Sauvegarde complète chaque jour, au démarrage
      </label>
      <p class="muted">
        Une sauvegarde complète copie le fichier de données et les pièces jointes ; les 10 plus récentes sont
        gardées. Une copie du fichier de données est aussi faite avant chaque import (les 20 dernières).
        {{ backupSettings.lastFullOn ? `Dernière sauvegarde complète le ${formatDate(backupSettings.lastFullOn)}.` : "" }}
      </p>
      <div class="path-block">
        <span class="path-label">Dossier des sauvegardes complètes</span>
        <code class="path">{{ backupSettings.folder }}</code>
        <span class="muted">Un disque externe ou un dossier synchronisé protège aussi d'une panne de l'ordinateur.</span>
        <span class="buttons">
          <button type="button" class="btn btn-link" @click="chooseBackupFolder">Changer…</button>
          <button
            v-if="backupSettings.customFolder"
            type="button"
            class="btn btn-link"
            @click="resetBackupFolder"
          >
            Revenir au dossier par défaut
          </button>
        </span>
      </div>
      <div>
        <button type="button" class="btn btn-secondary" :disabled="backingUp" @click="backupNow">
          {{ backingUp ? "Sauvegarde…" : "Sauvegarder maintenant" }}
        </button>
      </div>
      <p v-if="backupNotice" class="notice">{{ backupNotice }}</p>

      <ul v-if="backups.length > 0" class="list backups">
        <li v-for="backup in showAllBackups ? backups : backups.slice(0, BACKUPS_SHOWN)" :key="backup.path">
          <span class="backup-row">
            <span class="details">
              <span class="name">{{ backupDate(backup) }}</span>
              <span class="muted">{{ backupDescription(backup) }}</span>
            </span>
            <button v-if="restoring?.path !== backup.path" type="button" class="btn btn-link" @click="restoring = backup">
              Restaurer…
            </button>
          </span>
          <span v-if="restoring?.path === backup.path" class="confirm">
            <span>
              Les données actuelles seront d'abord sauvegardées, puis l'application redémarrera sur
              {{ backup.kind === "FULL" ? "cette sauvegarde" : "ces données (les pièces jointes actuelles sont gardées)" }}.
            </span>
            <span class="buttons">
              <button type="button" class="btn btn-secondary" @click="restoring = null">Annuler</button>
              <button type="button" class="btn btn-primary" @click="restore(backup)">Restaurer et redémarrer</button>
            </span>
          </span>
        </li>
      </ul>
      <button
        v-if="backups.length > BACKUPS_SHOWN && !showAllBackups"
        type="button"
        class="btn btn-link"
        @click="showAllBackups = true"
      >
        Afficher les {{ backups.length }} sauvegardes
      </button>
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

.buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
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

.backups > li {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 0;
  border-bottom: 1px solid var(--color-divider);
}

.backups > li:last-child {
  border-bottom: 0;
}

.backup-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.details {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.confirm {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  border-radius: var(--radius-nav);
  background: var(--cat-groceries-bg);
  color: var(--cat-groceries-fg);
  font-size: 13px;
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
