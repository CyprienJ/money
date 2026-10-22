import { createRouter, createWebHashHistory } from "vue-router";
import AccountsView from "./views/AccountsView.vue";
import CategoriesView from "./views/CategoriesView.vue";
import ImportView from "./views/ImportView.vue";
import OperationsView from "./views/OperationsView.vue";
import PayeesView from "./views/PayeesView.vue";
import PortfolioView from "./views/PortfolioView.vue";
import ScheduleView from "./views/ScheduleView.vue";
import SettingsView from "./views/SettingsView.vue";

// Hash history: the app is served from a custom protocol, not a web server.
export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    // Dashboard and Analyses are loaded on demand: they bring ECharts, the largest dependency.
    { path: "/", name: "dashboard", component: () => import("./views/DashboardView.vue") },
    { path: "/operations", name: "operations", component: OperationsView },
    { path: "/import", name: "import", component: ImportView },
    { path: "/analyses", name: "analyses", component: () => import("./views/AnalysesView.vue") },
    { path: "/accounts", name: "accounts", component: AccountsView },
    { path: "/accounts/:id/portfolio", name: "portfolio", component: PortfolioView },
    { path: "/schedule", name: "schedule", component: ScheduleView },
    { path: "/categories", name: "categories", component: CategoriesView },
    { path: "/payees", name: "payees", component: PayeesView },
    { path: "/settings", name: "settings", component: SettingsView },
  ],
});
