import { createApp } from "vue";
import App from "./App.vue";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/highlight.css";

// Default to the hero (dark) theme.
document.documentElement.setAttribute("data-theme", "dark");

createApp(App).mount("#app");
