<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  workingStatus,
  stagePaths,
  unstagePaths,
  stageHunk,
  unstageHunk,
  stageLines,
  unstageLines,
  discardPaths,
  addToGitignore,
  openInEditor,
  fileDiff,
  commit,
  gitIdentity,
  setGitIdentity,
  type GitIdentity,
  type StatusEntry,
} from "../lib/git";
import {
  openContextMenu,
  openFullscreen,
  openFileInspector,
  toast,
  aiStore,
  refreshAiConfig,
  openSettings,
  promptText,
  type MenuItem,
} from "../lib/ui";
import { generateCommitMessage, type GeneratedMessage } from "../lib/ai";
import DiffView from "./DiffView.vue";
import ResizeHandle from "./ResizeHandle.vue";
import SplitCommits from "./SplitCommits.vue";

const props = defineProps<{ repoPath: string }>();
const emit = defineEmits<{ (e: "committed", message: string): void }>();

const files = ref<StatusEntry[]>([]);
const selected = ref<string | null>(null);
const message = ref("");
const amend = ref(false);
const signOff = ref(false);
const sign = ref(false);
const identity = ref<GitIdentity | null>(null);
const hasIdentity = computed(() => !!identity.value?.name && !!identity.value?.email);

async function loadIdentity() {
  try {
    identity.value = await gitIdentity(props.repoPath);
    sign.value = identity.value.signing;
  } catch {
    identity.value = null;
  }
}

async function setupIdentity() {
  const name = await promptText({ title: "Commit identity", label: "Your name", placeholder: "Ada Lovelace" });
  if (name === null || !name.trim()) return;
  const email = await promptText({ title: "Commit identity", label: "Your email", placeholder: "ada@example.com" });
  if (email === null || !email.trim()) return;
  try {
    await setGitIdentity(props.repoPath, name.trim(), email.trim(), true);
    toast("Identity set", `${name.trim()} <${email.trim()}>`);
    await loadIdentity();
  } catch (e) {
    toast("Couldn't set identity", String(e), "error");
  }
}
const conventional = ref(true); // visual toggle; drives AI drafts once wired
const busy = ref(false);
const error = ref<string | null>(null);
const filesWidth = ref(360);

// Keyboard shortcuts while the Changes view is mounted:
//  ⌘⏎ commit · ⌘G draft AI message · j/k move file selection · space stage/unstage.
function onComposerKey(e: KeyboardEvent) {
  if (e.metaKey) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (canCommit.value && !busy.value) doCommit();
    } else if (e.key.toLowerCase() === "g") {
      e.preventDefault();
      if (!generating.value) generate();
    }
    return;
  }
  // Don't hijack keys while typing in the message box or a field.
  const el = document.activeElement as HTMLElement | null;
  if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;

  const list = [...leftOut.value, ...inCommit.value];
  if (!list.length) return;
  if (e.key === "j" || e.key === "ArrowDown") {
    e.preventDefault();
    moveSelection(list, 1);
  } else if (e.key === "k" || e.key === "ArrowUp") {
    e.preventDefault();
    moveSelection(list, -1);
  } else if (e.key === " ") {
    const cur = list.find((f) => f.path === selected.value);
    if (cur) {
      e.preventDefault();
      toggle(cur);
    }
  }
}
function moveSelection(list: StatusEntry[], dir: 1 | -1) {
  const i = list.findIndex((f) => f.path === selected.value);
  const next = i < 0 ? list[0] : list[Math.max(0, Math.min(list.length - 1, i + dir))];
  if (next) selected.value = next.path;
}
onMounted(() => window.addEventListener("keydown", onComposerKey));
onUnmounted(() => window.removeEventListener("keydown", onComposerKey));

// AI commit messages
const generating = ref(false);
const aiDraft = ref<GeneratedMessage | null>(null);
const splitOpen = ref(false);
function onSplitDone() {
  reload();
  emit("committed", "");
}
const defaultProvider = computed(
  () => aiStore.config.providers.find((p) => p.id === aiStore.config.defaultId) ?? null,
);
refreshAiConfig();

async function generate(style: "normal" | "shorter" | "detailed" = "normal") {
  if (!inCommit.value.length) {
    error.value = "Stage some files first — the draft is written from what's in the commit.";
    return;
  }
  if (!defaultProvider.value) {
    openSettings("ai");
    return;
  }
  generating.value = true;
  error.value = null;
  try {
    const res = await generateCommitMessage(
      props.repoPath,
      aiStore.config.defaultId,
      conventional.value,
      style,
    );
    message.value = res.message;
    aiDraft.value = res;
  } catch (e) {
    error.value = String(e);
    toast("Generate failed", String(e), "error");
  } finally {
    generating.value = false;
  }
}

// The chip is a claim about the current text; once you edit, it's yours.
watch(message, () => {
  if (aiDraft.value && message.value !== aiDraft.value.message) aiDraft.value = null;
});

const inCommit = computed(() => files.value.filter((f) => f.staged));
const leftOut = computed(() => files.value.filter((f) => !f.staged));
const canCommit = computed(
  () => inCommit.value.length > 0 && message.value.trim().length > 0 && !busy.value,
);

const selectedEntry = computed(() => files.value.find((f) => f.path === selected.value) ?? null);
const hasUnstaged = computed(() => !!selectedEntry.value?.unstaged);
const hasStaged = computed(() => !!selectedEntry.value?.staged);

// Which side of the diff we're viewing, and the matching per-hunk action.
const diffMode = ref<"unstaged" | "staged">("unstaged");
const diffRefresh = ref(0);
const actionLabel = computed(() => (diffMode.value === "unstaged" ? "Stage hunk" : "Unstage hunk"));

// When the selection changes, show whichever side actually has changes.
watch(selected, () => {
  diffMode.value = selectedEntry.value?.unstaged ? "unstaged" : "staged";
});

async function onHunkAction(index: number) {
  if (!selected.value) return;
  const file = selected.value;
  try {
    if (diffMode.value === "unstaged") await stageHunk(props.repoPath, file, index);
    else await unstageHunk(props.repoPath, file, index);
    await reload();
    diffRefresh.value++;
  } catch (e) {
    toast("Hunk failed", String(e), "error");
  }
}

async function onLineAction(hunkIndex: number, lines: number[]) {
  if (!selected.value || !lines.length) return;
  const file = selected.value;
  try {
    if (diffMode.value === "unstaged") await stageLines(props.repoPath, file, hunkIndex, lines);
    else await unstageLines(props.repoPath, file, hunkIndex, lines);
    await reload();
    diffRefresh.value++;
  } catch (e) {
    toast("Line staging failed", String(e), "error");
  }
}

async function reload() {
  files.value = await workingStatus(props.repoPath);
  if (selected.value && !files.value.some((f) => f.path === selected.value)) {
    selected.value = files.value[0]?.path ?? null;
  } else if (!selected.value) {
    selected.value = files.value[0]?.path ?? null;
  }
}

watch(
  () => props.repoPath,
  () => {
    reload();
    loadIdentity();
  },
  { immediate: true },
);

async function toggle(entry: StatusEntry) {
  error.value = null;
  try {
    if (entry.staged) await unstagePaths(props.repoPath, [entry.path]);
    else await stagePaths(props.repoPath, [entry.path]);
    await reload();
  } catch (e) {
    error.value = String(e);
  }
}

async function stageAll() {
  await stagePaths(props.repoPath, leftOut.value.map((f) => f.path));
  await reload();
}

async function doCommit() {
  if (!canCommit.value) return;
  if (!hasIdentity.value) {
    await setupIdentity();
    if (!hasIdentity.value) return; // user cancelled
  }
  busy.value = true;
  error.value = null;
  const msg = message.value;
  try {
    await commit(props.repoPath, msg, amend.value, signOff.value, sign.value);
    message.value = "";
    amend.value = false;
    await reload();
    emit("committed", msg);
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

const codeClass = (c: string) =>
  ({ A: "add", "?": "add", M: "mod", D: "del", R: "mod", U: "conflict" })[c] ?? "mod";

/** Diff-staged flag for one entry: prefer the working-tree (unstaged) view. */
const diffStaged = (e: StatusEntry) => (e.unstaged ? false : true);

function fileMenu(e: MouseEvent, entry: StatusEntry) {
  const items: MenuItem[] = [
    {
      label: entry.staged ? "Unstage file" : "Stage file",
      action: () => toggle(entry),
    },
    { label: "File history", action: () => openFileInspector(props.repoPath, entry.path, "history") },
    { label: "Blame", action: () => openFileInspector(props.repoPath, entry.path, "blame") },
    { label: "Open in editor", action: () => openInEditor(`${props.repoPath.replace(/\/$/, "")}/${entry.path}`).catch(() => {}) },
    {
      label: "Add to .gitignore",
      action: async () => {
        try {
          await addToGitignore(props.repoPath, entry.path);
          await reload();
          toast("Added to .gitignore", entry.path);
        } catch (err) {
          toast("Couldn't update .gitignore", String(err), "error");
        }
      },
    },
    {
      label: "Copy path",
      action: async () => {
        try {
          await navigator.clipboard.writeText(entry.path);
          toast("Path copied");
        } catch {
          toast("Couldn't copy path", undefined, "error");
        }
      },
    },
    { separator: true, label: "" },
    {
      label: "Discard changes…",
      danger: true,
      action: async () => {
        if (!window.confirm(`Discard changes to ${entry.path}? This cannot be undone.`)) return;
        try {
          await discardPaths(props.repoPath, [entry.path]);
          await reload();
          toast("Changes discarded", entry.path);
        } catch (err) {
          toast("Discard failed", String(err), "error");
        }
      },
    },
  ];
  openContextMenu(e, items);
}

function expand() {
  if (!files.value.length) return;
  openFullscreen({
    title: "Working changes",
    subtitle: `${files.value.length} files`,
    files: files.value.map((f) => ({ path: f.path, code: f.code })),
    activeFile: selected.value,
    load: (file) => {
      const entry = files.value.find((f) => f.path === file);
      return fileDiff(props.repoPath, file, entry ? diffStaged(entry) : false);
    },
  });
}
</script>

<template>
  <div class="changes">
    <div class="cols">
      <!-- Files -->
      <div class="files" :style="{ width: filesWidth + 'px' }">
        <div class="files-head">
          <span class="tick filled">✓</span>
          <span class="section-label">Files in this commit</span>
          <span class="hint mono">{{ inCommit.length }} selected</span>
        </div>
        <div
          v-for="f in inCommit"
          :key="f.path"
          class="file-row"
          :class="{ selected: f.path === selected }"
          @click="selected = f.path"
          @contextmenu="fileMenu($event, f)"
        >
          <span class="tick filled" @click.stop="toggle(f)">✓</span>
          <span class="code" :class="codeClass(f.code)">{{ f.code }}</span>
          <span class="path ellipsis mono">{{ f.path }}</span>
        </div>
        <div v-if="!inCommit.length" class="files-empty">Nothing staged yet.</div>

        <div class="files-head sub">
          <span class="tick"></span>
          <span class="section-label">Left out</span>
          <button v-if="leftOut.length" class="stage-all" @click="stageAll">Stage all</button>
        </div>
        <div
          v-for="f in leftOut"
          :key="f.path"
          class="file-row muted"
          :class="{ selected: f.path === selected }"
          @click="selected = f.path"
          @contextmenu="fileMenu($event, f)"
        >
          <span class="tick" @click.stop="toggle(f)"></span>
          <span class="code" :class="codeClass(f.code)">{{ f.code }}</span>
          <span class="path ellipsis mono">{{ f.path }}</span>
        </div>
        <div v-if="!leftOut.length && !inCommit.length" class="files-empty big">
          Working tree clean — nothing to commit.
        </div>
      </div>

      <ResizeHandle v-model="filesWidth" side="left" :min="220" :max="620" />

      <!-- Diff -->
      <div class="diff-col">
        <div class="diff-head mono">
          <span class="ellipsis">{{ selected ?? "No file selected" }}</span>
          <div v-if="selected" class="diff-seg">
            <button :class="{ on: diffMode === 'unstaged' }" :disabled="!hasUnstaged" @click="diffMode = 'unstaged'">Unstaged</button>
            <button :class="{ on: diffMode === 'staged' }" :disabled="!hasStaged" @click="diffMode = 'staged'">Staged</button>
          </div>
          <button
            class="expand"
            title="Full screen — file list on the right"
            :disabled="!files.length"
            @click="expand"
          >⤢</button>
        </div>
        <DiffView
          class="diff-pane"
          :repo-path="repoPath"
          :file="selected"
          :staged="diffMode === 'staged'"
          :action-label="actionLabel"
          :refresh="diffRefresh"
          selectable
          @hunk-action="onHunkAction"
          @line-action="onLineAction"
        />
      </div>
    </div>

    <!-- Composer -->
    <div class="composer">
      <div class="composer-head">
        <span class="section-label">Message</span>
        <button class="gen" :disabled="generating" @click="generate('normal')">
          <span v-if="generating" class="spinner-sm"></span><span v-else class="sq"></span>
          {{ generating ? "Generating…" : "Generate message" }} <kbd>⌘G</kbd>
        </button>
        <template v-if="aiDraft && !generating">
          <button class="gen-alt" @click="generate('normal')" title="Regenerate">Regenerate</button>
          <button class="gen-alt" @click="generate('shorter')">Shorter</button>
          <button class="gen-alt" @click="generate('detailed')">More detail</button>
        </template>
        <button class="gen-alt" :disabled="!files.length" title="Let AI split changes into several commits" @click="splitOpen = true">Split…</button>
        <span class="grow"></span>
        <button
          class="provider-pick mono"
          @click="openSettings('ai')"
          :title="defaultProvider ? 'Change AI provider' : 'Set up an AI provider'"
        >
          <template v-if="defaultProvider">
            <span class="sq-priv"></span>{{ defaultProvider.label }}
          </template>
          <template v-else>Set up AI…</template>
        </button>
        <label class="toggle mono">
          Conventional
          <input type="checkbox" v-model="conventional" />
        </label>
      </div>

      <div v-if="aiDraft" class="ai-chip mono">
        <span class="tag">AI DRAFT</span>
        <span>read {{ aiDraft.files }} file{{ aiDraft.files === 1 ? "" : "s" }} · +{{ aiDraft.added }} −{{ aiDraft.removed }} · {{ (aiDraft.ms / 1000).toFixed(1) }}s</span>
        <span class="grow"></span>
        <span class="src">{{ aiDraft.model }} · {{ aiDraft.isLocal ? "local" : aiDraft.host }}</span>
      </div>

      <div v-if="identity && !hasIdentity" class="id-warn">
        <span>No commit identity set — your commits need a name and email.</span>
        <button class="id-btn" @click="setupIdentity">Set name &amp; email</button>
      </div>

      <textarea
        v-model="message"
        class="msg mono"
        placeholder="Summary of this commit…"
        spellcheck="false"
      ></textarea>
      <div class="composer-actions">
        <button class="btn btn-accent" :disabled="!canCommit" @click="doCommit">
          {{ amend ? "Amend commit" : `Commit ${inCommit.length} file${inCommit.length === 1 ? "" : "s"}` }}
          <kbd>⌘⏎</kbd>
        </button>
        <label class="chk"><input type="checkbox" v-model="amend" />Amend last</label>
        <label class="chk"><input type="checkbox" v-model="signOff" />Sign off</label>
        <label class="chk" title="Sign this commit with your configured GPG/SSH key">
          <input type="checkbox" v-model="sign" />Sign
        </label>
        <span v-if="error" class="err mono">{{ error }}</span>
      </div>
    </div>

    <SplitCommits
      v-model="splitOpen"
      :repo-path="repoPath"
      :conventional="conventional"
      :changed="files"
      @done="onSplitDone"
    />
  </div>
</template>

<style scoped>
.changes { flex: 1; display: flex; flex-direction: column; min-height: 0; }
.cols { flex: 1; display: flex; min-height: 0; }

/* affordances */
.file-row, .tick, .stage-all, .btn:not(:disabled), .toggle, .chk { cursor: pointer; }
.msg { cursor: text; }

.files {
  width: 360px;
  flex: none;
  border-right: 2px solid var(--line);
  overflow-y: auto;
  background: var(--surface);
}
.files-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 28px;
  padding: 0 var(--space-4);
  background: var(--subtle);
  border-bottom: 1px solid var(--line);
}
.files-head.sub { border-top: 2px solid var(--line); }
.files-head .hint { margin-left: auto; font-size: 10.5px; color: var(--text-faint); }
.stage-all { margin-left: auto; font-size: 11px; background: transparent; border: 1px solid var(--line); padding: 2px 8px; color: var(--text-mid); }

.file-row {
  height: 32px;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 0 var(--space-4);
  border-bottom: 1px solid var(--line-soft);
  font-size: 11.5px;
}
.file-row:hover { background: color-mix(in srgb, var(--raised) 50%, transparent); }
.file-row.selected { background: var(--raised); box-shadow: inset 2px 0 0 var(--accent); }
.file-row.muted { color: var(--text-dim); }
.file-row .path { flex: 1; }

.tick {
  width: 13px; height: 13px; flex: none;
  border: 2px solid var(--text-faint);
  display: flex; align-items: center; justify-content: center;
  font-size: 9px; font-weight: 700; line-height: 1;
  color: transparent;
}
.tick.filled { background: var(--accent); border-color: var(--accent); color: var(--accent-on); }

.code { width: 10px; flex: none; font-weight: 700; }
.code.add { color: var(--lane-1); }
.code.mod { color: var(--lane-2); }
.code.del { color: var(--diff-del-fg); }
.code.conflict { color: var(--accent); }

.files-empty { padding: var(--space-3) var(--space-4); font-size: 11.5px; color: var(--text-faint); }
.files-empty.big { padding: var(--space-6) var(--space-4); }

.diff-col { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.diff-head {
  height: 30px; flex: none;
  display: flex; align-items: center; gap: var(--space-2);
  padding: 0 var(--space-3) 0 var(--space-4);
  background: var(--subtle);
  border-bottom: 1px solid var(--line);
  font-size: 11.5px;
}
.diff-head .ellipsis { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.diff-seg { display: flex; gap: 2px; flex: none; }
.diff-seg button { font-size: 10.5px; font-weight: 600; padding: 3px 9px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.diff-seg button.on { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }
.diff-seg button:disabled { opacity: 0.4; cursor: default; }
.diff-head .expand { width: 28px; height: 22px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; font-size: 13px; }
.diff-pane { flex: 1; min-width: 0; }

/* Composer */
.composer {
  flex: none;
  border-top: 2px solid var(--line);
  background: var(--surface);
  padding: var(--space-3) var(--space-4) var(--space-4);
}
.composer-head { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-2); }
.composer-head .grow { flex: 1; }
.gen {
  display: flex; align-items: center; gap: var(--space-2);
  font-size: 12px; font-weight: 700;
  padding: 6px 12px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent);
  cursor: pointer;
}
.gen:disabled { opacity: 0.7; cursor: default; }
.gen .sq { width: 9px; height: 9px; background: var(--accent-on); }
.gen kbd { font-family: var(--font-mono); font-size: 10px; opacity: 0.7; }
.spinner-sm {
  width: 10px; height: 10px; flex: none;
  border: 2px solid color-mix(in srgb, var(--accent-on) 40%, transparent);
  border-top-color: var(--accent-on);
  border-radius: 50%;
  animation: plumb-spin 0.7s linear infinite;
}
@keyframes plumb-spin { to { transform: rotate(360deg); } }
.gen-alt { font-size: 11.5px; font-weight: 600; padding: 6px 10px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.provider-pick {
  display: flex; align-items: center; gap: var(--space-2);
  font-size: 11px; padding: 5px 10px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer;
}
.provider-pick .sq-priv { width: 8px; height: 8px; background: var(--text-faint); flex: none; }
.toggle { display: flex; align-items: center; gap: var(--space-2); font-size: 10.5px; color: var(--text-dim); cursor: pointer; }

.ai-chip {
  display: flex; align-items: center; gap: var(--space-2);
  font-size: 10.5px; color: var(--text-dim);
  margin-bottom: var(--space-2);
}
.ai-chip .tag { font-weight: 700; color: var(--accent); border: 1px solid var(--accent-fill); padding: 2px 6px; letter-spacing: 0.08em; }
.ai-chip .grow { flex: 1; }
.ai-chip .src { color: var(--text-faint); }

.msg {
  width: 100%;
  min-height: 68px;
  resize: vertical;
  background: var(--bg);
  border: 1px solid var(--line);
  color: var(--text);
  font-size: 13px;
  line-height: 1.5;
  padding: var(--space-3);
  user-select: text;
}
.msg:focus { outline: none; border-color: var(--accent); }

.composer-actions { display: flex; align-items: center; gap: var(--space-3); margin-top: var(--space-3); }
.btn {
  height: 34px; padding: 0 18px;
  display: flex; align-items: center; gap: var(--space-2);
  background: var(--raised); border: 1px solid var(--line);
  font-size: 12.5px; font-weight: 600;
}
.btn:disabled { opacity: 0.5; }
.btn-accent { background: var(--accent); color: var(--accent-on); border-color: var(--accent); font-weight: 700; }
.btn-accent:disabled { opacity: 0.4; }
.btn kbd { font-family: var(--font-mono); font-size: 10px; opacity: 0.7; }
.chk { display: flex; align-items: center; gap: 6px; font-size: 11.5px; color: var(--text-dim); }
.err { color: var(--accent); font-size: 11px; margin-left: auto; }
.id-warn {
  display: flex; align-items: center; gap: var(--space-3); margin-top: var(--space-3);
  padding: 8px 12px; font-size: 12px; color: var(--text);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
}
.id-btn { margin-left: auto; flex: none; padding: 5px 12px; font-size: 11.5px; font-weight: 700;
  background: var(--accent); color: var(--accent-on); border: none; cursor: pointer; white-space: nowrap; }
</style>
