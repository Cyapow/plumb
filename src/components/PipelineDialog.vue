<script setup lang="ts">
// Pipeline(s) for a commit: stages/jobs with status, retry / cancel, open logs.
import { ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { pipelineDetail, pipelineAction, jobLog, type PipelineDetail, type PipelineJob } from "../lib/accounts";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; sha: string | null; title: string }>();

const pipelines = ref<PipelineDetail[]>([]);
const loading = ref(false);
const busyId = ref("");
const logJob = ref<{ name: string; text: string } | null>(null);
const logLoading = ref(false);

async function viewLog(j: PipelineJob) {
  if (!j.id) {
    openUrl(j.webUrl).catch(() => {});
    return;
  }
  logJob.value = { name: j.name, text: "" };
  logLoading.value = true;
  try {
    logJob.value = { name: j.name, text: await jobLog(props.repoPath, j.id) };
  } catch (e) {
    logJob.value = { name: j.name, text: String(e) };
  } finally {
    logLoading.value = false;
  }
}

async function reload() {
  if (!props.sha) return;
  loading.value = true;
  try {
    pipelines.value = await pipelineDetail(props.repoPath, props.sha);
  } catch {
    pipelines.value = [];
  } finally {
    loading.value = false;
  }
}
watch(open, (o) => o && reload());

function cls(status: string) {
  const s = status.toLowerCase();
  if (s === "success") return "ok";
  if (s === "failed" || s === "failure") return "fail";
  if (s === "running") return "run";
  if (s === "canceled" || s === "cancelled" || s === "skipped") return "muted";
  if (s === "manual") return "manual";
  return "pending";
}
const isRunning = (p: PipelineDetail) => ["running", "pending"].includes(p.status.toLowerCase());

async function act(p: PipelineDetail, action: "retry" | "cancel") {
  busyId.value = p.id;
  try {
    const msg = await pipelineAction(props.repoPath, p.id, action);
    toast("Pipeline", msg);
    setTimeout(reload, 1200);
  } catch (e) {
    toast("Failed", String(e), "error");
  } finally {
    busyId.value = "";
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head">
          <h2>Pipelines</h2>
          <span class="sub mono">{{ title }}</span>
          <span class="grow"></span>
          <button class="btn" :disabled="loading" @click="reload">{{ loading ? "…" : "Refresh" }}</button>
          <button class="x" @click="open = false">✕</button>
        </div>
        <!-- Inline job log -->
        <div v-if="logJob" class="log-panel">
          <div class="log-head">
            <button class="mini" @click="logJob = null">‹ Back</button>
            <span class="log-name mono">{{ logJob.name }}</span>
          </div>
          <pre v-if="!logLoading" class="log mono">{{ logJob.text || "(empty log)" }}</pre>
          <div v-else class="msg">Loading log…</div>
        </div>

        <div v-else class="body">
          <div v-if="loading" class="msg">Loading…</div>
          <div v-else-if="!pipelines.length" class="msg">No pipeline found for this commit.</div>

          <div v-for="p in pipelines" :key="p.id" class="pipe">
            <div class="p-head">
              <span class="dot" :class="cls(p.status)"></span>
              <span class="p-name">{{ p.name }}</span>
              <span class="p-status" :class="cls(p.status)">{{ p.status }}</span>
              <span class="grow"></span>
              <button class="mini" :disabled="busyId === p.id" @click="act(p, 'retry')">Retry</button>
              <button class="mini" :disabled="busyId === p.id || !isRunning(p)" @click="act(p, 'cancel')">Cancel</button>
              <button class="mini" @click="openUrl(p.webUrl)">Logs ↗</button>
            </div>
            <div class="jobs">
              <div v-for="(j, i) in p.jobs" :key="i" class="job" @click="viewLog(j)">
                <span class="dot sm" :class="cls(j.status)"></span>
                <span v-if="j.stage" class="stage mono">{{ j.stage }}</span>
                <span class="j-name">{{ j.name }}</span>
                <span class="grow"></span>
                <span class="j-status mono" :class="cls(j.status)">{{ j.status }}</span>
                <button class="jlog" title="Open in browser" @click.stop="openUrl(j.webUrl)">↗</button>
              </div>
              <div v-if="!p.jobs.length" class="no-jobs">No jobs reported.</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 58%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 720px; max-width: calc(100vw - 40px); max-height: 80vh; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 16px; font-weight: 800; }
.sub { font-size: 11px; color: var(--text-dim); }
.head .grow { flex: 1; }
.head .btn { height: 28px; padding: 0 12px; background: var(--raised); border: 1px solid var(--line); font-size: 12px; cursor: pointer; }
.head .x { width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); overflow-y: auto; }
.msg { padding: var(--space-6); text-align: center; color: var(--text-faint); font-size: 13px; }
.pipe { border: 1px solid var(--line); margin-bottom: var(--space-3); }
.p-head { display: flex; align-items: center; gap: var(--space-2); padding: 8px 12px; background: var(--subtle); border-bottom: 1px solid var(--line); }
.p-name { font-size: 13px; font-weight: 700; }
.p-status { font-size: 10.5px; font-weight: 700; text-transform: uppercase; }
.grow { flex: 1; }
.mini { padding: 3px 10px; font-size: 11px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.mini:disabled { opacity: 0.4; }
.jobs { padding: 4px 0; }
.job { display: flex; align-items: center; gap: var(--space-2); padding: 5px 12px; cursor: pointer; font-size: 12px; }
.job:hover { background: color-mix(in srgb, var(--accent) 10%, transparent); }
.stage { font-size: 10px; color: var(--text-faint); min-width: 60px; }
.j-name { color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.j-status { font-size: 10.5px; }
.no-jobs { padding: 8px 12px; font-size: 11.5px; color: var(--text-faint); }
.jlog { flex: none; width: 22px; height: 20px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); font-size: 11px; cursor: pointer; }
.log-panel { flex: 1; display: flex; flex-direction: column; min-height: 0; }
.log-head { display: flex; align-items: center; gap: var(--space-3); padding: 8px var(--space-4); border-bottom: 1px solid var(--line); }
.log-name { font-size: 12.5px; font-weight: 700; }
.log { flex: 1; margin: 0; padding: var(--space-3) var(--space-4); overflow: auto; font-size: 11.5px; line-height: 1.5; color: var(--text); white-space: pre-wrap; word-break: break-word; background: var(--bg); }
.dot { width: 9px; height: 9px; flex: none; border-radius: 50%; background: var(--text-faint); }
.dot.sm { width: 7px; height: 7px; }
.dot.ok, .ok { color: var(--lane-3); } .dot.ok { background: var(--lane-3); }
.dot.fail, .fail { color: var(--accent); } .dot.fail { background: var(--accent); }
.dot.run, .run { color: var(--lane-1); } .dot.run { background: var(--lane-1); }
.dot.pending, .pending { color: var(--lane-2); } .dot.pending { background: var(--lane-2); }
.dot.muted, .muted { color: var(--text-faint); } .dot.muted { background: var(--text-faint); }
.dot.manual, .manual { color: var(--lane-4); } .dot.manual { background: var(--lane-4); }
</style>
