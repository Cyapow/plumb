<script setup lang="ts">
// App settings modal. Sections live in a left rail; content on the right.
import { computed, onMounted, ref, watch } from "vue";
import {
  settings,
  appState,
  setThemeId,
  customThemesStore,
  createCustomTheme,
  updateCustomVar,
  setCustomMode,
  renameCustomTheme,
  deleteCustomTheme,
  fontStore,
  CODE_FONTS,
  systemFontsStore,
  loadSystemFonts,
  setCodeFontFamily,
  setCodeFontSize,
  setCodeLineHeight,
  resetCodeFont,
  prefs,
  setReopenSession,
  setIgnoreWs,
  setDiffSplit,
  setHighContrast,
  type SettingsSection,
} from "../lib/ui";
import { BUILTIN_THEMES, MODERNIST_BASE, type Theme, type TokenKey } from "../lib/themes";
import { getAutostart, setAutostart, installVscodeExtension } from "../lib/git";
import { openUrl } from "../lib/native";
import { toast } from "../lib/ui";
import AiProvidersPanel from "./AiProvidersPanel.vue";
import ConnectionsPanel from "./ConnectionsPanel.vue";
import ActionsPanel from "./ActionsPanel.vue";
import PlumbMark from "./PlumbMark.vue";

// Start-at-login for the background server (Integrations tab).
const autostart = ref(false);
watch(
  () => settings.section,
  (s) => {
    if (s === "integrations") getAutostart().then((v) => (autostart.value = v)).catch(() => {});
  },
  { immediate: true },
);
async function onAutostart(enabled: boolean) {
  try {
    await setAutostart(enabled);
    autostart.value = enabled;
  } catch (e) {
    toast("Autostart failed", String(e), "error");
  }
}

const installing = ref(false);
const installMsg = ref("");
const installErr = ref(false);
async function installVsc() {
  installing.value = true;
  installMsg.value = "";
  installErr.value = false;
  try {
    installMsg.value = await installVscodeExtension();
  } catch (e) {
    installErr.value = true;
    installMsg.value = String(e);
  } finally {
    installing.value = false;
  }
}

const sections: { id: SettingsSection; label: string }[] = [
  { id: "accounts", label: "Accounts" },
  { id: "ai", label: "AI providers" },
  { id: "appearance", label: "Appearance" },
  { id: "integrations", label: "Integrations" },
  { id: "actions", label: "Actions" },
  { id: "about", label: "About" },
];

// Only the built-in themes get their own preview grid; custom themes get a
// dedicated section below with per-theme edit/delete controls.
const builtinGroups = computed(() => {
  const groups: { name: string; themes: Theme[] }[] = [];
  for (const t of BUILTIN_THEMES) {
    let g = groups.find((x) => x.name === t.group);
    if (!g) groups.push((g = { name: t.group, themes: [] }));
    g.themes.push(t);
  }
  return groups;
});

// The custom theme currently applied (if any), for the editor.
const activeCustom = computed(() => customThemesStore.themes.find((t) => t.id === appState.themeId) ?? null);

// A small swatch preview from a theme's key colors. Modernist themes (and gaps
// in partial custom themes) resolve to the Modernist base for their mode, so a
// tile shows the theme's OWN colors rather than the currently-applied theme's.
function swatch(t: Theme, key: TokenKey, fallback: string) {
  return t.vars[key] ?? MODERNIST_BASE[t.mode][key] ?? fallback;
}

// The colors exposed in the custom editor.
const CUSTOM_FIELDS: { key: TokenKey; label: string }[] = [
  { key: "--bg", label: "Background" },
  { key: "--surface", label: "Surface" },
  { key: "--raised", label: "Raised" },
  { key: "--line", label: "Lines" },
  { key: "--text", label: "Text" },
  { key: "--text-mid", label: "Text (muted)" },
  { key: "--accent", label: "Accent" },
  { key: "--accent-on", label: "On accent" },
  { key: "--diff-add-fg", label: "Diff added" },
  { key: "--diff-del-fg", label: "Diff removed" },
];

// System fonts, minus the ones we already bundle (to avoid duplicates).
const bundledNames = new Set(CODE_FONTS.map((f) => f.name));
const systemFontOptions = computed(() => systemFontsStore.list.filter((f) => !bundledNames.has(f)));
onMounted(loadSystemFonts);

// <input type=color> needs a #rrggbb value; coerce whatever the theme has.
function hex(v: string | undefined): string {
  if (!v) return "#000000";
  const s = v.trim();
  if (/^#[0-9a-f]{6}$/i.test(s)) return s;
  if (/^#[0-9a-f]{3}$/i.test(s)) return "#" + s.slice(1).split("").map((c) => c + c).join("");
  return "#000000";
}
</script>

<template>
  <teleport to="body">
    <div v-if="settings.open" class="backdrop" @click.self="settings.open = false">
      <div class="sheet">
        <div class="rail">
          <div class="rail-title">Settings</div>
          <button
            v-for="s in sections"
            :key="s.id"
            class="rail-item"
            :class="{ on: settings.section === s.id }"
            @click="settings.section = s.id"
          >
            {{ s.label }}
          </button>
        </div>

        <div class="content">
          <div class="content-head">
            <h2>{{ sections.find((s) => s.id === settings.section)?.label }}</h2>
            <button class="x" @click="settings.open = false">✕</button>
          </div>
          <div class="content-body">
            <ConnectionsPanel v-if="settings.section === 'accounts'" />
            <AiProvidersPanel v-else-if="settings.section === 'ai'" />
            <ActionsPanel v-else-if="settings.section === 'actions'" />

            <div v-else-if="settings.section === 'integrations'" class="integrations">
              <div class="row-title">Editor panel</div>
              <div class="row-sub">
                Plumb can run inside VS Code and JetBrains as a panel, backed by a small local
                server. Whenever Plumb is open it also runs in the menu bar, and editors share and
                reuse that one background server.
              </div>

              <div class="int-block">
                <div class="int-h">VS Code</div>
                <div class="int-p">Installs the packaged extension from the latest release via the <code>code</code> CLI. Then run <b>Plumb: Open Plumb Panel</b>.</div>
                <div class="install-row">
                  <button class="btn-accent" :disabled="installing" @click="installVsc">
                    <span v-if="installing" class="spinner-sm"></span>{{ installing ? "Installing…" : "Install VS Code extension" }}
                  </button>
                </div>
                <div v-if="installMsg" class="install-msg" :class="{ err: installErr }">{{ installMsg }}</div>
              </div>

              <div class="int-block">
                <div class="int-h">JetBrains</div>
                <div class="int-p">
                  Download <code>plumb-jetbrains-&lt;version&gt;.zip</code> from the release, then in your IDE:
                  <b>Settings → Plugins → ⚙ → Install Plugin from Disk…</b> and pick the zip. Open the <b>Plumb</b> tool window.
                </div>
                <div class="install-row">
                  <button class="btn" @click="openUrl('https://github.com/Cyapow/plumb/releases/latest')">
                    Download from releases ↗
                  </button>
                  <button class="btn" @click="openUrl('https://github.com/Cyapow/plumb/tree/main/editors/jetbrains')">
                    Build from source ↗
                  </button>
                </div>
              </div>

              <label class="toggle-row">
                <input type="checkbox" :checked="autostart" @change="onAutostart(($event.target as HTMLInputElement).checked)" />
                <div>
                  <div class="tr-title">Start Plumb’s background server at login</div>
                  <div class="tr-sub">Runs in the menu bar (no Dock icon) so editor panels open instantly.</div>
                </div>
              </label>
            </div>

            <div v-else-if="settings.section === 'appearance'" class="appearance">
              <div class="row-title">Theme</div>
              <div class="row-sub">Pick a built-in theme, or craft your own below.</div>

              <div v-for="g in builtinGroups" :key="g.name" class="theme-group">
                <div class="group-label">{{ g.name }}</div>
                <div class="theme-grid">
                  <button
                    v-for="t in g.themes"
                    :key="t.id"
                    class="theme-card"
                    :class="{ on: appState.themeId === t.id }"
                    @click="setThemeId(t.id)"
                  >
                    <div
                      class="preview"
                      :style="{
                        background: swatch(t, '--bg', 'var(--bg)'),
                        borderColor: swatch(t, '--line', 'var(--line)'),
                      }"
                    >
                      <span class="p-surface" :style="{ background: swatch(t, '--surface', 'var(--surface)') }"></span>
                      <span class="p-accent" :style="{ background: swatch(t, '--accent', 'var(--accent)') }"></span>
                      <span class="p-text" :style="{ background: swatch(t, '--text', 'var(--text)') }"></span>
                    </div>
                    <span class="t-name">{{ t.name }}</span>
                    <span v-if="appState.themeId === t.id" class="t-check">✓</span>
                  </button>
                </div>
              </div>

              <!-- Custom themes: create any number, select, edit or delete. -->
              <div class="theme-group">
                <div class="group-label">Custom</div>
                <div class="row-sub">
                  Creates a copy of the theme you're currently viewing, then lets you tweak it.
                </div>
                <div class="theme-grid">
                  <button
                    v-for="t in customThemesStore.themes"
                    :key="t.id"
                    class="theme-card"
                    :class="{ on: appState.themeId === t.id }"
                    @click="setThemeId(t.id)"
                  >
                    <div class="preview" :style="{ background: swatch(t, '--bg', 'var(--bg)'), borderColor: swatch(t, '--line', 'var(--line)') }">
                      <span class="p-surface" :style="{ background: swatch(t, '--surface', 'var(--surface)') }"></span>
                      <span class="p-accent" :style="{ background: swatch(t, '--accent', 'var(--accent)') }"></span>
                      <span class="p-text" :style="{ background: swatch(t, '--text', 'var(--text)') }"></span>
                    </div>
                    <span class="t-name">{{ t.name }}</span>
                    <span v-if="appState.themeId === t.id" class="t-check">✓</span>
                  </button>
                  <button class="theme-card new-card" @click="createCustomTheme()">
                    <div class="new-plus">＋</div>
                    <span class="t-name">New from current</span>
                  </button>
                </div>

                <!-- Editor for the selected custom theme. -->
                <div v-if="activeCustom" class="custom-editor">
                  <div class="editor-head">
                    <input
                      class="name-input"
                      :value="activeCustom.name"
                      @input="renameCustomTheme(activeCustom.id, ($event.target as HTMLInputElement).value)"
                    />
                    <div class="seg small">
                      <button :class="{ on: activeCustom.mode === 'dark' }" @click="setCustomMode(activeCustom.id, 'dark')">Dark</button>
                      <button :class="{ on: activeCustom.mode === 'light' }" @click="setCustomMode(activeCustom.id, 'light')">Light</button>
                    </div>
                    <button class="del-btn" title="Delete this theme" @click="deleteCustomTheme(activeCustom.id)">Delete</button>
                  </div>
                  <div class="color-grid">
                    <label v-for="f in CUSTOM_FIELDS" :key="f.key" class="color-field">
                      <input
                        type="color"
                        :value="hex(activeCustom.vars[f.key])"
                        @input="updateCustomVar(activeCustom.id, f.key, ($event.target as HTMLInputElement).value)"
                      />
                      <span>{{ f.label }}</span>
                    </label>
                  </div>
                </div>
                <div v-else class="row-sub select-hint">Select a custom theme above to edit or delete it.</div>
              </div>

              <div class="theme-group">
                <div class="group-label">Code font</div>
                <div class="row-sub">Applies to diffs, blame and full-screen code.</div>

                <div class="font-controls">
                  <label class="fc-row">
                    <span>Font</span>
                    <select :value="fontStore.family" @change="setCodeFontFamily(($event.target as HTMLSelectElement).value)">
                      <optgroup label="Bundled">
                        <option v-for="f in CODE_FONTS" :key="f.name" :value="f.name">{{ f.name }}</option>
                      </optgroup>
                      <optgroup v-if="systemFontOptions.length" label="Installed on this Mac">
                        <option v-for="f in systemFontOptions" :key="f" :value="f">{{ f }}</option>
                      </optgroup>
                    </select>
                  </label>
                  <label class="fc-row">
                    <span>Size <b class="mono">{{ fontStore.size }}px</b></span>
                    <input type="range" min="9" max="22" step="1" :value="fontStore.size"
                      @input="setCodeFontSize(+($event.target as HTMLInputElement).value)" />
                  </label>
                  <label class="fc-row">
                    <span>Line height <b class="mono">{{ fontStore.lineHeight.toFixed(2) }}</b></span>
                    <input type="range" min="1.2" max="2.4" step="0.05" :value="fontStore.lineHeight"
                      @input="setCodeLineHeight(+($event.target as HTMLInputElement).value)" />
                  </label>
                </div>

                <div class="font-preview" :style="{ fontFamily: 'var(--code-font)', fontSize: 'var(--code-font-size)', lineHeight: 'var(--code-line-h)' }">
                  <div><span class="pv-key">const</span> <span class="pv-fn">plumb</span> = (repo) <span class="pv-key">=&gt;</span> {</div>
                  <div>&nbsp;&nbsp;<span class="pv-com">// a straight line through your history</span></div>
                  <div>&nbsp;&nbsp;<span class="pv-key">return</span> repo.<span class="pv-fn">commits</span>.<span class="pv-fn">map</span>(<span class="pv-str">"→"</span>);</div>
                  <div>}</div>
                </div>
                <button class="btn reset-font" @click="resetCodeFont">Reset to default</button>
              </div>

              <div class="theme-group">
                <div class="group-label">Behavior</div>
                <label class="pref-row">
                  <input type="checkbox" :checked="prefs.reopenSession" @change="setReopenSession(($event.target as HTMLInputElement).checked)" />
                  <span>Reopen last session on launch <em>— restore open tabs and the active repo</em></span>
                </label>
                <label class="pref-row">
                  <input type="checkbox" :checked="prefs.ignoreWs" @change="setIgnoreWs(($event.target as HTMLInputElement).checked)" />
                  <span>Ignore whitespace in diffs <em>— applies to newly opened diffs</em></span>
                </label>
                <label class="pref-row">
                  <input type="checkbox" :checked="prefs.split" @change="setDiffSplit(($event.target as HTMLInputElement).checked)" />
                  <span>Side-by-side diffs <em>— show old and new in two columns</em></span>
                </label>
                <label class="pref-row">
                  <input type="checkbox" :checked="prefs.highContrast" @change="setHighContrast(($event.target as HTMLInputElement).checked)" />
                  <span>High-contrast text <em>— boost text legibility on any theme</em></span>
                </label>
              </div>
            </div>

            <div v-else class="about">
              <div class="about-mark"><PlumbMark :size="48" /></div>
              <div class="about-name">Plumb</div>
              <div class="about-tag">A straight line through your history.</div>
              <div class="about-meta mono">
                A free, native macOS Git client · Tauri + Vue<br />
                Keys in your Keychain · no Plumb account · no server.
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop {
  position: fixed; inset: 0; z-index: 1200;
  background: color-mix(in srgb, #000 55%, transparent);
  display: flex; align-items: center; justify-content: center;
}
.sheet {
  width: 760px; height: 520px; max-width: calc(100vw - 48px); max-height: calc(100vh - 96px);
  background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg);
  display: flex; overflow: hidden;
}
.rail { width: 190px; flex: none; background: var(--subtle); border-right: 2px solid var(--line); padding: var(--space-3); }
.rail-title { font-size: 11px; font-weight: 600; letter-spacing: 0.12em; text-transform: uppercase; color: var(--text-faint); padding: var(--space-2) var(--space-2) var(--space-3); }
.rail-item { display: block; width: 100%; text-align: left; padding: 8px 10px; background: transparent; border: none; font-size: 13px; color: var(--text-mid); cursor: pointer; }
.rail-item:hover { background: var(--raised); }
.rail-item.on { background: var(--raised); color: var(--text); font-weight: 600; box-shadow: inset 2px 0 0 var(--accent); }

.content { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.content-head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.content-head h2 { margin: 0; font-size: 18px; font-weight: 800; }
.content-head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.content-body { flex: 1; overflow-y: auto; padding: var(--space-4); }

.row { display: flex; align-items: center; gap: var(--space-4); }
.row-title { font-size: 13px; font-weight: 600; }
.row-sub { font-size: 11.5px; color: var(--text-faint); margin-top: 2px; }
.integrations .toggle-row { display: flex; align-items: flex-start; gap: 10px; margin-top: var(--space-4); cursor: pointer; }
.integrations .toggle-row input { margin-top: 3px; accent-color: var(--accent); }
.integrations .tr-title { font-size: 13px; font-weight: 600; }
.integrations .tr-sub { font-size: 11.5px; color: var(--text-faint); margin-top: 2px; }
.integrations .int-block { margin-top: var(--space-4); padding-top: var(--space-3); border-top: 1px solid var(--line); }
.integrations .int-h { font-size: 12px; font-weight: 700; }
.integrations .int-p { font-size: 11.5px; color: var(--text-dim); line-height: 1.5; margin-top: 3px; }
.integrations .int-p code { font-size: 10.5px; background: var(--surface); border: 1px solid var(--line-soft); padding: 0 4px; }
.integrations .install-row { display: flex; flex-wrap: wrap; gap: var(--space-2); margin-top: var(--space-3); }
.integrations .btn-accent { display: inline-flex; align-items: center; gap: var(--space-2); height: 32px; padding: 0 14px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-size: 12.5px; font-weight: 700; cursor: pointer; }
.integrations .btn-accent:disabled { opacity: 0.7; }
.integrations .btn { height: 32px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); color: var(--text); font-size: 12.5px; cursor: pointer; }
.integrations .install-msg { margin-top: var(--space-2); font-size: 11.5px; color: var(--text-mid); }
.integrations .install-msg.err { color: var(--accent); }
.integrations .spinner-sm { width: 11px; height: 11px; border: 2px solid color-mix(in srgb, var(--accent-on) 40%, transparent); border-top-color: var(--accent-on); border-radius: 50%; animation: plumb-spin 0.7s linear infinite; }
@keyframes plumb-spin { to { transform: rotate(360deg); } }
.seg { display: flex; gap: 2px; margin-left: auto; }
.seg button { padding: 7px 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; font-weight: 600; color: var(--text-mid); cursor: pointer; }
.seg button.on { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }

.seg.small button { padding: 5px 12px; font-size: 11.5px; }

/* Appearance / themes */
.appearance .row-sub { margin-bottom: var(--space-3); }
.theme-group { margin-top: var(--space-4); }
.group-label { font-size: 10.5px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-faint); margin-bottom: var(--space-2); }
.theme-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: var(--space-2); }
.theme-card { position: relative; display: flex; flex-direction: column; gap: 8px; padding: 8px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; text-align: left; }
.theme-card.on { border-color: var(--accent); box-shadow: inset 0 0 0 1px var(--accent); }
.preview { height: 40px; border: 1px solid; display: flex; align-items: center; gap: 5px; padding: 0 8px; }
.preview .p-surface { width: 16px; height: 16px; }
.preview .p-accent { width: 16px; height: 16px; }
.preview .p-text { width: 24px; height: 6px; margin-left: auto; }
.t-name { font-size: 11.5px; font-weight: 600; color: var(--text); }
.t-check { position: absolute; top: 6px; right: 8px; color: var(--accent); font-weight: 800; font-size: 12px; }
.new-card { align-items: center; justify-content: center; border-style: dashed; color: var(--text-dim); }
.new-card:hover { border-color: var(--accent); color: var(--accent); }
.new-plus { height: 40px; display: flex; align-items: center; font-size: 22px; }
.select-hint { margin-top: var(--space-3); }
.custom-editor { margin-top: var(--space-3); border: 1px solid var(--line); padding: var(--space-3); }
.editor-head { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-3); }
.name-input { flex: 1; height: 30px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; font-weight: 600; }
.name-input:focus { outline: none; border-color: var(--accent); }
.del-btn { height: 30px; padding: 0 12px; background: var(--raised); border: 1px solid var(--line); color: var(--accent); font-size: 12px; font-weight: 600; cursor: pointer; }
.del-btn:hover { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }
.mode-row { display: flex; align-items: center; gap: var(--space-3); font-size: 12px; color: var(--text-mid); margin-bottom: var(--space-3); }
.mode-row .seg { margin-left: auto; }
.color-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: var(--space-2) var(--space-3); }
.color-field { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--text-mid); }
.color-field input[type="color"] { width: 30px; height: 26px; padding: 0; border: 1px solid var(--line); background: none; cursor: pointer; }
.custom-actions { display: flex; gap: var(--space-2); margin-top: var(--space-4); }
.custom-actions .btn { height: 30px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); font-size: 12px; color: var(--text-mid); cursor: pointer; }
.custom-actions .btn.on { border-color: var(--accent); color: var(--text); }

.font-controls { margin-top: var(--space-3); display: flex; flex-direction: column; gap: var(--space-3); }
.fc-row { display: flex; align-items: center; gap: var(--space-3); font-size: 12px; color: var(--text-mid); }
.fc-row > span { width: 150px; flex: none; }
.fc-row b { color: var(--text); }
.fc-row select { flex: 1; height: 30px; padding: 0 8px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 12.5px; }
.fc-row input[type="range"] { flex: 1; accent-color: var(--accent); }
.font-preview { margin-top: var(--space-4); padding: var(--space-3); background: var(--bg); border: 1px solid var(--line); color: var(--text); white-space: pre; overflow-x: auto; }
.font-preview .pv-key { color: var(--syn-keyword, #c678dd); }
.font-preview .pv-fn { color: var(--syn-func, #61afef); }
.font-preview .pv-str { color: var(--syn-string, #98c379); }
.font-preview .pv-com { color: var(--syn-comment, #7f848e); font-style: italic; }
.reset-font { margin-top: var(--space-3); height: 30px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); font-size: 12px; color: var(--text-mid); cursor: pointer; }
.pref-row { display: flex; align-items: flex-start; gap: 8px; font-size: 12.5px; color: var(--text); margin-bottom: var(--space-2); cursor: pointer; }
.pref-row input { margin-top: 2px; }
.pref-row em { color: var(--text-faint); font-style: normal; font-size: 11.5px; }

.about { text-align: center; padding-top: var(--space-8); }
.about-mark { display: flex; justify-content: center; margin-bottom: var(--space-4); color: var(--text); }
.about-name { font-size: 32px; font-weight: 800; letter-spacing: -0.02em; }
.about-tag { font-size: 14px; color: var(--text-mid); margin-top: var(--space-2); }
.about-meta { font-size: 11px; color: var(--text-faint); line-height: 1.7; margin-top: var(--space-6); }
</style>
