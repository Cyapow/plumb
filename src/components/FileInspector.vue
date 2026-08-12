<script setup lang="ts">
// File inspector modal: History (commits that touched the file) + Blame.
import { ref, watch } from "vue";
import { fileHistory, blameFile, type FileCommit, type BlameLine } from "../lib/git";
import { fileInspector } from "../lib/ui";
import { relativeTime } from "../lib/format";
import { highlightLine, langFromPath } from "../lib/highlight";

const history = ref<FileCommit[]>([]);
const blame = ref<BlameLine[]>([]);
const loading = ref(false);
const error = ref<string | null>(null);

async function load() {
  error.value = null;
  loading.value = true;
  try {
    if (fileInspector.tab === "history") {
      history.value = await fileHistory(fileInspector.repoPath, fileInspector.file);
    } else {
      blame.value = await blameFile(fileInspector.repoPath, fileInspector.file);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

watch(
  () => [fileInspector.open, fileInspector.file, fileInspector.tab] as const,
  () => {
    if (fileInspector.open) load();
  },
  { immediate: true },
);

function close() {
  fileInspector.open = false;
}
const lang = () => langFromPath(fileInspector.file);
const hl = (s: string) => highlightLine(s, lang());
</script>

<template>
  <teleport to="body">
    <div v-if="fileInspector.open" class="fi-backdrop" @click.self="close">
      <div class="fi">
        <div class="fi-head">
          <span class="fi-path mono">{{ fileInspector.file }}</span>
          <div class="fi-tabs">
            <button :class="{ on: fileInspector.tab === 'history' }" @click="fileInspector.tab = 'history'">History</button>
            <button :class="{ on: fileInspector.tab === 'blame' }" @click="fileInspector.tab = 'blame'">Blame</button>
          </div>
          <button class="fi-x" @click="close">✕</button>
        </div>

        <div class="fi-body">
          <div v-if="loading" class="msg">Loading…</div>
          <div v-else-if="error" class="msg err mono">{{ error }}</div>

          <!-- History -->
          <div v-else-if="fileInspector.tab === 'history'" class="hist">
            <div v-for="c in history" :key="c.id" class="hc">
              <span class="hc-hash mono">{{ c.short_id }}</span>
              <span class="hc-msg ellipsis">{{ c.summary }}</span>
              <span class="hc-meta mono">{{ c.author_name }} · {{ relativeTime(c.time) }}</span>
            </div>
            <div v-if="!history.length" class="msg">No history for this file.</div>
          </div>

          <!-- Blame -->
          <div v-else class="blame mono">
            <div v-for="b in blame" :key="b.line" class="bl">
              <span class="bl-commit" :title="b.author">{{ b.short_id || "·" }}</span>
              <span class="bl-author ellipsis">{{ b.author }}</span>
              <span class="bl-no">{{ b.line }}</span>
              <span class="bl-content" v-html="hl(b.content)"></span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.fi-backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.fi { width: 900px; max-width: calc(100vw - 48px); height: 640px; max-height: calc(100vh - 80px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.fi-head { display: flex; align-items: center; gap: var(--space-3); padding: 0 var(--space-3) 0 var(--space-4); height: 44px; border-bottom: 2px solid var(--line); flex: none; }
.fi-path { flex: 1; font-size: 12.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.fi-tabs { display: flex; gap: 2px; flex: none; }
.fi-tabs button { font-size: 12px; font-weight: 600; padding: 5px 12px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.fi-tabs button.on { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }
.fi-x { width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.fi-body { flex: 1; overflow: auto; }
.msg { padding: var(--space-6); color: var(--text-faint); font-size: 13px; }
.msg.err { color: var(--accent); font-size: 12px; }

.hc { display: flex; align-items: baseline; gap: var(--space-3); padding: 9px var(--space-4); border-bottom: 1px solid var(--line-soft); }
.hc-hash { font-size: 11.5px; color: var(--accent); font-weight: 700; flex: none; }
.hc-msg { flex: 1; font-size: 13px; }
.hc-meta { font-size: 10.5px; color: var(--text-faint); flex: none; }

.blame { font-family: var(--code-font); font-size: var(--code-font-size); line-height: var(--code-line-h); }
.bl { display: flex; align-items: baseline; }
.bl-commit { width: 64px; flex: none; padding-left: var(--space-3); color: var(--accent); }
.bl-author { width: 120px; flex: none; color: var(--text-faint); font-size: 10.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bl-no { width: 44px; flex: none; text-align: right; padding-right: var(--space-3); color: var(--text-faint); user-select: none; }
.bl-content { flex: 1; white-space: pre; color: var(--text); }
.bl:hover { background: color-mix(in srgb, var(--raised) 50%, transparent); }
</style>
