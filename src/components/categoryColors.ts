/** Category color families, mapped to the `--cat-<name>` tokens in styles/tokens.css. */
export const CATEGORY_COLORS = [
  "housing",
  "groceries",
  "restaurants",
  "transport",
  "leisure",
  "subscriptions",
  "health",
  "neutral",
] as const;

export type CategoryColor = (typeof CATEGORY_COLORS)[number];
