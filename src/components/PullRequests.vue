<script setup lang="ts">
// Unified pull request (GitHub) / merge request (GitLab) list for the open repo.
import { computed, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { listPullRequests, type PrList, type PullRequest } from "../lib/accounts";
import { openSettings } from "../lib/ui";
import { relativeTime } from "../lib/format";

const props = defineProps<{ repoPath: string }>();

const data = ref<PrList | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);

type Filter = "all" | "created" | "assigned" | "reviewing";
const filter = ref<Filter>("all");
const me = computed(() => data.value?.username ?? "");

function match(p: PullRequest, f: Filter): boolean {
  switch (f) {
    case "created":
      return p.author === me.value;
    case "assigned":
      return p.assignees.includes(me.value);
    case "reviewing":
      return p.reviewers.includes(me.value);
    default:
      return true;
  }
}
const items = computed(() => data.value?.items ?? []);
const filtered = computed(() => items.value.filter((p) => match(p, filter.value)));
const countFor = (f: Filter) => items.value.filter((p) => match(p, f)).length;
const tabs: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "created", label: "Created" },
  { id: "assigned", label: "Assigned" },
  { id: "reviewing", label: "Reviewing" },
];

async function load() {
  loading.value = true;
  error.value = null;
  try {
    data.value = await listPullRequests(props.repoPath);
  } catch (e) {
    error.value = String(e);
    data.value = null;
  } finally {
    loading.value = false;
  }
}

watch(() => props.repoPath, load, { immediate: true });

const isMr = () => data.value?.provider === "gitlab";
const term = () => (isMr() ? "merge requests" : "pull requests");
function iso(s: string) {
  const t = Date.parse(s);
  return Number.isNaN(t) ? "" : relativeTime(Math.floor(t / 1000));
}
</script>

<template>
  <section class="prs">
    <div class="pr-head">
      <span class="title">{{ data?.provider === "gitlab" ? "Merge requests" : "Pull requests" }}</span>
      <span v-if="data?.host" class="host mono">{{ data.host }}</span>
      <span class="grow"></span>
      <button class="btn" :disabled="loading" @click="load">{{ loading ? "…" : "Refresh" }}</button>
    </div>

    <div v-if="data?.status === 'ok' && me" class="pr-tabs">
      <button
        v-for="t in tabs"
        :key="t.id"
        class="tab"
        :class="{ on: filter === t.id }"
        @click="filter = t.id"
      >
        {{ t.label }} <span class="tab-count mono">{{ countFor(t.id) }}</span>
      </button>
    </div>

    <div class="pr-body">
      <div v-if="loading && !data" class="msg">Loading…</div>
      <div v-else-if="error" class="msg err mono">{{ error }}</div>

      <div v-else-if="data?.status === 'no_remote'" class="msg">
        This repository has no remote, so there's nothing to list.
      </div>

      <div v-else-if="data?.status === 'no_account'" class="msg center">
        <p>No connected account for <span class="mono">{{ data.host }}</span>.</p>
        <button class="btn-accent" @click="openSettings('accounts')">Connect an account</button>
      </div>

      <div v-else-if="data && data.items.length === 0" class="msg">
        No open {{ term() }}. 🎉
      </div>

      <div v-else-if="data && filtered.length === 0" class="msg">
        No {{ term() }} match "{{ tabs.find((t) => t.id === filter)?.label }}".
      </div>

      <div v-else-if="data" class="list">
        <div v-for="pr in filtered" :key="pr.number" class="pr" @click="openUrl(pr.url)">
          <img v-if="pr.authorAvatar" :src="pr.authorAvatar" class="avatar" alt="" />
          <div class="pr-main">
            <div class="pr-title-row">
              <span class="num mono">{{ pr.provider === "gitlab" ? "!" : "#" }}{{ pr.number }}</span>
              <span v-if="pr.draft" class="draft">DRAFT</span>
              <span class="pr-title">{{ pr.title }}</span>
            </div>
            <div class="pr-sub mono">
              {{ pr.author }} · <span class="branch">{{ pr.sourceBranch }}</span> →
              <span class="branch">{{ pr.targetBranch }}</span>
              <span v-if="iso(pr.updatedAt)"> · updated {{ iso(pr.updatedAt) }}</span>
            </div>
          </div>
          <span class="open">↗</span>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.prs { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.pr-head {
  height: 44px; flex: none;
  display: flex; align-items: center; gap: var(--space-3);
  padding: 0 var(--space-4);
  border-bottom: 2px solid var(--line);
  background: var(--subtle);
}
.pr-head .title { font-size: 15px; font-weight: 700; }
.pr-head .host { font-size: 11px; color: var(--text-faint); }
.pr-head .grow { flex: 1; }
.btn { height: 28px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); font-size: 12px; cursor: pointer; }

.pr-tabs { flex: none; display: flex; gap: 2px; padding: var(--space-2) var(--space-4); border-bottom: 1px solid var(--line); background: var(--subtle); }
.tab { display: flex; align-items: center; gap: 6px; font-size: 12px; font-weight: 600; padding: 5px 12px; background: transparent; border: 1px solid transparent; color: var(--text-mid); cursor: pointer; }
.tab:hover { color: var(--text); }
.tab.on { background: var(--raised); border-color: var(--line); color: var(--text); }
.tab-count { font-size: 10px; color: var(--text-faint); }
.tab.on .tab-count { color: var(--accent); }

.pr-body { flex: 1; overflow-y: auto; }
.msg { padding: var(--space-8) var(--space-4); color: var(--text-dim); font-size: 13px; }
.msg.center { text-align: center; display: flex; flex-direction: column; align-items: center; gap: var(--space-3); }
.msg.err { color: var(--accent); font-size: 12px; }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; }

.list { display: flex; flex-direction: column; }
.pr {
  display: flex; align-items: center; gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--line-soft);
  cursor: pointer;
}
.pr:hover { background: color-mix(in srgb, var(--raised) 55%, transparent); }
.pr:hover .open { opacity: 1; }
.avatar { width: 30px; height: 30px; flex: none; object-fit: cover; }
.pr-main { flex: 1; min-width: 0; }
.pr-title-row { display: flex; align-items: center; gap: var(--space-2); }
.num { font-size: 11.5px; color: var(--text-faint); flex: none; }
.draft { font-family: var(--font-mono); font-size: 9px; font-weight: 700; color: var(--lane-2); border: 1px solid var(--lane-2); padding: 1px 4px; flex: none; }
.pr-title { font-size: 13.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pr-sub { font-size: 11px; color: var(--text-faint); margin-top: 3px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.branch { color: var(--text-mid); }
.open { flex: none; color: var(--text-faint); opacity: 0; }
</style>
