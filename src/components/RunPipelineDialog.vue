<script setup lang="ts">
// Kick off CI: on GitHub pick a workflow to dispatch; on GitLab just a branch.
import { computed, ref, watch } from "vue";
import { prTarget, listWorkflows, triggerPipeline, type WorkflowRef } from "../lib/accounts";
import { toast, openSettings } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; branches: string[]; currentBranch: string | null }>();
const emit = defineEmits<{ (e: "triggered"): void }>();

const provider = ref("");
const workflows = ref<WorkflowRef[]>([]);
const workflowId = ref("");
const gitRef = ref("");
const loading = ref(false);
const busy = ref(false);
const error = ref<string | null>(null);

const hasAccount = computed(() => provider.value === "github" || provider.value === "gitlab");
const needsWorkflow = computed(() => provider.value === "github");

watch(open, async (o) => {
  if (!o) return;
  error.value = null;
  workflows.value = [];
  workflowId.value = "";
  gitRef.value = props.currentBranch ?? props.branches[0] ?? "";
  loading.value = true;
  try {
    const t = await prTarget(props.repoPath);
    provider.value = t.provider;
    if (t.provider === "github") {
      workflows.value = await listWorkflows(props.repoPath);
      workflowId.value = workflows.value[0]?.id ?? "";
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
});

async function run() {
  if (!gitRef.value.trim()) return;
  if (needsWorkflow.value && !workflowId.value) {
    error.value = "Pick a workflow.";
    return;
  }
  busy.value = true;
  error.value = null;
  try {
    const msg = await triggerPipeline(props.repoPath, gitRef.value.trim(), workflowId.value || undefined);
    toast("Pipeline", msg);
    open.value = false;
    emit("triggered");
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
        <div class="head"><h2>Run pipeline</h2><button class="x" @click="open = false">✕</button></div>
        <div v-if="loading" class="body msg">Loading…</div>
        <div v-else-if="!hasAccount" class="body no-account">
          <p>No connected account matches this repository's remote.</p>
          <button class="btn-accent" @click="openSettings('accounts')">Connect an account</button>
        </div>
        <div v-else class="body">
          <p class="intro">
            {{ needsWorkflow
              ? "Dispatch a GitHub Actions workflow. It must declare workflow_dispatch to be runnable manually."
              : "Start a new GitLab pipeline for a branch." }}
          </p>

          <label v-if="needsWorkflow" class="field">
            <span>Workflow</span>
            <select v-model="workflowId">
              <option v-for="w in workflows" :key="w.id" :value="w.id">{{ w.name }}</option>
            </select>
            <span v-if="!workflows.length" class="mini-note">No workflows found — none allow manual runs.</span>
          </label>

          <label class="field">
            <span>Branch / ref</span>
            <input v-model="gitRef" list="rp-branches" spellcheck="false" placeholder="main" />
            <datalist id="rp-branches"><option v-for="b in branches" :key="b" :value="b" /></datalist>
          </label>

          <p v-if="error" class="err mono">{{ error }}</p>
          <div class="actions">
            <button class="btn-accent" :disabled="busy || (needsWorkflow && !workflows.length)" @click="run">
              {{ busy ? "Starting…" : "Run pipeline" }}
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
.msg { color: var(--text-dim); font-size: 13px; }
.no-account { text-align: center; color: var(--text-mid); }
.no-account .btn-accent { margin-top: var(--space-3); height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; cursor: pointer; }
.intro { font-size: 12.5px; color: var(--text-mid); line-height: 1.55; margin: 0 0 var(--space-4); }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field select, .field input { height: 34px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field select:focus, .field input:focus { outline: none; border-color: var(--accent); }
.mini-note { font-size: 10.5px; color: var(--text-faint); }
.err { color: var(--accent); font-size: 11px; margin: 0 0 var(--space-3); }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-2); }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
