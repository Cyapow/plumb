<script setup lang="ts">
// Manage a repo's remotes: add, rename, change URL, prune, remove.
import { ref, watch } from "vue";
import {
  listRemotes,
  addRemote,
  renameRemote,
  removeRemote,
  setRemoteUrl,
  pruneRemote,
  type RemoteInfo,
} from "../lib/git";
import { promptText, promptConfirm, toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string }>();

const remotes = ref<RemoteInfo[]>([]);
const newName = ref("");
const newUrl = ref("");
const busy = ref(false);

async function reload() {
  remotes.value = await listRemotes(props.repoPath).catch(() => []);
}

watch(open, (o) => {
  if (o) {
    newName.value = "";
    newUrl.value = "";
    reload();
  }
});

async function run(fn: () => Promise<unknown>, ok: string) {
  busy.value = true;
  try {
    await fn();
    if (ok) toast(ok);
    await reload();
  } catch (e) {
    toast("Failed", String(e), "error");
  } finally {
    busy.value = false;
  }
}

function add() {
  const name = newName.value.trim();
  const url = newUrl.value.trim();
  if (!name || !url) return;
  run(async () => {
    await addRemote(props.repoPath, name, url);
    newName.value = "";
    newUrl.value = "";
  }, `Added ${name}`);
}

async function rename(r: RemoteInfo) {
  const to = await promptText({ title: "Rename remote", label: `New name for "${r.name}"`, value: r.name });
  if (to && to.trim() && to.trim() !== r.name) run(() => renameRemote(props.repoPath, r.name, to.trim()), `Renamed to ${to.trim()}`);
}

async function editUrl(r: RemoteInfo) {
  const url = await promptText({ title: "Change URL", label: `URL for "${r.name}"`, value: r.url });
  if (url && url.trim() && url.trim() !== r.url) run(() => setRemoteUrl(props.repoPath, r.name, url.trim()), "URL updated");
}

async function remove(r: RemoteInfo) {
  if (await promptConfirm({ title: `Remove remote "${r.name}"?`, body: "This only affects your local config.", confirmLabel: "Remove", danger: true }))
    run(() => removeRemote(props.repoPath, r.name), `Removed ${r.name}`);
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head"><h2>Remotes</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <div v-if="!remotes.length" class="empty">No remotes configured.</div>
          <div v-for="r in remotes" :key="r.name" class="remote">
            <div class="r-top">
              <span class="r-name mono">{{ r.name }}</span>
              <div class="r-actions">
                <button @click="rename(r)" :disabled="busy">Rename</button>
                <button @click="editUrl(r)" :disabled="busy">URL</button>
                <button @click="run(() => pruneRemote(props.repoPath, r.name), '')" :disabled="busy">Prune</button>
                <button class="danger" @click="remove(r)" :disabled="busy">Remove</button>
              </div>
            </div>
            <div class="r-url mono">{{ r.url }}</div>
          </div>

          <div class="add">
            <div class="add-label">Add a remote</div>
            <div class="add-row">
              <input v-model="newName" placeholder="name (e.g. upstream)" spellcheck="false" />
              <input v-model="newUrl" placeholder="git@… or https://…" spellcheck="false" @keydown.enter="add" />
              <button class="btn-accent" :disabled="busy || !newName.trim() || !newUrl.trim()" @click="add">Add</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 560px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); max-height: 60vh; overflow-y: auto; }
.empty { font-size: 12.5px; color: var(--text-dim); padding-bottom: var(--space-3); }
.remote { border: 1px solid var(--line); padding: 10px 12px; margin-bottom: var(--space-2); }
.r-top { display: flex; align-items: center; gap: var(--space-3); }
.r-name { font-size: 13px; font-weight: 700; }
.r-actions { margin-left: auto; display: flex; gap: 4px; }
.r-actions button { padding: 3px 8px; font-size: 11px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.r-actions button.danger { color: var(--accent); }
.r-actions button:disabled { opacity: 0.5; }
.r-url { font-size: 11px; color: var(--text-dim); margin-top: 6px; word-break: break-all; }
.add { margin-top: var(--space-4); border-top: 1px solid var(--line); padding-top: var(--space-3); }
.add-label { font-size: 11px; color: var(--text-dim); margin-bottom: 6px; }
.add-row { display: flex; gap: var(--space-2); }
.add-row input { flex: 1; height: 32px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 12.5px; }
.add-row input:focus { outline: none; border-color: var(--accent); }
.btn-accent { flex: none; height: 32px; padding: 0 16px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.5; }
</style>
