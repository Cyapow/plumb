<script setup lang="ts">
// Read-only repository overview (like Tower's repo card).
import { ref, watch } from "vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  openRepo,
  listRemotes,
  gitIdentity,
  listCommits,
  workingStatus,
  getRepoDescription,
  type RemoteInfo,
} from "../lib/git";
import { relativeTime } from "../lib/format";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string }>();

const info = ref<{
  name: string;
  branch: string;
  description: string;
  lastCommit: string;
  lastCommitTime: number | null;
  status: number;
  remotes: RemoteInfo[];
  who: string;
}>({ name: "", branch: "", description: "", lastCommit: "", lastCommitTime: null, status: 0, remotes: [], who: "" });

watch(open, async (o) => {
  if (!o) return;
  const [repo, remotes, id, commits, status, desc] = await Promise.all([
    openRepo(props.repoPath).catch(() => null),
    listRemotes(props.repoPath).catch(() => [] as RemoteInfo[]),
    gitIdentity(props.repoPath).catch(() => null),
    listCommits(props.repoPath, 1).catch(() => []),
    workingStatus(props.repoPath).catch(() => []),
    getRepoDescription(props.repoPath).catch(() => ""),
  ]);
  info.value = {
    name: repo?.name ?? "",
    branch: repo?.head_branch ?? (repo?.detached ? "(detached)" : "—"),
    description: desc || "—",
    lastCommit: commits[0]?.summary ?? "—",
    lastCommitTime: commits[0]?.time ?? null,
    status: status.length,
    remotes,
    who: id?.name ? `${id.name}${id.email ? ` <${id.email}>` : ""}` : "not set",
  };
});
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head">
          <span class="folder">🗂</span>
          <h2>{{ info.name }}</h2>
          <span class="grow"></span>
          <button class="x" @click="open = false">✕</button>
        </div>
        <div class="body">
          <div class="sec">Repository</div>
          <div class="row"><span class="k">Location</span><span class="v mono">{{ repoPath }} <button class="reveal" title="Reveal in Finder" @click="revealItemInDir(repoPath)">↗</button></span></div>
          <div class="row"><span class="k">Description</span><span class="v">{{ info.description }}</span></div>
          <div class="row"><span class="k">Committer</span><span class="v mono">{{ info.who }}</span></div>
          <div class="row"><span class="k">Last commit</span><span class="v">{{ info.lastCommit }}<em v-if="info.lastCommitTime"> · {{ relativeTime(info.lastCommitTime) }}</em></span></div>

          <div class="sec">Working copy</div>
          <div class="row"><span class="k">Current branch</span><span class="v mono">{{ info.branch }}</span></div>
          <div class="row"><span class="k">Status</span><span class="v">{{ info.status }} changed file{{ info.status === 1 ? "" : "s" }}</span></div>

          <div class="sec">Remotes</div>
          <div v-if="!info.remotes.length" class="row"><span class="v muted">No remotes.</span></div>
          <div v-for="r in info.remotes" :key="r.name" class="row"><span class="k mono">{{ r.name }}</span><span class="v mono">{{ r.url }}</span></div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 640px; max-width: calc(100vw - 48px); max-height: 80vh; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.head .grow { flex: 1; }
.body { padding: var(--space-4); overflow-y: auto; }
.sec { font-size: 10.5px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-faint); margin: var(--space-4) 0 var(--space-2); }
.sec:first-child { margin-top: 0; }
.row { display: flex; gap: var(--space-4); padding: 5px 0; font-size: 12.5px; }
.k { width: 130px; flex: none; color: var(--text-dim); text-align: right; }
.v { flex: 1; color: var(--text); word-break: break-all; }
.v em { color: var(--text-faint); font-style: normal; }
.v.muted { color: var(--text-faint); }
.reveal { width: 20px; height: 18px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); font-size: 10px; cursor: pointer; }
</style>
