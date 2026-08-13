<script setup lang="ts">
// Open a pull request (GitHub) or merge request (GitLab) for the current repo,
// via the connected account that matches its origin.
import { computed, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { prTarget, createPullRequest } from "../lib/accounts";
import { pushBranch } from "../lib/git";
import { toast, openSettings } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; branches: string[]; currentBranch: string | null }>();
const emit = defineEmits<{ (e: "created"): void }>();

const provider = ref(""); // "github" | "gitlab" | ""
const repo = ref("");
const source = ref("");
const target = ref("main");
const title = ref("");
const body = ref("");
const draft = ref(false);
const push = ref(true);
const busy = ref(false);
const stage = ref<"push" | "create" | null>(null);
const error = ref<string | null>(null);

const noun = computed(() => (provider.value === "gitlab" ? "merge request" : "pull request"));
const hasAccount = computed(() => ["github", "gitlab", "azure"].includes(provider.value));

watch(open, async (o) => {
  if (!o) return;
  error.value = null;
  busy.value = false;
  source.value = props.currentBranch ?? props.branches[0] ?? "";
  const others = props.branches.filter((b) => b !== source.value);
  target.value = ["main", "master"].find((b) => props.branches.includes(b)) ?? others[0] ?? "main";
  title.value = source.value.replace(/[-_/]/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  body.value = "";
  draft.value = false;
  try {
    const t = await prTarget(props.repoPath);
    provider.value = t.provider;
    repo.value = t.repo;
  } catch (e) {
    error.value = String(e);
  }
});

async function submit() {
  if (!source.value || !target.value || !title.value.trim()) return;
  if (source.value === target.value) {
    error.value = "Source and target branches must differ.";
    return;
  }
  busy.value = true;
  error.value = null;
  try {
    if (push.value) {
      stage.value = "push";
      await pushBranch(props.repoPath, source.value);
    }
    stage.value = "create";
    const pr = await createPullRequest(props.repoPath, {
      sourceBranch: source.value,
      targetBranch: target.value,
      title: title.value.trim(),
      body: body.value,
      draft: draft.value,
    });
    toast(`${noun.value} #${pr.number} created`, "Opening in browser…");
    openUrl(pr.url).catch(() => {});
    open.value = false;
    emit("created");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
    stage.value = null;
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head">
          <h2>New {{ noun }}</h2>
          <span v-if="repo" class="sub mono">{{ repo }}</span>
          <button class="x" @click="open = false">✕</button>
        </div>

        <div v-if="!hasAccount" class="body no-account">
          <p>No connected account matches this repository's remote.</p>
          <button class="btn-accent" @click="openSettings('accounts')">Connect an account</button>
        </div>

        <div v-else class="body">
          <div class="branch-row">
            <label class="field">
              <span>From</span>
              <select v-model="source">
                <option v-for="b in branches" :key="b" :value="b">{{ b }}</option>
              </select>
            </label>
            <span class="arrow">→</span>
            <label class="field">
              <span>Into</span>
              <select v-model="target">
                <option v-for="b in branches" :key="b" :value="b">{{ b }}</option>
              </select>
            </label>
          </div>

          <label class="field">
            <span>Title</span>
            <input v-model="title" spellcheck="false" />
          </label>
          <label class="field">
            <span>Description</span>
            <textarea v-model="body" rows="5" spellcheck="false" placeholder="Optional"></textarea>
          </label>
          <label class="check"><input type="checkbox" v-model="draft" /> Create as draft</label>
          <label class="check"><input type="checkbox" v-model="push" /> Push <b class="mono">{{ source }}</b> to origin first</label>

          <p v-if="error" class="err mono">{{ error }}</p>
          <div class="actions">
            <button class="btn-accent" :disabled="busy" @click="submit">
              <span v-if="busy" class="spinner-sm"></span>{{ busy ? (stage === "push" ? "Pushing…" : "Creating…") : `Create ${noun}` }}
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
.sheet { width: 580px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; text-transform: capitalize; }
.head .sub { font-size: 11px; color: var(--text-dim); }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); }
.no-account { text-align: center; color: var(--text-mid); }
.no-account .btn-accent { margin-top: var(--space-3); height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; cursor: pointer; }
.branch-row { display: flex; align-items: flex-end; gap: var(--space-3); margin-bottom: var(--space-3); }
.branch-row .arrow { padding-bottom: 6px; color: var(--text-dim); }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); flex: 1; }
.field select, .field input, .field textarea { padding: 8px 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field textarea { resize: vertical; font-family: var(--font-ui); }
.field select:focus, .field input:focus, .field textarea:focus { outline: none; border-color: var(--accent); }
.check { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-mid); margin-bottom: var(--space-3); cursor: pointer; }
.err { color: var(--accent); font-size: 11px; margin: 0 0 var(--space-3); }
.actions { display: flex; align-items: center; gap: var(--space-2); }
.btn-accent { height: 34px; padding: 0 18px; display: flex; align-items: center; gap: var(--space-2); background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; text-transform: capitalize; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.hint { margin-left: auto; font-size: 10.5px; color: var(--text-faint); }
.spinner-sm { width: 11px; height: 11px; border: 2px solid color-mix(in srgb, var(--accent-on) 40%, transparent); border-top-color: var(--accent-on); border-radius: 50%; animation: plumb-spin 0.7s linear infinite; }
@keyframes plumb-spin { to { transform: rotate(360deg); } }
</style>
