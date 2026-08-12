<script setup lang="ts">
// Rendered once at the app root. Reads the shared contextMenu state and shows
// a floating menu at the cursor. Closes on outside-click, Esc, or item choice.
import { computed, onMounted, onUnmounted } from "vue";
import { contextMenu, closeContextMenu, type MenuItem } from "../lib/ui";

function onKey(e: KeyboardEvent) {
  if (contextMenu.open && e.key === "Escape") closeContextMenu();
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));

const style = computed(() => {
  // Keep the menu on-screen by nudging away from the right/bottom edges.
  const maxX = window.innerWidth - 220;
  const maxY = window.innerHeight - contextMenu.items.length * 30 - 16;
  return {
    left: Math.min(contextMenu.x, maxX) + "px",
    top: Math.min(contextMenu.y, Math.max(8, maxY)) + "px",
  };
});

function choose(item: MenuItem) {
  if (item.disabled || item.separator) return;
  closeContextMenu();
  item.action?.();
}
</script>

<template>
  <teleport to="body">
    <div v-if="contextMenu.open" class="cm-backdrop" @click="closeContextMenu" @contextmenu.prevent="closeContextMenu">
      <div class="cm" :style="style" @click.stop>
        <template v-for="(item, i) in contextMenu.items" :key="i">
          <div v-if="item.separator" class="cm-sep"></div>
          <div
            v-else
            class="cm-item"
            :class="{ danger: item.danger, disabled: item.disabled, checked: item.checked }"
            @click="choose(item)"
          >
            <span class="tick">{{ item.checked ? "✓" : "" }}</span>{{ item.label }}
          </div>
        </template>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.cm-backdrop { position: fixed; inset: 0; z-index: 1000; }
.cm {
  position: fixed;
  min-width: 200px;
  background: var(--raised);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-lg);
  padding: 4px 0;
  font-size: 12.5px;
}
.cm-item { padding: 6px 14px; color: var(--text); cursor: pointer; white-space: nowrap; }
.cm-item .tick { display: inline-block; width: 14px; color: var(--accent); }
.cm-item:hover { background: color-mix(in srgb, var(--accent) 18%, var(--raised)); }
.cm-item.danger { color: var(--accent); }
.cm-item.danger:hover { background: var(--accent); color: var(--accent-on); }
.cm-item.disabled { color: var(--text-faint); cursor: default; }
.cm-item.disabled:hover { background: transparent; }
.cm-sep { height: 1px; background: var(--line); margin: 4px 0; }
</style>
