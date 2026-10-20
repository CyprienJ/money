import type { Category } from "../bindings";

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
