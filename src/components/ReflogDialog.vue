<script setup lang="ts">
// The HEAD reflog — every place HEAD has been. A safety net: if a reset or
// rebase lost commits, they're still reachable here. Row actions are handled by
// the parent (which owns branch/reset/checkout + refresh).
import { ref, watch } from "vue";
import { reflog, type ReflogEntry } from "../lib/git";
import { relativeTime } from "../lib/format";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string }>();
const emit = defineEmits<{ (e: "menu", ev: MouseEvent, entry: ReflogEntry): void }>();

const entries = ref<ReflogEntry[]>([]);
const loading = ref(false);

watch(open, async (o) => {
  if (!o) return;
  loading.value = true;
  try {
    entries.value = await reflog(props.repoPath);
  } catch {
    entries.value = [];
  } finally {
    loading.value = false;
  }
});

// Colour the action tag by kind.
function tagClass(action: string) {
  const a = action.toLowerCase();
  if (a.includes("commit")) return "commit";
  if (a.includes("reset")) return "reset";
  if (a.includes("rebase")) return "rebase";
  if (a.includes("merge") || a.includes("pull")) return "merge";
  if (a.includes("checkout")) return "checkout";
  return "other";
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head">
          <h2>History (reflog)</h2>
          <span class="sub mono">recover anything HEAD has pointed at</span>
          <button class="x" @click="open = false">✕</button>
        </div>
        <div class="body">
          <div v-if="loading" class="msg">Reading reflog…</div>
          <div v-else-if="!entries.length" class="msg">No reflog entries.</div>
          <div
            v-for="e in entries"
            :key="e.index"
            class="row"
            @contextmenu.prevent="emit('menu', $event, e)"
            @click="emit('menu', $event, e)"
          >
            <span class="sel mono">HEAD@{{ '{' + e.index + '}' }}</span>
            <span class="tag" :class="tagClass(e.action)">{{ e.action || "—" }}</span>
            <span class="sha mono">{{ e.short_id }}</span>
            <span class="msg-text">{{ e.message.replace(/^[^:]+:\s*/, "") }}</span>
            <span class="when mono">{{ relativeTime(e.time) }}</span>
          </div>
        </div>
        <div class="foot mono">Click a row for actions — create a branch here, or reset to it.</div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 60%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 820px; max-width: calc(100vw - 40px); height: 78vh; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 16px; font-weight: 800; }
.sub { font-size: 11px; color: var(--text-dim); }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { flex: 1; overflow-y: auto; }
.msg { padding: var(--space-6); text-align: center; color: var(--text-faint); font-size: 13px; }
.row { display: flex; align-items: center; gap: var(--space-3); padding: 7px var(--space-4); border-bottom: 1px solid var(--line-soft); cursor: pointer; font-size: 12.5px; }
.row:hover { background: color-mix(in srgb, var(--accent) 12%, var(--surface)); }
.sel { flex: none; width: 84px; color: var(--text-faint); font-size: 11px; }
.tag { flex: none; width: 72px; text-align: center; font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; padding: 2px 0; color: var(--accent-on); background: var(--text-dim); }
.tag.commit { background: var(--lane-1); }
.tag.reset { background: var(--lane-6); }
.tag.rebase { background: var(--lane-4); }
.tag.merge { background: var(--lane-3); }
.tag.checkout { background: var(--lane-2); }
.tag.other { background: var(--text-faint); }
.sha { flex: none; width: 60px; color: var(--text-dim); }
.msg-text { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text); }
.when { flex: none; color: var(--text-faint); font-size: 11px; }
.foot { padding: 8px var(--space-4); border-top: 1px solid var(--line); font-size: 10.5px; color: var(--text-faint); }
</style>
