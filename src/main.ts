import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/controls.css";
import { applyTheme } from "./utils/theme";

applyTheme();

createApp(App).use(router).mount("#app");
