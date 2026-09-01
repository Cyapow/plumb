<script setup lang="ts">
// App-level confirm (window.confirm no-ops in WKWebView). Reads the shared
// confirmDialog state; Enter confirms, Esc cancels.
import { confirmDialog, resolveConfirm } from "../lib/ui";
</script>

<template>
  <teleport to="body">
    <transition name="cd-fade">
      <div v-if="confirmDialog.open" class="backdrop" @click.self="resolveConfirm(false)">
        <div class="sheet" :class="{ danger: confirmDialog.danger }" tabindex="0" @keydown.enter.prevent="resolveConfirm(true)" @keydown.esc.prevent="resolveConfirm(false)">
          <div class="glyph" :class="confirmDialog.danger ? 'is-danger' : 'is-accent'">{{ confirmDialog.danger ? "!" : "?" }}</div>
          <h2>{{ confirmDialog.title }}</h2>
          <p v-if="confirmDialog.body" class="body">{{ confirmDialog.body }}</p>
          <div class="actions">
            <button class="btn ghost" @click="resolveConfirm(false)">Cancel</button>
            <button :class="['btn', confirmDialog.danger ? 'danger' : 'accent']" autofocus @click="resolveConfirm(true)">
              {{ confirmDialog.confirmLabel }}
            </button>
          </div>
        </div>
      </div>
    </transition>
  </teleport>
</template>

<style scoped>
.backdrop {
  position: fixed; inset: 0; z-index: 1250;
  background: color-mix(in srgb, #000 60%, transparent);
  backdrop-filter: blur(2px);
  display: flex; align-items: flex-start; justify-content: center; padding-top: 16vh;
}
.sheet {
  position: relative;
  width: 440px; max-width: calc(100vw - 48px);
  background: var(--surface); border: 1px solid var(--line);
  border-top: 2px solid var(--accent);
  box-shadow: 0 18px 50px -12px color-mix(in srgb, #000 70%, transparent);
  padding: var(--space-6);
  outline: none;
}
.sheet.danger { border-top-color: #e5484d; }
.glyph {
  width: 32px; height: 32px;
  display: grid; place-items: center;
  font-size: 18px; font-weight: 800; line-height: 1;
  margin-bottom: var(--space-4);
}
.glyph.is-accent { background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent); }
.glyph.is-danger { background: color-mix(in srgb, #e5484d 18%, transparent); color: #ff6b6f; }
h2 { margin: 0 0 var(--space-3); font-size: 15px; font-weight: 800; line-height: 1.35; }
.body { margin: 0; font-size: 12.5px; color: var(--text-mid); line-height: 1.6; white-space: pre-line; }
/* Match the rest of the app's buttons: sharp, 1px line, compact. */
.actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-6); }
.btn {
  height: 30px; padding: 0 16px;
  font-weight: 600; font-size: 12.5px; cursor: pointer;
  border: 1px solid var(--line); background: var(--raised); color: var(--text);
}
.btn.ghost:hover { border-color: var(--text-faint); color: var(--text); }
.btn.accent { background: var(--accent); color: var(--accent-on); border-color: var(--accent); font-weight: 700; }
.btn.accent:hover { filter: brightness(1.06); }
.btn.danger { background: #e5484d; color: #fff; border-color: #e5484d; font-weight: 700; }
.btn.danger:hover { background: #d33a3f; border-color: #d33a3f; }

.cd-fade-enter-active, .cd-fade-leave-active { transition: opacity 0.12s ease; }
.cd-fade-enter-from, .cd-fade-leave-to { opacity: 0; }
</style>
