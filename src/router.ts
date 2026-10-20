import { createRouter, createWebHashHistory } from "vue-router";
import AccountsView from "./views/AccountsView.vue";
import AnalysesView from "./views/AnalysesView.vue";
import CategoriesView from "./views/CategoriesView.vue";
import DashboardView from "./views/DashboardView.vue";
import ImportView from "./views/ImportView.vue";
import OperationsView from "./views/OperationsView.vue";
import ScheduleView from "./views/ScheduleView.vue";
import SettingsView from "./views/SettingsView.vue";

// Hash history: the app is served from a custom protocol, not a web server.
export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "dashboard", component: DashboardView },
    { path: "/operations", name: "operations", component: OperationsView },
    { path: "/import", name: "import", component: ImportView },
    { path: "/analyses", name: "analyses", component: AnalysesView },
    { path: "/accounts", name: "accounts", component: AccountsView },
    { path: "/schedule", name: "schedule", component: ScheduleView },
    { path: "/categories", name: "categories", component: CategoriesView },
    { path: "/settings", name: "settings", component: SettingsView },
  ],
});
