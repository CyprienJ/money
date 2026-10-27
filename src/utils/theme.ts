import { computed, ref, watchEffect } from "vue";

/** Theme chosen in the settings; "system" follows the operating system. */
export type ThemePreference = "system" | "light" | "dark";
export type Theme = "light" | "dark";

export const THEME_PREFERENCES: { value: ThemePreference; label: string }[] = [
  { value: "system", label: "Système" },
  { value: "light", label: "Clair" },
  { value: "dark", label: "Sombre" },
];

/** Key of the preference in the local storage of the webview. */
const STORAGE_KEY = "money.theme";

export function parsePreference(stored: string | null): ThemePreference {
  return stored === "light" || stored === "dark" ? stored : "system";
}

export function resolveTheme(preference: ThemePreference, systemIsDark: boolean): Theme {
  if (preference === "system") return systemIsDark ? "dark" : "light";
  return preference;
}

function readStored(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

export const themePreference = ref<ThemePreference>(
  typeof window === "undefined" ? "system" : parsePreference(readStored()),
);
const systemIsDark = ref(false);

/** Theme in use, from the preference and the operating system. */
export const resolvedTheme = computed(() => resolveTheme(themePreference.value, systemIsDark.value));

export function setThemePreference(preference: ThemePreference) {
  themePreference.value = preference;
  try {
    localStorage.setItem(STORAGE_KEY, preference);
  } catch {
    // Kept for this session only.
  }
}

/**
 * Applies the theme to the document (`data-theme` on `<html>`, read by `tokens.css`) before
 * the first render, then follows the preference and the operating system.
 */
export function applyTheme() {
  const media = window.matchMedia?.("(prefers-color-scheme: dark)");
  systemIsDark.value = media?.matches ?? false;
  media?.addEventListener("change", (event) => (systemIsDark.value = event.matches));
  // Synchronous, so that the charts read the tokens of the new theme.
  watchEffect(
    () => {
      document.documentElement.dataset.theme = resolvedTheme.value;
    },
    { flush: "sync" },
  );
}
