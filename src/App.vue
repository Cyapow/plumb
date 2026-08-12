<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, provide, reactive, ref } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { revealItemInDir, openUrl } from "@tauri-apps/plugin-opener";
import {
  openRepo,
  isRepo,
  initRepo,
  openInTerminal,
  initialCommit,
  gitIdentity,
  setGitIdentity,
  listRemotes,
  listCommits,
  listBranches,
  workingStatus,
  type RepoInfo,
  type RemoteInfo,
  type CommitRow,
  type BranchInfo,
  type StatusEntry,
} from "./lib/git";
import { relativeTime, initials } from "./lib/format";
import {
  checkoutBranch,
  checkoutRemoteBranch,
  checkoutCommit,
  createBranch,
  deleteBranch,
  reset as gitReset,
  uncommit,
  commit as gitCommit,
  fetch as gitFetch,
  pull as gitPull,
  push as gitPush,
  pushAdvanced,
  pullMode,
  deleteRemoteBranch,
  listStashes,
  stashSave,
  stashApply,
  stashPop,
  stashDrop,
  listTags,
  listFiles,
  mergeBranch,
  rebaseBranch,
  cherryPick,
  revertCommit,
  opAbort,
  opContinue,
  repoState,
  watchRepo,
  type StashEntry,
  type TagInfo,
  type RepoState,
} from "./lib/git";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  openContextMenu,
  promptText,
  toast,
  fullscreen,
  appState,
  toggleTheme,
  openSettings,
  refreshConnections,
  type MenuItem,
} from "./lib/ui";
import { listPullRequests } from "./lib/accounts";
import CommitGraph from "./components/CommitGraph.vue";
import ChangesView from "./components/ChangesView.vue";
import PullRequests from "./components/PullRequests.vue";
import CommitDetail from "./components/CommitDetail.vue";
import ContextMenu from "./components/ContextMenu.vue";
import FileInspector from "./components/FileInspector.vue";
import Toasts from "./components/Toasts.vue";
import DiffFullscreen from "./components/DiffFullscreen.vue";
import ResizeHandle from "./components/ResizeHandle.vue";
import Settings from "./components/Settings.vue";
import PlumbMark from "./components/PlumbMark.vue";
import CommandPalette from "./components/CommandPalette.vue";
import CloneDialog from "./components/CloneDialog.vue";
import PublishDialog from "./components/PublishDialog.vue";
import RemotesDialog from "./components/RemotesDialog.vue";
import ConflictDialog from "./components/ConflictDialog.vue";
import RebaseDialog from "./components/RebaseDialog.vue";
import ConnectRemoteDialog from "./components/ConnectRemoteDialog.vue";
import InputDialog from "./components/InputDialog.vue";
import HomePage from "./components/HomePage.vue";
import BranchTree from "./components/BranchTree.vue";
import { buildBranchTree } from "./lib/branchtree";
import type { PaletteItem } from "./lib/palette";
import { loadRecents, saveRecents, type RecentRepo } from "./lib/recents";

const repo = ref<RepoInfo | null>(null);
const commits = ref<CommitRow[]>([]);
const branches = ref<BranchInfo[]>([]);
const status = ref<StatusEntry[]>([]);
const selected = ref<string | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const view = ref<"history" | "changes" | "prs">("history");
const prCount = ref<number | null>(null);
const paletteOpen = ref(false);
const commitFilter = ref("");
const cloneOpen = ref(false);
const publishOpen = ref(false);
const remotesOpen = ref(false);
const conflictOpen = ref(false);
const rebaseOpen = ref(false);
const rebaseBase = ref<string | null>(null);
const rebaseCommits = ref<CommitRow[]>([]);
const connectRemoteOpen = ref(false);

// Open-repo tabs + recents + home state
interface Tab {
  path: string;
  name: string;
}
const tabs = ref<Tab[]>([]);
const activePath = ref<string>(""); // "" = home screen
const recents = ref<RecentRepo[]>(loadRecents());
const showWorkspace = computed(
  () => !!activePath.value && !!repo.value && repo.value.path === activePath.value,
);

function pushRecent(t: RecentRepo) {
  recents.value = [t, ...recents.value.filter((r) => r.path !== t.path)].slice(0, 12);
  saveRecents(recents.value);
}
function forgetRecent(path: string) {
  recents.value = recents.value.filter((r) => r.path !== path);
  saveRecents(recents.value);
}
function selectTab(path: string) {
  if (repo.value?.path === path) {
    activePath.value = path;
    return;
  }
  loadRepo(path);
}
function closeTab(path: string) {
  const idx = tabs.value.findIndex((t) => t.path === path);
  tabs.value = tabs.value.filter((t) => t.path !== path);
  if (activePath.value === path) {
    const next = tabs.value[idx] ?? tabs.value[idx - 1] ?? null;
    if (next) selectTab(next.path);
    else activePath.value = "";
  }
}
function goHome() {
  activePath.value = "";
}

const stashes = ref<StashEntry[]>([]);
const tags = ref<TagInfo[]>([]);
const files = ref<string[]>([]);
const remotes = ref<RemoteInfo[]>([]);
const state = ref<RepoState>({ state: "clean", conflicts: false });

// Compact "host/owner/repo" from a git URL for the remote row.
function shortHost(url: string): string {
  const m = url.match(/[/:]([^/:]+\/[^/]+?)(?:\.git)?$/);
  return m ? m[1] : url;
}

function loadExtras(path: string) {
  listStashes(path).then((s) => (stashes.value = s)).catch(() => (stashes.value = []));
  listTags(path).then((t) => (tags.value = t)).catch(() => (tags.value = []));
  listFiles(path).then((f) => (files.value = f)).catch(() => (files.value = []));
  listRemotes(path).then((r) => (remotes.value = r)).catch(() => (remotes.value = []));
  repoState(path).then((r) => (state.value = r)).catch(() => (state.value = { state: "clean", conflicts: false }));
}

const opLabel = computed(
  () =>
    ({ merge: "Merging", rebase: "Rebasing", cherrypick: "Cherry-picking", revert: "Reverting" } as Record<string, string>)[
      state.value.state
    ] ?? "",
);

function loadPrCount(path: string) {
  prCount.value = null;
  listPullRequests(path)
    .then((r) => (prCount.value = r.status === "ok" ? r.items.length : null))
    .catch(() => (prCount.value = null));
}

// A single-level operation history, à la GitKraken. Undo soft-resets the last
// commit (keeping its changes staged); redo re-creates it.
const undoOp = ref<{ message: string } | null>(null);
const redoOp = ref<{ message: string } | null>(null);

// Resizable panels
const sidebarWidth = ref(250);
const detailWidth = ref(360);

const syncing = ref(false);
const syncLabel = ref("");

const localBranches = computed(() => branches.value.filter((b) => !b.is_remote));
const remoteBranches = computed(() => branches.value.filter((b) => b.is_remote));
const headBranch = computed(() => repo.value?.head_branch ?? "HEAD");
const headInfo = computed(() => localBranches.value.find((b) => b.is_head));

// Toolbar search filters the commit list (by message, author, or hash).
const visibleCommits = computed(() => {
  const q = commitFilter.value.trim().toLowerCase();
  if (!q) return commits.value;
  return commits.value.filter(
    (c) =>
      c.summary.toLowerCase().includes(q) ||
      c.author_name.toLowerCase().includes(q) ||
      c.id.toLowerCase().includes(q),
  );
});

const laneColor = (i: number) => `var(--lane-${i % 7})`;

// Branch tree (local + remote) + a stable colour per branch.
const localTree = computed(() => buildBranchTree(localBranches.value));
const remoteTree = computed(() => buildBranchTree(remoteBranches.value));
const branchColors = computed(() => {
  const m = new Map<string, string>();
  branches.value.forEach((b, i) => m.set(b.name, laneColor(i)));
  return m;
});
const colorFor = (name: string) => branchColors.value.get(name) ?? "var(--text-dim)";

// Collapsible sidebar sections.
const collapsedSections = reactive<Record<string, boolean>>({});
const toggleSection = (key: string) => (collapsedSections[key] = !collapsedSections[key]);

// Shared with the recursive BranchTree.
provide("branchActions", {
  checkout: (name: string) => {
    const b = branches.value.find((x) => x.name === name);
    if (b?.is_remote) checkoutRemote(name);
    else checkout(name);
  },
  jump: (target: string | null) => scrollToCommit(target),
  menu: (e: MouseEvent, b: BranchInfo) => branchMenu(e, b),
  colorFor,
});

// Drag the window from empty toolbar areas (hidden title bar → no native bar).
function startDrag(e: MouseEvent) {
  if (e.button !== 0) return;
  const el = e.target as HTMLElement;
  if (el.closest("button, input, a, select, textarea, .pill, [data-no-drag]")) return;
  getCurrentWindow().startDragging();
}

async function chooseRepo() {
  const picked = await openDialog({ directory: true, multiple: false, title: "Open a repository" });
  if (typeof picked !== "string") return;
  // Offer to initialize a repo if the folder isn't one yet.
  const already = await isRepo(picked).catch(() => true);
  if (!already) {
    const folder = picked.split("/").pop() || picked;
    const branch = await promptText({
      title: "Initialize repository",
      label: `"${folder}" isn't a Git repository yet.\nName the first branch:`,
      value: "main",
      confirmLabel: "Initialize",
    });
    if (branch === null) return; // cancelled
    try {
      await initRepo(picked, branch.trim() || "main");
      toast("Repository initialized", `${folder} · ${branch.trim() || "main"}`);
    } catch (e) {
      toast("Couldn't initialize", String(e), "error");
      return;
    }
  }
  await loadRepo(picked);
}

async function loadRepo(path: string) {
  loading.value = true;
  error.value = null;
  try {
    repo.value = await openRepo(path);
    const [c, b, s] = await Promise.all([
      listCommits(repo.value.path, 500),
      listBranches(repo.value.path),
      workingStatus(repo.value.path),
    ]);
    commits.value = c;
    branches.value = b;
    status.value = s;
    selected.value = null; // nothing highlighted until the user clicks
    const r = repo.value;
    if (!tabs.value.some((t) => t.path === r.path)) tabs.value.push({ path: r.path, name: r.name });
    activePath.value = r.path;
    pushRecent({ path: r.path, name: r.name, branch: r.head_branch ?? "", at: Date.now() });
    loadPrCount(r.path);
    loadExtras(r.path);
    watchRepo(r.path).catch(() => {}); // auto-refresh on external changes
  } catch (e) {
    error.value = String(e);
    repo.value = null;
    toast("Couldn't open repository", String(e), "error");
  } finally {
    loading.value = false;
  }
}

/** Re-read commits/branches/status for the already-open repo (e.g. after a commit). */
async function refresh() {
  if (!repo.value) return;
  const path = repo.value.path;
  repo.value = await openRepo(path);
  const [c, b, s] = await Promise.all([
    listCommits(path, 500),
    listBranches(path),
    workingStatus(path),
  ]);
  commits.value = c;
  branches.value = b;
  status.value = s;
  loadExtras(path);
  // Keep the selection only if that commit still exists; never auto-select.
  if (selected.value && !commits.value.some((x) => x.id === selected.value)) {
    selected.value = null;
  }
}

/** Click a commit to open its detail panel; click it again to close. */
function toggleSelect(id: string) {
  selected.value = selected.value === id ? null : id;
}

/** Refs that aren't the HEAD arrow, for pills next to a commit summary. */
function pillRefs(c: CommitRow): string[] {
  return c.refs.filter((r) => !r.startsWith("HEAD →"));
}

async function checkout(name: string) {
  if (!repo.value || syncing.value) return;
  syncing.value = true;
  syncLabel.value = "Switching branch";
  error.value = null;
  try {
    await checkoutBranch(repo.value.path, name);
    await refresh();
    toast("Checked out", name);
  } catch (e) {
    error.value = String(e);
    toast("Checkout failed", String(e), "error");
  } finally {
    syncing.value = false;
  }
}

// Check out a remote branch by creating/switching to a local tracking branch.
async function checkoutRemote(remoteBranch: string) {
  if (!repo.value || syncing.value) return;
  syncing.value = true;
  syncLabel.value = "Switching branch";
  error.value = null;
  try {
    const msg = await checkoutRemoteBranch(repo.value.path, remoteBranch);
    await refresh();
    toast("Checked out", msg);
  } catch (e) {
    error.value = String(e);
    toast("Checkout failed", String(e), "error");
  } finally {
    syncing.value = false;
  }
}

/* ── Remote sync ──────────────────────────────────────────────────── */
async function sync(fn: (path: string) => Promise<string>, label: string, gerund: string) {
  if (!repo.value || syncing.value) return;
  syncing.value = true;
  syncLabel.value = gerund;
  try {
    const msg = await fn(repo.value.path);
    await refresh();
    toast(label, msg);
  } catch (e) {
    toast(`${label} failed`, String(e), "error");
  } finally {
    syncing.value = false;
  }
}
const doFetch = () => sync(gitFetch, "Fetch", "Fetching");
const doPull = () => sync(gitPull, "Pull", "Pulling");
async function doPush() {
  if (!repo.value) return;
  const remotes = await listRemotes(repo.value.path).catch(() => []);
  if (!remotes.length) {
    publishOpen.value = true;
    return;
  }
  sync(gitPush, "Push", "Pushing");
}
function onPublished() {
  sync(gitPush, "Push", "Pushing");
}

// Right-click Push for options beyond a plain push.
function pushMenu(e: MouseEvent) {
  e.preventDefault();
  if (!repo.value) return;
  openContextMenu(e, [
    { label: "Push", action: () => doPush() },
    { label: "Push & set upstream", action: () => sync((p) => pushAdvanced(p, { setUpstream: true }), "Push", "Pushing") },
    { label: "Push tags", action: () => sync((p) => pushAdvanced(p, { pushTags: true }), "Push tags", "Pushing") },
    { separator: true, label: "" },
    {
      label: "Force push (with lease)…",
      danger: true,
      action: () => {
        if (window.confirm("Force-push with lease? This overwrites the remote branch if no one else has pushed."))
          sync((p) => pushAdvanced(p, { forceWithLease: true }), "Force push", "Pushing");
      },
    },
  ]);
}

// Right-click Pull to choose the integration mode.
function pullMenu(e: MouseEvent) {
  e.preventDefault();
  if (!repo.value) return;
  openContextMenu(e, [
    { label: "Pull (merge)", action: () => sync((p) => pullMode(p, "merge"), "Pull", "Pulling") },
    { label: "Pull (rebase)", action: () => sync((p) => pullMode(p, "rebase"), "Pull", "Pulling") },
    { label: "Pull (fast-forward only)", action: () => sync((p) => pullMode(p, "ff-only"), "Pull", "Pulling") },
  ]);
}

// Native menu clicks/accelerators arrive here as "menu-action" events. The menu
// owns the shortcuts (⌘K, ⌘R, ⌘P, …) so we no longer bind them in JS.
function handleMenuAction(id: string) {
  const hasRepo = !!repo.value;
  switch (id) {
    case "settings": return openSettings();
    case "new_tab": return goHome();
    case "close_tab": return activePath.value ? closeTab(activePath.value) : undefined;
    case "open_repo":
    case "init_repo": return void chooseRepo();
    case "clone_repo": return void (cloneOpen.value = true);
    case "reveal": return hasRepo ? void revealItemInDir(repo.value!.path) : undefined;
    case "terminal": return hasRepo ? void openInTerminal(repo.value!.path).catch((e) => toast("Terminal", String(e), "error")) : undefined;
    case "command_palette": return void (paletteOpen.value = !paletteOpen.value);
    case "view_changes": return hasRepo ? void (view.value = "changes") : undefined;
    case "view_history": return hasRepo ? void (view.value = "history") : undefined;
    case "view_prs": return hasRepo ? void (view.value = "prs") : undefined;
    case "toggle_theme": return toggleTheme();
    case "fetch": return hasRepo ? doFetch() : undefined;
    case "pull": return hasRepo ? doPull() : undefined;
    case "push": return hasRepo ? void doPush() : undefined;
    case "new_branch": return hasRepo ? void newBranchPrompt() : undefined;
    case "stash": return hasRepo ? void doStash() : undefined;
    case "remotes": return hasRepo ? void (remotesOpen.value = true) : undefined;
    case "github": return void openUrl("https://github.com").catch(() => {});
  }
}

// Everything reachable from ⌘K: views, sync actions, branches, and commits.
const paletteItems = computed<PaletteItem[]>(() => {
  const items: PaletteItem[] = [];
  if (!repo.value) {
    items.push({ id: "open", label: "Open a repository…", group: "Action", action: chooseRepo });
    items.push({ id: "settings", label: "Settings", group: "Action", action: () => openSettings() });
    return items;
  }
  items.push(
    { id: "v-changes", label: "Go to Changes", group: "View", action: () => (view.value = "changes") },
    { id: "v-history", label: "Go to History", group: "View", action: () => (view.value = "history") },
    { id: "v-prs", label: "Go to Pull requests", group: "View", action: () => (view.value = "prs") },
    { id: "a-fetch", label: "Fetch", hint: "⌘R", group: "Action", action: doFetch },
    { id: "a-pull", label: "Pull", hint: "⇧⌘P", group: "Action", action: doPull },
    { id: "a-push", label: "Push", hint: "⌘P", group: "Action", action: doPush },
    { id: "a-open", label: "Open a repository…", group: "Action", action: chooseRepo },
    { id: "a-remotes", label: "Manage remotes…", group: "Action", action: () => (remotesOpen.value = true) },
    { id: "a-newbranch", label: "New branch…", group: "Action", action: newBranchPrompt },
    { id: "a-settings", label: "Settings", group: "Action", action: () => openSettings() },
    { id: "a-accounts", label: "Accounts", group: "Action", action: () => openSettings("accounts") },
    { id: "a-theme", label: "Toggle theme", group: "Action", action: toggleTheme },
    { id: "a-stash", label: "Stash all changes", group: "Action", action: doStash },
  );
  if (undoOp.value)
    items.push({ id: "a-undo", label: `Undo commit "${undoOp.value.message.split("\n")[0]}"`, group: "Action", action: undo });
  if (redoOp.value)
    items.push({ id: "a-redo", label: "Redo commit", group: "Action", action: redo });

  for (const b of localBranches.value) {
    if (!b.is_head)
      items.push({ id: `br-${b.name}`, label: `Checkout ${b.name}`, hint: "branch", group: "Branch", action: () => checkout(b.name) });
  }
  for (const c of commits.value) {
    items.push({
      id: `c-${c.id}`,
      label: c.summary,
      hint: `${c.short_id} · ${c.author_name}`,
      group: "Commit",
      action: () => scrollToCommit(c.id),
    });
  }
  const base = repo.value.path.replace(/\/$/, "");
  for (const f of files.value) {
    items.push({
      id: `f-${f}`,
      label: f,
      hint: "file",
      group: "File",
      action: () => revealItemInDir(`${base}/${f}`),
    });
  }
  return items;
});
// Auto-refresh: the backend watches the open repo and emits "repo-changed".
let autoTimer: number | undefined;
let unlisten: UnlistenFn | undefined;
function scheduleRefresh() {
  if (!repo.value) return;
  clearTimeout(autoTimer);
  autoTimer = window.setTimeout(() => refresh(), 250);
}
let unlistenMenu: UnlistenFn | undefined;
onMounted(async () => {
  refreshConnections();
  unlisten = await listen("repo-changed", scheduleRefresh);
  unlistenMenu = await listen<string>("menu-action", (e) => handleMenuAction(e.payload));
});
onUnmounted(() => {
  unlisten?.();
  unlistenMenu?.();
});

/* ── Undo / redo (commit-level) ───────────────────────────────────── */
function onCommitted(message: string) {
  undoOp.value = { message };
  redoOp.value = null;
  refresh();
}

async function undo() {
  if (!repo.value || !undoOp.value) return;
  const op = undoOp.value;
  try {
    await uncommit(repo.value.path);
    undoOp.value = null;
    redoOp.value = op;
    await refresh();
    toast("Undo successful", `Commit "${op.message.split("\n")[0]}"`);
  } catch (e) {
    toast("Undo failed", String(e), "error");
  }
}

async function redo() {
  if (!repo.value || !redoOp.value) return;
  const op = redoOp.value;
  try {
    await gitCommit(repo.value.path, op.message, false, false);
    redoOp.value = null;
    undoOp.value = op;
    await refresh();
    toast("Redo successful", `Commit "${op.message.split("\n")[0]}"`);
  } catch (e) {
    toast("Redo failed", String(e), "error");
  }
}

/* ── Clipboard ────────────────────────────────────────────────────── */
async function copy(text: string, what: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast(`${what} copied`);
  } catch {
    toast(`Couldn't copy ${what.toLowerCase()}`, undefined, "error");
  }
}

/* ── Context menus ────────────────────────────────────────────────── */
function commitMenu(e: MouseEvent, c: CommitRow) {
  if (!repo.value) return;
  const path = repo.value.path;
  const items: MenuItem[] = [
    { label: "Check out this commit", action: () => runOp(() => checkoutCommit(path, c.id), "Checked out commit") },
    {
      label: "Create branch here…",
      action: async () => {
        const name = await promptText({ title: "New branch", label: `From ${c.short_id}`, placeholder: "feature/…" });
        if (name && name.trim())
          runOp(() => createBranch(path, name.trim(), c.id, true), `Branch "${name.trim()}" created`);
      },
    },
    { separator: true, label: "" },
    {
      label: "Interactive rebase from here…",
      action: () => startInteractiveRebase(c),
    },
    { label: "Cherry-pick onto current", action: () => opRun(() => cherryPick(path, c.id), "Cherry-pick") },
    { label: "Revert this commit", action: () => opRun(() => revertCommit(path, c.id), "Revert") },
    { separator: true, label: "" },
    { label: "Copy SHA", action: () => copy(c.id, "SHA") },
    { label: "Copy message", action: () => copy(c.summary, "Message") },
    { separator: true, label: "" },
    { label: "Soft reset branch to here", action: () => runOp(() => gitReset(path, c.id, "soft"), "Reset (soft)") },
    { label: "Mixed reset branch to here", action: () => runOp(() => gitReset(path, c.id, "mixed"), "Reset (mixed)") },
    {
      label: "Hard reset branch to here…",
      danger: true,
      action: () => {
        if (window.confirm(`Hard reset ${headBranch.value} to ${c.short_id}? This discards working changes.`))
          runOp(() => gitReset(path, c.id, "hard"), "Reset (hard)");
      },
    },
  ];
  openContextMenu(e, items);
}

function branchMenu(e: MouseEvent, b: BranchInfo) {
  if (!repo.value) return;
  const path = repo.value.path;
  const head = headBranch.value;
  const items: MenuItem[] = [
    b.is_remote
      ? { label: `Checkout ${b.name}`, action: () => checkoutRemote(b.name) }
      : { label: "Check out", disabled: b.is_head, action: () => checkout(b.name) },
    { separator: true, label: "" },
    {
      label: `Merge ${b.name} into ${head}`,
      disabled: b.is_head,
      action: () => opRun(() => mergeBranch(path, b.name), "Merge"),
    },
    {
      label: `Rebase ${head} onto ${b.name}`,
      disabled: b.is_head,
      action: () => opRun(() => rebaseBranch(path, b.name), "Rebase"),
    },
    { separator: true, label: "" },
    { label: "Copy name", action: () => copy(b.name, "Branch name") },
    { separator: true, label: "" },
  ];
  if (b.is_remote) {
    // b.name is like "origin/feature" — split remote from branch.
    const slash = b.name.indexOf("/");
    const remote = slash === -1 ? "origin" : b.name.slice(0, slash);
    const branch = slash === -1 ? b.name : b.name.slice(slash + 1);
    items.push({
      label: "Delete remote branch…",
      danger: true,
      action: () => {
        if (window.confirm(`Delete "${branch}" on ${remote}? This removes it for everyone.`))
          runOp(() => deleteRemoteBranch(path, remote, branch), `Deleted ${remote}/${branch}`);
      },
    });
  } else {
    items.push({
      label: "Delete branch…",
      danger: true,
      disabled: b.is_head,
      action: () => {
        if (window.confirm(`Delete branch "${b.name}"?`))
          runOp(() => deleteBranch(path, b.name), `Branch "${b.name}" deleted`);
      },
    });
  }
  openContextMenu(e, items);
}

/* ── Stashes ──────────────────────────────────────────────────────── */
async function doStash() {
  if (!repo.value) return;
  const msg = await promptText({ title: "Stash changes", label: "Message (optional)", confirmLabel: "Stash" });
  if (msg === null) return; // cancelled
  runOp(() => stashSave(repo.value!.path, msg || undefined), "Changes stashed");
}
// Ensure a commit identity exists, prompting for one if not. Returns false if
// the user cancels. Shared by the initial-commit flow.
async function ensureIdentity(path: string): Promise<boolean> {
  const id = await gitIdentity(path).catch(() => null);
  if (id?.name && id?.email) return true;
  const name = await promptText({ title: "Commit identity", label: "Your name", placeholder: "Ada Lovelace" });
  if (name === null || !name.trim()) return false;
  const email = await promptText({ title: "Commit identity", label: "Your email", placeholder: "ada@example.com" });
  if (email === null || !email.trim()) return false;
  await setGitIdentity(path, name.trim(), email.trim(), true);
  toast("Identity set", `${name.trim()} <${email.trim()}>`);
  return true;
}

// Give an empty (unborn) repo a first commit so main is born and history,
// branching and everything else become available.
const creatingInitial = ref(false);
async function createInitialCommit() {
  if (!repo.value || creatingInitial.value) return;
  const path = repo.value.path;
  creatingInitial.value = true;
  try {
    if (!(await ensureIdentity(path))) return;
    const msg = await initialCommit(path, "Initial commit");
    toast("Repository ready", msg);
    await refresh();
    view.value = status.value.length ? "changes" : "history";
  } catch (e) {
    toast("Couldn't create commit", String(e), "error");
  } finally {
    creatingInitial.value = false;
  }
}

// Prompt for a name and branch off the current tip (works even on an unborn
// repo, where the backend just moves HEAD). Shared by the chip and palette.
async function newBranchPrompt() {
  if (!repo.value) return;
  const path = repo.value.path;
  const name = await promptText({ title: "New branch", label: "Branch name", placeholder: "feature/…" });
  if (name && name.trim())
    runOp(() => createBranch(path, name.trim(), "HEAD", true), `Branch "${name.trim()}" created`);
}

// Dropdown on the toolbar branch chip: switch between local branches and
// create a new one — the only branch UI a repo with no commits (or no
// visible Branches section) can reach.
function branchChipMenu(e: MouseEvent) {
  if (!repo.value) return;
  const items: MenuItem[] = [{ label: "New branch…", action: newBranchPrompt }];
  if (localBranches.value.length) {
    items.push({ separator: true, label: "" });
    for (const b of localBranches.value) {
      items.push({
        label: b.name,
        checked: b.is_head,
        disabled: b.is_head,
        action: () => checkout(b.name),
      });
    }
  }
  openContextMenu(e, items);
}

function stashMenu(e: MouseEvent, s: StashEntry) {
  if (!repo.value) return;
  const path = repo.value.path;
  openContextMenu(e, [
    { label: "Apply", action: () => runOp(() => stashApply(path, s.index), "Stash applied") },
    { label: "Pop (apply & drop)", action: () => runOp(() => stashPop(path, s.index), "Stash popped") },
    { separator: true, label: "" },
    {
      label: "Drop",
      danger: true,
      action: () => {
        if (window.confirm(`Drop stash@{${s.index}}?`)) runOp(() => stashDrop(path, s.index), "Stash dropped");
      },
    },
  ]);
}

// Open the interactive-rebase planner for the commits newer than `c`, keeping
// `c` as the base. Uses the linear graph order we already loaded.
function startInteractiveRebase(c: CommitRow) {
  const idx = commits.value.findIndex((x) => x.id === c.id);
  if (idx <= 0) {
    toast("Nothing to rebase", "This is already the newest commit.", "error");
    return;
  }
  rebaseCommits.value = commits.value.slice(0, idx); // newer than c, newest-first
  rebaseBase.value = c.id;
  rebaseOpen.value = true;
}

/* ── Merge / rebase / conflict ops ────────────────────────────────── */
async function opRun(fn: () => Promise<string>, label: string) {
  if (!repo.value) return;
  syncing.value = true;
  syncLabel.value = label;
  try {
    const msg = await fn();
    await refresh();
    toast(label, msg);
    if (state.value.conflicts) {
      toast("Conflicts", "Resolve them to continue", "error");
      conflictOpen.value = true;
    }
  } catch (e) {
    toast(`${label} failed`, String(e), "error");
  } finally {
    syncing.value = false;
  }
}

/** Jump the history to a commit and scroll it into view. */
function scrollToCommit(id: string | null) {
  if (!id) return;
  view.value = "history";
  commitFilter.value = "";
  selected.value = id;
  nextTick(() =>
    document.querySelector(`.commit-row[data-id="${id}"]`)?.scrollIntoView({ block: "center" }),
  );
}

/** Run a git op, refresh, toast on success/failure. */
async function runOp(fn: () => Promise<unknown>, okMsg: string) {
  try {
    await fn();
    await refresh();
    toast(okMsg);
  } catch (e) {
    toast("Operation failed", String(e), "error");
  }
}
</script>

<template>
  <div class="app" :data-theme="appState.theme">
    <!-- ── Tab bar (open repos) ─────────────────────────────────────── -->
    <header class="tabbar" data-tauri-drag-region @mousedown="startDrag">
      <div class="traffic-spacer" data-tauri-drag-region></div>
      <button class="home-tab" :class="{ on: !showWorkspace }" title="Home" @click="goHome">
        <PlumbMark :size="15" />
      </button>
      <button
        v-for="t in tabs"
        :key="t.path"
        class="repo-tab"
        :class="{ on: showWorkspace && activePath === t.path }"
        :title="t.path"
        @click="selectTab(t.path)"
      >
        <span class="tab-name ellipsis">{{ t.name }}</span>
        <span class="tab-x" title="Close" @click.stop="closeTab(t.path)">✕</span>
      </button>
      <button class="add-tab" title="Open a repository" @click="goHome">+</button>
      <div class="spacer" data-tauri-drag-region></div>
      <button class="icon-btn" @click="toggleTheme" :title="`Theme: ${appState.theme}`">
        {{ appState.theme === "dark" ? "◐" : "◑" }}
      </button>
      <button class="icon-btn" @click="openSettings()" title="Settings">⚙</button>
      <button class="pill kbd-pill mono" title="Command palette (⌘K)" @click="paletteOpen = true">⌘K</button>
    </header>

    <!-- ── Repo toolbar (workspace only) ────────────────────────────── -->
    <header v-if="showWorkspace && repo" class="toolbar">
      <button class="pill repo-switcher" title="Open another repository" @click="chooseRepo">
        <PlumbMark :size="16" />
        <span class="repo-name">{{ repo.name }}</span>
        <span class="caret">▾</span>
      </button>

      <button class="pill branch-chip" title="Switch or create branch" @click="branchChipMenu">
          <span class="dot" :style="{ background: 'var(--accent)' }"></span>
          <span class="mono">{{ headBranch }}</span>
          <span class="caret">▾</span>
        </button>

        <div class="divergence mono" v-if="headInfo">
          <span>↑ {{ headInfo.ahead }}</span><span>↓ {{ headInfo.behind }}</span>
        </div>

        <div class="vsep"></div>

        <div class="undo-redo">
          <button class="icon-btn" :disabled="!undoOp" @click="undo" title="Undo last commit">↺</button>
          <button class="icon-btn" :disabled="!redoOp" @click="redo" title="Redo">↻</button>
        </div>

        <div class="vsep"></div>

        <div class="sync-actions">
          <button class="btn" :disabled="syncing" @click="doFetch">Fetch <kbd>⌘R</kbd></button>
          <button class="btn" :disabled="syncing" @click="doPull" @contextmenu="pullMenu" title="Pull · right-click for rebase / ff-only">Pull <kbd>⇧⌘P</kbd></button>
          <button class="btn btn-accent" :disabled="syncing" @click="doPush" @contextmenu="pushMenu" title="Push · right-click for force / tags / upstream">
            Push<span v-if="headInfo && headInfo.ahead"> {{ headInfo.ahead }}</span>
            <kbd>⌘P</kbd>
          </button>
        </div>

        <div v-if="syncing" class="sync-status">
          <span class="spinner"></span><span class="sync-label">{{ syncLabel }}…</span>
        </div>

      <div class="spacer" data-tauri-drag-region></div>

      <div class="search">
        <span class="glyph">⌕</span>
        <input
          v-model="commitFilter"
          class="search-input"
          placeholder="Search commits"
          spellcheck="false"
          @focus="view = 'history'"
        />
        <span v-if="commitFilter" class="clear" title="Clear" @click="commitFilter = ''">✕</span>
      </div>
    </header>

    <!-- Indeterminate activity bar — overlays the header's bottom edge, so it
         signals work without shifting any layout. -->
    <div v-if="syncing" class="progress-bar" aria-hidden="true"></div>

    <!-- Merge/rebase in-progress banner -->
    <div v-if="showWorkspace && state.state !== 'clean'" class="op-banner">
      <span class="op-text">
        <strong>{{ opLabel }}</strong>
        {{ state.conflicts ? "· resolve the conflicts, then Continue" : "in progress" }}
      </span>
      <span class="grow"></span>
      <button v-if="state.conflicts" class="op-btn" @click="conflictOpen = true">Resolve conflicts</button>
      <button class="op-btn" @click="opRun(() => opContinue(repo!.path), 'Continue')">Continue</button>
      <button class="op-btn danger" @click="opRun(() => opAbort(repo!.path), 'Abort')">Abort</button>
    </div>

    <!-- ── Full-screen diff (keeps the header above, like GitKraken) ── -->
    <DiffFullscreen v-if="showWorkspace && fullscreen.open" />

    <!-- ── Home ────────────────────────────────────────────────────── -->
    <HomePage
      v-else-if="!showWorkspace"
      :recents="recents"
      @open="chooseRepo"
      @clone="cloneOpen = true"
      @connect="openSettings('accounts')"
      @select="loadRepo"
      @forget="forgetRecent"
    />

    <!-- ── Workspace ───────────────────────────────────────────────── -->
    <div v-else-if="repo" class="workspace">
      <!-- Sidebar -->
      <aside class="sidebar" :style="{ width: sidebarWidth + 'px' }">
        <div class="repo-head">
          <div class="repo-title">{{ repo.name }}</div>
          <div class="repo-path mono">{{ repo.path }}</div>
        </div>

        <nav class="side-section">
          <div class="sect-head" @click="toggleSection('workspace')">
            <span class="sect-chev">{{ collapsedSections.workspace ? "▸" : "▾" }}</span>
            <span class="section-label">Workspace</span>
          </div>
          <template v-if="!collapsedSections.workspace">
            <div class="side-row clickable" :class="{ active: view === 'changes' }" @click="view = 'changes'">
              <span class="ico">◧</span>Changes
              <span class="count mono">{{ status.length }}</span>
            </div>
            <div class="side-row clickable" :class="{ active: view === 'history' }" @click="view = 'history'">
              <span class="ico">≡</span>History
            </div>
            <div class="side-row clickable" :class="{ active: view === 'prs' }" @click="view = 'prs'">
              <span class="ico">⇄</span>Pull requests
              <span v-if="prCount !== null" class="count mono">{{ prCount }}</span>
            </div>
          </template>
        </nav>

        <nav class="side-section" v-if="localBranches.length">
          <div class="sect-head" @click="toggleSection('branches')">
            <span class="sect-chev">{{ collapsedSections.branches ? "▸" : "▾" }}</span>
            <span class="section-label">Branches</span>
          </div>
          <BranchTree v-if="!collapsedSections.branches" :nodes="localTree" />
        </nav>

        <nav class="side-section" v-if="remoteTree.length || remotes.length">
          <div class="sect-head" @click="toggleSection('remotes')">
            <span class="sect-chev">{{ collapsedSections.remotes ? "▸" : "▾" }}</span>
            <span class="section-label">Remotes</span>
            <span class="plus" title="Manage remotes" @click.stop="remotesOpen = true">⚙</span>
          </div>
          <template v-if="!collapsedSections.remotes">
            <BranchTree v-if="remoteTree.length" :nodes="remoteTree" />
            <!-- Configured remotes with no fetched branches yet (e.g. an empty
                 origin) — still show the connection so it's visible. -->
            <template v-else>
              <div v-for="r in remotes" :key="r.name" class="side-row remote-row" :title="r.url">
                <span class="ico">⛁</span>{{ r.name }}
                <span class="remote-host mono">{{ shortHost(r.url) }}</span>
              </div>
            </template>
          </template>
        </nav>

        <nav class="side-section">
          <div class="sect-head" @click="toggleSection('stashes')">
            <span class="sect-chev">{{ collapsedSections.stashes ? "▸" : "▾" }}</span>
            <span class="section-label">Stashes</span>
            <span class="plus" title="Stash all changes" @click.stop="doStash">+</span>
          </div>
          <template v-if="!collapsedSections.stashes">
            <div
              v-for="s in stashes"
              :key="s.index"
              class="side-row mono muted clickable"
              :title="s.message"
              @click="stashMenu($event, s)"
              @contextmenu="stashMenu($event, s)"
            >
              <span class="ellipsis">stash@{{ s.index }}: {{ s.message.replace(/^WIP on /, "") }}</span>
            </div>
            <div v-if="!stashes.length" class="conn-empty">No stashes.</div>
          </template>
        </nav>

        <nav class="side-section" v-if="tags.length">
          <div class="sect-head" @click="toggleSection('tags')">
            <span class="sect-chev">{{ collapsedSections.tags ? "▸" : "▾" }}</span>
            <span class="section-label">Tags</span>
          </div>
          <template v-if="!collapsedSections.tags">
            <div
              v-for="t in tags.slice(0, 12)"
              :key="t.name"
              class="side-row mono muted clickable"
              @click="scrollToCommit(t.target)"
            >
              <span class="ellipsis">⌾ {{ t.name }}</span>
            </div>
          </template>
        </nav>

      </aside>

      <ResizeHandle v-model="sidebarWidth" side="left" :min="190" :max="460" />

      <!-- Changes -->
      <ChangesView
        v-if="view === 'changes'"
        :repo-path="repo.path"
        @committed="onCommitted"
      />

      <!-- Pull / merge requests -->
      <PullRequests v-else-if="view === 'prs'" :repo-path="repo.path" />

      <!-- History -->
      <section v-else class="history">
        <!-- Empty repo: no commits yet — offer to start history. -->
        <div v-if="!commits.length" class="repo-empty">
          <div class="re-mark">↓</div>
          <div class="re-title">No commits yet</div>
          <div class="re-sub">
            Connect an existing remote to build on one of its branches, or start fresh history on
            <span class="mono">{{ headBranch }}</span> with an empty commit. Either way you stage and
            commit your files normally afterwards.
          </div>
          <div class="re-actions">
            <button class="btn-accent" @click="connectRemoteOpen = true">Connect a remote…</button>
            <button class="btn" :disabled="creatingInitial" @click="createInitialCommit">
              {{ creatingInitial ? "Creating…" : "Start with an empty commit" }}
            </button>
          </div>
        </div>

        <div v-else class="hist-list">
          <div class="hist-head mono">
            <span class="col-graph">GRAPH</span>
            <span>COMMIT</span>
            <span>AUTHOR</span>
            <span>HASH</span>
            <span class="col-when">WHEN</span>
          </div>

          <div class="hist-body">
            <div v-if="!commitFilter" class="graph-col"><CommitGraph :commits="commits" /></div>

            <div class="rows" :class="{ filtered: commitFilter }">
            <div
              v-for="c in visibleCommits"
              :key="c.id"
              class="commit-row"
              :data-id="c.id"
              :class="{ selected: c.id === selected }"
              @click="toggleSelect(c.id)"
              @contextmenu="commitMenu($event, c)"
            >
              <span class="cell-graph"></span>
              <div class="cell-commit">
                <span
                  v-for="r in pillRefs(c)"
                  :key="r"
                  class="ref-pill mono"
                  :class="{ tag: r.startsWith('tag:') }"
                  >{{ r.replace("tag: ", "⌾ ") }}</span
                >
                <span class="summary" :class="{ merge: c.is_merge }">{{ c.summary }}</span>
              </div>
              <div class="cell-author">
                <span class="avatar mono">{{ initials(c.author_name) }}</span>
                <span class="ellipsis author-name">{{ c.author_name }}</span>
              </div>
              <span class="cell-hash mono">{{ c.short_id }}</span>
              <span class="cell-when">{{ relativeTime(c.time) }}</span>
            </div>
            </div>
          </div>
        </div>

        <!-- Commit detail — right dock, only when a commit is selected -->
        <template v-if="selected">
          <ResizeHandle v-model="detailWidth" side="right" :min="280" :max="640" />
          <div class="commit-detail-dock" :style="{ width: detailWidth + 'px' }">
            <CommitDetail :repo-path="repo.path" :commit-id="selected" @close="selected = null" />
          </div>
        </template>
      </section>
    </div>

    <!-- App-level overlays -->
    <ContextMenu />
    <FileInspector />
    <Toasts />
    <Settings />
    <CommandPalette :open="paletteOpen" :items="paletteItems" @close="paletteOpen = false" />
    <CloneDialog v-model="cloneOpen" @cloned="loadRepo" />
    <InputDialog />
    <PublishDialog
      v-if="repo"
      v-model="publishOpen"
      :repo-path="repo.path"
      :repo-name="repo.name"
      @published="onPublished"
    />
    <RemotesDialog v-if="repo" v-model="remotesOpen" :repo-path="repo.path" />
    <ConflictDialog v-if="repo" v-model="conflictOpen" :repo-path="repo.path" @resolved="refresh" />
    <RebaseDialog
      v-if="repo"
      v-model="rebaseOpen"
      :repo-path="repo.path"
      :base="rebaseBase"
      :commits="rebaseCommits"
      @done="refresh"
    />
    <ConnectRemoteDialog v-if="repo" v-model="connectRemoteOpen" :repo-path="repo.path" @connected="refresh" />
  </div>
</template>

<style scoped>
.app {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg);
  color: var(--text);
  position: relative;
}

/* Indeterminate activity bar — absolutely placed over the header's bottom edge
 * so showing/hiding it never reflows anything. */
.progress-bar {
  position: absolute;
  top: calc(40px + var(--toolbar-h) - 2px);
  left: 0;
  right: 0;
  height: 2px;
  overflow: hidden;
  z-index: 50;
  background: color-mix(in srgb, var(--accent) 20%, transparent);
}
.progress-bar::after {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  left: -35%;
  width: 35%;
  background: var(--accent);
  animation: plumb-slide 1.05s ease-in-out infinite;
}
@keyframes plumb-slide {
  0% { left: -35%; }
  100% { left: 100%; }
}

.spinner {
  width: 12px;
  height: 12px;
  flex: none;
  border: 2px solid color-mix(in srgb, var(--text-faint) 45%, transparent);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: plumb-spin 0.7s linear infinite;
}
@keyframes plumb-spin {
  to { transform: rotate(360deg); }
}
.sync-status { display: flex; align-items: center; gap: var(--space-2); }
.sync-label { font-size: 12px; color: var(--text-mid); }

/* Interactive affordances: real controls get a pointer + hover, so the app
 * reads as clickable rather than a static mockup. */
.app button:not(:disabled),
.pill,
.commit-row,
.side-row.clickable,
.side-row.active,
.side-row:has(.count),
.conn-head .plus {
  cursor: pointer;
}
.pill:hover,
.btn:not(:disabled):hover,
.icon-btn:hover { background: color-mix(in srgb, var(--accent) 14%, var(--raised)); }
.side-row.clickable:hover:not(.head) { background: color-mix(in srgb, var(--raised) 60%, transparent); }
.conn-head .plus { cursor: pointer; }

/* ── Toolbar ─────────────────────────────────────────────────────── */
/* Tab bar (row 1) — holds the traffic lights + open-repo tabs. */
.tabbar {
  height: 40px;
  min-height: 40px;
  flex: none;
  display: flex;
  align-items: stretch;
  gap: 0;
  padding: 0 var(--space-3) 0 0;
  background: var(--bg);
  border-bottom: 1px solid var(--line);
}
.tabbar .spacer { flex: 1; }
.home-tab, .repo-tab, .add-tab {
  display: flex; align-items: center; background: transparent;
  border: none; border-right: 1px solid var(--line); cursor: pointer;
  color: var(--text-mid); font-size: 12.5px;
}
.home-tab { padding: 0 14px; }
.repo-tab { padding: 0 12px; gap: var(--space-2); max-width: 220px; }
.home-tab.on, .repo-tab.on { color: var(--text); background: var(--surface); box-shadow: inset 0 -2px 0 var(--accent); }
.repo-tab.on { font-weight: 600; }
.repo-tab .tab-name { max-width: 160px; }
.repo-tab .tab-x { opacity: 0; width: 14px; font-size: 10px; color: var(--text-faint); }
.repo-tab:hover .tab-x { opacity: 1; }
.repo-tab:hover, .home-tab:hover, .add-tab:hover { color: var(--text); }
.add-tab { border-right: none; width: 34px; justify-content: center; font-size: 17px; color: var(--text-dim); }

.toolbar {
  height: var(--toolbar-h);
  min-height: var(--toolbar-h);
  max-height: var(--toolbar-h);
  flex: none;
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: var(--space-3);
  padding: 0 var(--space-4);
  background: var(--surface);
  border-bottom: 2px solid var(--line);
  overflow: hidden;
}
.traffic-spacer { width: var(--traffic-inset); flex: none; }
.spacer { flex: 1; }
.vsep { width: 1px; height: 22px; background: var(--line); }

/* Merge/rebase banner */
.op-banner {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-4);
  height: 34px;
  background: color-mix(in srgb, var(--accent) 18%, var(--surface));
  border-bottom: 2px solid var(--accent);
  font-size: 12.5px;
}
.op-banner .grow { flex: 1; }
.op-btn { height: 24px; padding: 0 12px; background: var(--raised); border: 1px solid var(--line); font-size: 11.5px; font-weight: 600; cursor: pointer; }
.op-btn.danger { color: var(--accent); border-color: var(--accent); }

.pill {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 30px;
  padding: 0 var(--space-3);
  background: var(--raised);
  border: 1px solid var(--line);
  font-size: 13px;
  font-weight: 600;
}
.repo-switcher { min-width: 190px; }
.repo-name { flex: 1; text-align: left; }
.caret { color: var(--text-dim); font-size: 10px; }
.branch-chip { font-weight: 500; }
.branch-chip .mono { font-size: 12px; }
.dot { width: 8px; height: 8px; flex: none; }

.divergence { display: flex; gap: var(--space-2); font-size: 11.5px; color: var(--text-mid); }

.undo-redo { display: flex; gap: 2px; }
.undo-redo .icon-btn { font-size: 15px; }
.icon-btn:disabled { opacity: 0.35; }

.sync-actions { display: flex; gap: 2px; }
.btn {
  height: 30px;
  padding: 0 14px;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  background: var(--raised);
  border: 1px solid var(--line);
  font-size: 12.5px;
  font-weight: 500;
}
.btn:disabled { opacity: 0.5; }
.btn-accent {
  background: var(--accent);
  color: var(--accent-on);
  border-color: var(--accent);
  font-weight: 700;
}
kbd {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-faint);
}
.btn-accent kbd { color: var(--accent-on); opacity: 0.7; }

.search {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 30px;
  width: 250px;
  padding: 0 var(--space-3);
  background: var(--bg);
  border: 1px solid var(--line);
}
.search .glyph { color: var(--text-faint); font-size: 12.5px; flex: none; }
.search-input { flex: 1; min-width: 0; background: transparent; border: none; color: var(--text); font-size: 12.5px; font-family: var(--font-ui); }
.search-input:focus { outline: none; }
.search-input::placeholder { color: var(--text-faint); }
.search .clear { flex: none; color: var(--text-faint); font-size: 11px; cursor: pointer; }
.icon-btn {
  height: 30px;
  width: 30px;
  background: transparent;
  border: 1px solid var(--line);
  font-size: 14px;
  color: var(--text-mid);
}
.kbd-pill { height: 30px; padding: 0 10px; font-size: 11.5px; font-weight: 700; color: var(--text-mid); }

.brand-idle { display: flex; align-items: center; gap: var(--space-3); }
.wordmark { font-weight: 800; font-size: 18px; letter-spacing: 0.02em; }

/* ── Onboarding ──────────────────────────────────────────────────── */
.onboarding {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
.onboard-card { text-align: center; display: flex; flex-direction: column; align-items: center; gap: var(--space-3); color: var(--text); }
.wordmark-lg { font-weight: 800; font-size: 64px; letter-spacing: -0.03em; margin: var(--space-4) 0 0; }
.tagline { font-size: 18px; font-weight: 500; color: var(--text-mid); margin: 0 0 var(--space-6); }
.onboard-actions { display: flex; gap: var(--space-2); }
.btn.lg { height: 40px; padding: 0 20px; font-size: 13px; }
.err { color: var(--accent); font-family: var(--font-mono); font-size: 12px; max-width: 60ch; }
.trust { margin-top: var(--space-6); font-size: 11.5px; color: var(--text-faint); }

/* ── Workspace ───────────────────────────────────────────────────── */
.workspace { flex: 1; display: flex; min-height: 0; }

.sidebar {
  width: 250px;
  flex: none;
  background: var(--surface);
  border-right: 2px solid var(--line);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}
.repo-head { padding: 14px var(--space-3); border-bottom: 1px solid var(--raised); }
.repo-title { font-size: 13px; font-weight: 700; }
.repo-path { font-size: 10.5px; color: var(--text-faint); margin-top: 3px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.side-section { padding: var(--space-4) 0 0; }
.side-section .section-label { padding: 0 var(--space-3) var(--space-2); }
/* Collapsible section header — provides the single left inset. */
.sect-head { display: flex; align-items: center; padding: 0 var(--space-3) var(--space-2); cursor: pointer; }
.sect-head .section-label { padding: 0; }
.sect-head:hover .section-label { color: var(--text-mid); }
.sect-chev { width: 12px; flex: none; color: var(--text-faint); font-size: 9px; }
.sect-head .plus { margin-left: auto; color: var(--accent); font-size: 16px; line-height: 1; }
.side-row {
  height: var(--row-sidebar);
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  font-size: 12.5px;
  color: var(--text-mid);
}
.side-row.mono { font-size: 12px; }
.side-row .ico { width: 12px; color: var(--text-dim); }
.side-row .count { margin-left: auto; font-size: 10.5px; background: var(--line); padding: 1px 5px; }
.side-row.active { font-weight: 600; background: var(--raised); box-shadow: inset 2px 0 0 var(--accent); color: var(--text); }
.side-row.branch.head { color: var(--text); font-weight: 500; }
.remote-row .remote-host { margin-left: auto; font-size: 10.5px; color: var(--text-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 55%; }
.side-row .ellipsis { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.head-tag { margin-left: auto; font-size: 9.5px; font-weight: 700; background: var(--accent); color: var(--accent-on); padding: 1px 4px; }
.side-row.muted { color: var(--text-dim); }

.connections { margin-top: auto; border-top: 2px solid var(--line); background: var(--subtle); }
.conn-head { display: flex; align-items: center; padding: 0 var(--space-3) var(--space-2); }
.conn-head .plus { margin-left: auto; color: var(--accent); font-size: 16px; line-height: 1; cursor: pointer; }
.conn-empty { padding: 0 var(--space-3) var(--space-4); font-size: 11.5px; color: var(--text-faint); line-height: 1.45; }
.conn-row { gap: var(--space-2); }
.conn-badge { font-size: 9px; font-weight: 700; border: 1px solid var(--text-dim); padding: 1px 3px; color: var(--text-mid); flex: none; }

/* ── History ─────────────────────────────────────────────────────── */
.history { flex: 1; display: flex; min-width: 0; }
.repo-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: var(--space-6); gap: var(--space-3); }
.re-mark { font-size: 34px; line-height: 1; color: var(--accent); font-weight: 800; }
.re-title { font-size: 20px; font-weight: 800; }
.re-sub { max-width: 440px; font-size: 13px; color: var(--text-mid); line-height: 1.6; }
.re-actions { display: flex; gap: var(--space-2); margin-top: var(--space-2); }
.re-actions .btn-accent { height: 36px; padding: 0 20px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.re-actions .btn-accent:disabled { opacity: 0.6; }
.re-actions .btn { height: 36px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.hist-list { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.hist-head {
  height: 28px;
  flex: none;
  display: grid;
  grid-template-columns: 130px 1fr 160px 92px 84px;
  align-items: center;
  background: var(--subtle);
  border-bottom: 1px solid var(--line);
  font-size: 10px;
  letter-spacing: 0.1em;
  color: var(--text-faint);
  padding-right: var(--space-4);
}
.hist-head .col-graph { padding-left: var(--space-3); }
.hist-head .col-when { text-align: right; }

.hist-body { position: relative; flex: 1; overflow-y: auto; }
.graph-col { position: absolute; left: 12px; top: 0; pointer-events: none; z-index: 1; }

.rows { position: relative; z-index: 0; }
.commit-row {
  display: grid;
  grid-template-columns: 130px 1fr 160px 92px 84px;
  height: var(--row-commit);
  align-items: center;
  border-bottom: 1px solid var(--line-soft);
  padding-right: var(--space-4);
}
.commit-row:hover { background: color-mix(in srgb, var(--raised) 55%, transparent); }
.commit-row.selected { background: var(--raised); box-shadow: inset 2px 0 0 var(--accent); }

.cell-commit { display: flex; align-items: center; gap: var(--space-2); min-width: 0; overflow: hidden; }
.summary { font-size: 12.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.summary.merge { font-weight: 600; }
.ref-pill {
  font-size: 9.5px; font-weight: 700; padding: 1px 4px; flex: none;
  border: 1px solid var(--accent); color: var(--accent);
  max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.ref-pill.tag { border-color: var(--text-dim); color: var(--text-mid); }
.cell-author { overflow: hidden; }
.cell-hash { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.cell-author { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
.avatar {
  width: 18px; height: 18px; flex: none;
  display: flex; align-items: center; justify-content: center;
  background: var(--raised);
  font-size: 9px; font-weight: 700; color: var(--text-mid);
}
.author-name { font-size: 12px; color: var(--text-mid); }
.cell-hash { font-size: 11.5px; color: var(--text-dim); }
.cell-when { font-size: 11.5px; color: var(--text-faint); text-align: right; }

.commit-detail-dock {
  width: 360px;
  flex: none;
  display: flex;
  border-left: 2px solid var(--line);
  min-height: 0;
}
</style>
