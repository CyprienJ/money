<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import {
  commands,
  type Account,
  type InvestmentTransaction,
  type Performance,
  type Position,
  type PriceSettings,
  type Security,
} from "../bindings";
import InvestmentTransactionFormModal from "../components/InvestmentTransactionFormModal.vue";
import PageHeader from "../components/PageHeader.vue";
import SecurityFormModal from "../components/SecurityFormModal.vue";
import { errorMessage, unwrap } from "../utils/commands";
import { formatAmount, formatDate, todayIso } from "../utils/format";
import {
  formatBasisPoints,
  formatQuantity,
  gainBasisPoints,
  kindLabel,
  priceUpdateLines,
} from "../utils/portfolio";

const route = useRoute();
const accountId = computed(() => Number(route.params.id));

const account = ref<Account | null>(null);
/** Currency of the amounts of the account; prices are in the currency of each security. */
const currency = computed(() => account.value?.currency);
const positions = ref<Position[]>([]);
const performance = ref<Performance | null>(null);
const transactions = ref<InvestmentTransaction[]>([]);
const securities = ref<Security[]>([]);
const priceSettings = ref<PriceSettings | null>(null);
const updatingPrices = ref(false);
const priceLines = ref<string[]>([]);
const loaded = ref(false);
const error = ref("");

const transactionModalOpen = ref(false);
const editedTransaction = ref<InvestmentTransaction | null>(null);
const securityModalOpen = ref(false);
const editedSecurity = ref<Security | null>(null);

const totalCost = computed(() => positions.value.reduce((sum, p) => sum + p.costCents, 0));
const totalValue = computed(() => positions.value.reduce((sum, p) => sum + p.marketValueCents, 0));
const unpriced = computed(() => positions.value.filter((p) => p.lastPrice === null).length);
const totalDividends = computed(() => positions.value.reduce((sum, p) => sum + p.dividendsCents, 0));

async function load() {
  error.value = "";
  try {
    account.value = await unwrap(commands.getAccount(accountId.value));
    [positions.value, performance.value, transactions.value, securities.value, priceSettings.value] = await Promise.all([
      unwrap(commands.portfolioPositions(accountId.value, todayIso())),
      unwrap(commands.portfolioPerformance(accountId.value, todayIso())),
      unwrap(commands.listInvestmentTransactions(accountId.value)),
      unwrap(commands.listSecurities()),
      unwrap(commands.priceSettings()),
    ]);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    loaded.value = true;
  }
}

function signClass(value: number): string {
  return value > 0 ? "positive" : value < 0 ? "negative" : "";
}

function percent(gainCents: number, costCents: number): string {
  const basisPoints = gainBasisPoints(gainCents, costCents);
  return basisPoints === null ? "" : formatBasisPoints(basisPoints);
}

async function updatePrices() {
  error.value = "";
  updatingPrices.value = true;
  try {
    priceLines.value = priceUpdateLines(await unwrap(commands.updateSecurityPrices()));
    await load();
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    updatingPrices.value = false;
  }
}

function openTransaction(transaction: InvestmentTransaction | null) {
  editedTransaction.value = transaction;
  transactionModalOpen.value = true;
}

function openSecurity(security: Security | null) {
  editedSecurity.value = security;
  securityModalOpen.value = true;
}

async function onTransactionsChanged() {
  transactionModalOpen.value = false;
  await load();
}

async function onSecurityChanged() {
  securityModalOpen.value = false;
  securities.value = await unwrap(commands.listSecurities());
  await load();
}

/** "+ 12,5 parts", "− 3 parts" or the amount received for a dividend. */
function movement(transaction: InvestmentTransaction): string {
  if (transaction.kind === "DIVIDEND") {
    return formatAmount(transaction.amountCents, { signed: true, currency: currency.value });
  }
  const sign = transaction.kind === "BUY" ? "+" : "−";
  const quantity = formatQuantity(transaction.quantityMicro);
  return `${sign} ${quantity} part${transaction.quantityMicro > 1_000_000 ? "s" : ""}`;
}

/** Cash moved: paid for a buy (fees included), received for a sell (fees deducted). */
function cashMoved(transaction: InvestmentTransaction): number {
  switch (transaction.kind) {
    case "BUY":
      return -(transaction.amountCents + transaction.feesCents);
    case "SELL":
      return transaction.amountCents - transaction.feesCents;
    case "DIVIDEND":
      return transaction.amountCents - transaction.feesCents;
  }
}

watch(accountId, load, { immediate: true });
</script>

<template>
  <PageHeader :title="account ? `Portefeuille · ${account.name}` : 'Portefeuille'" subtitle="Patrimoine">
    <template #actions>
      <RouterLink to="/accounts" class="btn btn-secondary">Comptes</RouterLink>
      <button
        v-if="priceSettings?.updatesEnabled"
        type="button"
        class="btn btn-secondary"
        :disabled="updatingPrices"
        @click="updatePrices"
      >
        {{ updatingPrices ? "Mise à jour…" : "Mettre à jour les cours" }}
      </button>
      <button
        type="button"
        class="btn btn-primary"
        :disabled="account?.type !== 'INVESTMENT'"
        @click="openTransaction(null)"
      >
        + Mouvement
      </button>
    </template>
  </PageHeader>

  <p v-if="error" class="error-banner">{{ error }}</p>
  <ul v-if="priceLines.length > 0" class="notice">
    <li v-for="line in priceLines" :key="line">{{ line }}</li>
  </ul>

  <template v-if="loaded && account">
    <p class="muted intro">
      Dès le premier mouvement, la valeur du compte (et du patrimoine net) est celle de ses titres au dernier
      cours ; un titre sans cours compte pour son prix de revient. Les liquidités non investies ne sont pas suivies.
    </p>

    <section v-if="performance && transactions.length > 0" class="card">
      <div class="card-header">
        <h2 class="card-title">Performance</h2>
        <span class="muted">au {{ formatDate(todayIso()) }}</span>
      </div>
      <dl class="figures">
        <div>
          <dt>Valeur</dt>
          <dd>{{ formatAmount(performance.marketValueCents, { currency }) }}</dd>
        </div>
        <div>
          <dt>Plus-value latente</dt>
          <dd :class="signClass(performance.unrealizedCents)">
            {{ formatAmount(performance.unrealizedCents, { signed: true, currency }) }}
          </dd>
        </div>
        <div>
          <dt>Plus-values réalisées</dt>
          <dd :class="signClass(performance.realizedCents)">
            {{ formatAmount(performance.realizedCents, { signed: true, currency }) }}
          </dd>
        </div>
        <div>
          <dt>Dividendes</dt>
          <dd>{{ formatAmount(performance.dividendsCents, { currency }) }}</dd>
        </div>
        <div>
          <dt>Gain total</dt>
          <dd :class="signClass(performance.totalGainCents)">
            {{ formatAmount(performance.totalGainCents, { signed: true, currency }) }}
            <span v-if="performance.totalGainBasisPoints !== null" class="sub">
              {{ formatBasisPoints(performance.totalGainBasisPoints) }} des achats
            </span>
          </dd>
        </div>
        <div>
          <dt>Rendement annualisé</dt>
          <dd v-if="performance.annualizedBasisPoints !== null" :class="signClass(performance.annualizedBasisPoints)">
            {{ formatBasisPoints(performance.annualizedBasisPoints) }}
          </dd>
          <dd v-else class="sub">Après un an de détention</dd>
        </div>
      </dl>
      <table v-if="performance.years.length > 0" class="years">
        <thead>
          <tr>
            <th scope="col">Année</th>
            <th scope="col" class="number">Plus-values réalisées</th>
            <th scope="col" class="number">Dividendes</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="year in performance.years" :key="year.year">
            <td>{{ year.year }}</td>
            <td class="number" :class="signClass(year.realizedCents)">
              {{ formatAmount(year.realizedCents, { signed: true, currency }) }}
            </td>
            <td class="number">{{ formatAmount(year.dividendsCents, { currency }) }}</td>
          </tr>
        </tbody>
      </table>
      <p class="muted">
        Rendement annualisé : taux qui égalise les achats, ventes, dividendes et la valeur actuelle à leurs dates (TRI).
      </p>
    </section>

    <section class="card">
      <div class="card-header">
        <h2 class="card-title">Positions</h2>
        <span class="muted">au {{ formatDate(todayIso()) }}</span>
      </div>
      <div v-if="positions.length > 0" class="table-scroll">
      <table>
        <thead>
          <tr>
            <th scope="col">Titre</th>
            <th scope="col" class="number">Quantité</th>
            <th scope="col" class="number">PRU</th>
            <th scope="col" class="number">Prix de revient</th>
            <th scope="col" class="number">Dernier cours</th>
            <th scope="col" class="number">Valeur</th>
            <th scope="col" class="number">Plus-value latente</th>
            <th scope="col" class="number">Dividendes</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="position in positions" :key="position.securityId">
            <td>
              <span class="name">{{ position.securityName }}</span>
              <span v-if="position.isin" class="sub">{{ position.isin }}</span>
            </td>
            <td class="number">{{ formatQuantity(position.quantityMicro) }}</td>
            <td class="number">{{ formatAmount(position.unitCostCents, { currency }) }}</td>
            <td class="number">{{ formatAmount(position.costCents, { currency }) }}</td>
            <td class="number">
              <template v-if="position.lastPrice">
                {{ formatAmount(position.lastPrice.priceCents, { currency: position.currency }) }}
                <span class="sub">{{ formatDate(position.lastPrice.date) }}</span>
              </template>
              <span v-else class="sub">sans cours</span>
            </td>
            <td class="number">{{ formatAmount(position.marketValueCents, { currency }) }}</td>
            <td class="number" :class="signClass(position.unrealizedCents)">
              <template v-if="position.lastPrice">
                {{ formatAmount(position.unrealizedCents, { signed: true, currency }) }}
                <span class="sub">{{ percent(position.unrealizedCents, position.costCents) }}</span>
              </template>
              <template v-else>—</template>
            </td>
            <td class="number">{{ position.dividendsCents > 0 ? formatAmount(position.dividendsCents, { currency }) : "—" }}</td>
          </tr>
        </tbody>
        <tfoot>
          <tr>
            <th scope="row" colspan="3">Total</th>
            <td class="number"><strong>{{ formatAmount(totalCost, { currency }) }}</strong></td>
            <td />
            <td class="number"><strong>{{ formatAmount(totalValue, { currency }) }}</strong></td>
            <td class="number" :class="signClass(totalValue - totalCost)">
              {{ formatAmount(totalValue - totalCost, { signed: true, currency }) }}
            </td>
            <td class="number">{{ totalDividends > 0 ? formatAmount(totalDividends, { currency }) : "—" }}</td>
          </tr>
        </tfoot>
      </table>
      </div>
      <p v-if="unpriced > 0" class="muted">
        {{ unpriced }} titre{{ unpriced > 1 ? "s" : "" }} sans cours, compté{{ unpriced > 1 ? "s" : "" }} pour son prix de
        revient : saisis un cours dans la fiche du titre<template v-if="priceSettings?.updatesEnabled">, ou vérifie son symbole</template>.
      </p>
      <p v-if="positions.length === 0" class="muted">
        Aucun titre détenu. Ajoute tes achats avec « + Mouvement » : le prix de revient se calcule au coût moyen
        pondéré, frais d'achat inclus.
      </p>
    </section>

    <div class="grid">
      <section class="card">
        <div class="card-header">
          <h2 class="card-title">Mouvements</h2>
          <span class="muted">{{ transactions.length }}</span>
        </div>
        <ul v-if="transactions.length > 0" class="list">
          <li v-for="transaction in transactions" :key="transaction.id">
            <button type="button" class="row-button" @click="openTransaction(transaction)">
              <span class="date">{{ formatDate(transaction.date, "day-month") }}</span>
              <span class="details">
                <span class="name">{{ kindLabel(transaction.kind) }} · {{ transaction.securityName }}</span>
                <span class="sub">
                  {{ movement(transaction) }}
                  <template v-if="transaction.feesCents > 0"> · frais {{ formatAmount(transaction.feesCents, { currency }) }}</template>
                  <template v-if="transaction.realizedCents !== null">
                    · plus-value {{ formatAmount(transaction.realizedCents, { signed: true, currency }) }}
                  </template>
                  <template v-if="transaction.note"> · {{ transaction.note }}</template>
                  <template v-if="transaction.hasSourceDocument">
                    · relevé{{ transaction.sourcePage !== null ? ` p. ${transaction.sourcePage}` : "" }}
                  </template>
                </span>
              </span>
              <strong class="amount" :class="{ positive: cashMoved(transaction) > 0 }">
                {{ formatAmount(cashMoved(transaction), { signed: true, currency }) }}
              </strong>
            </button>
          </li>
        </ul>
        <p v-else class="muted">Aucun mouvement.</p>
      </section>

      <section class="card">
        <div class="card-header">
          <h2 class="card-title">Titres</h2>
          <button type="button" class="btn btn-link" @click="openSecurity(null)">+ Nouveau titre</button>
        </div>
        <ul v-if="securities.length > 0" class="list">
          <li v-for="security in securities" :key="security.id">
            <button type="button" class="row-button" @click="openSecurity(security)">
              <span class="details">
                <span class="name">{{ security.name }}</span>
                <span class="sub">
                  {{ security.isin ?? "Sans ISIN" }}<template v-if="security.symbol"> · {{ security.symbol }}</template>
                  · {{ security.transactionCount }} mouvement{{
                    security.transactionCount > 1 ? "s" : ""
                  }}
                </span>
              </span>
            </button>
          </li>
        </ul>
        <p v-else class="muted">Les titres sont communs à tous les comptes de placement.</p>
      </section>
    </div>

    <InvestmentTransactionFormModal
      :open="transactionModalOpen"
      :account="account"
      :securities="securities"
      :transaction="editedTransaction"
      @close="transactionModalOpen = false"
      @changed="onTransactionsChanged"
      @new-security="openSecurity(null)"
    />
    <SecurityFormModal
      :open="securityModalOpen"
      :security="editedSecurity"
      :price-updates-enabled="priceSettings?.updatesEnabled ?? false"
      @close="securityModalOpen = false"
      @changed="onSecurityChanged"
    />
  </template>
</template>

<style scoped>
.notice {
  margin: 0;
  padding: 10px 14px 10px 30px;
  border-radius: var(--radius-nav);
  background: var(--cat-transport-bg);
  color: var(--cat-transport-fg);
  font-size: 13px;
  font-weight: 600;
}

.figures {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 14px 24px;
  margin: 0;
}

.figures dt {
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-muted);
}

.figures dd {
  margin: 2px 0 0;
  font-size: 18px;
  font-weight: 800;
}

.years {
  max-width: 480px;
}

.positive {
  color: var(--color-positive);
}

.negative {
  color: var(--cat-restaurants-fg);
}

.intro {
  margin-top: -8px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
  align-items: start;
  gap: 16px;
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

.table-scroll {
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 14px;
}

th {
  padding: 8px 10px;
  border-bottom: 1px solid var(--color-border);
  font-size: 12px;
  font-weight: 700;
  color: var(--color-text-muted);
  text-align: left;
}

td {
  padding: 10px;
  border-bottom: 1px solid var(--color-divider);
}

tfoot th,
tfoot td {
  border-bottom: 0;
  font-size: 14px;
  color: var(--color-text);
}

.number {
  text-align: right;
  white-space: nowrap;
}

.list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 14px;
}

.list > li {
  padding: 10px 0;
  border-bottom: 1px solid var(--color-divider);
}

.list > li:last-child {
  border-bottom: 0;
}

.row-button {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  margin: -6px -8px;
  padding: 6px 8px;
  box-sizing: content-box;
  border: 0;
  border-radius: var(--radius-nav);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.row-button:hover {
  background: var(--color-page);
}

.date {
  width: 44px;
  font-weight: 700;
  color: var(--color-text-muted);
}

.details {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.name {
  display: block;
  font-weight: 700;
}

.sub {
  display: block;
  font-size: 12px;
  color: var(--color-text-muted);
}

.amount {
  white-space: nowrap;
}

.amount.positive {
  color: var(--color-positive);
}
</style>
