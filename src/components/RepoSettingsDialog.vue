<script setup lang="ts">
// Per-repo settings — identity, signing, commit template, issue tracking, and
// the .gitignore. Git-backed fields write to the repo's git config / files;
// app-only fields (GitMoji, issue tracker) persist in localStorage per repo.
import { ref, watch } from "vue";
import {
  gitIdentity,
  setGitIdentity,
  getConfig,
  setConfig,
  unsetConfig,
  getRepoDescription,
  setRepoDescription,
  getGitignore,
  setGitignore,
} from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; repoName: string }>();

type Tab = "general" | "signing" | "issues" | "ignore";
const tab = ref<Tab>("general");

// General
const name = ref("");
const email = ref("");
const description = ref("");
const template = ref("");
// Signing
const sign = ref(false);
const format = ref("openpgp"); // openpgp | ssh
const signingKey = ref("");
const allowedSigners = ref("");
// Issue tracking (app-level)
const issueUrl = ref("");
const issuePrefix = ref("#");
// GitMoji (app-level)
const gitmoji = ref(false);
// Ignore
const ignore = ref("");

const CONFIG_KEYS = ["commit.gpgsign", "gpg.format", "user.signingkey", "gpg.ssh.allowedSignersFile", "commit.template"];

function appKey() {
  return `plumb.repo.${props.repoPath}`;
}
function loadApp() {
  try {
    const d = JSON.parse(localStorage.getItem(appKey()) || "{}");
    issueUrl.value = d.issueUrl ?? "";
    issuePrefix.value = d.issuePrefix ?? "#";
    gitmoji.value = !!d.gitmoji;
  } catch {
    /* defaults */
  }
}
function saveApp() {
  localStorage.setItem(
    appKey(),
    JSON.stringify({ issueUrl: issueUrl.value, issuePrefix: issuePrefix.value, gitmoji: gitmoji.value }),
  );
}

watch(open, async (o) => {
  if (!o) return;
  tab.value = "general";
  loadApp();
  const [id, cfg, desc, gi] = await Promise.all([
    gitIdentity(props.repoPath).catch(() => null),
    getConfig(props.repoPath, CONFIG_KEYS).catch(() => ({}) as Record<string, string>),
    getRepoDescription(props.repoPath).catch(() => ""),
    getGitignore(props.repoPath).catch(() => ""),
  ]);
  name.value = id?.name ?? "";
  email.value = id?.email ?? "";
  description.value = desc;
  ignore.value = gi;
  sign.value = cfg["commit.gpgsign"] === "true";
  format.value = cfg["gpg.format"] || "openpgp";
  signingKey.value = cfg["user.signingkey"] || "";
  allowedSigners.value = cfg["gpg.ssh.allowedSignersFile"] || "";
  template.value = cfg["commit.template"] || "";
});

async function guard(fn: () => Promise<unknown>, ok?: string) {
  try {
    await fn();
    if (ok) toast(ok);
  } catch (e) {
    toast("Couldn't save", String(e), "error");
  }
}

const saveIdentity = () =>
  guard(() => setGitIdentity(props.repoPath, name.value.trim(), email.value.trim(), false), "Identity saved");
const saveDescription = () => guard(() => setRepoDescription(props.repoPath, description.value));
const saveTemplate = () =>
  template.value.trim()
    ? guard(() => setConfig(props.repoPath, "commit.template", template.value.trim()))
    : guard(() => unsetConfig(props.repoPath, "commit.template"));
const saveSign = () => guard(() => setConfig(props.repoPath, "commit.gpgsign", String(sign.value)));
const saveFormat = () => guard(() => setConfig(props.repoPath, "gpg.format", format.value));
const saveSigningKey = () =>
  signingKey.value.trim()
    ? guard(() => setConfig(props.repoPath, "user.signingkey", signingKey.value.trim()))
    : guard(() => unsetConfig(props.repoPath, "user.signingkey"));
const saveAllowed = () =>
  allowedSigners.value.trim()
    ? guard(() => setConfig(props.repoPath, "gpg.ssh.allowedSignersFile", allowedSigners.value.trim()))
    : guard(() => unsetConfig(props.repoPath, "gpg.ssh.allowedSignersFile"));
const saveIgnore = () => guard(() => setGitignore(props.repoPath, ignore.value), "Saved .gitignore");
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="rail">
          <div class="rail-title">{{ repoName }}</div>
          <button v-for="t in (['general','signing','issues','ignore'] as Tab[])" :key="t" class="rail-item" :class="{ on: tab === t }" @click="tab = t">
            {{ ({ general: 'General', signing: 'Signing', issues: 'Issue tracking', ignore: 'Ignore' })[t] }}
          </button>
        </div>
        <div class="content">
          <div class="content-head">
            <h2>Repository settings</h2>
            <button class="x" @click="open = false">✕</button>
          </div>
          <div class="body">
            <!-- General -->
            <template v-if="tab === 'general'">
              <label class="field"><span>Committer name</span><input v-model="name" @blur="saveIdentity" spellcheck="false" /></label>
              <label class="field"><span>Committer email</span><input v-model="email" @blur="saveIdentity" spellcheck="false" /></label>
              <div class="hint">Saved to this repository's git config (overrides your global identity).</div>
              <label class="field"><span>Description</span><input v-model="description" @blur="saveDescription" spellcheck="false" /></label>
              <label class="field"><span>Commit template file</span><input v-model="template" @blur="saveTemplate" placeholder="~/.git-commit-template" spellcheck="false" /></label>
              <label class="check"><input type="checkbox" v-model="gitmoji" @change="saveApp" /> Convert GitMoji <b class="mono">:sparkles:</b> to emoji hints in the composer</label>
            </template>

            <!-- Signing -->
            <template v-else-if="tab === 'signing'">
              <label class="check"><input type="checkbox" v-model="sign" @change="saveSign" /> Sign commits by default</label>
              <label class="field"><span>Signature format</span>
                <select v-model="format" @change="saveFormat"><option value="openpgp">OpenPGP (GPG)</option><option value="ssh">SSH</option></select>
              </label>
              <label class="field"><span>Signing key</span><input v-model="signingKey" @blur="saveSigningKey" placeholder="key id or ssh key path" spellcheck="false" /></label>
              <label class="field"><span>Allowed signers file</span><input v-model="allowedSigners" @blur="saveAllowed" placeholder="~/.ssh/allowed_signers" spellcheck="false" /></label>
              <div class="hint">These map to <span class="mono">commit.gpgsign</span>, <span class="mono">gpg.format</span>, <span class="mono">user.signingkey</span>, and <span class="mono">gpg.ssh.allowedSignersFile</span>.</div>
            </template>

            <!-- Issue tracking -->
            <template v-else-if="tab === 'issues'">
              <label class="field"><span>Issue tracker URL</span><input v-model="issueUrl" @blur="saveApp" placeholder="https://…/issues/{id}" spellcheck="false" /></label>
              <label class="field"><span>Issue number prefix</span><input v-model="issuePrefix" @blur="saveApp" placeholder="#" spellcheck="false" /></label>
              <div class="hint">Use <span class="mono">{id}</span> in the URL where the issue number goes; commit messages with <span class="mono">{{ issuePrefix }}123</span> become links.</div>
            </template>

            <!-- Ignore -->
            <template v-else>
              <div class="hint">Edit this repository's <span class="mono">.gitignore</span>.</div>
              <textarea v-model="ignore" class="ignore mono" spellcheck="false"></textarea>
              <button class="btn-accent" @click="saveIgnore">Save .gitignore</button>
            </template>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 760px; max-width: calc(100vw - 40px); height: 560px; max-height: calc(100vh - 80px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; }
.rail { width: 180px; flex: none; background: var(--subtle); border-right: 2px solid var(--line); padding: var(--space-3); }
.rail-title { font-size: 12px; font-weight: 800; padding: 0 var(--space-2) var(--space-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rail-item { display: block; width: 100%; text-align: left; padding: 7px var(--space-2); background: none; border: none; color: var(--text-mid); font-size: 12.5px; cursor: pointer; }
.rail-item.on { background: var(--raised); color: var(--text); box-shadow: inset 2px 0 0 var(--accent); }
.content { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.content-head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.content-head h2 { margin: 0; font-size: 15px; font-weight: 800; }
.content-head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); overflow-y: auto; display: flex; flex-direction: column; }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field input, .field select { height: 32px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus, .field select:focus { outline: none; border-color: var(--accent); }
.check { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--text); margin-bottom: var(--space-3); cursor: pointer; }
.hint { font-size: 11px; color: var(--text-faint); margin-bottom: var(--space-3); line-height: 1.5; }
.ignore { flex: 1; min-height: 260px; resize: vertical; padding: var(--space-3); background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 12.5px; line-height: 1.5; margin-bottom: var(--space-3); }
.ignore:focus { outline: none; border-color: var(--accent); }
.btn-accent { align-self: flex-start; height: 32px; padding: 0 16px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
</style>
