<script setup lang="ts">
import { toasts } from "../lib/ui";
</script>

<template>
  <teleport to="body">
    <div class="toast-stack">
      <div v-for="t in toasts.list" :key="t.id" class="toast" :class="t.kind">
        <span class="glyph">{{ t.kind === "ok" ? "✓" : "!" }}</span>
        <div class="text">
          <div class="title">{{ t.title }}</div>
          <div v-if="t.detail" class="detail mono">{{ t.detail }}</div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.toast-stack {
  position: fixed;
  left: var(--space-4);
  bottom: var(--space-6);
  z-index: 1300;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.toast {
  display: flex;
  gap: var(--space-3);
  align-items: center;
  min-width: 240px;
  max-width: 360px;
  padding: var(--space-3) var(--space-4);
  background: var(--raised);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-lg);
}
.toast .glyph {
  width: 22px; height: 22px; flex: none;
  display: flex; align-items: center; justify-content: center;
  font-weight: 700; color: var(--accent-on);
  background: var(--lane-3);
}
.toast.error .glyph { background: var(--accent); }
.toast .title { font-size: 13px; font-weight: 600; }
.toast .detail { font-size: 11px; color: var(--text-dim); margin-top: 2px; }
</style>
