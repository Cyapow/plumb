<script setup lang="ts">
// Where a push goes when it isn't obvious: no upstream yet, several remotes to
// choose between, or an explicit "Push to…". The remote branch name is editable
// because it needn't match the local one — and a typo here is a stray branch on
// the server, so it's shown rather than assumed.
import { computed, ref, watch } from "vue";
import { listRemotes, pushTarget, type RemoteInfo } from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; branch: string; upstream?: string | null }>();
const emit = defineEmits<{ (e: "pushed"): void }>();

const remotes = ref<RemoteInfo[]>([]);
const remote = ref("");
const remoteBranch = ref("");
const setUpstream = ref(true);
const busy = ref(false);
const error = ref("");

// "origin/feature" → remote "origin", branch "feature".
const tracked = computed(() => {
  const up = props.upstream ?? "";
  const slash = up.indexOf("/");
  return slash === -1 ? null : { remote: up.slice(0, slash), branch: up.slice(slash + 1) };
});

watch(open, async (isOpen) => {
  if (!isOpen) return;
  error.value = "";
  busy.value = false;
  remotes.value = await listRemotes(props.repoPath).catch(() => []);
  const preferred = tracked.value?.remote ?? (remotes.value.some((r) => r.name === "origin") ? "origin" : remotes.value[0]?.name);
  remote.value = preferred ?? "";
  remoteBranch.value = tracked.value?.branch ?? props.branch;
  // Already tracking this remote — nothing to set.
  setUpstream.value = !tracked.value;
});

async function push() {
  error.value = "";
  if (!remote.value) return void (error.value = "Add a remote first.");
  if (!remoteBranch.value.trim()) return void (error.value = "Name the branch to push to.");
  busy.value = true;
  try {
    const msg = await pushTarget(props.repoPath, {
      remote: remote.value,
      remoteBranch: remoteBranch.value.trim(),
      setUpstream: setUpstream.value,
    });
    toast("Push", msg);
    open.value = false;
    emit("pushed");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head"><h2>Push</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <p class="intro">Pushing <strong>{{ branch }}</strong></p>

          <div v-if="!remotes.length" class="empty">
            This repository has no remotes. Add one from Manage remotes, then push.
          </div>
          <template v-else>
            <label class="field">
              <span>Remote</span>
              <select v-model="remote">
                <option v-for="r in remotes" :key="r.name" :value="r.name">{{ r.name }} — {{ r.url }}</option>
              </select>
            </label>
            <label class="field">
              <span>Branch on the remote</span>
              <input v-model="remoteBranch" spellcheck="false" @keydown.enter="push" />
            </label>
            <label class="check"><input type="checkbox" v-model="setUpstream" /> Track this branch (set upstream)</label>
          </template>

          <p v-if="error" class="err mono">{{ error }}</p>
          <div class="actions">
            <button class="btn-accent" :disabled="busy || !remotes.length" @click="push">
              {{ busy ? "Pushing…" : "Push" }}
            </button>
            <button class="btn" @click="open = false">Cancel</button>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 520px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); }
.intro { font-size: 12.5px; color: var(--text-mid); margin: 0 0 var(--space-4); }
.empty { font-size: 12.5px; color: var(--text-mid); margin-bottom: var(--space-3); }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field input, .field select { height: 34px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus, .field select:focus { outline: none; border-color: var(--accent); }
.check { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--text); margin-bottom: var(--space-2); cursor: pointer; }
.err { color: var(--accent); font-size: 11px; margin: var(--space-2) 0 0; }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-4); }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
