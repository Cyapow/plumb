// Theme registry. A theme is a set of CSS-variable overrides plus a light/dark
// "mode" (which drives color-scheme and the syntax-highlight palette). Applying
// a theme writes its vars as inline custom properties on <html>, overriding the
// defaults in tokens.css. The two "Modernist" themes carry no overrides — they
// ARE the tokens.css defaults.

export const TOKEN_KEYS = [
  "--bg", "--surface", "--raised", "--subtle", "--line", "--line-soft",
  "--text", "--text-mid", "--text-dim", "--text-faint",
  "--accent", "--accent-strong", "--accent-fill", "--accent-on",
  "--diff-add-bg", "--diff-add-fg", "--diff-add-num", "--diff-del-bg", "--diff-del-fg",
  "--lane-0", "--lane-1", "--lane-2", "--lane-3", "--lane-4", "--lane-5", "--lane-6",
  // Syntax-highlight palette (per-theme code colors).
  "--syn-keyword", "--syn-string", "--syn-number", "--syn-func", "--syn-type",
  "--syn-tag", "--syn-attr", "--syn-comment", "--syn-meta", "--syn-var",
] as const;

export type TokenKey = (typeof TOKEN_KEYS)[number];
export type Vars = Partial<Record<TokenKey, string>>;

export interface Theme {
  id: string;
  name: string;
  group: string;
  mode: "dark" | "light";
  counterpart: string; // theme id to switch to when toggling light/dark
  vars: Vars;
}

// A tidy way to author a full palette without repeating keys.
function theme(
  id: string,
  name: string,
  group: string,
  mode: "dark" | "light",
  counterpart: string,
  vars: Vars,
): Theme {
  return { id, name, group, mode, counterpart, vars };
}

// The Modernist themes carry no overrides — they ARE the tokens.css defaults,
// so their values don't live in a Vars map. Expose the key colors here so
// previews (and partial custom themes, which inherit these) render truthfully
// instead of borrowing whatever theme is currently applied.
export const MODERNIST_BASE: Record<"dark" | "light", Partial<Record<TokenKey, string>>> = {
  dark: { "--bg": "#171616", "--surface": "#201e1d", "--line": "#3a3736", "--text": "#f3f2f2", "--accent": "#ff563c" },
  light: { "--bg": "#f3f2f2", "--surface": "#eae9e9", "--line": "#d7d3d3", "--text": "#201e1d", "--accent": "#ec3013" },
};

export const BUILTIN_THEMES: Theme[] = [
  theme("modernist-dark", "Modernist Dark", "Modernist", "dark", "modernist-light", {}),
  theme("modernist-light", "Modernist Light", "Modernist", "light", "modernist-dark", {}),

  theme("catppuccin-mocha", "Catppuccin Mocha", "Catppuccin", "dark", "catppuccin-latte", {
    "--bg": "#11111b", "--surface": "#1e1e2e", "--raised": "#313244", "--subtle": "#181825",
    "--line": "#45475a", "--line-soft": "#313244",
    "--text": "#cdd6f4", "--text-mid": "#bac2de", "--text-dim": "#a6adc8", "--text-faint": "#7f849c",
    "--accent": "#cba6f7", "--accent-strong": "#b4befe", "--accent-fill": "#45376b", "--accent-on": "#11111b",
    "--diff-add-bg": "#1c2c46", "--diff-add-fg": "#89b4fa", "--diff-add-num": "#74c7ec",
    "--diff-del-bg": "#3a2230", "--diff-del-fg": "#f38ba8",
    "--lane-0": "#cba6f7", "--lane-1": "#89b4fa", "--lane-2": "#f9e2af", "--lane-3": "#94e2d5",
    "--lane-4": "#b4befe", "--lane-5": "#a6adc8", "--lane-6": "#f5c2e7",
    "--syn-keyword": "#cba6f7", "--syn-string": "#a6e3a1", "--syn-number": "#fab387", "--syn-func": "#89b4fa",
    "--syn-type": "#f9e2af", "--syn-tag": "#f38ba8", "--syn-attr": "#fab387", "--syn-comment": "#6c7086",
    "--syn-meta": "#94e2d5", "--syn-var": "#cdd6f4",
  }),
  theme("catppuccin-macchiato", "Catppuccin Macchiato", "Catppuccin", "dark", "catppuccin-latte", {
    "--bg": "#181926", "--surface": "#24273a", "--raised": "#363a4f", "--subtle": "#1e2030",
    "--line": "#494d64", "--line-soft": "#363a4f",
    "--text": "#cad3f5", "--text-mid": "#b8c0e0", "--text-dim": "#a5adcb", "--text-faint": "#8087a2",
    "--accent": "#c6a0f6", "--accent-strong": "#b7bdf8", "--accent-fill": "#48376b", "--accent-on": "#181926",
    "--diff-add-bg": "#1e2c46", "--diff-add-fg": "#8aadf4", "--diff-add-num": "#7dc4e4",
    "--diff-del-bg": "#38232f", "--diff-del-fg": "#ed8796",
    "--lane-0": "#c6a0f6", "--lane-1": "#8aadf4", "--lane-2": "#eed49f", "--lane-3": "#8bd5ca",
    "--lane-4": "#b7bdf8", "--lane-5": "#a5adcb", "--lane-6": "#f5bde6",
    "--syn-keyword": "#c6a0f6", "--syn-string": "#a6da95", "--syn-number": "#f5a97f", "--syn-func": "#8aadf4",
    "--syn-type": "#eed49f", "--syn-tag": "#ed8796", "--syn-attr": "#f5a97f", "--syn-comment": "#6e738d",
    "--syn-meta": "#8bd5ca", "--syn-var": "#cad3f5",
  }),
  theme("catppuccin-frappe", "Catppuccin Frappé", "Catppuccin", "dark", "catppuccin-latte", {
    "--bg": "#232634", "--surface": "#303446", "--raised": "#414559", "--subtle": "#292c3c",
    "--line": "#51576d", "--line-soft": "#414559",
    "--text": "#c6d0f5", "--text-mid": "#b5bfe2", "--text-dim": "#a5adce", "--text-faint": "#838ba7",
    "--accent": "#ca9ee6", "--accent-strong": "#babbf1", "--accent-fill": "#4b3b66", "--accent-on": "#232634",
    "--diff-add-bg": "#263349", "--diff-add-fg": "#8caaee", "--diff-add-num": "#85c1dc",
    "--diff-del-bg": "#3a2833", "--diff-del-fg": "#e78284",
    "--lane-0": "#ca9ee6", "--lane-1": "#8caaee", "--lane-2": "#e5c890", "--lane-3": "#81c8be",
    "--lane-4": "#babbf1", "--lane-5": "#a5adce", "--lane-6": "#f4b8e4",
    "--syn-keyword": "#ca9ee6", "--syn-string": "#a6d189", "--syn-number": "#ef9f76", "--syn-func": "#8caaee",
    "--syn-type": "#e5c890", "--syn-tag": "#e78284", "--syn-attr": "#ef9f76", "--syn-comment": "#737994",
    "--syn-meta": "#81c8be", "--syn-var": "#c6d0f5",
  }),
  theme("catppuccin-latte", "Catppuccin Latte", "Catppuccin", "light", "catppuccin-mocha", {
    "--bg": "#e6e9ef", "--surface": "#eff1f5", "--raised": "#ffffff", "--subtle": "#dce0e8",
    "--line": "#bcc0cc", "--line-soft": "#ccd0da",
    "--text": "#4c4f69", "--text-mid": "#5c5f77", "--text-dim": "#6c6f85", "--text-faint": "#8c8fa1",
    "--accent": "#8839ef", "--accent-strong": "#7a1fd8", "--accent-fill": "#8839ef", "--accent-on": "#ffffff",
    "--diff-add-bg": "#dbe6ff", "--diff-add-fg": "#1e66f5", "--diff-add-num": "#1e66f5",
    "--diff-del-bg": "#fbe0e6", "--diff-del-fg": "#d20f39",
    "--lane-0": "#8839ef", "--lane-1": "#1e66f5", "--lane-2": "#df8e1d", "--lane-3": "#179299",
    "--lane-4": "#7287fd", "--lane-5": "#6c6f85", "--lane-6": "#ea76cb",
    "--syn-keyword": "#8839ef", "--syn-string": "#40a02b", "--syn-number": "#fe640b", "--syn-func": "#1e66f5",
    "--syn-type": "#df8e1d", "--syn-tag": "#d20f39", "--syn-attr": "#fe640b", "--syn-comment": "#9ca0b0",
    "--syn-meta": "#179299", "--syn-var": "#4c4f69",
  }),

  theme("material-palenight", "Material Palenight", "Material", "dark", "material-lighter", {
    "--bg": "#232635", "--surface": "#292d3e", "--raised": "#363b52", "--subtle": "#242736",
    "--line": "#444267", "--line-soft": "#34324a",
    "--text": "#dfe2f2", "--text-mid": "#a6accd", "--text-dim": "#8f93b3", "--text-faint": "#676e95",
    "--accent": "#c792ea", "--accent-strong": "#82aaff", "--accent-fill": "#4b3a63", "--accent-on": "#232635",
    "--diff-add-bg": "#1f2d40", "--diff-add-fg": "#82aaff", "--diff-add-num": "#89ddff",
    "--diff-del-bg": "#3a2230", "--diff-del-fg": "#f07178",
    "--lane-0": "#c792ea", "--lane-1": "#82aaff", "--lane-2": "#ffcb6b", "--lane-3": "#89ddff",
    "--lane-4": "#f78c6c", "--lane-5": "#a6accd", "--lane-6": "#ff9cac",
    "--syn-keyword": "#c792ea", "--syn-string": "#c3e88d", "--syn-number": "#f78c6c", "--syn-func": "#82aaff",
    "--syn-type": "#ffcb6b", "--syn-tag": "#f07178", "--syn-attr": "#ffcb6b", "--syn-comment": "#676e95",
    "--syn-meta": "#89ddff", "--syn-var": "#dfe2f2",
  }),
  theme("material-ocean", "Material Ocean", "Material", "dark", "material-lighter", {
    "--bg": "#090b10", "--surface": "#0f111a", "--raised": "#1a1c28", "--subtle": "#0c0e15",
    "--line": "#2a2d3f", "--line-soft": "#1a1c28",
    "--text": "#cfd3e6", "--text-mid": "#8f93a2", "--text-dim": "#717693", "--text-faint": "#5a5f7a",
    "--accent": "#82aaff", "--accent-strong": "#c792ea", "--accent-fill": "#24365e", "--accent-on": "#090b10",
    "--diff-add-bg": "#10233d", "--diff-add-fg": "#82aaff", "--diff-add-num": "#89ddff",
    "--diff-del-bg": "#33161f", "--diff-del-fg": "#f07178",
    "--lane-0": "#82aaff", "--lane-1": "#89ddff", "--lane-2": "#ffcb6b", "--lane-3": "#c792ea",
    "--lane-4": "#f78c6c", "--lane-5": "#8f93a2", "--lane-6": "#f07178",
    "--syn-keyword": "#c792ea", "--syn-string": "#c3e88d", "--syn-number": "#f78c6c", "--syn-func": "#82aaff",
    "--syn-type": "#ffcb6b", "--syn-tag": "#f07178", "--syn-attr": "#ffcb6b", "--syn-comment": "#464b5d",
    "--syn-meta": "#89ddff", "--syn-var": "#cfd3e6",
  }),
  theme("material-lighter", "Material Lighter", "Material", "light", "material-palenight", {
    "--bg": "#eceff1", "--surface": "#fafafa", "--raised": "#ffffff", "--subtle": "#e4e7e9",
    "--line": "#cfd8dc", "--line-soft": "#e0e4e6",
    "--text": "#37474f", "--text-mid": "#546e7a", "--text-dim": "#78909c", "--text-faint": "#90a4ae",
    "--accent": "#7c4dff", "--accent-strong": "#6182b8", "--accent-fill": "#7c4dff", "--accent-on": "#ffffff",
    "--diff-add-bg": "#e3ecfb", "--diff-add-fg": "#6182b8", "--diff-add-num": "#6182b8",
    "--diff-del-bg": "#fbe4e4", "--diff-del-fg": "#e53935",
    "--lane-0": "#7c4dff", "--lane-1": "#6182b8", "--lane-2": "#f6a434", "--lane-3": "#39adb5",
    "--lane-4": "#f76d47", "--lane-5": "#78909c", "--lane-6": "#e53935",
    "--syn-keyword": "#7c4dff", "--syn-string": "#91b859", "--syn-number": "#f76d47", "--syn-func": "#6182b8",
    "--syn-type": "#f6a434", "--syn-tag": "#e53935", "--syn-attr": "#f76d47", "--syn-comment": "#90a4ae",
    "--syn-meta": "#39adb5", "--syn-var": "#37474f",
  }),
  theme("material-oceanic", "Material Oceanic", "Material", "dark", "material-lighter", {
    "--bg": "#1b262c", "--surface": "#263238", "--raised": "#314549", "--subtle": "#1e2a30",
    "--line": "#3e5359", "--line-soft": "#2c3c42",
    "--text": "#c3cfd4", "--text-mid": "#a7b6bd", "--text-dim": "#83969d", "--text-faint": "#5c6f76",
    "--accent": "#80cbc4", "--accent-strong": "#009688", "--accent-fill": "#204a47", "--accent-on": "#12191c",
    "--diff-add-bg": "#16323a", "--diff-add-fg": "#80cbc4", "--diff-add-num": "#89ddff",
    "--diff-del-bg": "#33222a", "--diff-del-fg": "#f07178",
    "--lane-0": "#80cbc4", "--lane-1": "#82aaff", "--lane-2": "#ffcb6b", "--lane-3": "#c3e88d",
    "--lane-4": "#c792ea", "--lane-5": "#a7b6bd", "--lane-6": "#f78c6c",
    "--syn-keyword": "#c792ea", "--syn-string": "#c3e88d", "--syn-number": "#f78c6c", "--syn-func": "#82aaff",
    "--syn-type": "#ffcb6b", "--syn-tag": "#f07178", "--syn-attr": "#ffcb6b", "--syn-comment": "#546e7a",
    "--syn-meta": "#89ddff", "--syn-var": "#c3cfd4",
  }),

  theme("solarized-dark", "Solarized Dark", "Solarized", "dark", "solarized-light", {
    "--bg": "#002b36", "--surface": "#073642", "--raised": "#0a4453", "--subtle": "#04303b",
    "--line": "#0f4c5c", "--line-soft": "#073642",
    "--text": "#93a1a1", "--text-mid": "#839496", "--text-dim": "#657b83", "--text-faint": "#586e75",
    "--accent": "#268bd2", "--accent-strong": "#2aa198", "--accent-fill": "#14384d", "--accent-on": "#002b36",
    "--diff-add-bg": "#04323a", "--diff-add-fg": "#859900", "--diff-add-num": "#2aa198",
    "--diff-del-bg": "#3a1f22", "--diff-del-fg": "#dc322f",
    "--lane-0": "#268bd2", "--lane-1": "#2aa198", "--lane-2": "#b58900", "--lane-3": "#859900",
    "--lane-4": "#6c71c4", "--lane-5": "#839496", "--lane-6": "#d33682",
    "--syn-keyword": "#859900", "--syn-string": "#2aa198", "--syn-number": "#d33682", "--syn-func": "#268bd2",
    "--syn-type": "#b58900", "--syn-tag": "#268bd2", "--syn-attr": "#b58900", "--syn-comment": "#586e75",
    "--syn-meta": "#2aa198", "--syn-var": "#839496",
  }),
  theme("solarized-light", "Solarized Light", "Solarized", "light", "solarized-dark", {
    "--bg": "#fdf6e3", "--surface": "#eee8d5", "--raised": "#ffffff", "--subtle": "#f5efdc",
    "--line": "#d9d2bf", "--line-soft": "#e6dfca",
    "--text": "#586e75", "--text-mid": "#657b83", "--text-dim": "#839496", "--text-faint": "#93a1a1",
    "--accent": "#268bd2", "--accent-strong": "#2aa198", "--accent-fill": "#268bd2", "--accent-on": "#fdf6e3",
    "--diff-add-bg": "#e8ecd0", "--diff-add-fg": "#859900", "--diff-add-num": "#2aa198",
    "--diff-del-bg": "#f6e0d6", "--diff-del-fg": "#dc322f",
    "--lane-0": "#268bd2", "--lane-1": "#2aa198", "--lane-2": "#b58900", "--lane-3": "#859900",
    "--lane-4": "#6c71c4", "--lane-5": "#657b83", "--lane-6": "#d33682",
    "--syn-keyword": "#859900", "--syn-string": "#2aa198", "--syn-number": "#d33682", "--syn-func": "#268bd2",
    "--syn-type": "#b58900", "--syn-tag": "#268bd2", "--syn-attr": "#b58900", "--syn-comment": "#93a1a1",
    "--syn-meta": "#2aa198", "--syn-var": "#657b83",
  }),

  theme("nebula-dark", "Nebula Dark", "Nebula", "dark", "modernist-light", {
    "--bg": "#0c0d13", "--surface": "#14151f", "--raised": "#1e2030", "--subtle": "#101019",
    "--line": "#2b2d42", "--line-soft": "#1e2030",
    "--text": "#e6e6f0", "--text-mid": "#b4b6cf", "--text-dim": "#8a8db0", "--text-faint": "#5f6288",
    "--accent": "#21d4c4", "--accent-strong": "#7c6cff", "--accent-fill": "#12463f", "--accent-on": "#06120f",
    "--diff-add-bg": "#10233d", "--diff-add-fg": "#4dd0e1", "--diff-add-num": "#21d4c4",
    "--diff-del-bg": "#331522", "--diff-del-fg": "#ff5d8f",
    "--lane-0": "#21d4c4", "--lane-1": "#7c6cff", "--lane-2": "#ff6ec7", "--lane-3": "#4dd0e1",
    "--lane-4": "#ffb454", "--lane-5": "#b4b6cf", "--lane-6": "#ff5d8f",
    "--syn-keyword": "#c792ea", "--syn-string": "#9ccc65", "--syn-number": "#ffb454", "--syn-func": "#4dd0e1",
    "--syn-type": "#ffca7b", "--syn-tag": "#ff5d8f", "--syn-attr": "#ffca7b", "--syn-comment": "#5f6288",
    "--syn-meta": "#21d4c4", "--syn-var": "#e6e6f0",
  }),
];

const LS_ID = "plumb.themeId";
const LS_CUSTOMS = "plumb.customThemes";
const LS_CUSTOM_LEGACY = "plumb.customTheme"; // single-theme storage, pre-multi

/** Apply a theme: set the light/dark mode, then its var overrides on <html>. */
export function applyTheme(t: Theme) {
  const el = document.documentElement;
  el.setAttribute("data-theme", t.mode);
  for (const k of TOKEN_KEYS) el.style.removeProperty(k);
  for (const [k, v] of Object.entries(t.vars)) el.style.setProperty(k, v);
}

function makeCustom(d: { id: string; name: string; mode: "dark" | "light"; vars: Vars }): Theme {
  return theme(d.id, d.name, "Custom", d.mode, d.mode === "dark" ? "modernist-light" : "modernist-dark", d.vars);
}

/** Load the user's custom themes, migrating the old single-theme format once. */
export function loadCustomThemes(): Theme[] {
  const list: Theme[] = [];
  const raw = localStorage.getItem(LS_CUSTOMS);
  if (raw) {
    try {
      for (const d of JSON.parse(raw) as { id: string; name: string; mode: "dark" | "light"; vars: Vars }[]) {
        list.push(makeCustom(d));
      }
    } catch {
      /* ignore corrupt data */
    }
  }
  // Migrate a theme saved under the old single-slot key.
  const legacy = localStorage.getItem(LS_CUSTOM_LEGACY);
  if (legacy) {
    try {
      const d = JSON.parse(legacy) as { mode: "dark" | "light"; vars: Vars };
      list.push(makeCustom({ id: "custom", name: "Custom", mode: d.mode, vars: d.vars }));
    } catch {
      /* ignore */
    }
    localStorage.removeItem(LS_CUSTOM_LEGACY);
    saveCustomThemes(list);
  }
  return list;
}

export function saveCustomThemes(list: Theme[]) {
  localStorage.setItem(
    LS_CUSTOMS,
    JSON.stringify(list.map((t) => ({ id: t.id, name: t.name, mode: t.mode, vars: t.vars }))),
  );
}

/** All selectable themes, including the user's custom ones. */
export function allThemes(): Theme[] {
  return [...BUILTIN_THEMES, ...loadCustomThemes()];
}

export function getTheme(id: string): Theme | null {
  return BUILTIN_THEMES.find((t) => t.id === id) ?? loadCustomThemes().find((t) => t.id === id) ?? null;
}

export function savedThemeId(): string {
  return localStorage.getItem(LS_ID) || "modernist-dark";
}
export function persistThemeId(id: string) {
  localStorage.setItem(LS_ID, id);
}

/** Read the currently-applied token values (used to seed a custom theme from
 *  whatever theme is active — works for the Modernist themes too, whose values
 *  live in CSS rather than in a vars map). */
export function readCurrentVars(): Vars {
  const cs = getComputedStyle(document.documentElement);
  const out: Vars = {};
  for (const k of TOKEN_KEYS) {
    const v = cs.getPropertyValue(k).trim();
    if (v) out[k] = v;
  }
  return out;
}
