<script setup lang="ts">
// Interactive rebase: reorder commits and set a per-commit action, then hand
// the plan to `git rebase -i`. Rows are shown oldest→newest (git todo order).
import { ref, watch } from "vue";
import { rebaseInteractive, type RebaseAction, type CommitRow } from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; base: string | null; commits: CommitRow[] }>();
const emit = defineEmits<{ (e: "done"): void }>();

interface Row {
  sha: string;
  summary: string;
  action: RebaseAction;
  message: string;
}
const rows = ref<Row[]>([]);
const busy = ref(false);

const ACTIONS: { value: RebaseAction; hint: string }[] = [
  { value: "pick", hint: "keep the commit as-is" },
  { value: "reword", hint: "keep changes, edit the message below" },
  { value: "squash", hint: "merge into the commit above, combine messages" },
  { value: "fixup", hint: "merge into the commit above, drop this message" },
  { value: "drop", hint: "discard this commit entirely" },
];

watch(open, (o) => {
  if (!o) return;
  // props.commits is newest-first; git applies oldest-first.
  rows.value = [...props.commits].reverse().map((c) => ({
    sha: c.id,
    summary: c.summary,
    action: "pick" as RebaseAction,
    message: c.summary,
  }));
});

function move(i: number, dir: -1 | 1) {
  const j = i + dir;
  if (j < 0 || j >= rows.value.length) return;
  const arr = rows.value;
  [arr[i], arr[j]] = [arr[j], arr[i]];
}

async function start() {
  busy.value = true;
  try {
    const steps = rows.value.map((r) => ({
      action: r.action,
      sha: r.sha,
      message: r.action === "reword" ? r.message : undefined,
    }));
    const msg = await rebaseInteractive(props.repoPath, props.base, steps);
    toast("Interactive rebase", msg);
    open.value = false;
    emit("done");
  } catch (e) {
    toast("Rebase failed", String(e), "error");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head">
          <h2>Interactive rebase</h2>
          <span class="sub mono">{{ rows.length }} commit{{ rows.length === 1 ? "" : "s" }}</span>
          <button class="x" @click="open = false">✕</button>
        </div>
        <p class="intro">Reorder with the arrows and choose an action per commit. Applied oldest first.</p>

        <div class="rows">
          <div v-for="(r, i) in rows" :key="r.sha" class="rowwrap">
            <div class="row" :class="{ drop: r.action === 'drop' }">
              <div class="mv">
                <button :disabled="i === 0" @click="move(i, -1)" title="Move up">▲</button>
                <button :disabled="i === rows.length - 1" @click="move(i, 1)" title="Move down">▼</button>
              </div>
              <span class="sha mono">{{ r.sha.slice(0, 7) }}</span>
              <span class="summary">{{ r.summary }}</span>
              <select v-model="r.action" :title="ACTIONS.find((a) => a.value === r.action)?.hint">
                <option v-for="a in ACTIONS" :key="a.value" :value="a.value">{{ a.value }}</option>
              </select>
            </div>
            <input v-if="r.action === 'reword'" v-model="r.message" class="reword" spellcheck="false" placeholder="New commit message" />
          </div>
        </div>

        <div class="foot">
          <span class="warn mono">Rewrites history on this branch. Force-push afterwards if already pushed.</span>
          <span class="grow"></span>
          <button class="btn" @click="open = false">Cancel</button>
          <button class="btn-accent" :disabled="busy || !rows.length" @click="start">
            {{ busy ? "Rebasing…" : "Start rebase" }}
          </button>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 60%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 720px; max-width: calc(100vw - 40px); max-height: 84vh; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 16px; font-weight: 800; }
.sub { font-size: 11px; color: var(--text-dim); }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.intro { margin: 0; padding: var(--space-3) var(--space-4) 0; font-size: 12px; color: var(--text-mid); }
.rows { flex: 1; overflow-y: auto; padding: var(--space-3) var(--space-4); }
.row { display: flex; align-items: center; gap: var(--space-3); padding: 6px 8px; border: 1px solid var(--line); margin-bottom: 4px; background: var(--bg); }
.row.drop { opacity: 0.5; text-decoration: line-through; }
.reword { width: 100%; height: 28px; margin: 2px 0 6px; padding: 0 10px; background: var(--bg); border: 1px solid var(--accent); color: var(--text); font-size: 12.5px; }
.reword:focus { outline: none; }
.mv { display: flex; flex-direction: column; gap: 1px; }
.mv button { width: 20px; height: 15px; line-height: 1; font-size: 8px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; padding: 0; }
.mv button:disabled { opacity: 0.35; }
.sha { flex: none; font-size: 11px; color: var(--text-dim); }
.summary { flex: 1; font-size: 12.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
select { flex: none; height: 26px; background: var(--raised); border: 1px solid var(--line); color: var(--text); font-size: 12px; padding: 0 6px; }
.foot { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3) var(--space-4); border-top: 1px solid var(--line); }
.warn { font-size: 10.5px; color: var(--text-dim); }
.btn { height: 32px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.btn-accent { height: 32px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.5; }
</style>
