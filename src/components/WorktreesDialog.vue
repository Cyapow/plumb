<script setup lang="ts">
// Manage git worktrees: open, add, remove.
import { ref, watch } from "vue";
import { openFolder } from "../lib/native";
import { listWorktrees, addWorktree, removeWorktree, type WorktreeInfo } from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; branches: string[] }>();
const emit = defineEmits<{ (e: "open", path: string): void }>();

const trees = ref<WorktreeInfo[]>([]);
const busy = ref(false);
const newPath = ref("");
const branch = ref("");
const newBranch = ref(false);

async function reload() {
  trees.value = await listWorktrees(props.repoPath).catch(() => []);
}
watch(open, (o) => {
  if (!o) return;
  newPath.value = "";
  branch.value = "";
  newBranch.value = false;
  reload();
});

async function chooseDir() {
  const picked = await openFolder("New worktree folder");
  if (picked) newPath.value = picked;
}

async function add() {
  if (!newPath.value.trim() || !branch.value.trim()) return;
  busy.value = true;
  try {
    const msg = await addWorktree(props.repoPath, newPath.value.trim(), branch.value.trim(), newBranch.value);
    toast("Worktree", msg);
    newPath.value = "";
    branch.value = "";
    await reload();
  } catch (e) {
    toast("Couldn't add worktree", String(e), "error");
  } finally {
    busy.value = false;
  }
}

async function remove(t: WorktreeInfo) {
  if (!window.confirm(`Remove worktree at ${t.path}? (Your commits stay in the repo.)`)) return;
  try {
    await removeWorktree(props.repoPath, t.path);
    await reload();
  } catch (e) {
    toast("Couldn't remove", String(e), "error");
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head"><h2>Worktrees</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <div v-for="t in trees" :key="t.path" class="wt">
            <div class="w-top">
              <span class="w-branch mono">{{ t.branch }}</span>
              <span class="w-head mono">{{ t.head }}</span>
              <span v-if="t.is_main" class="badge">main</span>
              <span class="grow"></span>
              <button class="mini" @click="emit('open', t.path); open = false">Open</button>
              <button class="mini danger" :disabled="t.is_main" @click="remove(t)">Remove</button>
            </div>
            <div class="w-path mono">{{ t.path }}</div>
          </div>

          <div class="add">
            <div class="add-label">Add a worktree</div>
            <label class="check"><input type="checkbox" v-model="newBranch" /> Create a new branch</label>
            <div class="add-row">
              <input v-model="branch" :placeholder="newBranch ? 'new branch name' : 'existing branch'" list="wt-branches" spellcheck="false" />
              <datalist id="wt-branches"><option v-for="b in branches" :key="b" :value="b" /></datalist>
              <input v-model="newPath" placeholder="folder…" spellcheck="false" />
              <button class="btn" @click="chooseDir">…</button>
              <button class="btn-accent" :disabled="busy || !newPath.trim() || !branch.trim()" @click="add">Add</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 660px; max-width: calc(100vw - 48px); max-height: 74vh; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); overflow-y: auto; }
.wt { border: 1px solid var(--line); padding: 10px 12px; margin-bottom: var(--space-2); }
.w-top { display: flex; align-items: center; gap: var(--space-2); }
.w-branch { font-size: 13px; font-weight: 700; }
.w-head { font-size: 11px; color: var(--text-dim); }
.badge { font-size: 10px; font-weight: 700; text-transform: uppercase; padding: 2px 7px; background: var(--lane-1); color: var(--accent-on); }
.grow { flex: 1; }
.mini { padding: 3px 10px; font-size: 11px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.mini.danger { color: var(--accent); }
.mini:disabled { opacity: 0.4; }
.w-path { font-size: 11px; color: var(--text-dim); margin-top: 6px; word-break: break-all; }
.add { margin-top: var(--space-4); border-top: 1px solid var(--line); padding-top: var(--space-3); }
.add-label { font-size: 11px; color: var(--text-dim); margin-bottom: 6px; }
.check { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-mid); margin-bottom: var(--space-2); cursor: pointer; }
.add-row { display: flex; gap: var(--space-2); }
.add-row input { flex: 1; height: 32px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 12.5px; }
.add-row input:focus { outline: none; border-color: var(--accent); }
.btn { flex: none; height: 32px; padding: 0 12px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.btn-accent { flex: none; height: 32px; padding: 0 16px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.5; }
</style>
