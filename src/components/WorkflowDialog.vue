<script setup lang="ts">
// Workflows: pick a branching model (Git Flow, GitHub Flow, GitLab Flow,
// Trunk-based, or a custom Git Flow) and drive its branch operations natively.
import { computed, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  flowConfig, flowInit, flowStart, flowFinish, flowSetType, flowSetEnvironments,
  createBranch, mergeInto, setConfig, type FlowConfig, type WorkflowType,
} from "../lib/git";
import { promptText, toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; branches: string[]; currentBranch: string | null }>();
const emit = defineEmits<{ (e: "done"): void; (e: "create-pr", source: string): void }>();

const cfg = ref<FlowConfig | null>(null);
const busy = ref(false);
// "chooser" forces the picker even when a type is already saved (via "Change").
const choosing = ref(false);

// Git Flow / custom init form
const main = ref("main");
const develop = ref("develop");
const versiontag = ref("v");
const featurePfx = ref("feature/");
const releasePfx = ref("release/");
const hotfixPfx = ref("hotfix/");
// GitHub / GitLab / trunk base + envs
const base = ref("main");
const envs = ref("staging,production");

const WORKFLOWS: { id: WorkflowType; name: string; desc: string }[] = [
  { id: "gitflow", name: "Git Flow", desc: "Long-lived main + develop, with feature, release & hotfix branches. The classic Driessen model." },
  { id: "github", name: "GitHub Flow", desc: "A single main branch. Branch off, open a pull request, merge back. Simple and continuous." },
  { id: "gitlab", name: "GitLab Flow", desc: "main plus environment branches (staging → production) you promote changes through in order." },
  { id: "trunk", name: "Trunk-based", desc: "Integrate into main constantly via tiny, short-lived branches. Optimised for CI." },
  { id: "custom", name: "Custom Git Flow", desc: "Git Flow with your own branch names and prefixes." },
];

const type = computed<WorkflowType>(() => cfg.value?.workflow ?? "");
const meta = computed(() => WORKFLOWS.find((w) => w.id === type.value) ?? null);
const isFlow = computed(() => type.value === "gitflow" || type.value === "custom");

function detectMain() {
  return props.branches.includes("main") ? "main" : props.branches.includes("master") ? "master" : (cfg.value?.main || "main");
}

async function reload() {
  cfg.value = await flowConfig(props.repoPath).catch(() => null);
  if (cfg.value) {
    main.value = detectMain();
    develop.value = cfg.value.develop;
    versiontag.value = cfg.value.versiontag;
    featurePfx.value = cfg.value.feature;
    releasePfx.value = cfg.value.release;
    hotfixPfx.value = cfg.value.hotfix;
    base.value = detectMain();
    if (cfg.value.environments.length) envs.value = cfg.value.environments.join(",");
    choosing.value = !cfg.value.workflow;
  }
}
watch(open, (o) => o && reload());

// Detect whether the current branch is a finishable Git Flow branch.
const active = computed(() => {
  const c = props.currentBranch;
  const f = cfg.value;
  if (!c || !f || !isFlow.value) return null;
  for (const kind of ["feature", "release", "hotfix", "bugfix"] as const) {
    const p = (f as unknown as Record<string, string>)[kind];
    if (p && c.startsWith(p)) return { kind, name: c.slice(p.length) };
  }
  return null;
});

// For GitHub/GitLab/trunk: is the current branch a topic branch (not the base)?
const onTopic = computed(() => !!props.currentBranch && props.currentBranch !== base.value);
// GitLab Flow promotion chain: base → env1 → env2 …
const chain = computed(() => {
  const list = envs.value.split(",").map((s) => s.trim()).filter(Boolean);
  const nodes = [base.value, ...list];
  return nodes.slice(0, -1).map((from, i) => ({ from, to: nodes[i + 1] }));
});

async function guard(fn: () => Promise<unknown>, ok = "Done") {
  busy.value = true;
  try {
    const r = await fn();
    toast("Workflow", typeof r === "string" && r ? r : ok);
    await reload();
    emit("done");
  } catch (e) {
    toast("Workflow failed", String(e), "error");
  } finally {
    busy.value = false;
  }
}

async function choose(id: WorkflowType) {
  // Clicking the current workflow again clears it (deselect).
  const next: WorkflowType = type.value === id ? "" : id;
  await flowSetType(props.repoPath, next);
  await reload(); // reload() sets `choosing` from whether a workflow is set
}
async function clearType() {
  await flowSetType(props.repoPath, "");
  await reload();
}

// ── Git Flow / custom ──
async function initFlow() {
  await guard(async () => {
    if (type.value === "custom") {
      await setConfig(props.repoPath, "gitflow.prefix.feature", featurePfx.value.trim() || "feature/", false);
      await setConfig(props.repoPath, "gitflow.prefix.release", releasePfx.value.trim() || "release/", false);
      await setConfig(props.repoPath, "gitflow.prefix.hotfix", hotfixPfx.value.trim() || "hotfix/", false);
    }
    return flowInit(props.repoPath, main.value.trim(), develop.value.trim(), versiontag.value.trim() || "v");
  }, "Workflow initialised");
}

async function startFlow(kind: string) {
  const name = await promptText({ title: `Start ${kind}`, label: "Name", placeholder: kind === "release" ? "1.2.0" : "short-name" });
  if (name && name.trim()) guard(() => flowStart(props.repoPath, kind, name.trim()));
}

async function finishFlow() {
  const a = active.value;
  if (!a) return;
  let version: string | undefined;
  if (a.kind === "release" || a.kind === "hotfix") {
    const v = await promptText({ title: `Finish ${a.kind}`, label: "Version tag", value: a.name });
    if (v === null) return;
    version = v.trim() || a.name;
  }
  guard(() => flowFinish(props.repoPath, a.kind, a.name, version));
}

// ── GitHub / GitLab / trunk ──
async function startTopic(label: string) {
  const name = await promptText({ title: label, label: "Branch name", placeholder: "my-change" });
  if (name && name.trim()) guard(() => createBranch(props.repoPath, name.trim(), base.value, true), `Started ${name.trim()}`);
}
function mergeCurrent(del: boolean) {
  const c = props.currentBranch;
  if (c) guard(() => mergeInto(props.repoPath, c, base.value, del));
}
function promote(from: string, to: string) {
  guard(() => mergeInto(props.repoPath, from, to, false));
}
const saveEnvs = () => flowSetEnvironments(props.repoPath, envs.value.trim()).catch(() => {});
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head">
          <h2>{{ choosing || !meta ? "Choose a workflow" : meta.name }}</h2>
          <button v-if="!choosing && meta" class="change" @click="choosing = true">Change</button>
          <button class="x" @click="open = false">✕</button>
        </div>
        <div class="body">
          <!-- Workflow chooser -->
          <template v-if="choosing || !cfg?.workflow">
            <p class="intro">Pick the branching model this repository follows. Plumb drives the branch operations for you.</p>
            <button v-for="w in WORKFLOWS" :key="w.id" class="card" :class="{ on: type === w.id }" :disabled="busy" @click="choose(w.id)">
              <div class="card-name">{{ w.name }}<span v-if="type === w.id" class="tag">current</span></div>
              <div class="card-desc">{{ w.desc }}</div>
            </button>
            <div v-if="type" class="clear-row">
              <span class="hint">Click the current workflow again to deselect it.</span>
              <button class="btn" :disabled="busy" @click="clearType">Use no workflow</button>
            </div>
            <div class="ext">
              Using stacked pull requests?
              <a @click.prevent="openUrl('https://graphite.dev')">Graphite ↗</a> works alongside Plumb via its own CLI.
            </div>
          </template>

          <!-- Git Flow / Custom Git Flow -->
          <template v-else-if="isFlow">
            <template v-if="cfg && !cfg.initialized">
              <p class="intro">{{ meta?.desc }} Set it up:</p>
              <label class="field"><span>Production branch</span>
                <input v-model="main" list="wf-branches" spellcheck="false" /></label>
              <label class="field"><span>Development branch</span>
                <input v-model="develop" spellcheck="false" /></label>
              <label class="field"><span>Version tag prefix</span>
                <input v-model="versiontag" spellcheck="false" /></label>
              <template v-if="type === 'custom'">
                <div class="row3">
                  <label class="field"><span>Feature prefix</span><input v-model="featurePfx" spellcheck="false" /></label>
                  <label class="field"><span>Release prefix</span><input v-model="releasePfx" spellcheck="false" /></label>
                  <label class="field"><span>Hotfix prefix</span><input v-model="hotfixPfx" spellcheck="false" /></label>
                </div>
              </template>
              <datalist id="wf-branches"><option v-for="b in branches" :key="b" :value="b" /></datalist>
              <button class="btn-accent" :disabled="busy" @click="initFlow">Initialise</button>
            </template>

            <template v-else-if="cfg">
              <div class="cfg mono">
                <div><span>production</span>{{ cfg.main }}</div>
                <div><span>develop</span>{{ cfg.develop }}</div>
                <div><span>prefixes</span>{{ cfg.feature }} · {{ cfg.release }} · {{ cfg.hotfix }}</div>
              </div>
              <div v-if="active" class="active">
                On <span class="mono">{{ currentBranch }}</span> — a {{ active.kind }} branch.
                <button class="btn-accent" :disabled="busy" @click="finishFlow">Finish {{ active.kind }}</button>
              </div>
              <div class="starts">
                <div class="starts-label">Create a branch</div>
                <div class="start-row">
                  <button class="btn create" :disabled="busy" @click="startFlow('feature')">＋ Feature…</button>
                  <button class="btn create" :disabled="busy" @click="startFlow('release')">＋ Release…</button>
                  <button class="btn create" :disabled="busy" @click="startFlow('hotfix')">＋ Hotfix…</button>
                </div>
              </div>
            </template>
          </template>

          <!-- GitHub Flow -->
          <template v-else-if="type === 'github'">
            <p class="intro">{{ meta?.desc }}</p>
            <label class="field"><span>Base branch</span>
              <input v-model="base" list="wf-branches" spellcheck="false" /></label>
            <datalist id="wf-branches"><option v-for="b in branches" :key="b" :value="b" /></datalist>
            <button class="btn-accent" :disabled="busy" @click="startTopic('Start a branch')">Start a branch…</button>
            <div v-if="onTopic" class="active">
              On <span class="mono">{{ currentBranch }}</span>.
              <div class="active-actions">
                <button class="btn" :disabled="busy" @click="emit('create-pr', currentBranch!)">Open pull request…</button>
                <button class="btn-accent" :disabled="busy" @click="mergeCurrent(true)">Merge into {{ base }}</button>
              </div>
            </div>
          </template>

          <!-- GitLab Flow -->
          <template v-else-if="type === 'gitlab'">
            <p class="intro">{{ meta?.desc }}</p>
            <label class="field"><span>Base branch</span>
              <input v-model="base" list="wf-branches" spellcheck="false" /></label>
            <label class="field"><span>Environment branches (in promotion order)</span>
              <input v-model="envs" spellcheck="false" placeholder="staging,production" @blur="saveEnvs" /></label>
            <datalist id="wf-branches"><option v-for="b in branches" :key="b" :value="b" /></datalist>
            <button class="btn-accent" :disabled="busy" @click="startTopic('Start a branch')">Start a branch…</button>
            <div v-if="onTopic" class="active">
              On <span class="mono">{{ currentBranch }}</span>.
              <div class="active-actions">
                <button class="btn" :disabled="busy" @click="emit('create-pr', currentBranch!)">Open merge request…</button>
                <button class="btn-accent" :disabled="busy" @click="mergeCurrent(true)">Merge into {{ base }}</button>
              </div>
            </div>
            <div class="starts">
              <div class="starts-label">Promote through environments</div>
              <div class="start-row wrap">
                <button v-for="p in chain" :key="p.to" class="btn" :disabled="busy" @click="promote(p.from, p.to)">
                  {{ p.from }} → {{ p.to }}
                </button>
                <span v-if="!chain.length" class="hint">Add environment branches above.</span>
              </div>
            </div>
          </template>

          <!-- Trunk-based -->
          <template v-else-if="type === 'trunk'">
            <p class="intro">{{ meta?.desc }} Keep branches tiny and merge back the same day.</p>
            <label class="field"><span>Trunk branch</span>
              <input v-model="base" list="wf-branches" spellcheck="false" /></label>
            <datalist id="wf-branches"><option v-for="b in branches" :key="b" :value="b" /></datalist>
            <button class="btn-accent" :disabled="busy" @click="startTopic('Start a short-lived branch')">Start a short-lived branch…</button>
            <div v-if="onTopic" class="active">
              On <span class="mono">{{ currentBranch }}</span>.
              <button class="btn-accent" :disabled="busy" @click="mergeCurrent(true)">Merge into {{ base }} &amp; delete</button>
            </div>
          </template>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 540px; max-width: calc(100vw - 48px); max-height: calc(100vh - 80px); overflow-x: hidden; overflow-y: auto; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-bottom: 2px solid var(--line); position: sticky; top: 0; background: var(--surface); z-index: 1; }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .change { margin-left: auto; height: 24px; padding: 0 10px; background: var(--raised); border: 1px solid var(--line); font-size: 11px; cursor: pointer; color: var(--text-mid); }
.head .x { width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.head .change + .x { margin-left: 0; }
.head .x:only-of-type { margin-left: auto; }
.body { padding: var(--space-4); }
.intro { font-size: 12.5px; color: var(--text-mid); line-height: 1.55; margin: 0 0 var(--space-4); }
.card { display: block; width: 100%; text-align: left; padding: var(--space-3); margin-bottom: var(--space-2); background: var(--bg); border: 1px solid var(--line); cursor: pointer; }
.card:hover { border-color: var(--accent); }
.card.on { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, transparent); }
.card-name { font-size: 13px; font-weight: 700; color: var(--text); display: flex; align-items: center; gap: var(--space-2); }
.card-name .tag { font-size: 9.5px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: var(--accent-on); background: var(--accent); padding: 1px 6px; }
.card-desc { font-size: 11.5px; color: var(--text-dim); line-height: 1.5; margin-top: 3px; }
.clear-row { display: flex; align-items: center; gap: var(--space-3); margin-top: var(--space-3); }
.clear-row .hint { flex: 1; font-size: 11px; color: var(--text-faint); line-height: 1.4; }
.ext { margin-top: var(--space-3); padding-top: var(--space-3); border-top: 1px solid var(--line); font-size: 11.5px; color: var(--text-dim); line-height: 1.5; }
.ext a { color: var(--accent); cursor: pointer; }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); min-width: 0; font-size: 11px; color: var(--text-dim); }
.field input { width: 100%; min-width: 0; box-sizing: border-box; height: 32px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus { outline: none; border-color: var(--accent); }
.row3 { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: var(--space-2); }
.row3 .field { min-width: 0; }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.cfg { border: 1px solid var(--line); padding: var(--space-3); margin-bottom: var(--space-4); font-size: 12px; }
.cfg div { display: flex; gap: var(--space-3); padding: 2px 0; color: var(--text); }
.cfg span { width: 90px; flex: none; color: var(--text-faint); }
.active { background: color-mix(in srgb, var(--accent) 12%, transparent); border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent); padding: var(--space-3); font-size: 12.5px; color: var(--text); margin: var(--space-4) 0; }
.active .btn-accent { margin-top: var(--space-3); }
.active-actions { display: flex; gap: var(--space-2); margin-top: var(--space-3); align-items: center; }
.active-actions .btn-accent { margin-top: 0; margin-left: auto; }
.starts { margin-top: var(--space-4); }
.starts-label { font-size: 11px; color: var(--text-dim); margin-bottom: var(--space-2); }
.start-row { display: flex; gap: var(--space-2); }
.start-row.wrap { flex-wrap: wrap; }
.hint { font-size: 11.5px; color: var(--text-faint); }
.btn { height: 32px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); color: var(--text); font-size: 12.5px; cursor: pointer; }
.btn:disabled { opacity: 0.5; }
.btn.create { font-weight: 600; }
.btn.create:not(:disabled):hover { border-color: var(--accent); color: var(--accent); }
</style>
