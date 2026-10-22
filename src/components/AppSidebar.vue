<script setup lang="ts">
import { watch } from "vue";
import { useRoute } from "vue-router";
import { alertCount, refreshAlertCount } from "../utils/alerts";

interface NavItem {
  to: string;
  label: string;
  /** SVG markup drawn in a 24×24 viewBox with `currentColor` strokes. */
  icon: string;
}

const mainItems: NavItem[] = [
  {
    to: "/",
    label: "Tableau de bord",
    icon: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  },
  {
    to: "/operations",
    label: "Opérations",
    icon: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" stroke-linecap="round"/>',
  },
  {
    to: "/import",
    label: "Importer un relevé",
    icon: '<path d="M12 15V3M7 8l5-5 5 5M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4" stroke-linecap="round" stroke-linejoin="round"/>',
  },
  {
    to: "/analyses",
    label: "Analyses",
    icon: '<path d="M4 20V10M10 20V4M16 20v-7M22 20H2" stroke-linecap="round"/>',
  },
];

const wealthItems: NavItem[] = [
  {
    to: "/accounts",
    label: "Comptes",
    icon: '<rect x="2" y="6" width="20" height="14" rx="2"/><path d="M2 10h20M16 15h2" stroke-linecap="round"/>',
  },
  {
    to: "/schedule",
    label: "Échéancier",
    icon: '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4" stroke-linecap="round"/>',
  },
  {
    to: "/categories",
    label: "Catégories",
    icon: '<path d="M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9-9-9z" stroke-linejoin="round"/><circle cx="8" cy="8" r="1.5"/>',
  },
  {
    to: "/payees",
    label: "Tiers",
    icon: '<circle cx="9" cy="8" r="4"/><path d="M2 21a7 7 0 0 1 14 0M16 4a4 4 0 0 1 0 8M22 21a7 7 0 0 0-4-6.3" stroke-linecap="round"/>',
  },
];

const settingsItem: NavItem = {
  to: "/settings",
  label: "Paramètres",
  icon: '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1" stroke-linecap="round"/>',
};

// Imports, schedule and operation changes all happen on other screens: navigating refreshes
// the alert badge.
const route = useRoute();
watch(() => route.path, refreshAlertCount, { immediate: true });
</script>

<template>
  <nav class="sidebar" aria-label="Navigation principale">
    <div class="brand">
      <svg width="32" height="32" viewBox="0 0 32 32" aria-hidden="true">
        <circle cx="16" cy="16" r="15" fill="var(--color-accent)" />
        <circle cx="16" cy="16" r="9" fill="none" stroke="var(--color-primary)" stroke-width="2.5" />
        <path d="M16 11v10M13 14h6" stroke="var(--color-primary)" stroke-width="2.5" stroke-linecap="round" />
      </svg>
      <span>Finances perso</span>
    </div>

    <RouterLink v-for="item in mainItems" :key="item.to" :to="item.to" class="link">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true" v-html="item.icon" />
      {{ item.label }}
      <span
        v-if="item.to === '/' && alertCount > 0"
        class="badge"
        :title="`${alertCount} alerte${alertCount > 1 ? 's' : ''}`"
        :aria-label="`${alertCount} alerte${alertCount > 1 ? 's' : ''}`"
      >
        {{ alertCount }}
      </span>
      <span class="active-dot" aria-hidden="true" />
    </RouterLink>

    <div class="section">PATRIMOINE</div>

    <RouterLink v-for="item in wealthItems" :key="item.to" :to="item.to" class="link">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true" v-html="item.icon" />
      {{ item.label }}
      <span class="active-dot" aria-hidden="true" />
    </RouterLink>

    <RouterLink :to="settingsItem.to" class="link settings">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true" v-html="settingsItem.icon" />
      {{ settingsItem.label }}
      <span class="active-dot" aria-hidden="true" />
    </RouterLink>
  </nav>
</template>

<style scoped>
.sidebar {
  position: sticky;
  top: 0;
  height: 100vh;
  overflow-y: auto;
  flex: 0 0 var(--sidebar-width);
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 24px 16px;
  background: var(--color-primary);
  color: var(--sidebar-text);
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 10px 24px;
  font-size: 17px;
  font-weight: 800;
  letter-spacing: -0.01em;
}

.link {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 11px 12px;
  border-radius: var(--radius-nav);
  color: var(--sidebar-link);
  text-decoration: none;
  font-size: 14px;
  font-weight: 500;
}

.link:hover {
  color: var(--color-on-primary);
}

.link.router-link-active {
  background: var(--sidebar-active-bg);
  color: var(--color-on-primary);
  font-weight: 600;
}

.active-dot {
  display: none;
  width: 6px;
  height: 6px;
  margin-left: auto;
  border-radius: 3px;
  background: var(--color-accent);
}

.badge {
  min-width: 20px;
  height: 20px;
  margin-left: auto;
  padding: 0 6px;
  box-sizing: border-box;
  border-radius: 10px;
  background: var(--color-accent);
  color: var(--color-on-primary);
  font-size: 12px;
  font-weight: 800;
  line-height: 20px;
  text-align: center;
}

.badge + .active-dot {
  margin-left: 0;
}

.router-link-active .active-dot {
  display: block;
}

.section {
  padding: 20px 12px 6px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--sidebar-section);
}

.settings {
  margin-top: auto;
}
</style>
