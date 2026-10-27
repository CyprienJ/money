/** Tag name without surrounding nor repeated spaces, as the backend stores it. */
export function cleanTagName(name: string): string {
  return name.trim().replace(/\s+/g, " ");
}

/**
 * Adds a tag typed by the user. An empty name or one already in the list (ignoring case) leaves
 * the list unchanged; a name known from `existing` (ignoring case) is taken with its spelling.
 */
export function addTag(tags: string[], typed: string, existing: string[] = []): string[] {
  const name = cleanTagName(typed);
  if (name === "") return tags;
  const key = name.toLocaleLowerCase();
  if (tags.some((tag) => tag.toLocaleLowerCase() === key)) return tags;
  const known = existing.find((tag) => tag.toLocaleLowerCase() === key);
  return [...tags, known ?? name];
}
