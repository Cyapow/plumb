// Small shared UI state: context menus, toasts, and the full-screen diff.
// These are simple reactive singletons any component can drive.

import { reactive, shallowReactive } from "vue";
import type { FileDiff, ChangedFile } from "./git";
import { listAiProviders, type AiConfig } from "./ai";
import { listConnections, type ConnectionConfig } from "./accounts";

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
export const appState = reactive<{ theme: "dark" | "light" }>({ theme: "dark" });

export function setTheme(t: "dark" | "light") {
  appState.theme = t;
  document.documentElement.setAttribute("data-theme", t);
}

export function toggleTheme() {
  setTheme(appState.theme === "dark" ? "light" : "dark");
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
