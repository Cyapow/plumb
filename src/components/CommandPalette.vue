<script setup lang="ts">
// ⌘K command palette: fuzzy-jump to branches/commits and run any action.
import { computed, nextTick, ref, watch } from "vue";
import { fuzzyScore } from "../lib/fuzzy";
import type { PaletteItem } from "../lib/palette";

const props = defineProps<{ open: boolean; items: PaletteItem[] }>();
const emit = defineEmits<{ (e: "close"): void }>();

const query = ref("");
const activeIndex = ref(0);
const inputEl = ref<HTMLInputElement | null>(null);
const listEl = ref<HTMLElement | null>(null);

const results = computed(() => {
  return props.items
    .map((it) => ({ it, s: fuzzyScore(query.value, `${it.label} ${it.hint ?? ""}`) }))
    .filter((x) => x.s !== null)
    .sort((a, b) => (b.s as number) - (a.s as number))
    .slice(0, 60)
    .map((x) => x.it);
});

watch(
  () => props.open,
  (o) => {
    if (o) {
      query.value = "";
      activeIndex.value = 0;
      nextTick(() => inputEl.value?.focus());
    }
  },
);
watch(results, () => (activeIndex.value = 0));

function scrollActive() {
  nextTick(() => listEl.value?.querySelector(".on")?.scrollIntoView({ block: "nearest" }));
}
function onKey(e: KeyboardEvent) {
  if (e.key === "ArrowDown") {
    e.preventDefault();
    activeIndex.value = Math.min(activeIndex.value + 1, results.value.length - 1);
    scrollActive();
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    activeIndex.value = Math.max(activeIndex.value - 1, 0);
    scrollActive();
  } else if (e.key === "Enter") {
    e.preventDefault();
    choose(results.value[activeIndex.value]);
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
  }
}
function choose(it?: PaletteItem) {
  if (!it) return;
  emit("close");
  it.action();
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="pal-backdrop" @click.self="$emit('close')" @contextmenu.prevent>
      <div class="pal">
        <input
          ref="inputEl"
          v-model="query"
          class="pal-input"
          placeholder="Jump to a branch, commit, or run a command…"
          spellcheck="false"
          @keydown="onKey"
        />
        <div ref="listEl" class="pal-list">
          <div
            v-for="(it, i) in results"
            :key="it.id"
            class="pal-item"
            :class="{ on: i === activeIndex }"
            @click="choose(it)"
            @mousemove="activeIndex = i"
          >
            <span class="pal-label">{{ it.label }}</span>
            <span v-if="it.hint" class="pal-hint mono">{{ it.hint }}</span>
            <span class="pal-group mono">{{ it.group }}</span>
          </div>
          <div v-if="!results.length" class="pal-empty">No matches</div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.pal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1250;
  background: color-mix(in srgb, #000 45%, transparent);
  display: flex;
  justify-content: center;
  padding-top: 12vh;
}
.pal {
  width: 640px;
  max-width: calc(100vw - 48px);
  max-height: 60vh;
  background: var(--surface);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-lg);
  display: flex;
  flex-direction: column;
}
.pal-input {
  height: 48px;
  padding: 0 var(--space-4);
  background: var(--surface);
  border: none;
  border-bottom: 2px solid var(--line);
  color: var(--text);
  font-size: 15px;
  font-family: var(--font-ui);
}
.pal-input:focus { outline: none; }
.pal-list { overflow-y: auto; }
.pal-item {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  padding: 9px var(--space-4);
  cursor: pointer;
  border-bottom: 1px solid var(--line-soft);
}
.pal-item.on { background: var(--accent); color: var(--accent-on); }
.pal-label { flex: 1; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pal-hint { font-size: 11px; color: var(--text-faint); flex: none; }
.pal-item.on .pal-hint { color: color-mix(in srgb, var(--accent-on) 80%, transparent); }
.pal-group { font-size: 10px; color: var(--text-faint); flex: none; text-transform: uppercase; letter-spacing: 0.08em; }
.pal-item.on .pal-group { color: color-mix(in srgb, var(--accent-on) 80%, transparent); }
.pal-empty { padding: var(--space-6); text-align: center; color: var(--text-faint); font-size: 13px; }
</style>
