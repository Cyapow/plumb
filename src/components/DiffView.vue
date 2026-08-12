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

watch(
  () => [props.file, props.staged, props.repoPath, props.refresh, diffReloadKey.value] as const,
  async () => {
    if (!props.file) {
      diff.value = null;
      return;
    }
    loading.value = true;
    try {
      diff.value = await fileDiff(props.repoPath, props.file, props.staged);
    } catch {
      diff.value = null;
    } finally {
      loading.value = false;
    }
  },
  { immediate: true },
);
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
      @hunk-action="(i) => $emit('hunkAction', i)"
      @line-action="(hi, lines) => $emit('lineAction', hi, lines)"
    />
  </div>
</template>

<style scoped>
.diff-wrap { height: 100%; }
.empty { padding: var(--space-6); color: var(--text-faint); font-size: 12.5px; }
</style>
