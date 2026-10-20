import type { Category, CategoryColor } from "../bindings";

export interface CategoryGroup {
  parent: Category;
  children: Category[];
}

/** Two-level tree of categories, sorted by name at each level. */
export function categoryTree(categories: Category[]): CategoryGroup[] {
  const byName = (a: Category, b: Category) => a.name.localeCompare(b.name, "fr");
  return categories
    .filter((category) => category.parentId === null)
    .sort(byName)
    .map((parent) => ({
      parent,
      children: categories.filter((c) => c.parentId === parent.id).sort(byName),
    }));
}

/** French names of the color families, in the order shown to the user. */
export const CATEGORY_COLORS: { value: CategoryColor; label: string }[] = [
  { value: "housing", label: "Bleu" },
  { value: "groceries", label: "Jaune" },
  { value: "restaurants", label: "Corail" },
  { value: "transport", label: "Vert" },
  { value: "leisure", label: "Rose" },
  { value: "subscriptions", label: "Ciel" },
  { value: "health", label: "Violet" },
  { value: "neutral", label: "Gris" },
];

/**
 * Tree for the Categories screen: top-level categories by name, with the system internal
 * transfer category last.
 */
export function managementTree(categories: Category[]): CategoryGroup[] {
  const tree = categoryTree(categories);
  return [
    ...tree.filter((group) => !group.parent.isInternalTransfer),
    ...tree.filter((group) => group.parent.isInternalTransfer),
  ];
}
