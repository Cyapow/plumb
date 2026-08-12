<script setup lang="ts">
// Design plate 13: AI groups the working tree into several focused commits,
// each with its own message. The user reviews/edits, then Plumb creates them
// in order (unstage all → per group: stage its files, commit).
import { ref, watch } from "vue";
import { aiGroupChanges, type CommitGroup } from "../lib/ai";
import { unstageAll, stagePaths, commit, type StatusEntry } from "../lib/git";
import { aiStore, toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; conventional: boolean; changed: StatusEntry[] }>();
const emit = defineEmits<{ (e: "done"): void }>();

const groups = ref<CommitGroup[]>([]);
const loading = ref(false);
const creating = ref(false);
const error = ref<string | null>(null);

const defaultProvider = () =>
  aiStore.config.providers.find((p) => p.id === aiStore.config.defaultId) ?? null;

async function generate() {
  if (!defaultProvider()) {
    error.value = "Set up an AI provider first (⚙ → AI providers).";
    return;
  }
  loading.value = true;
  error.value = null;
  try {
    groups.value = await aiGroupChanges(props.repoPath, aiStore.config.defaultId, props.conventional);
  } catch (e) {
    error.value = String(e);
    groups.value = [];
  } finally {
    loading.value = false;
  }
}

watch(open, (o) => {
  if (o) {
    groups.value = [];
    error.value = null;
    generate();
  }
});

async function create() {
  if (!groups.value.length) return;
  creating.value = true;
  error.value = null;
  try {
    await unstageAll(props.repoPath); // unstage everything first (works on an unborn repo too)
    for (const g of groups.value) {
      if (!g.files.length || !g.message.trim()) continue;
      await stagePaths(props.repoPath, g.files);
      await commit(props.repoPath, g.message, false, false);
    }
    open.value = false;
    emit("done");
    toast("Split complete", `${groups.value.length} commits created`);
  } catch (e) {
    error.value = String(e);
  } finally {
    creating.value = false;
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head">
          <h2>Split into commits</h2>
          <span class="sub mono">{{ changed.length }} files → {{ groups.length }} commits</span>
          <span class="grow"></span>
          <button class="btn" :disabled="loading" @click="generate">{{ loading ? "…" : "Re-group" }}</button>
          <button class="x" @click="open = false">✕</button>
        </div>

        <div class="body">
          <div v-if="loading" class="msg">Grouping your changes…</div>
          <div v-else-if="error" class="msg err mono">{{ error }}</div>
          <div v-else-if="!groups.length" class="msg">No groups.</div>

          <div v-for="(g, i) in groups" :key="i" class="group">
            <div class="g-head">
              <span class="g-num mono">{{ i + 1 }}</span>
              <input v-model="g.message" class="g-msg" spellcheck="false" />
              <span class="g-count mono">{{ g.files.length }} file{{ g.files.length === 1 ? "" : "s" }}</span>
            </div>
            <div class="g-files">
              <div v-for="f in g.files" :key="f" class="g-file mono">{{ f }}</div>
            </div>
          </div>
        </div>

        <div class="foot">
          <button class="btn-accent" :disabled="creating || loading || !groups.length" @click="create">
            <span v-if="creating" class="spinner-sm"></span>{{ creating ? "Creating…" : `Create ${groups.length} commits` }}
          </button>
          <button class="btn" @click="open = false">Cancel</button>
          <span class="foot-note">AI proposes; you edit the messages. Nothing is committed until you click Create.</span>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 720px; max-width: calc(100vw - 48px); height: 620px; max-height: calc(100vh - 80px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-bottom: 2px solid var(--line); flex: none; }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .sub { font-size: 11px; color: var(--text-faint); }
.head .grow { flex: 1; }
.head .btn { height: 28px; padding: 0 12px; background: var(--raised); border: 1px solid var(--line); font-size: 12px; cursor: pointer; }
.head .x { width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { flex: 1; overflow-y: auto; padding: var(--space-4); }
.msg { padding: var(--space-6); color: var(--text-faint); font-size: 13px; text-align: center; }
.msg.err { color: var(--accent); font-size: 12px; }

.group { border: 1px solid var(--line); margin-bottom: var(--space-3); }
.g-head { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-2); background: var(--subtle); border-bottom: 1px solid var(--line); }
.g-num { width: 20px; height: 20px; flex: none; display: flex; align-items: center; justify-content: center; background: var(--accent); color: var(--accent-on); font-weight: 700; font-size: 11px; }
.g-msg { flex: 1; height: 28px; padding: 0 8px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; font-weight: 600; }
.g-msg:focus { outline: none; border-color: var(--accent); }
.g-count { font-size: 10.5px; color: var(--text-faint); flex: none; }
.g-files { padding: var(--space-2); }
.g-file { font-size: 11.5px; color: var(--text-mid); padding: 2px 4px; }

.foot { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-top: 2px solid var(--line); flex: none; }
.btn-accent { height: 34px; padding: 0 18px; display: flex; align-items: center; gap: var(--space-2); background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.5; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.foot-note { font-size: 11px; color: var(--text-faint); }
.spinner-sm { width: 11px; height: 11px; border: 2px solid color-mix(in srgb, var(--accent-on) 40%, transparent); border-top-color: var(--accent-on); border-radius: 50%; animation: plumb-spin 0.7s linear infinite; }
@keyframes plumb-spin { to { transform: rotate(360deg); } }
</style>
