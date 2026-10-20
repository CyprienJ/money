import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/controls.css";

createApp(App).use(router).mount("#app");
