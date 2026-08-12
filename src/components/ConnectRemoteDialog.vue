<script setup lang="ts">
// For an empty repo: attach it to an existing remote branch and build on top.
// Enter a URL, optionally list its branches, pick one, and check it out —
// working files stay put as uncommitted changes over that branch's history.
import { computed, ref, watch } from "vue";
import { listRemoteBranches, connectRemoteBranch } from "../lib/git";
import { listAccountRepos, type RepoRef } from "../lib/accounts";
import { connectionsStore, refreshConnections, openSettings, toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string }>();
const emit = defineEmits<{ (e: "connected"): void }>();

const url = ref("");
const branch = ref("main");
const branches = ref<string[]>([]);
const listing = ref(false);
const busy = ref(false);
const error = ref<string | null>(null);
const notice = ref<string | null>(null);

// Browse a connected account (mirrors the clone dialog).
const connections = computed(() => connectionsStore.config.connections);
const accountId = ref("");
const repos = ref<RepoRef[]>([]);
const loadingRepos = ref(false);
const repoFilter = ref("");
const selectedRepo = ref("");

const filteredRepos = computed(() => {
  const q = repoFilter.value.trim().toLowerCase();
  if (!q) return repos.value;
  return repos.value.filter(
    (r) => r.name.toLowerCase().includes(q) || r.description.toLowerCase().includes(q),
  );
});

watch(open, (o) => {
  if (!o) return;
  url.value = "";
  branch.value = "main";
  branches.value = [];
  repos.value = [];
  selectedRepo.value = "";
  error.value = null;
  notice.value = null;
  refreshConnections().then(() => {
    if (!accountId.value && connections.value.length) accountId.value = connections.value[0].id;
    if (accountId.value) loadRepos();
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
  branches.value = [];
  listBranches();
}

async function listBranches() {
  if (!url.value.trim()) return;
  listing.value = true;
  error.value = null;
  notice.value = null;
  try {
    branches.value = await listRemoteBranches(url.value.trim());
    if (branches.value.length && !branches.value.includes(branch.value)) {
      branch.value = branches.value.includes("main")
        ? "main"
        : branches.value.includes("master")
          ? "master"
          : branches.value[0];
    }
    if (!branches.value.length) {
      // An empty remote is fine — the branch gets created on the first push.
      notice.value = "This remote has no branches yet. Enter a name and it'll be created when you push.";
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    listing.value = false;
  }
}

async function connect() {
  if (!url.value.trim() || !branch.value.trim()) return;
  busy.value = true;
  error.value = null;
  try {
    const msg = await connectRemoteBranch(props.repoPath, url.value.trim(), branch.value.trim());
    toast("Connected", msg);
    open.value = false;
    emit("connected");
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
        <div class="head"><h2>Connect a remote</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <p class="intro">
            Base this repository on an existing remote branch. Your current files stay as uncommitted
            changes on top, ready to commit.
          </p>

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
            <span>Remote URL</span>
            <div class="url-row">
              <input v-model="url" placeholder="git@github.com:you/repo.git" spellcheck="false" @keydown.enter="listBranches" />
              <button class="btn" :disabled="listing || !url.trim()" @click="listBranches">
                {{ listing ? "…" : "List branches" }}
              </button>
            </div>
          </label>

          <div v-if="branches.length" class="chips">
            <button
              v-for="b in branches"
              :key="b"
              class="chip mono"
              :class="{ on: b === branch }"
              @click="branch = b"
            >{{ b }}</button>
          </div>

          <label class="field">
            <span>Branch to build on</span>
            <input v-model="branch" spellcheck="false" placeholder="main" />
          </label>

          <p v-if="notice" class="notice">{{ notice }}</p>
          <p v-if="error" class="err mono">{{ error }}</p>
          <div class="actions">
            <button class="btn-accent" :disabled="busy || !url.trim() || !branch.trim()" @click="connect">
              {{ busy ? "Connecting…" : "Connect & check out" }}
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
.sheet { width: 560px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); max-height: calc(100vh - 140px); overflow-y: auto; }
.intro { font-size: 12.5px; color: var(--text-mid); margin: 0 0 var(--space-4); line-height: 1.55; }

.browse { border: 1px solid var(--line); margin-bottom: var(--space-4); }
.browse-head { display: flex; gap: var(--space-2); padding: var(--space-2); background: var(--subtle); border-bottom: 1px solid var(--line); }
.browse-head select, .repo-filter { height: 30px; padding: 0 8px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 12.5px; }
.browse-head select { flex: none; max-width: 45%; }
.repo-filter { flex: 1; }
.repo-list { max-height: 200px; overflow-y: auto; }
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
.url-row { display: flex; gap: var(--space-2); }
.url-row input { flex: 1; }
.chips { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: var(--space-3); }
.chip { padding: 4px 10px; font-size: 11.5px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.chip.on { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }
.btn { height: 32px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; white-space: nowrap; }
.btn:disabled { opacity: 0.5; }
.err { color: var(--accent); font-size: 11px; margin: 0 0 var(--space-3); }
.notice { color: var(--text-mid); font-size: 11.5px; margin: 0 0 var(--space-3); line-height: 1.5; }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-2); }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
</style>
