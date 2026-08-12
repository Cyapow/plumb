<script setup lang="ts">
// App settings modal. Sections live in a left rail; content on the right.
import { settings, appState, setTheme, type SettingsSection } from "../lib/ui";
import AiProvidersPanel from "./AiProvidersPanel.vue";
import ConnectionsPanel from "./ConnectionsPanel.vue";
import PlumbMark from "./PlumbMark.vue";

const sections: { id: SettingsSection; label: string }[] = [
  { id: "accounts", label: "Accounts" },
  { id: "ai", label: "AI providers" },
  { id: "appearance", label: "Appearance" },
  { id: "about", label: "About" },
];
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

            <div v-else-if="settings.section === 'appearance'">
              <div class="row">
                <div>
                  <div class="row-title">Theme</div>
                  <div class="row-sub">Dark is Plumb's hero theme.</div>
                </div>
                <div class="seg">
                  <button :class="{ on: appState.theme === 'dark' }" @click="setTheme('dark')">Dark</button>
                  <button :class="{ on: appState.theme === 'light' }" @click="setTheme('light')">Light</button>
                </div>
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
.seg { display: flex; gap: 2px; margin-left: auto; }
.seg button { padding: 7px 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; font-weight: 600; color: var(--text-mid); cursor: pointer; }
.seg button.on { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }

.about { text-align: center; padding-top: var(--space-8); }
.about-mark { display: flex; justify-content: center; margin-bottom: var(--space-4); color: var(--text); }
.about-name { font-size: 32px; font-weight: 800; letter-spacing: -0.02em; }
.about-tag { font-size: 14px; color: var(--text-mid); margin-top: var(--space-2); }
.about-meta { font-size: 11px; color: var(--text-faint); line-height: 1.7; margin-top: var(--space-6); }
</style>
