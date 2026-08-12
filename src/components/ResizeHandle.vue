<script setup lang="ts">
// A thin draggable divider that resizes an adjacent panel.
//  side="left"  → handle sits on the RIGHT edge of a left-hand panel (width grows as you drag right)
//  side="right" → handle sits on the LEFT edge of a right-hand panel (width grows as you drag left)
const props = defineProps<{ side: "left" | "right"; min?: number; max?: number }>();
const width = defineModel<number>({ required: true });

function onDown(e: MouseEvent) {
  const startX = e.clientX;
  const startW = width.value;
  const min = props.min ?? 140;
  const max = props.max ?? 700;

  function move(ev: MouseEvent) {
    const dx = ev.clientX - startX;
    const next = props.side === "left" ? startW + dx : startW - dx;
    width.value = Math.max(min, Math.min(max, next));
  }
  function up() {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", up);
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  }
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  e.preventDefault();
}
</script>

<template>
  <div class="resizer" @mousedown="onDown" @dblclick="width = side === 'left' ? 250 : 360"></div>
</template>

<style scoped>
.resizer {
  width: 6px;
  flex: none;
  margin: 0 -3px;
  z-index: 6;
  cursor: col-resize;
  background: transparent;
}
.resizer:hover { background: color-mix(in srgb, var(--accent) 45%, transparent); }
.resizer:active { background: var(--accent); }
</style>
