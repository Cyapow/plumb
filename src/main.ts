import { createApp } from "vue";
import App from "./App.vue";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/highlight.css";
import { initTheme, initPrefs } from "./lib/ui";

// Apply the saved theme (defaults to Modernist Dark) before mount to avoid a flash.
initTheme();
initPrefs();

createApp(App).mount("#app");
