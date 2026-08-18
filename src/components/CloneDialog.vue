<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { openFolder } from "../lib/native";
import { cloneRepo } from "../lib/git";
import { listAccountRepos, type RepoRef } from "../lib/accounts";
import { connectionsStore, refreshConnections, openSettings } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const emit = defineEmits<{ (e: "cloned", path: string): void }>();

const url = ref("");
const dir = ref("");
const busy = ref(false);
const error = ref<string | null>(null);

// Browse-account state
const connections = computed(() => connectionsStore.config.connections);
const accountId = ref<string>("");
const repos = ref<RepoRef[]>([]);
const loadingRepos = ref(false);
const repoFilter = ref("");
const selectedRepo = ref<string>("");

const filteredRepos = computed(() => {
  const q = repoFilter.value.trim().toLowerCase();
  if (!q) return repos.value;
  return repos.value.filter(
    (r) => r.name.toLowerCase().includes(q) || r.description.toLowerCase().includes(q),
  );
});

watch(open, (o) => {
  if (!o) return;
  error.value = null;
  refreshConnections().then(() => {
    if (!accountId.value && connections.value.length) {
      accountId.value = connections.value[0].id;
      loadRepos();
    }
  });
});

async function loadRepos() {
  repos.value = [];
  selectedRepo.value = "";
  if (!accountId.value) return;
  loadingRepos.value = true;
  error.value = null;
  try {
    repos.value = await listAccountRepos(accountId.value);
  } catch (e) {
    error.value = String(e);
  } finally {
    loadingRepos.value = false;
  }
}

function pickRepo(r: RepoRef) {
  selectedRepo.value = r.name;
  url.value = r.sshUrl || r.httpUrl;
}

async function chooseDir() {
  const picked = await openFolder("Choose a destination folder");
  if (picked) dir.value = picked;
}

async function clone() {
  error.value = null;
  if (!url.value.trim()) return (error.value = "Pick a repository or paste a URL.");
  if (!dir.value.trim()) return (error.value = "Choose a destination folder.");
  busy.value = true;
  try {
    const path = await cloneRepo(url.value.trim(), dir.value.trim());
    open.value = false;
    url.value = "";
    selectedRepo.value = "";
    emit("cloned", path);
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
        <div class="head"><h2>Clone a repository</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <!-- Browse a connected account -->
          <div v-if="connections.length" class="browse">
            <div class="browse-head">
              <select v-model="accountId" @change="loadRepos">
                <option v-for="c in connections" :key="c.id" :value="c.id">
                  {{ c.provider === "github" ? "GitHub" : "GitLab" }} · {{ c.label }}
                </option>
              </select>
              <input v-model="repoFilter" class="repo-filter" placeholder="Filter repositories…" spellcheck="false" />
            </div>
            <div class="repo-list">
              <div v-if="loadingRepos" class="repo-msg">Loading repositories…</div>
              <div v-else-if="!repos.length" class="repo-msg">No repositories found.</div>
              <div
                v-for="r in filteredRepos"
                :key="r.name"
                class="repo"
                :class="{ on: selectedRepo === r.name }"
                @click="pickRepo(r)"
              >
                <span class="repo-name mono">{{ r.name }}</span>
                <span v-if="r.description" class="repo-desc">{{ r.description }}</span>
              </div>
            </div>
          </div>
          <div v-else class="no-account">
            No accounts connected.
            <a class="key-link" @click.prevent="openSettings('accounts')">Connect one</a> to browse, or paste a URL below.
          </div>

          <label class="field">
            <span>Repository URL</span>
            <input v-model="url" placeholder="git@gitlab.com:group/repo.git" spellcheck="false" />
          </label>
          <label class="field">
            <span>Destination folder</span>
            <div class="dir-row">
              <input v-model="dir" placeholder="~/Code" spellcheck="false" />
              <button class="btn" @click="chooseDir">Choose…</button>
            </div>
          </label>
          <p v-if="error" class="err mono">{{ error }}</p>
          <div class="actions">
            <button class="btn-accent" :disabled="busy" @click="clone">
              <span v-if="busy" class="spinner-sm"></span>{{ busy ? "Cloning…" : "Clone" }}
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
.sheet { width: 620px; max-width: calc(100vw - 48px); max-height: calc(100vh - 96px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); flex: none; }
.head h2 { margin: 0; font-size: 18px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); overflow-y: auto; }

.browse { border: 1px solid var(--line); margin-bottom: var(--space-4); }
.browse-head { display: flex; gap: var(--space-2); padding: var(--space-2); background: var(--subtle); border-bottom: 1px solid var(--line); }
.browse-head select, .repo-filter { height: 30px; padding: 0 8px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 12.5px; }
.browse-head select { flex: none; max-width: 45%; }
.repo-filter { flex: 1; }
.repo-list { max-height: 240px; overflow-y: auto; }
.repo-msg { padding: var(--space-4); color: var(--text-faint); font-size: 12.5px; }
.repo { display: flex; flex-direction: column; gap: 2px; padding: 8px var(--space-3); cursor: pointer; border-bottom: 1px solid var(--line-soft); }
.repo:hover { background: color-mix(in srgb, var(--raised) 55%, transparent); }
.repo.on { background: var(--accent); color: var(--accent-on); }
.repo-name { font-size: 12.5px; font-weight: 600; }
.repo-desc { font-size: 11px; color: var(--text-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.repo.on .repo-desc { color: color-mix(in srgb, var(--accent-on) 80%, transparent); }
.no-account { font-size: 12.5px; color: var(--text-dim); margin-bottom: var(--space-4); }
.key-link { color: var(--accent); cursor: pointer; }

.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field input { height: 32px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus { outline: none; border-color: var(--accent); }
.dir-row { display: flex; gap: var(--space-2); }
.dir-row input { flex: 1; }
.err { color: var(--accent); font-size: 11px; margin: 0 0 var(--space-3); }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-2); }
.btn-accent { height: 34px; padding: 0 18px; display: flex; align-items: center; gap: var(--space-2); background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.spinner-sm { width: 11px; height: 11px; border: 2px solid color-mix(in srgb, var(--accent-on) 40%, transparent); border-top-color: var(--accent-on); border-radius: 50%; animation: plumb-spin 0.7s linear infinite; }
@keyframes plumb-spin { to { transform: rotate(360deg); } }
</style>
