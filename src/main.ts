import { createApp } from "vue";
import App from "./App.vue";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/highlight.css";
import { initTheme, initPrefs } from "./lib/ui";

// Tag the root with the OS so platform-specific chrome (e.g. the macOS
// traffic-light gutter) can adapt. WKWebView/WebView2/WebKitGTK all report the
// platform in the UA string.
const ua = navigator.userAgent;
const os = /Windows/.test(ua) ? "windows" : /Mac OS X|Macintosh/.test(ua) ? "macos" : "linux";
document.documentElement.setAttribute("data-os", os);

// Suppress the WebView's native right-click menu (its "Reload" reloads the
// whole app). Plumb draws its own context menus programmatically, so the only
// place the native menu is still useful is real text fields — keep it there for
// copy/paste, block it everywhere else.
window.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement | null;
  if (t?.closest("input, textarea, [contenteditable], [data-selectable], .diff .content, .hunks .content")) return;
  e.preventDefault();
});

// Apply the saved theme (defaults to Modernist Dark) before mount to avoid a flash.
initTheme();
initPrefs();

createApp(App).mount("#app");
