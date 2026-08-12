<script setup lang="ts">
// App-level text prompt (window.prompt doesn't work in WKWebView). Reads the
// shared inputDialog state; Enter confirms, Esc cancels.
import { nextTick, ref, watch } from "vue";
import { inputDialog, resolveInput } from "../lib/ui";

const field = ref<HTMLInputElement | null>(null);

watch(
  () => inputDialog.open,
  (open) => {
    if (open) nextTick(() => field.value?.focus());
  },
);

function confirm() {
  resolveInput(inputDialog.value);
}
function cancel() {
  resolveInput(null);
}
</script>

<template>
  <teleport to="body">
    <div v-if="inputDialog.open" class="backdrop" @click.self="cancel" @keydown.esc="cancel">
      <div class="sheet">
        <h2>{{ inputDialog.title }}</h2>
        <label v-if="inputDialog.label" class="lbl">{{ inputDialog.label }}</label>
        <input
          ref="field"
          v-model="inputDialog.value"
          :placeholder="inputDialog.placeholder"
          spellcheck="false"
          autocapitalize="off"
          @keydown.enter.prevent="confirm"
          @keydown.esc.prevent="cancel"
        />
        <div class="actions">
          <button class="btn-accent" @click="confirm">{{ inputDialog.confirmLabel }}</button>
          <button class="btn" @click="cancel">Cancel</button>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1250; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: flex-start; justify-content: center; padding-top: 18vh; }
.sheet { width: 420px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); padding: var(--space-4); }
h2 { margin: 0 0 var(--space-3); font-size: 15px; font-weight: 800; }
.lbl { display: block; font-size: 12px; color: var(--text-mid); margin-bottom: 6px; white-space: pre-line; }
input { width: 100%; height: 34px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
input:focus { outline: none; border-color: var(--accent); }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-4); }
.btn-accent { height: 32px; padding: 0 16px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn { height: 32px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
