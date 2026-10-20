import type { AccountType, CategoryColor } from "../bindings";

interface AccountTypeInfo {
  label: string;
  /** Color family of the initials badge (see the Dashboard mockup). */
  color: CategoryColor;
  /** Balance computed from operations; otherwise tracked by valuations. */
  hasOperations: boolean;
}

export const ACCOUNT_TYPES: Record<AccountType, AccountTypeInfo> = {
  CHECKING: { label: "Compte courant", color: "transport", hasOperations: true },
  CREDIT_CARD: { label: "Carte à débit différé", color: "groceries", hasOperations: true },
  SAVINGS: { label: "Épargne", color: "subscriptions", hasOperations: true },
  INVESTMENT: { label: "Placement", color: "health", hasOperations: false },
  ASSET: { label: "Actif", color: "housing", hasOperations: false },
  LIABILITY: { label: "Dette", color: "restaurants", hasOperations: false },
};

/** Sections of the accounts screen, in display order. */
export const ACCOUNT_GROUPS: { title: string; types: AccountType[] }[] = [
  { title: "Comptes courants et cartes", types: ["CHECKING", "CREDIT_CARD"] },
  { title: "Épargne", types: ["SAVINGS"] },
  { title: "Placements", types: ["INVESTMENT"] },
  { title: "Actifs", types: ["ASSET"] },
  { title: "Dettes", types: ["LIABILITY"] },
];

/**
 * Initials shown in the account badge: first letters of the first two words
 * ("Compte courant" → "CC"), a short single word as is ("PEA"), otherwise its first two letters.
 */
export function accountInitials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) {
    return "?";
  }
  if (words.length === 1) {
    const word = words[0];
    return (word.length <= 3 ? word : word.slice(0, 2)).toUpperCase();
  }
  return (words[0][0] + words[1][0]).toUpperCase();
}

const MONTH_NAMES = [
  "janvier",
  "février",
  "mars",
  "avril",
  "mai",
  "juin",
  "juillet",
  "août",
  "septembre",
  "octobre",
  "novembre",
  "décembre",
];

/**
 * Secondary line of a valued account, as in the Dashboard mockup: "Valorisé au 30/09",
 * "Actif · estimé en janvier", "Dette · capital restant dû". `valuedOn` is the date of the
 * last valuation, `null` when there is none yet.
 */
export function valuationSubtitle(
  type: AccountType,
  valuedOn: string | null,
  today: Date = new Date(),
): string {
  if (valuedOn === null) return `${ACCOUNT_TYPES[type].label} · à valoriser`;
  const [year, month, day] = valuedOn.split("-");
  switch (type) {
    case "ASSET": {
      const sameYear = Number(year) === today.getFullYear();
      return `Actif · estimé en ${MONTH_NAMES[Number(month) - 1]}${sameYear ? "" : ` ${year}`}`;
    }
    case "LIABILITY":
      return "Dette · capital restant dû";
    default:
      return `Valorisé au ${day}/${month}${Number(year) === today.getFullYear() ? "" : `/${year}`}`;
  }
}
