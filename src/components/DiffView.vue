<script setup lang="ts">
// Loads a working-tree diff (staged or unstaged) and renders it via DiffBody.
// Optionally shows a per-hunk action button (stage/unstage) and forwards clicks.
import { ref, watch } from "vue";
import { fileDiff, type FileDiff } from "../lib/git";
import { diffReloadKey } from "../lib/ui";
import DiffBody from "./DiffBody.vue";

const props = defineProps<{
  repoPath: string;
  file: string | null;
  staged: boolean;
  actionLabel?: string;
  refresh?: number;
  selectable?: boolean;
}>();

defineEmits<{
  (e: "hunkAction", index: number): void;
  (e: "lineAction", hunkIndex: number, lines: number[]): void;
}>();

const diff = ref<FileDiff | null>(null);
const loading = ref(false);
// "Show anyway" opt-in for diffs over the backend line cap. Reset when the
// file changes, but not on refresh/reload so a stage/unstage keeps it open.
// Declared before the load watcher so it runs first in the same flush.
const force = ref(false);
watch(() => [props.file, props.staged, props.repoPath] as const, () => (force.value = false));

// Ignore responses from superseded loads (a slow forced load must not
// overwrite the diff of a file selected afterwards).
let loadSeq = 0;
async function load() {
  if (!props.file) {
    diff.value = null;
    return;
  }
  const mine = ++loadSeq;
  loading.value = true;
  try {
    const d = await fileDiff(props.repoPath, props.file, props.staged, force.value);
    if (mine === loadSeq) diff.value = d;
  } catch {
    if (mine === loadSeq) diff.value = null;
  } finally {
    if (mine === loadSeq) loading.value = false;
  }
}
watch(() => [props.file, props.staged, props.repoPath, props.refresh, diffReloadKey.value] as const, load, {
  immediate: true,
});
function showAnyway() {
  force.value = true;
  void load();
}
</script>

<template>
  <div class="diff-wrap">
    <div v-if="!file" class="empty">Select a file to see its changes.</div>
    <DiffBody
      v-else
      :hunks="diff?.hunks ?? []"
      :binary="diff?.binary"
      :loading="loading"
      :action-label="actionLabel"
      :selectable="selectable"
      :file-path="file"
      :truncated="diff?.truncated"
      :total-lines="diff?.total_lines"
      @show-anyway="showAnyway"
      @hunk-action="(i) => $emit('hunkAction', i)"
      @line-action="(hi, lines) => $emit('lineAction', hi, lines)"
    />
  </div>
</template>

<style scoped>
.diff-wrap { height: 100%; }
.empty { padding: var(--space-6); color: var(--text-faint); font-size: 12.5px; }
</style>
