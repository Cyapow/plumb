<script setup lang="ts">
// Repo-wide CI runs — every recent pipeline for the open repo, not tied to a
// single commit. Click a run to inspect its jobs, or ↗ to open it on the host.
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { openUrl } from "../lib/native";
import { listPipelines, type PipelineRunList, type PipelineRun } from "../lib/accounts";
import { openSettings } from "../lib/ui";
import { relativeTime } from "../lib/format";

const props = defineProps<{ repoPath: string }>();
const emit = defineEmits<{ (e: "pipeline", sha: string, title: string): void }>();

const data = ref<PipelineRunList | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);

function isFailed(s: string): boolean {
  return s === "failure" || s === "failed";
}
function isActive(s: string): boolean {
  return s === "running" || s === "pending";
}
// Collapse provider-specific words to the CSS badge classes.
function badge(s: string): string {
  if (s === "success") return "success";
  if (isFailed(s)) return "failure";
  if (isActive(s)) return "pending";
  return "other";
}
function glyph(s: string): string {
  if (s === "success") return "✓";
  if (isFailed(s)) return "✕";
  if (s === "running") return "●";
  if (isActive(s)) return "◔";
  if (s === "canceled") return "⊘";
  return "·";
}

type Filter = "all" | "running" | "failed";
const filter = ref<Filter>("all");
const items = computed(() => data.value?.items ?? []);
function match(r: PipelineRun, f: Filter): boolean {
  if (f === "running") return isActive(r.status);
  if (f === "failed") return isFailed(r.status);
  return true;
}
const filtered = computed(() => items.value.filter((r) => match(r, filter.value)));
const countFor = (f: Filter) => items.value.filter((r) => match(r, f)).length;
const tabs: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "running", label: "Running" },
  { id: "failed", label: "Failed" },
];

// `silent` polls in the background without the spinner or clobbering the list
// with an error toast — used by the auto-refresh loop.
async function load(silent = false) {
  if (!silent) loading.value = true;
  if (!silent) error.value = null;
  try {
    const d = await listPipelines(props.repoPath);
    data.value = d;
    if (silent) error.value = null;
  } catch (e) {
    if (!silent) {
      error.value = String(e);
      data.value = null;
    }
  } finally {
    if (!silent) loading.value = false;
  }
}
watch(() => props.repoPath, () => load(), { immediate: true });

// Auto-refresh: fast while any run is active, slow baseline otherwise; paused
// while the window is hidden. Keeps a running pipeline's status live without a
// manual Refresh.
const hasActive = computed(() => items.value.some((r) => isActive(r.status)));
let timer: number | undefined;
function arm() {
  const delay = hasActive.value ? 6000 : 30000;
  timer = window.setTimeout(run, delay);
}
async function run() {
  if (!document.hidden) await load(true);
  arm();
}
onMounted(arm);
onUnmounted(() => {
  if (timer) clearTimeout(timer);
});

function iso(s: string): string {
  const t = Date.parse(s);
  return Number.isNaN(t) ? "" : relativeTime(Math.floor(t / 1000));
}
</script>

<template>
  <section class="pipes">
    <div class="ph-head">
      <span class="title">Pipelines</span>
      <span v-if="data?.host" class="host mono">{{ data.host }}</span>
      <span class="grow"></span>
      <button class="btn" :disabled="loading" @click="load()">{{ loading ? "…" : "Refresh" }}</button>
    </div>

    <div v-if="data?.status === 'ok' && items.length" class="ph-tabs">
      <button v-for="t in tabs" :key="t.id" class="tab" :class="{ on: filter === t.id }" @click="filter = t.id">
        {{ t.label }} <span class="tab-count mono">{{ countFor(t.id) }}</span>
      </button>
    </div>

    <div class="ph-body">
      <div v-if="loading && !data" class="msg">Loading…</div>
      <div v-else-if="error" class="msg err mono">{{ error }}</div>

      <div v-else-if="data?.status === 'no_remote'" class="msg">
        This repository has no remote, so there are no pipelines to list.
      </div>

      <div v-else-if="data?.status === 'no_account'" class="msg center">
        <p>No connected account for <span class="mono">{{ data.host }}</span>.</p>
        <button class="btn-accent" @click="openSettings('accounts')">Connect an account</button>
      </div>

      <div v-else-if="data && items.length === 0" class="msg">No pipeline runs yet.</div>

      <div v-else-if="data && filtered.length === 0" class="msg">
        No {{ filter }} pipelines right now.
      </div>

      <div v-else-if="data" class="list">
        <div
          v-for="r in filtered"
          :key="r.id"
          class="run"
          :title="r.sha ? 'Show jobs for this run' : ''"
          @click="r.sha && emit('pipeline', r.sha, r.name)"
        >
          <span class="st" :class="badge(r.status)" :title="r.status">{{ glyph(r.status) }}</span>
          <div class="run-main">
            <div class="run-title-row">
              <span class="run-name">{{ r.name }}</span>
              <span v-if="r.event" class="event mono">{{ r.event }}</span>
            </div>
            <div class="run-sub mono">
              <span v-if="r.branch" class="branch">{{ r.branch }}</span>
              <span v-if="r.shortSha"> · {{ r.shortSha }}</span>
              <span v-if="iso(r.createdAt)"> · {{ iso(r.createdAt) }}</span>
              <span v-if="r.title" class="ct"> · {{ r.title }}</span>
            </div>
          </div>
          <span v-if="r.webUrl" class="open" title="Open on host" @click.stop="openUrl(r.webUrl)">↗</span>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.pipes { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.ph-head {
  height: 44px; flex: none;
  display: flex; align-items: center; gap: var(--space-3);
  padding: 0 var(--space-4);
  border-bottom: 2px solid var(--line);
  background: var(--subtle);
}
.ph-head .title { font-size: 15px; font-weight: 700; }
.ph-head .host { font-size: 11px; color: var(--text-faint); }
.grow { flex: 1; }
.btn { height: 28px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); font-size: 12px; cursor: pointer; }

.ph-tabs { flex: none; display: flex; gap: 2px; padding: var(--space-2) var(--space-4); border-bottom: 1px solid var(--line); background: var(--subtle); }
.tab { display: flex; align-items: center; gap: 6px; font-size: 12px; font-weight: 600; padding: 5px 12px; background: transparent; border: 1px solid transparent; color: var(--text-mid); cursor: pointer; }
.tab:hover { color: var(--text); }
.tab.on { background: var(--raised); border-color: var(--line); color: var(--text); }
.tab-count { font-size: 10px; color: var(--text-faint); }
.tab.on .tab-count { color: var(--accent); }

.ph-body { flex: 1; overflow-y: auto; }
.msg { padding: var(--space-8) var(--space-4); color: var(--text-dim); font-size: 13px; }
.msg.center { text-align: center; display: flex; flex-direction: column; align-items: center; gap: var(--space-3); }
.msg.err { color: var(--accent); font-size: 12px; }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; }

.list { display: flex; flex-direction: column; }
.run {
  display: flex; align-items: center; gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--line-soft);
  cursor: pointer;
}
.run:hover { background: color-mix(in srgb, var(--raised) 55%, transparent); }
.run:hover .open { opacity: 1; }
.st { flex: none; width: 18px; height: 18px; display: inline-grid; place-items: center; font-size: 11px; font-weight: 800; color: var(--accent-on); }
.st.success { background: var(--lane-3); }
.st.failure { background: var(--accent); }
.st.pending { background: var(--lane-2); }
.st.other { background: var(--text-faint); }
.run-main { flex: 1; min-width: 0; }
.run-title-row { display: flex; align-items: center; gap: var(--space-2); }
.run-name { font-size: 13.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.event { font-size: 9.5px; color: var(--text-faint); border: 1px solid var(--line); padding: 1px 5px; flex: none; text-transform: lowercase; }
.run-sub { font-size: 11px; color: var(--text-faint); margin-top: 3px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.branch { color: var(--text-mid); }
.ct { color: var(--text-dim); }
.open { flex: none; color: var(--text-faint); opacity: 0; padding: 0 4px; }
.open:hover { color: var(--accent); }
</style>
