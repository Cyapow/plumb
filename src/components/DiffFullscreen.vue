<script setup lang="ts">
// Full-screen diff: the diff fills the area below the app header, the changed-
// file list is docked (and resizable) on the right so you can move between files
// without leaving the view. Modelled on GitKraken's expanded diff. The app
// toolbar stays put above this — Esc or the ✕ closes it.
import { ref, watch, onMounted, onUnmounted } from "vue";
import { fullscreen, closeFullscreen, prefs, toggleDiffSplit, setIgnoreWs, diffReloadKey } from "../lib/ui";
import type { FileDiff } from "../lib/git";
import DiffBody from "./DiffBody.vue";
import ResizeHandle from "./ResizeHandle.vue";

const diff = ref<FileDiff | null>(null);
const loading = ref(false);
const listW = ref(320);

async function loadActive() {
  if (!fullscreen.load || !fullscreen.activeFile) {
    diff.value = null;
    return;
  }
  loading.value = true;
  try {
    diff.value = await fullscreen.load(fullscreen.activeFile);
  } catch {
    diff.value = null;
  } finally {
    loading.value = false;
  }
}

watch(() => [fullscreen.open, fullscreen.activeFile, diffReloadKey.value], loadActive, { immediate: true });

function onKey(e: KeyboardEvent) {
  if (!fullscreen.open) return;
  if (e.key === "Escape") closeFullscreen();
  if (e.key === "ArrowDown" || e.key === "j" || e.key === "ArrowUp" || e.key === "k") {
    const idx = fullscreen.files.findIndex((f) => f.path === fullscreen.activeFile);
    const delta = e.key === "ArrowDown" || e.key === "j" ? 1 : -1;
    const next = fullscreen.files[idx + delta];
    if (next) {
      e.preventDefault();
      fullscreen.activeFile = next.path;
    }
  }
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));

const codeClass = (c: string) =>
  ({ A: "add", "?": "add", M: "mod", D: "del", R: "mod", U: "conflict" })[c] ?? "mod";
</script>

<template>
  <div class="fs">
    <div class="fs-bar">
      <span class="path mono">{{ fullscreen.activeFile ?? "" }}</span>
      <span class="ctx">{{ fullscreen.subtitle }}</span>
      <span class="grow"></span>
      <button class="opt" :class="{ on: prefs.ignoreWs }" title="Ignore whitespace" @click="setIgnoreWs(!prefs.ignoreWs)">ws</button>
      <button class="opt" :class="{ on: prefs.split }" title="Toggle side-by-side" @click="toggleDiffSplit">
        {{ prefs.split ? "Split" : "Unified" }}
      </button>
      <button class="close" title="Close (Esc)" @click="closeFullscreen">✕ Close</button>
    </div>
    <div class="fs-body">
      <DiffBody
        class="fs-diff"
        :hunks="diff?.hunks ?? []"
        :binary="diff?.binary"
        :loading="loading"
        :file-path="fullscreen.activeFile"
        empty-text="Select a file on the right."
      />
      <ResizeHandle v-model="listW" side="right" :min="220" :max="560" />
      <aside class="fs-files" :style="{ width: listW + 'px' }">
        <div class="fs-files-head">
          <span class="title mono">{{ fullscreen.title }}</span>
          <span class="count mono">{{ fullscreen.files.length }} files</span>
        </div>
        <div
          v-for="f in fullscreen.files"
          :key="f.path"
          class="fs-file mono"
          :class="{ active: f.path === fullscreen.activeFile }"
          @click="fullscreen.activeFile = f.path"
        >
          <span class="code" :class="codeClass(f.code)">{{ f.code }}</span>
          <span class="p ellipsis">{{ f.path }}</span>
        </div>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.fs { flex: 1; display: flex; flex-direction: column; min-height: 0; background: var(--bg); }
.fs-bar {
  height: 34px;
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 0 var(--space-2) 0 var(--space-4);
  background: var(--subtle);
  border-bottom: 2px solid var(--line);
}
.fs-bar .path { font-size: 12.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.fs-bar .ctx { font-size: 11px; color: var(--text-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 40%; }
.fs-bar .grow { flex: 1; }
.opt { height: 24px; padding: 0 10px; margin-right: var(--space-2); background: var(--raised); border: 1px solid var(--line); cursor: pointer; font-size: 11.5px; white-space: nowrap; color: var(--text-mid); }
.opt.on { border-color: var(--accent); color: var(--accent); }
.close { height: 24px; padding: 0 10px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; font-size: 11.5px; white-space: nowrap; }

.fs-body { flex: 1; display: flex; min-height: 0; }
.fs-diff { flex: 1; min-width: 0; }
.fs-files {
  flex: none;
  border-left: 2px solid var(--line);
  background: var(--surface);
  overflow-y: auto;
}
.fs-files-head {
  display: flex; align-items: center; gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--line);
  position: sticky; top: 0; background: var(--surface);
}
.fs-files-head .title { font-size: 12px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.fs-files-head .count { margin-left: auto; font-size: 10.5px; color: var(--text-faint); flex: none; }
.fs-file {
  height: 30px;
  display: flex; align-items: center; gap: var(--space-2);
  padding: 0 var(--space-4);
  font-size: 11.5px;
  color: var(--text-mid);
  cursor: pointer;
  border-bottom: 1px solid var(--line-soft);
}
.fs-file:hover { background: color-mix(in srgb, var(--raised) 55%, transparent); }
.fs-file.active { background: var(--raised); box-shadow: inset 2px 0 0 var(--accent); color: var(--text); }
.fs-file .code { width: 12px; flex: none; font-weight: 700; }
.fs-file .p { flex: 1; }
.code.add { color: var(--lane-1); }
.code.mod { color: var(--lane-2); }
.code.del { color: var(--diff-del-fg); }
.code.conflict { color: var(--accent); }
</style>
