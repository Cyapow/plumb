<script setup lang="ts">
// Shown when Push has no remote: connect an existing remote URL, or create a
// new repo on a connected account. Either way it wires up `origin`, then the
// caller pushes.
import { computed, ref, watch } from "vue";
import { addRemote } from "../lib/git";
import { createRemoteRepo } from "../lib/accounts";
import { connectionsStore, refreshConnections, openSettings, toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; repoName: string }>();
const emit = defineEmits<{ (e: "published"): void }>();

const mode = ref<"existing" | "create">("existing");
const url = ref("");
const accountId = ref("");
const name = ref("");
const isPrivate = ref(true);
const busy = ref(false);
const error = ref<string | null>(null);

const connections = computed(() => connectionsStore.config.connections);

watch(open, (o) => {
  if (!o) return;
  error.value = "";
  url.value = "";
  name.value = props.repoName;
  refreshConnections().then(() => {
    if (!accountId.value && connections.value.length) accountId.value = connections.value[0].id;
    if (!connections.value.length) mode.value = "existing";
  });
});

async function confirm() {
  error.value = null;
  busy.value = true;
  try {
    let remoteUrl = url.value.trim();
    if (mode.value === "create") {
      if (!accountId.value) throw new Error("Pick an account.");
      if (!name.value.trim()) throw new Error("Enter a repository name.");
      const repo = await createRemoteRepo(accountId.value, name.value.trim(), isPrivate.value);
      remoteUrl = repo.sshUrl || repo.httpUrl;
      toast("Repository created", repo.name);
    }
    if (!remoteUrl) throw new Error("Enter a remote URL.");
    await addRemote(props.repoPath, "origin", remoteUrl);
    open.value = false;
    emit("published");
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
        <div class="head"><h2>Publish this repository</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <p class="intro">This repository has no remote yet. Connect one to push.</p>

          <div class="seg">
            <button :class="{ on: mode === 'existing' }" @click="mode = 'existing'">Existing remote</button>
            <button :class="{ on: mode === 'create' }" @click="mode = 'create'">Create new</button>
          </div>

          <template v-if="mode === 'existing'">
            <label class="field">
              <span>Remote URL</span>
              <input v-model="url" placeholder="git@github.com:you/repo.git" spellcheck="false" />
            </label>
          </template>

          <template v-else>
            <div v-if="!connections.length" class="no-account">
              No accounts connected.
              <a class="key-link" @click.prevent="openSettings('accounts')">Connect one</a> to create a repo,
              or use an existing URL.
            </div>
            <template v-else>
              <label class="field">
                <span>Account</span>
                <select v-model="accountId">
                  <option v-for="c in connections" :key="c.id" :value="c.id">
                    {{ c.provider === "github" ? "GitHub" : "GitLab" }} · {{ c.label }}
                  </option>
                </select>
              </label>
              <label class="field">
                <span>Repository name</span>
                <input v-model="name" spellcheck="false" />
              </label>
              <label class="check"><input type="checkbox" v-model="isPrivate" /> Private</label>
            </template>
          </template>

          <p v-if="error" class="err mono">{{ error }}</p>
          <div class="actions">
            <button class="btn-accent" :disabled="busy" @click="confirm">
              <span v-if="busy" class="spinner-sm"></span>{{ busy ? "Working…" : mode === "create" ? "Create & connect" : "Connect & push" }}
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
.sheet { width: 540px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); }
.intro { font-size: 12.5px; color: var(--text-mid); margin: 0 0 var(--space-4); }
.seg { display: flex; gap: 2px; margin-bottom: var(--space-4); }
.seg button { flex: 1; padding: 7px 0; background: var(--raised); border: 1px solid var(--line); font-size: 12px; font-weight: 600; color: var(--text-mid); cursor: pointer; }
.seg button.on { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field input, .field select { height: 32px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus, .field select:focus { outline: none; border-color: var(--accent); }
.check { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-mid); margin-bottom: var(--space-3); cursor: pointer; }
.no-account { font-size: 12.5px; color: var(--text-dim); margin-bottom: var(--space-3); }
.key-link { color: var(--accent); cursor: pointer; }
.err { color: var(--accent); font-size: 11px; margin: 0 0 var(--space-3); }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-2); }
.btn-accent { height: 34px; padding: 0 18px; display: flex; align-items: center; gap: var(--space-2); background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.spinner-sm { width: 11px; height: 11px; border: 2px solid color-mix(in srgb, var(--accent-on) 40%, transparent); border-top-color: var(--accent-on); border-radius: 50%; animation: plumb-spin 0.7s linear infinite; }
@keyframes plumb-spin { to { transform: rotate(360deg); } }
</style>
