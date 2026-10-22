import { ref } from "vue";
import { commands, type Alert } from "../bindings";
import { unwrap } from "./commands";
import { formatAmount, formatDate } from "./format";

export interface AlertView {
  key: string;
  title: string;
  detail: string;
  /** Screen where the alert can be looked into. */
  to: string;
  /** A negative forecast only goes away with the forecast itself. */
  dismissible: boolean;
}

/** Names used in the alert messages. */
export interface AlertNames {
  accounts: Map<number, string>;
  categories: Map<number, string>;
}

export function describeAlert(alert: Alert, names: AlertNames): AlertView {
  switch (alert.kind) {
    case "negativeForecast": {
      const account = names.accounts.get(alert.accountId) ?? "Compte";
      return {
        key: `negative-${alert.accountId}`,
        title: `${account} : solde projeté négatif`,
        detail: `Il passerait sous zéro le ${formatDate(alert.firstNegativeDate)}, avec un point bas de ${formatAmount(
          alert.lowest.balanceCents,
        )} le ${formatDate(alert.lowest.date, "day-month")}.`,
        to: "/schedule",
        dismissible: false,
      };
    }
    case "missingOccurrence": {
      const { occurrence } = alert;
      const account = names.accounts.get(occurrence.accountId) ?? "le compte";
      return {
        key: `missing-${occurrence.scheduledOperationId}-${occurrence.date}`,
        title: `Échéance manquante : ${occurrence.label}`,
        detail: `Prévue le ${formatDate(occurrence.date)} (${formatAmount(occurrence.amountCents, {
          signed: true,
        })}) sur ${account}, mais absente des relevés importés.`,
        to: "/schedule",
        dismissible: true,
      };
    }
    case "unusualExpense": {
      const category = names.categories.get(alert.categoryId) ?? "Catégorie";
      return {
        key: `unusual-${alert.categoryId}-${alert.month}`,
        title: `Dépense inhabituelle : ${category}`,
        detail: `${formatAmount(alert.expenseCents)} ce mois-ci, contre ${formatAmount(
          alert.averageCents,
        )} par mois en moyenne sur les 6 mois précédents.`,
        to: "/analyses",
        dismissible: true,
      };
    }
  }
}

/** Number of alerts shown in the sidebar, refreshed on each navigation and dismissal. */
export const alertCount = ref(0);

export async function refreshAlertCount(): Promise<void> {
  try {
    alertCount.value = (await unwrap(commands.listAlerts())).length;
  } catch {
    // The badge is a hint: the dashboard reports the error.
  }
}
