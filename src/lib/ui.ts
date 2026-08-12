// Small shared UI state: context menus, toasts, and the full-screen diff.
// These are simple reactive singletons any component can drive.

import { reactive, ref, shallowReactive } from "vue";
import type { FileDiff, ChangedFile } from "./git";
import { listAiProviders, type AiConfig } from "./ai";
import { listConnections, type ConnectionConfig } from "./accounts";
import {
  applyTheme,
  getTheme,
  persistThemeId,
  savedThemeId,
  saveCustomThemes,
  loadCustomThemes,
  readCurrentVars,
  type Theme,
  type TokenKey,
} from "./themes";

/* ── Context menu ─────────────────────────────────────────────────── */
export interface MenuItem {
  label: string;
  action?: () => void;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  checked?: boolean;
}

export const contextMenu = reactive<{
  open: boolean;
  x: number;
  y: number;
  items: MenuItem[];
}>({ open: false, x: 0, y: 0, items: [] });

export function openContextMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  contextMenu.items = items;
  contextMenu.x = e.clientX;
  contextMenu.y = e.clientY;
  contextMenu.open = true;
}

export function closeContextMenu() {
  contextMenu.open = false;
  contextMenu.items = [];
}

/* ── Toasts ───────────────────────────────────────────────────────── */
export interface Toast {
  id: number;
  title: string;
  detail?: string;
  kind: "ok" | "error";
}

export const toasts = reactive<{ list: Toast[] }>({ list: [] });
let toastSeq = 1;

export function toast(title: string, detail?: string, kind: "ok" | "error" = "ok") {
  const id = toastSeq++;
  toasts.list.push({ id, title, detail, kind });
  setTimeout(() => {
    const i = toasts.list.findIndex((t) => t.id === id);
    if (i !== -1) toasts.list.splice(i, 1);
  }, 4000);
}

/* ── Text prompt ──────────────────────────────────────────────────── */
// WKWebView (Tauri on macOS) doesn't implement window.prompt — it returns
// null — so we roll our own promise-based input dialog.
export const inputDialog = reactive<{
  open: boolean;
  title: string;
  label: string;
  value: string;
  placeholder: string;
  confirmLabel: string;
  resolve: ((v: string | null) => void) | null;
}>({ open: false, title: "", label: "", value: "", placeholder: "", confirmLabel: "Confirm", resolve: null });

export function promptText(opts: {
  title: string;
  label?: string;
  value?: string;
  placeholder?: string;
  confirmLabel?: string;
}): Promise<string | null> {
  return new Promise((resolve) => {
    inputDialog.title = opts.title;
    inputDialog.label = opts.label ?? "";
    inputDialog.value = opts.value ?? "";
    inputDialog.placeholder = opts.placeholder ?? "";
    inputDialog.confirmLabel = opts.confirmLabel ?? "Confirm";
    inputDialog.resolve = resolve;
    inputDialog.open = true;
  });
}

export function resolveInput(value: string | null) {
  const r = inputDialog.resolve;
  inputDialog.resolve = null;
  inputDialog.open = false;
  r?.(value);
}

/* ── Full-screen diff ─────────────────────────────────────────────── */
export interface FullscreenDiff {
  open: boolean;
  title: string;
  subtitle: string;
  files: ChangedFile[];
  activeFile: string | null;
  load: ((file: string) => Promise<FileDiff>) | null;
}

export const fullscreen = shallowReactive<FullscreenDiff>({
  open: false,
  title: "",
  subtitle: "",
  files: [],
  activeFile: null,
  load: null,
});

export function openFullscreen(opts: {
  title: string;
  subtitle?: string;
  files: ChangedFile[];
  activeFile?: string | null;
  load: (file: string) => Promise<FileDiff>;
}) {
  fullscreen.title = opts.title;
  fullscreen.subtitle = opts.subtitle ?? "";
  fullscreen.files = opts.files;
  fullscreen.activeFile = opts.activeFile ?? opts.files[0]?.path ?? null;
  fullscreen.load = opts.load;
  fullscreen.open = true;
}

export function closeFullscreen() {
  fullscreen.open = false;
  fullscreen.load = null;
}

/* ── Settings panel ───────────────────────────────────────────────── */
export type SettingsSection = "accounts" | "ai" | "appearance" | "about";

export const settings = reactive<{ open: boolean; section: SettingsSection }>({
  open: false,
  section: "accounts",
});

export function openSettings(section: SettingsSection = "accounts") {
  settings.section = section;
  settings.open = true;
}

/* ── File inspector (history / blame) ─────────────────────────────── */
export const fileInspector = reactive<{
  open: boolean;
  repoPath: string;
  file: string;
  tab: "history" | "blame";
}>({ open: false, repoPath: "", file: "", tab: "history" });

export function openFileInspector(repoPath: string, file: string, tab: "history" | "blame" = "history") {
  fileInspector.repoPath = repoPath;
  fileInspector.file = file;
  fileInspector.tab = tab;
  fileInspector.open = true;
}

/* ── Theme ────────────────────────────────────────────────────────── */
export const appState = reactive<{ theme: "dark" | "light"; themeId: string }>({
  theme: "dark",
  themeId: "modernist-dark",
});

export function setThemeId(id: string) {
  const t = getTheme(id);
  if (!t) return;
  applyTheme(t);
  appState.themeId = t.id;
  appState.theme = t.mode;
  persistThemeId(t.id);
}

// Back-compat: a couple of call sites still ask for a bare mode.
export function setTheme(mode: "dark" | "light") {
  setThemeId(`modernist-${mode}`);
}

export function toggleTheme() {
  const t = getTheme(appState.themeId);
  setThemeId(t?.counterpart ?? (appState.theme === "dark" ? "modernist-light" : "modernist-dark"));
}

export function initTheme() {
  customThemesStore.themes = loadCustomThemes();
  const id = savedThemeId();
  setThemeId(getTheme(id) ? id : "modernist-dark");
  initFonts();
}

/* ── Misc preferences ─────────────────────────────────────────────── */
export const prefs = reactive<{ reopenSession: boolean; ignoreWs: boolean; split: boolean }>({
  reopenSession: localStorage.getItem("plumb.reopenSession") !== "false", // default on
  ignoreWs: localStorage.getItem("plumb.ignoreWs") === "true",
  split: localStorage.getItem("plumb.diffSplit") === "true",
});
export function setReopenSession(v: boolean) {
  prefs.reopenSession = v;
  localStorage.setItem("plumb.reopenSession", String(v));
}
export function setDiffSplit(v: boolean) {
  prefs.split = v;
  localStorage.setItem("plumb.diffSplit", String(v));
}
export function toggleDiffSplit() {
  setDiffSplit(!prefs.split);
}
// Bumped to force open diffs to re-fetch (e.g. after toggling ignore-whitespace).
export const diffReloadKey = ref(0);
export function setIgnoreWs(v: boolean) {
  prefs.ignoreWs = v;
  localStorage.setItem("plumb.ignoreWs", String(v));
  import("./git")
    .then((g) => g.setDiffIgnoreWs(v))
    .then(() => diffReloadKey.value++)
    .catch(() => {});
}
export function initPrefs() {
  import("./git").then((g) => g.setDiffIgnoreWs(prefs.ignoreWs)).catch(() => {});
}

/* ── Code font settings (independent of theme) ────────────────────── */
export interface CodeFont {
  name: string;
  stack: string;
}
// Fonts bundled with the app (see src/styles/fonts.css) — always available,
// shown at the top of the picker. Anything else comes from the system list.
export const CODE_FONTS: CodeFont[] = [
  { name: "JetBrains Mono", stack: '"JetBrains Mono", ui-monospace, monospace' },
  { name: "Fira Code", stack: '"Fira Code", ui-monospace, monospace' },
  { name: "IBM Plex Mono", stack: '"IBM Plex Mono", ui-monospace, monospace' },
];

// All font families installed on this Mac (filled lazily from the backend).
export const systemFontsStore = reactive<{ list: string[]; loaded: boolean }>({ list: [], loaded: false });

export async function loadSystemFonts() {
  if (systemFontsStore.loaded) return;
  try {
    const { listSystemFonts } = await import("./git");
    systemFontsStore.list = await listSystemFonts();
    systemFontsStore.loaded = true;
  } catch {
    /* stays empty */
  }
}

export const fontStore = reactive<{ family: string; size: number; lineHeight: number }>({
  family: "JetBrains Mono",
  size: 12,
  lineHeight: 1.7,
});

function applyFonts() {
  const el = document.documentElement.style;
  // Bundled fonts have a curated stack; any other family (a system font) gets a
  // generic monospace fallback.
  const bundled = CODE_FONTS.find((f) => f.name === fontStore.family);
  const stack = bundled ? bundled.stack : `"${fontStore.family}", ui-monospace, monospace`;
  el.setProperty("--code-font", stack);
  el.setProperty("--code-font-size", `${fontStore.size}px`);
  el.setProperty("--code-line-h", String(fontStore.lineHeight));
  localStorage.setItem(
    "plumb.codeFont",
    JSON.stringify({ family: fontStore.family, size: fontStore.size, lineHeight: fontStore.lineHeight }),
  );
}

export function setCodeFontFamily(name: string) {
  fontStore.family = name;
  applyFonts();
}
export function setCodeFontSize(px: number) {
  fontStore.size = Math.max(9, Math.min(22, px));
  applyFonts();
}
export function setCodeLineHeight(h: number) {
  fontStore.lineHeight = Math.max(1.2, Math.min(2.4, Math.round(h * 100) / 100));
  applyFonts();
}
export function resetCodeFont() {
  fontStore.family = "JetBrains Mono";
  fontStore.size = 12;
  fontStore.lineHeight = 1.7;
  applyFonts();
}

function initFonts() {
  const raw = localStorage.getItem("plumb.codeFont");
  if (raw) {
    try {
      const d = JSON.parse(raw);
      if (d.family) fontStore.family = d.family;
      if (typeof d.size === "number") fontStore.size = d.size;
      if (typeof d.lineHeight === "number") fontStore.lineHeight = d.lineHeight;
    } catch {
      /* ignore */
    }
  }
  applyFonts();
}

// The user's custom themes. Reactive so the picker updates as they're
// added/edited/deleted; mirrored to localStorage on every change.
export const customThemesStore = reactive<{ themes: Theme[] }>({ themes: [] });

function persistCustoms() {
  saveCustomThemes(customThemesStore.themes);
}
function findCustom(id: string) {
  return customThemesStore.themes.find((t) => t.id === id);
}

let customSeq = 0;
// Create a new custom theme seeded from whatever theme is currently applied,
// select it, and return its id. Seed-from-current means "base this on the
// theme I'm looking at" — pick a built-in first, then create.
export function createCustomTheme(): string {
  const id = `custom-${Date.now()}-${customSeq++}`;
  const existing = customThemesStore.themes.length + 1;
  const t: Theme = {
    id,
    name: `Custom ${existing}`,
    group: "Custom",
    mode: appState.theme,
    counterpart: appState.theme === "dark" ? "modernist-light" : "modernist-dark",
    vars: readCurrentVars(),
  };
  customThemesStore.themes.push(t);
  persistCustoms();
  setThemeId(id);
  return id;
}

export function updateCustomVar(id: string, key: TokenKey, value: string) {
  const t = findCustom(id);
  if (!t) return;
  t.vars[key] = value;
  persistCustoms();
  if (appState.themeId === id) document.documentElement.style.setProperty(key, value);
}

export function setCustomMode(id: string, mode: "dark" | "light") {
  const t = findCustom(id);
  if (!t) return;
  t.mode = mode;
  t.counterpart = mode === "dark" ? "modernist-light" : "modernist-dark";
  persistCustoms();
  if (appState.themeId === id) setThemeId(id);
}

export function renameCustomTheme(id: string, name: string) {
  const t = findCustom(id);
  if (!t) return;
  t.name = name.trim() || t.name;
  persistCustoms();
}

export function deleteCustomTheme(id: string) {
  const i = customThemesStore.themes.findIndex((t) => t.id === id);
  if (i < 0) return;
  const mode = customThemesStore.themes[i].mode;
  const wasActive = appState.themeId === id;
  customThemesStore.themes.splice(i, 1);
  persistCustoms();
  // If we just deleted the active theme, fall back to a matching built-in.
  if (wasActive) setThemeId(`modernist-${mode}`);
}

/* ── Shared AI config (kept in sync between Settings and the composer) ── */
export const aiStore = reactive<{ config: AiConfig }>({
  config: { providers: [], defaultId: null },
});

export async function refreshAiConfig(): Promise<AiConfig> {
  aiStore.config = await listAiProviders();
  return aiStore.config;
}

/* ── Shared connections (accounts) ────────────────────────────────── */
export const connectionsStore = reactive<{ config: ConnectionConfig }>({
  config: { connections: [] },
});

export async function refreshConnections(): Promise<ConnectionConfig> {
  connectionsStore.config = await listConnections();
  return connectionsStore.config;
}
