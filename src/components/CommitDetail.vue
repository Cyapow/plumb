<script setup lang="ts">
// Right-dock panel for a selected history commit: metadata + changed-file list.
// Clicking a file opens the full-screen diff (list docked on the right).
import { ref, watch } from "vue";
import { commitDetails, commitFileDiff, type CommitDetail } from "../lib/git";
import { initials } from "../lib/format";
import { openFullscreen } from "../lib/ui";

const props = defineProps<{ repoPath: string; commitId: string | null }>();
defineEmits<{ (e: "close"): void }>();

const detail = ref<CommitDetail | null>(null);

watch(
  () => [props.commitId, props.repoPath] as const,
  async () => {
    detail.value = null;
    if (!props.commitId) return;
    try {
      detail.value = await commitDetails(props.repoPath, props.commitId);
    } catch {
      detail.value = null;
    }
  },
  { immediate: true },
);

function openFile(file: string) {
  if (!detail.value || !props.commitId) return;
  const id = props.commitId;
  openFullscreen({
    title: detail.value.short_id,
    subtitle: detail.value.summary,
    files: detail.value.files,
    activeFile: file,
    load: (f) => commitFileDiff(props.repoPath, id, f),
  });
}

const codeClass = (c: string) =>
  ({ A: "add", "?": "add", M: "mod", D: "del", R: "mod", U: "conflict" })[c] ?? "mod";

function fmtDate(unix: number): string {
  return new Date(unix * 1000).toLocaleString(undefined, {
    weekday: "short",
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
</script>

<template>
  <div class="panel">
    <div class="panel-head">
      <span class="mono short">{{ detail?.short_id ?? "" }}</span>
      <span v-if="detail && detail.parents.length > 1" class="merge-tag mono">
        merge · {{ detail.parents.length }} parents
      </span>
      <span class="grow"></span>
      <button class="close" title="Close (or click the commit again)" @click="$emit('close')">✕</button>
    </div>

    <div v-if="detail" class="panel-body">
      <div class="summary">{{ detail.summary }}</div>
      <div class="author">
        <span class="avatar mono">{{ initials(detail.author_name) }}</span>
        <div class="who">
          <div class="mono name">{{ detail.author_name }} &lt;{{ detail.author_email }}&gt;</div>
          <div class="mono when">{{ fmtDate(detail.time) }}</div>
        </div>
      </div>
      <p v-if="detail.body" class="body">{{ detail.body }}</p>

      <div class="files-label section-label">
        Changed files <span class="count mono">{{ detail.files.length }}</span>
        <span class="hint">click a file →</span>
      </div>
      <div class="files">
        <div
          v-for="f in detail.files"
          :key="f.path"
          class="file-row mono"
          :title="`Open ${f.path} full screen`"
          @click="openFile(f.path)"
        >
          <span class="code" :class="codeClass(f.code)">{{ f.code }}</span>
          <span class="path ellipsis">{{ f.path }}</span>
          <span class="go">⤢</span>
        </div>
        <div v-if="!detail.files.length" class="empty">No file changes (empty or root commit).</div>
      </div>
    </div>
    <div v-else class="panel-body loading">Loading…</div>
  </div>
</template>

<style scoped>
.panel { flex: 1; display: flex; flex-direction: column; min-height: 0; background: var(--surface); }
.panel-head {
  height: 40px;
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-2) 0 var(--space-4);
  border-bottom: 2px solid var(--line);
}
.short { font-size: 12px; color: var(--accent); font-weight: 700; }
.merge-tag { font-size: 10px; color: var(--text-faint); }
.grow { flex: 1; }
.close { width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }

.panel-body { flex: 1; overflow-y: auto; padding: var(--space-4); }
.panel-body.loading { color: var(--text-faint); font-size: 12.5px; }

.summary { font-size: 14px; font-weight: 600; line-height: 1.4; margin-bottom: var(--space-3); }
.author { display: flex; gap: var(--space-2); align-items: center; margin-bottom: var(--space-3); }
.avatar {
  width: 24px; height: 24px; flex: none;
  display: flex; align-items: center; justify-content: center;
  background: var(--raised); font-size: 9px; font-weight: 700; color: var(--text-mid);
}
.who .name { font-size: 11px; color: var(--text-mid); overflow: hidden; text-overflow: ellipsis; }
.who .when { font-size: 10.5px; color: var(--text-faint); margin-top: 2px; }
.body { font-size: 12px; color: var(--text-mid); line-height: 1.55; white-space: pre-wrap; margin: 0 0 var(--space-4); }

.files-label {
  display: flex; align-items: center; gap: var(--space-2);
  padding: var(--space-3) 0 var(--space-2);
  border-top: 1px solid var(--line);
}
.files-label .count { color: var(--text-faint); font-size: 10.5px; }
.files-label .hint { margin-left: auto; font-size: 10px; color: var(--text-faint); text-transform: none; letter-spacing: 0; }

.file-row {
  height: 30px;
  display: flex; align-items: center; gap: var(--space-2);
  font-size: 11.5px; color: var(--text-mid);
  cursor: pointer;
  border-bottom: 1px solid var(--line-soft);
}
.file-row:hover { background: color-mix(in srgb, var(--raised) 60%, transparent); color: var(--text); }
.file-row:hover .go { opacity: 1; }
.file-row .code { width: 12px; flex: none; font-weight: 700; }
.file-row .path { flex: 1; }
.file-row .go { flex: none; opacity: 0; color: var(--text-faint); }
.code.add { color: var(--lane-1); }
.code.mod { color: var(--lane-2); }
.code.del { color: var(--diff-del-fg); }
.code.conflict { color: var(--accent); }
.empty { font-size: 11.5px; color: var(--text-faint); padding: var(--space-2) 0; }
</style>
