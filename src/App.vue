<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, provide, reactive, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { revealItemInDir, openUrl, openFolder } from "./lib/native";
import {
  openRepo,
  isRepo,
  initRepo,
  openInTerminal,
  openInEditor,
  rewordCommit,
  initialCommit,
  gitIdentity,
  setGitIdentity,
  listRemotes,
  listCommits,
  listBranches,
  workingStatus,
  searchCommits as searchAllCommits,
  type RepoInfo,
  type RemoteInfo,
  type ReflogEntry,
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
  bisectStatus,
  bisectMark,
  bisectReset,
  type BisectStatus,
  pushAdvanced,
  pullMode,
  deleteRemoteBranch,
  listStashes,
  stashApply,
  stashPop,
  stashDrop,
  listTags,
  listFiles,
  mergeBranchEx,
  rebaseBranchEx,
  cherryPick,
  revertCommit,
  opAbort,
  opContinue,
  repoState,
  watchRepo,
  flowConfig,
  flowStart,
  flowFinish,
  getConfig,
  type FlowConfig,
  type StashEntry,
  type TagInfo,
  type RepoState,
} from "./lib/git";
import { listen, type UnlistenFn } from "./lib/transport";
import { invoke, isServed, servedRepo } from "./lib/transport";
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
  prefs,
} from "./lib/ui";
import { listPullRequests, listCiStatuses } from "./lib/accounts";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import CommitGraph from "./components/CommitGraph.vue";
import ChangesView from "./components/ChangesView.vue";
import PullRequests from "./components/PullRequests.vue";
import PipelinesView from "./components/PipelinesView.vue";
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
import CreatePrDialog from "./components/CreatePrDialog.vue";
import ReflogDialog from "./components/ReflogDialog.vue";
import CompareDialog from "./components/CompareDialog.vue";
import SubmodulesDialog from "./components/SubmodulesDialog.vue";
import WorktreesDialog from "./components/WorktreesDialog.vue";
import BisectDialog from "./components/BisectDialog.vue";
import RunPipelineDialog from "./components/RunPipelineDialog.vue";
import PipelineDialog from "./components/PipelineDialog.vue";
import RepoSettingsDialog from "./components/RepoSettingsDialog.vue";
import RepoInfoDialog from "./components/RepoInfoDialog.vue";
import IntegrateDialog from "./components/IntegrateDialog.vue";
import StashSaveDialog from "./components/StashSaveDialog.vue";
import StashApplyDialog from "./components/StashApplyDialog.vue";
import WorkflowDialog from "./components/WorkflowDialog.vue";
import InputDialog from "./components/InputDialog.vue";
import HomePage from "./components/HomePage.vue";
import BranchTree from "./components/BranchTree.vue";
import { buildBranchTree } from "./lib/branchtree";
import type { PaletteItem } from "./lib/palette";
import { loadRecents, saveRecents, type RecentRepo } from "./lib/recents";

const repo = ref<RepoInfo | null>(null);
const commits = ref<CommitRow[]>([]);
// History pages in from newest; more loads as you scroll toward the bottom, so
// history reaches the first commit without rendering every row up front.
const COMMIT_PAGE = 500;
// First page kept small so a fresh (or switched-to) repo paints fast even when
// it has hundreds of thousands of commits; the rest streams in behind it.
const FIRST_PAGE = 120;
const allCommitsLoaded = ref(false);
const loadingMore = ref(false);
async function loadMoreCommits() {
  if (!repo.value || loadingMore.value || allCommitsLoaded.value || commitFilter.value) return;
  loadingMore.value = true;
  try {
    const next = await listCommits(repo.value.path, COMMIT_PAGE, commits.value.length);
    if (next.length < COMMIT_PAGE) allCommitsLoaded.value = true;
    if (next.length) {
      const seen = new Set(commits.value.map((c) => c.id));
      commits.value = commits.value.concat(next.filter((c) => !seen.has(c.id)));
    }
  } catch {
    /* leave what we have */
  } finally {
    loadingMore.value = false;
  }
}
// Row virtualization: only the rows in (and near) the viewport are in the DOM,
// so huge histories stay light. Rows are a fixed height; a spacer holds the
// full scroll height and the window is translated into place. ROW_H must match
// --row-commit and the graph's ROW_H.
const HIST_ROW_H = 34;
const histScrollTop = ref(0);
const histHeight = ref(800);
const histBodyEl = ref<HTMLElement | null>(null);
const histWindow = computed(() => {
  const total = visibleCommits.value.length;
  const buf = 12;
  const start = Math.max(0, Math.floor(histScrollTop.value / HIST_ROW_H) - buf);
  const end = Math.min(total, Math.ceil((histScrollTop.value + histHeight.value) / HIST_ROW_H) + buf);
  return { items: visibleCommits.value.slice(start, end), offset: start * HIST_ROW_H, total };
});
function onHistScroll(e: Event) {
  const el = e.target as HTMLElement;
  histScrollTop.value = el.scrollTop;
  histHeight.value = el.clientHeight;
  if (el.scrollHeight - el.scrollTop - el.clientHeight < 600) loadMoreCommits();
}
function measureHist() {
  if (histBodyEl.value) histHeight.value = histBodyEl.value.clientHeight;
}
const branches = ref<BranchInfo[]>([]);
const status = ref<StatusEntry[]>([]);
const selected = ref<string | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);
const view = ref<"history" | "changes" | "prs" | "pipelines">("history");
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
const createPrOpen = ref(false);
const prSourceBranch = ref<string | null>(null);
function openCreatePr(source?: string) {
  prSourceBranch.value = source ?? null;
  createPrOpen.value = true;
}

const pipelineOpen = ref(false);
const pipelineSha = ref<string | null>(null);
const pipelineTitle = ref("");
function openPipeline(sha: string, title: string) {
  pipelineSha.value = sha;
  pipelineTitle.value = title;
  pipelineOpen.value = true;
}

const runPipelineOpen = ref(false);
const pipelineRef = ref<string | null>(null);
function openRunPipeline(branch?: string) {
  pipelineRef.value = branch ?? null;
  runPipelineOpen.value = true;
}

const repoSettingsOpen = ref(false);
const repoInfoOpen = ref(false);

// Guided merge / rebase / stash dialogs.
const integrateOpen = ref(false);
const integrateMode = ref<"merge" | "rebase">("merge");
const integratePreset = ref<string | null>(null);
function openIntegrate(mode: "merge" | "rebase", branch?: string) {
  integrateMode.value = mode;
  integratePreset.value = branch ?? null;
  integrateOpen.value = true;
}
function onIntegrate(mode: "merge" | "rebase", branch: string, opts: Record<string, boolean>) {
  if (!repo.value) return;
  const path = repo.value.path;
  if (mode === "merge") opRun(() => mergeBranchEx(path, branch, opts), "Merge");
  else opRun(() => rebaseBranchEx(path, branch, !!opts.autostash, !!opts.noVerify), "Rebase");
}
const workflowOpen = ref(false);

// Git Flow quick actions (also surfaced in the command palette when initialised).
async function doFlowStart(kind: string) {
  if (!repo.value) return;
  const name = await promptText({ title: `Start ${kind}`, label: "Name", placeholder: kind === "release" ? "1.2.0" : "short-name" });
  if (name && name.trim()) opRun(() => flowStart(repo.value!.path, kind, name.trim()), `Start ${kind}`);
}
// The current branch as a finishable flow branch (feature/release/hotfix/bugfix), if any.
const flowActive = computed(() => {
  const f = flowCfg.value;
  const c = repo.value?.head_branch;
  if (!f || !f.initialized || !c || (f.workflow !== "gitflow" && f.workflow !== "custom")) return null;
  for (const kind of ["feature", "release", "hotfix", "bugfix"] as const) {
    const p = (f as unknown as Record<string, string>)[kind];
    if (p && c.startsWith(p)) return { kind, name: c.slice(p.length) };
  }
  return null;
});
async function doFlowFinish() {
  const a = flowActive.value;
  if (!a || !repo.value) return;
  let version: string | undefined;
  if (a.kind === "release" || a.kind === "hotfix") {
    const v = await promptText({ title: `Finish ${a.kind}`, label: "Version tag", value: a.name });
    if (v === null) return;
    version = v.trim() || a.name;
  }
  opRun(() => flowFinish(repo.value!.path, a.kind, a.name, version), `Finish ${a.kind}`);
}

const stashSaveOpen = ref(false);
const stashApplyOpen = ref(false);
const stashApplyTarget = ref<{ index: number; label: string }>({ index: 0, label: "" });
function openStashApply(index: number, label: string) {
  stashApplyTarget.value = { index, label };
  stashApplyOpen.value = true;
}
const submodulesOpen = ref(false);
const worktreesOpen = ref(false);
const bisectOpen = ref(false);
const bisect = ref<BisectStatus>({ active: false, current: null, current_short: null });

async function doBisectMark(verdict: "good" | "bad" | "skip") {
  if (!repo.value) return;
  try {
    const msg = await bisectMark(repo.value.path, verdict);
    await refresh();
    // git prints the result — flag the winner prominently.
    if (/first bad commit/i.test(msg)) toast("Found it 🎯", msg.split("\n")[0]);
    else toast("Bisect", msg.split("\n")[0] || "Marked");
  } catch (e) {
    toast("Bisect failed", String(e), "error");
  }
}
async function doBisectReset() {
  if (!repo.value) return;
  try {
    await bisectReset(repo.value.path);
    await refresh();
    toast("Bisect ended");
  } catch (e) {
    toast("Couldn't end bisect", String(e), "error");
  }
}

const compareOpen = ref(false);
const compareBase = ref<string | null>(null);
function openCompare(base?: string) {
  compareBase.value = base ?? null;
  compareOpen.value = true;
}

const reflogOpen = ref(false);
// Row actions in the reflog: recover a lost commit safely (new branch) or
// move the current branch back to it. Reuses the commit-level ops.
function reflogMenu(e: MouseEvent, entry: ReflogEntry) {
  if (!repo.value) return;
  const path = repo.value.path;
  openContextMenu(e, [
    {
      label: "Create branch here…",
      action: async () => {
        const name = await promptText({ title: "Recover to a new branch", label: `From ${entry.short_id}`, placeholder: "recovered" });
        if (name && name.trim()) {
          runOp(() => createBranch(path, name.trim(), entry.id, true), `Branch "${name.trim()}" created`);
          reflogOpen.value = false;
        }
      },
    },
    { label: "Check out this commit", action: () => { runOp(() => checkoutCommit(path, entry.id), "Checked out"); reflogOpen.value = false; } },
    { separator: true, label: "" },
    { label: "Soft reset branch to here", action: () => { runOp(() => gitReset(path, entry.id, "soft"), "Reset (soft)"); reflogOpen.value = false; } },
    {
      label: "Hard reset branch to here…",
      danger: true,
      action: () => {
        if (window.confirm(`Hard reset ${headBranch.value} to ${entry.short_id}? This discards working changes.`)) {
          runOp(() => gitReset(path, entry.id, "hard"), "Reset (hard)");
          reflogOpen.value = false;
        }
      },
    },
    { separator: true, label: "" },
    { label: "Copy SHA", action: () => copy(entry.id, "SHA") },
  ]);
}

// Open-repo tabs + recents + home state
interface Tab {
  path: string;
  name: string;
}
const tabs = ref<Tab[]>([]);
const activePath = ref<string>(""); // "" = home screen
const recents = ref<RecentRepo[]>(loadRecents());
// Home only when no tab is active. Deliberately NOT tied to repo.value
// matching activePath: while switching to another repo the new commits load
// asynchronously, and gating on an exact match would flash the home screen
// (and briefly highlight the home tab) until the load finished.
const showWorkspace = computed(() => !!activePath.value);
// Basename of the tab being opened, for the loading placeholder.
const activeName = computed(() => {
  const t = tabs.value.find((t) => t.path === activePath.value);
  return t?.name || activePath.value.split("/").filter(Boolean).pop() || "repository";
});

function pushRecent(t: RecentRepo) {
  recents.value = [t, ...recents.value.filter((r) => r.path !== t.path)].slice(0, 12);
  saveRecents(recents.value);
}
function forgetRecent(path: string) {
  recents.value = recents.value.filter((r) => r.path !== path);
  saveRecents(recents.value);
}

// Favorite repos on the home screen (persisted).
const favorites = ref<string[]>(JSON.parse(localStorage.getItem("plumb.favorites") || "[]"));
function toggleFavorite(path: string) {
  favorites.value = favorites.value.includes(path)
    ? favorites.value.filter((p) => p !== path)
    : [...favorites.value, path];
  localStorage.setItem("plumb.favorites", JSON.stringify(favorites.value));
}
function selectTab(path: string) {
  if (activePath.value === path) return;
  captureTab(); // snapshot the tab we're leaving
  const cached = tabCache.get(path);
  if (cached) {
    restoreTab(cached); // instant paint from cache
    activePath.value = path;
    void revalidateTab(path); // then refresh quietly in place
  } else {
    loadRepo(path); // first visit — full load
  }
}
function closeTab(path: string) {
  const idx = tabs.value.findIndex((t) => t.path === path);
  tabs.value = tabs.value.filter((t) => t.path !== path);
  tabCache.delete(path);
  if (activePath.value === path) {
    const next = tabs.value[idx] ?? tabs.value[idx - 1] ?? null;
    if (next) selectTab(next.path);
    else activePath.value = "";
  }
}
function goHome() {
  captureTab(); // so returning to this repo is instant
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
  bisectStatus(path).then((b) => (bisect.value = b)).catch(() => (bisect.value = { active: false, current: null, current_short: null }));
  repoState(path).then((r) => (state.value = r)).catch(() => (state.value = { state: "clean", conflicts: false }));
  flowConfig(path).then((f) => (flowCfg.value = f)).catch(() => (flowCfg.value = null));
}

// Current repo's workflow config, used to surface Git Flow start/finish actions
// in the command palette when a Git Flow model is initialised.
const flowCfg = ref<FlowConfig | null>(null);

const opLabel = computed(
  () =>
    ({ merge: "Merging", rebase: "Rebasing", cherrypick: "Cherry-picking", revert: "Reverting" } as Record<string, string>)[
      state.value.state
    ] ?? "",
);

// CI status per commit sha (for graph badges). Loaded on repo open and fetch —
// not on every refresh — to stay light on API rate limits. A slow background
// poll notifies when a pending pipeline finishes.
const ciMap = ref<Map<string, string>>(new Map());
const prevCi = new Map<string, string>();

// Per-repo pipeline-notification preference (set in Repository settings →
// Notifications). Default "mine" so you aren't pinged for everyone's commits.
function ciNotifyPref(path: string): "all" | "mine" | "off" {
  try {
    return JSON.parse(localStorage.getItem(`plumb.repo.${path}`) || "{}").ciNotify ?? "mine";
  } catch {
    return "mine";
  }
}
// The email you commit as in a repo (git config user.email), memoized.
const repoEmail = new Map<string, string>();
async function myEmailFor(path: string): Promise<string> {
  const hit = repoEmail.get(path);
  if (hit !== undefined) return hit;
  let e = "";
  try {
    e = ((await getConfig(path, ["user.email"]))["user.email"] || "").toLowerCase();
  } catch {
    /* no identity configured */
  }
  repoEmail.set(path, e);
  return e;
}
async function refreshCiMap(path: string, notify: boolean) {
  let list;
  try {
    list = await listCiStatuses(path);
  } catch {
    return;
  }
  const next = new Map(list.map((c) => [c.sha, c.status]));
  if (notify) {
    const pref = ciNotifyPref(path); // "all" | "mine" | "off"
    if (pref !== "off") {
      const me = pref === "mine" ? await myEmailFor(path) : "";
      for (const [sha, status] of next) {
        if (prevCi.get(sha) === "pending" && (status === "success" || status === "failure")) {
          if (pref === "mine") {
            // Only my own commits — skip anything we can't attribute to `me`.
            const c = commits.value.find((x) => x.id === sha);
            if (!c || c.author_email.toLowerCase() !== me) continue;
          }
          notifyCi(sha, status);
        }
      }
    }
  }
  ciMap.value = next;
  prevCi.clear();
  next.forEach((v, k) => prevCi.set(k, v));
}
function loadCiMap(path: string) {
  refreshCiMap(path, false);
}
async function notifyCi(sha: string, status: string) {
  const c = commits.value.find((x) => x.id === sha);
  const short = c?.short_id ?? sha.slice(0, 7);
  try {
    let granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
    if (!granted) return;
    sendNotification({
      title: status === "success" ? "Pipeline passed ✓" : "Pipeline failed ✕",
      body: `${short}${c?.summary ? " · " + c.summary : ""} — ${repo.value?.name ?? ""}`,
    });
  } catch {
    /* notifications unavailable */
  }
}
function commitCi(id: string): string | undefined {
  return ciMap.value.get(id);
}
function ciGlyph(status: string): string {
  return status === "success" ? "✓" : status === "failure" ? "✕" : "●";
}

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

// Toolbar search. "view" filters the loaded commits client-side; "message" and
// "code" search all history via the backend (git log --grep / pickaxe -G).
type SearchScope = "view" | "message" | "code";
const searchScope = ref<SearchScope>("view");
const searchResults = ref<CommitRow[]>([]);
const searching = ref(false);

const visibleCommits = computed(() => {
  const q = commitFilter.value.trim();
  if (!q) return commits.value;
  if (searchScope.value !== "view") return searchResults.value;
  const lc = q.toLowerCase();
  return commits.value.filter(
    (c) =>
      c.summary.toLowerCase().includes(lc) ||
      c.author_name.toLowerCase().includes(lc) ||
      c.id.toLowerCase().includes(lc),
  );
});

let searchTimer: number | undefined;
async function runDeepSearch() {
  if (!repo.value || searchScope.value === "view") return;
  const q = commitFilter.value.trim();
  if (!q) {
    searchResults.value = [];
    return;
  }
  searching.value = true;
  try {
    searchResults.value = await searchAllCommits(repo.value.path, q, searchScope.value, 300);
  } catch {
    searchResults.value = [];
  } finally {
    searching.value = false;
  }
}
// Debounce typing; re-run immediately when the scope changes.
function onSearchInput() {
  if (searchScope.value === "view") return;
  clearTimeout(searchTimer);
  searchTimer = window.setTimeout(runDeepSearch, 300);
}
function onScopeChange() {
  selected.value = null;
  runDeepSearch();
}

const laneColor = (i: number) => `var(--lane-${i % 7})`;

// Text gutter reserved for the commit graph. Tracks the graph's real pixel
// width (reported by CommitGraph) so lanes never overlap the message text;
// collapses to the base width when filtering (graph hidden).
const graphWidth = ref(0);
// Graph gutter: auto-fits the graph up to a cap, past which it's scrollable;
// draggable to override. graphScrollX pans the (clipped) graph horizontally.
const GRAPH_GUTTER_MAX = 300;
const graphGutterManual = ref<number | null>(null);
const graphScrollX = ref(0);
const graphGutterPx = computed(() => {
  if (commitFilter.value) return 130;
  if (graphGutterManual.value != null) return graphGutterManual.value;
  return Math.min(Math.max(130, graphWidth.value + 22), GRAPH_GUTTER_MAX);
});
const graphGutter = computed(() => `${graphGutterPx.value}px`);
// The overlay starts at left:12, so its visible width is the gutter minus that.
const graphColWidth = computed(() => Math.max(0, graphGutterPx.value - 12));
const graphMaxScroll = computed(() => Math.max(0, graphWidth.value - graphColWidth.value));
watch([graphGutterPx, graphWidth], () => {
  if (graphScrollX.value > graphMaxScroll.value) graphScrollX.value = graphMaxScroll.value;
});
function onGraphWheel(e: WheelEvent) {
  if (Math.abs(e.deltaX) <= Math.abs(e.deltaY) || graphMaxScroll.value === 0) return; // let vertical scroll the list
  e.preventDefault();
  graphScrollX.value = Math.min(graphMaxScroll.value, Math.max(0, graphScrollX.value + e.deltaX));
}
function startGraphResize(e: PointerEvent) {
  e.preventDefault();
  const startX = e.clientX;
  const startW = graphGutterPx.value;
  const move = (m: PointerEvent) => { graphGutterManual.value = Math.max(60, startW + (m.clientX - startX)); };
  const up = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

// History columns: resizable widths + which are shown. Persisted per install.
interface HistCols { author: number; hash: number; when: number; showAuthor: boolean; showHash: boolean; showWhen: boolean }
const HIST_COLS_LS = "plumb.hist.cols";
const histCols = reactive<HistCols>({
  author: 160, hash: 92, when: 84, showAuthor: true, showHash: true, showWhen: true,
  ...(() => { try { return JSON.parse(localStorage.getItem(HIST_COLS_LS) || "{}"); } catch { return {}; } })(),
});
watch(histCols, () => localStorage.setItem(HIST_COLS_LS, JSON.stringify(histCols)), { deep: true });
const histGrid = computed(() => {
  const parts = ["var(--graph-gutter, 130px)", "minmax(140px, 1fr)"];
  if (histCols.showAuthor) parts.push(`${histCols.author}px`);
  if (histCols.showHash) parts.push(`${histCols.hash}px`);
  if (histCols.showWhen) parts.push(`${histCols.when}px`);
  return parts.join(" ");
});
// Drag a column's left edge to resize (dragging left widens it).
function startColResize(col: "author" | "hash" | "when", e: PointerEvent) {
  e.preventDefault();
  const startX = e.clientX;
  const startW = histCols[col];
  const move = (m: PointerEvent) => { histCols[col] = Math.max(56, startW - (m.clientX - startX)); };
  const up = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}
// Column visibility picker.
function histColsMenu(e: MouseEvent) {
  const item = (label: string, key: "showAuthor" | "showHash" | "showWhen") => ({
    label: `${histCols[key] ? "✓ " : "   "}${label}`,
    action: () => (histCols[key] = !histCols[key]),
  });
  openContextMenu(e, [item("Author", "showAuthor"), item("Hash", "showHash"), item("When", "showWhen")]);
}

// Sidebar filter — narrows branches, remotes, stashes and tags at once.
const sideFilter = ref("");
const sideMatch = (s: string) => s.toLowerCase().includes(sideFilter.value.trim().toLowerCase());

// Branch tree (local + remote) + a stable colour per branch. Filtered by the
// sidebar query when one is set.
const fLocalBranches = computed(() => (sideFilter.value ? localBranches.value.filter((b) => sideMatch(b.name)) : localBranches.value));
const fRemoteBranches = computed(() => (sideFilter.value ? remoteBranches.value.filter((b) => sideMatch(b.name)) : remoteBranches.value));
const localTree = computed(() => buildBranchTree(fLocalBranches.value));
const remoteTree = computed(() => buildBranchTree(fRemoteBranches.value));
const fStashes = computed(() => (sideFilter.value ? stashes.value.filter((s) => sideMatch(s.message)) : stashes.value));
const fTags = computed(() => (sideFilter.value ? tags.value.filter((t) => sideMatch(t.name)) : tags.value));
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
  const picked = await openFolder("Open a repository");
  if (!picked) return;
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

// Per-tab state cache. repo.value only ever holds one repo, so without this
// every tab click re-ran the full git load (openRepo + commits + branches +
// status) — the source of the switch lag. We snapshot a tab's rendered state
// when leaving it and restore it instantly on return, then quietly revalidate.
type TabSnap = {
  repo: NonNullable<typeof repo.value>;
  commits: typeof commits.value;
  allCommitsLoaded: boolean;
  branches: typeof branches.value;
  status: typeof status.value;
  selected: typeof selected.value;
  view: typeof view.value;
  stashes: typeof stashes.value;
  tags: typeof tags.value;
  files: typeof files.value;
  remotes: typeof remotes.value;
  bisect: typeof bisect.value;
  state: typeof state.value;
  flowCfg: typeof flowCfg.value;
  prCount: typeof prCount.value;
  ciMap: typeof ciMap.value;
};
const tabCache = new Map<string, TabSnap>();

function captureTab() {
  if (!repo.value) return;
  tabCache.set(repo.value.path, {
    repo: repo.value,
    commits: commits.value,
    allCommitsLoaded: allCommitsLoaded.value,
    branches: branches.value,
    status: status.value,
    selected: selected.value,
    view: view.value,
    stashes: stashes.value,
    tags: tags.value,
    files: files.value,
    remotes: remotes.value,
    bisect: bisect.value,
    state: state.value,
    flowCfg: flowCfg.value,
    prCount: prCount.value,
    ciMap: ciMap.value,
  });
}

function restoreTab(s: TabSnap) {
  repo.value = s.repo;
  commits.value = s.commits;
  allCommitsLoaded.value = s.allCommitsLoaded;
  branches.value = s.branches;
  status.value = s.status;
  selected.value = s.selected;
  view.value = s.view;
  stashes.value = s.stashes;
  tags.value = s.tags;
  files.value = s.files;
  remotes.value = s.remotes;
  bisect.value = s.bisect;
  state.value = s.state;
  flowCfg.value = s.flowCfg;
  prCount.value = s.prCount;
  ciMap.value = s.ciMap;
}

// Refresh a restored tab in the background — no spinner, applied in place only
// if the user is still on it.
async function revalidateTab(path: string) {
  try {
    const r = await openRepo(path);
    const [c, b, s] = await Promise.all([
      listCommits(r.path, FIRST_PAGE),
      listBranches(r.path),
      workingStatus(r.path),
    ]);
    if (activePath.value !== path) return; // moved on — keep the cached view
    repo.value = r;
    branches.value = b;
    status.value = s;
    // The fs watcher has a single slot; re-point it at the tab we switched to
    // so external edits/commits here emit "repo-changed".
    watchRepo(path).catch(() => {});
    // Only rebuild the commit list if the tip actually moved, so switching
    // between unchanged tabs doesn't discard how far the user had scrolled.
    if (c[0]?.id !== commits.value[0]?.id) {
      commits.value = c;
      allCommitsLoaded.value = c.length < FIRST_PAGE;
      if (!allCommitsLoaded.value) void loadMoreCommits();
    }
    loadPrCount(path);
    loadCiMap(path);
    loadExtras(path);
  } catch {
    /* keep the cached view */
  }
}

async function loadRepo(path: string) {
  const prevPath = activePath.value;
  // Highlight the target tab immediately so the switch feels instant; the
  // current repo stays visible in place until the new data arrives (no home
  // flash). Restored to the previous tab if the open fails.
  activePath.value = path;
  loading.value = true;
  error.value = null;
  try {
    repo.value = await openRepo(path);
    const [c, b, s] = await Promise.all([
      listCommits(repo.value.path, FIRST_PAGE),
      listBranches(repo.value.path),
      workingStatus(repo.value.path),
    ]);
    commits.value = c;
    allCommitsLoaded.value = c.length < FIRST_PAGE;
    branches.value = b;
    status.value = s;
    selected.value = null; // nothing highlighted until the user clicks
    const r = repo.value;
    if (!tabs.value.some((t) => t.path === r.path)) tabs.value.push({ path: r.path, name: r.name });
    activePath.value = r.path;
    pushRecent({ path: r.path, name: r.name, branch: r.head_branch ?? "", at: Date.now() });
    loadPrCount(r.path);
    loadCiMap(r.path);
    loadExtras(r.path);
    watchRepo(r.path).catch(() => {}); // auto-refresh on external changes
    // Stream the next page in behind the first paint so scrolling has a buffer
    // ready without the initial switch waiting on it.
    if (!allCommitsLoaded.value) void loadMoreCommits();
  } catch (e) {
    error.value = String(e);
    // Fall back to the repo we were on (or home) rather than a blank screen.
    activePath.value = repo.value?.path ?? prevPath ?? "";
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
  allCommitsLoaded.value = c.length < COMMIT_PAGE;
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
    loadCiMap(repo.value.path); // refresh CI badges after fetch/pull/push
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
    case "accounts": return openSettings("accounts");
    case "new_tab": return goHome();
    case "close_tab": return activePath.value ? closeTab(activePath.value) : undefined;
    case "open_repo":
    case "init_repo": return void chooseRepo();
    case "clone_repo": return void (cloneOpen.value = true);
    case "reveal": return hasRepo ? void revealItemInDir(repo.value!.path) : undefined;
    case "terminal": return hasRepo ? void openInTerminal(repo.value!.path).catch((e) => toast("Terminal", String(e), "error")) : undefined;
    case "editor": return hasRepo ? void openInEditor(repo.value!.path).catch((e) => toast("Editor", String(e), "error")) : undefined;
    case "command_palette": return void (paletteOpen.value = !paletteOpen.value);
    case "view_changes": return hasRepo ? void (view.value = "changes") : undefined;
    case "view_history": return hasRepo ? void (view.value = "history") : undefined;
    case "view_prs": return hasRepo ? void (view.value = "prs") : undefined;
    case "toggle_theme": return toggleTheme();
    case "fetch": return hasRepo ? doFetch() : undefined;
    case "pull": return hasRepo ? doPull() : undefined;
    case "push": return hasRepo ? void doPush() : undefined;
    case "new_branch": return hasRepo ? void newBranchPrompt() : undefined;
    case "merge": return hasRepo ? openIntegrate("merge") : undefined;
    case "rebase": return hasRepo ? openIntegrate("rebase") : undefined;
    case "flow": return hasRepo ? void (workflowOpen.value = true) : undefined;
    case "stash": return hasRepo ? void doStash() : undefined;
    case "remotes": return hasRepo ? void (remotesOpen.value = true) : undefined;
    case "new_pr": return hasRepo ? openCreatePr() : undefined;
    case "run_pipeline": return hasRepo ? openRunPipeline() : undefined;
    case "reflog": return hasRepo ? void (reflogOpen.value = true) : undefined;
    case "compare": return hasRepo ? openCompare() : undefined;
    case "submodules": return hasRepo ? void (submodulesOpen.value = true) : undefined;
    case "worktrees": return hasRepo ? void (worktreesOpen.value = true) : undefined;
    case "bisect": return hasRepo ? void (bisect.value.active ? doBisectReset() : (bisectOpen.value = true)) : undefined;
    case "repo_settings": return hasRepo ? void (repoSettingsOpen.value = true) : undefined;
    case "repo_info": return hasRepo ? void (repoInfoOpen.value = true) : undefined;
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
    { id: "a-editor", label: "Open in editor", group: "Action", action: () => repo.value && openInEditor(repo.value.path).catch(() => {}) },
    { id: "a-remotes", label: "Manage remotes…", group: "Action", action: () => (remotesOpen.value = true) },
    { id: "a-reposettings", label: "Repository settings…", group: "Action", action: () => (repoSettingsOpen.value = true) },
    { id: "a-repoinfo", label: "Repository info…", group: "Action", action: () => (repoInfoOpen.value = true) },
    { id: "a-newpr", label: "New pull / merge request…", group: "Action", action: () => openCreatePr() },
    { id: "a-runci", label: "Run pipeline…", group: "Action", action: () => openRunPipeline() },
    { id: "a-reflog", label: "History (reflog) — recover lost commits…", group: "Action", action: () => (reflogOpen.value = true) },
    { id: "a-compare", label: "Compare branches…", group: "Action", action: () => openCompare() },
    { id: "a-submodules", label: "Submodules…", group: "Action", action: () => (submodulesOpen.value = true) },
    { id: "a-worktrees", label: "Worktrees…", group: "Action", action: () => (worktreesOpen.value = true) },
    { id: "a-bisect", label: bisect.value.active ? "Bisect — end" : "Bisect — start…", group: "Action", action: () => (bisect.value.active ? doBisectReset() : (bisectOpen.value = true)) },
    { id: "a-newbranch", label: "New branch…", group: "Action", action: newBranchPrompt },
    { id: "a-merge", label: "Merge…", group: "Action", action: () => openIntegrate("merge") },
    { id: "a-rebase", label: "Rebase…", group: "Action", action: () => openIntegrate("rebase") },
    { id: "a-flow", label: "Workflows…", group: "Action", action: () => (workflowOpen.value = true) },
    { id: "a-settings", label: "Settings", group: "Action", action: () => openSettings() },
    { id: "a-accounts", label: "Accounts", group: "Action", action: () => openSettings("accounts") },
    { id: "a-theme", label: "Toggle theme", group: "Action", action: toggleTheme },
    { id: "a-stash", label: "Stash all changes", group: "Action", action: doStash },
    { id: "a-clone", label: "Clone repository…", group: "Action", action: () => (cloneOpen.value = true) },
    { id: "a-reveal", label: "Reveal in file manager", group: "Action", action: () => repo.value && revealItemInDir(repo.value.path) },
    { id: "a-terminal", label: "Open in terminal", group: "Action", action: () => repo.value && openInTerminal(repo.value.path).catch(() => {}) },
    { id: "a-newtab", label: "New tab", group: "Action", action: goHome },
    { id: "a-copybranch", label: `Copy branch name (${headBranch.value})`, group: "Action", action: () => copy(headBranch.value, "Branch name") },
  );
  // Git Flow branch actions when a Git Flow model is initialised.
  const f = flowCfg.value;
  if (f && f.initialized && (f.workflow === "gitflow" || f.workflow === "custom")) {
    items.push(
      { id: "flow-feature", label: "Git Flow: start feature…", group: "Action", action: () => doFlowStart("feature") },
      { id: "flow-release", label: "Git Flow: start release…", group: "Action", action: () => doFlowStart("release") },
      { id: "flow-hotfix", label: "Git Flow: start hotfix…", group: "Action", action: () => doFlowStart("hotfix") },
    );
    if (flowActive.value)
      items.push({ id: "flow-finish", label: `Git Flow: finish ${flowActive.value.kind} (${flowActive.value.name})`, group: "Action", action: doFlowFinish });
  }

  if (undoOp.value)
    items.push({ id: "a-undo", label: `Undo commit "${undoOp.value.message.split("\n")[0]}"`, group: "Action", action: undo });
  if (redoOp.value)
    items.push({ id: "a-redo", label: "Redo commit", group: "Action", action: redo });

  const rpath = repo.value.path;
  for (const b of localBranches.value) {
    if (!b.is_head)
      items.push({ id: `br-${b.name}`, label: `Checkout ${b.name}`, hint: "branch", group: "Branch", action: () => checkout(b.name) });
  }
  for (const t of tags.value) {
    items.push({ id: `t-${t.name}`, label: `Go to tag ${t.name}`, hint: "tag", group: "Tag", action: () => scrollToCommit(t.target) });
  }
  for (const s of stashes.value) {
    const label = `stash@{${s.index}}: ${s.message.replace(/^WIP on /, "")}`;
    items.push({ id: `st-a-${s.index}`, label: `Apply ${label}`, hint: "stash", group: "Stash", action: () => runOp(() => stashApply(rpath, s.index), "Stash applied") });
    items.push({ id: `st-p-${s.index}`, label: `Pop ${label}`, hint: "stash", group: "Stash", action: () => runOp(() => stashPop(rpath, s.index), "Stash popped") });
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
let unlistenOpen: UnlistenFn | undefined;
/* ── Session restore (reopen where you left off) ──────────────────── */
const SESSION_KEY = "plumb.session";
function persistSession() {
  try {
    localStorage.setItem(
      SESSION_KEY,
      JSON.stringify({ tabs: tabs.value, active: activePath.value, view: view.value }),
    );
  } catch {
    /* ignore quota errors */
  }
}
// Save whenever the open tabs, active repo, or active view change.
watch([tabs, activePath, view], persistSession, { deep: true });

async function restoreSession() {
  if (!prefs.reopenSession) return;
  let s: { tabs?: Tab[]; active?: string; view?: string } = {};
  try {
    s = JSON.parse(localStorage.getItem(SESSION_KEY) || "{}");
  } catch {
    return;
  }
  const saved = Array.isArray(s.tabs) ? s.tabs : [];
  // Keep only tabs whose repo still exists on disk.
  const valid: Tab[] = [];
  for (const t of saved) {
    if (t?.path && (await isRepo(t.path).catch(() => false))) valid.push({ path: t.path, name: t.name });
  }
  if (!valid.length) return;
  tabs.value = valid;
  const active = valid.find((t) => t.path === s.active)?.path;
  if (active) await loadRepo(active);
  if (s.view === "changes" || s.view === "history" || s.view === "prs") view.value = s.view;
}

let ciPollTimer: number | undefined;
// Re-measure the history viewport when it appears or the repo changes.
watch([() => view.value, () => repo.value?.path], () => nextTick(measureHist));

onMounted(async () => {
  refreshConnections();
  unlisten = await listen("repo-changed", scheduleRefresh);
  unlistenMenu = await listen<string>("menu-action", (e) => handleMenuAction(e.payload));
  // Open in Plumb: a folder passed on launch (CLI / editor), or forwarded from
  // a second `plumb <path>` invocation to this running window.
  unlistenOpen = await listen<string>("open-path", (e) => e.payload && loadRepo(e.payload));
  restoreSession();
  // In served mode (editor/browser), open the repo the session was launched
  // with; otherwise honour a desktop launch path.
  if (isServed) {
    if (servedRepo) loadRepo(servedRepo);
  } else {
    const launch = await invoke<string | null>("initial_path").catch(() => null);
    if (launch) loadRepo(launch);
  }
  // Poll CI every 90s so finished pipelines can notify.
  ciPollTimer = window.setInterval(() => {
    if (repo.value) refreshCiMap(repo.value.path, true);
  }, 90_000);
});
onUnmounted(() => {
  unlisten?.();
  unlistenMenu?.();
  unlistenOpen?.();
  if (ciPollTimer) clearInterval(ciPollTimer);
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
    {
      label: "Reword message…",
      action: async () => {
        const msg = await promptText({ title: "Reword commit", label: c.short_id, value: c.summary, confirmLabel: "Save" });
        if (msg && msg.trim()) runOp(() => rewordCommit(path, c.id, msg.trim()), "Message updated");
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
    {
      label: "Create branch here…",
      disabled: !b.target,
      action: async () => {
        const name = await promptText({ title: "New branch", label: `From ${b.name}`, placeholder: "feature/…" });
        if (name && name.trim() && b.target)
          runOp(() => createBranch(path, name.trim(), b.target!, true), `Branch "${name.trim()}" created`);
      },
    },
    { separator: true, label: "" },
    {
      label: `Merge ${b.name} into ${head}…`,
      disabled: b.is_head,
      action: () => openIntegrate("merge", b.name),
    },
    {
      label: `Rebase ${head} onto ${b.name}…`,
      disabled: b.is_head,
      action: () => openIntegrate("rebase", b.name),
    },
    { separator: true, label: "" },
    { label: `Compare ${b.name} with ${head}`, disabled: b.is_head, action: () => openCompare(b.name) },
    { label: "Run pipeline for this branch…", action: () => openRunPipeline(b.is_remote ? b.name.split("/").slice(1).join("/") : b.name) },
    { label: "Create pull request…", action: () => openCreatePr(b.is_remote ? b.name.split("/").slice(1).join("/") : b.name) },
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
function doStash() {
  if (!repo.value) return;
  stashSaveOpen.value = true;
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
function stashMenu(e: MouseEvent, s: StashEntry) {
  if (!repo.value) return;
  const path = repo.value.path;
  openContextMenu(e, [
    { label: "Apply", action: () => runOp(() => stashApply(path, s.index), "Stash applied") },
    { label: "Pop (apply & drop)", action: () => runOp(() => stashPop(path, s.index), "Stash popped") },
    { label: "Apply with options…", action: () => openStashApply(s.index, s.message || `stash@{${s.index}}`) },
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
  // Rows are virtualized, so the target may not be in the DOM — scroll by index.
  nextTick(() => {
    const el = histBodyEl.value;
    const idx = visibleCommits.value.findIndex((c) => c.id === id);
    if (el && idx >= 0) el.scrollTop = Math.max(0, idx * HIST_ROW_H - el.clientHeight / 2);
  });
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
      <button class="icon-btn" @click="openSettings()" title="Settings">⚙</button>
      <button class="pill kbd-pill mono" title="Command palette (⌘K)" @click="paletteOpen = true">⌘K</button>
    </header>

    <!-- ── Repo toolbar (workspace only) ────────────────────────────── -->
    <header v-if="showWorkspace && repo" class="toolbar">
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

      <div class="vsep"></div>

      <div class="repo-actions">
        <button class="btn" @click="openIntegrate('merge')" title="Merge a branch into the current branch">Merge</button>
        <button class="btn" @click="openIntegrate('rebase')" title="Rebase the current branch onto another">Rebase</button>
        <button class="btn" @click="doStash" title="Stash changes with options">Stash</button>
        <button class="btn" @click="workflowOpen = true" title="Workflows — Git Flow, GitHub Flow, GitLab Flow and more">Workflows</button>
      </div>

      <div class="spacer" data-tauri-drag-region></div>

      <div class="search">
        <span class="glyph">{{ searching ? "◌" : "⌕" }}</span>
        <input
          v-model="commitFilter"
          class="search-input"
          :placeholder="searchScope === 'code' ? 'Search code in history' : searchScope === 'message' ? 'Search all messages' : 'Search commits'"
          spellcheck="false"
          @focus="view = 'history'"
          @input="onSearchInput"
        />
        <select v-model="searchScope" class="scope" title="Search scope" @change="onScopeChange">
          <option value="view">In view</option>
          <option value="message">All · message</option>
          <option value="code">All · code</option>
        </select>
        <span v-if="commitFilter" class="clear" title="Clear" @click="commitFilter = ''">✕</span>
      </div>
    </header>

    <!-- Indeterminate activity bar — overlays the header's bottom edge, so it
         signals work without shifting any layout. -->
    <div v-if="syncing || loading" class="progress-bar" aria-hidden="true"></div>

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

    <!-- Bisect in-progress banner -->
    <div v-if="showWorkspace && bisect.active" class="op-banner bisect">
      <span class="op-text">
        <strong>Bisecting</strong> · testing <span class="mono">{{ bisect.current_short }}</span> — is the bug present?
      </span>
      <span class="grow"></span>
      <button class="op-btn" @click="doBisectMark('good')">Good</button>
      <button class="op-btn danger" @click="doBisectMark('bad')">Bad</button>
      <button class="op-btn" @click="doBisectMark('skip')">Skip</button>
      <button class="op-btn" @click="doBisectReset">End</button>
    </div>

    <!-- ── Full-screen diff (keeps the header above, like GitKraken) ── -->
    <DiffFullscreen v-if="showWorkspace && fullscreen.open" />

    <!-- ── Home ────────────────────────────────────────────────────── -->
    <HomePage
      v-else-if="!showWorkspace"
      :recents="recents"
      :favorites="favorites"
      @open="chooseRepo"
      @clone="cloneOpen = true"
      @connect="openSettings('accounts')"
      @select="loadRepo"
      @forget="forgetRecent"
      @favorite="toggleFavorite"
    />

    <!-- Opening the first repo (nothing to show in place yet) — a light
         placeholder instead of a blank frame or a home-screen flash. -->
    <div v-else-if="!repo" class="loading-screen">
      <span class="spinner big"></span>
      <span class="loading-name">Opening {{ activeName }}…</span>
    </div>

    <!-- ── Workspace ───────────────────────────────────────────────── -->
    <div v-else class="workspace">
      <!-- Sidebar -->
      <aside class="sidebar" :style="{ width: sidebarWidth + 'px' }">
        <div class="repo-head">
          <div class="repo-title-row">
            <div class="repo-title">{{ repo.name }}</div>
            <button class="rh-btn" title="Repository info" @click="repoInfoOpen = true">ⓘ</button>
            <button class="rh-btn" title="Repository settings" @click="repoSettingsOpen = true">⚙</button>
          </div>
          <div class="repo-path mono">{{ repo.path }}</div>
        </div>

        <div class="side-filter">
          <span class="sf-ico">⌕</span>
          <input v-model="sideFilter" placeholder="Filter branches, tags, stashes…" spellcheck="false" />
          <button v-if="sideFilter" class="sf-x" title="Clear" @click="sideFilter = ''">✕</button>
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
            <div class="side-row clickable" :class="{ active: view === 'pipelines' }" @click="view = 'pipelines'">
              <span class="ico">⚙</span>Pipelines
            </div>
          </template>
        </nav>

        <nav class="side-section" v-if="localTree.length">
          <div class="sect-head" @click="toggleSection('branches')">
            <span class="sect-chev">{{ collapsedSections.branches ? "▸" : "▾" }}</span>
            <span class="section-label">Branches</span>
          </div>
          <BranchTree v-if="!collapsedSections.branches" :nodes="localTree" />
        </nav>

        <nav class="side-section" v-if="remoteTree.length || (!sideFilter && remotes.length)">
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

        <nav class="side-section" v-if="!sideFilter || fStashes.length">
          <div class="sect-head" @click="toggleSection('stashes')">
            <span class="sect-chev">{{ collapsedSections.stashes ? "▸" : "▾" }}</span>
            <span class="section-label">Stashes</span>
            <span class="plus" title="Stash all changes" @click.stop="doStash">+</span>
          </div>
          <template v-if="!collapsedSections.stashes">
            <div
              v-for="s in fStashes"
              :key="s.index"
              class="side-row mono muted clickable"
              :title="s.message"
              @click="stashMenu($event, s)"
              @contextmenu="stashMenu($event, s)"
            >
              <span class="ellipsis">stash@{{ s.index }}: {{ s.message.replace(/^WIP on /, "") }}</span>
            </div>
            <div v-if="!fStashes.length" class="conn-empty">No stashes.</div>
          </template>
        </nav>

        <nav class="side-section" v-if="fTags.length">
          <div class="sect-head" @click="toggleSection('tags')">
            <span class="sect-chev">{{ collapsedSections.tags ? "▸" : "▾" }}</span>
            <span class="section-label">Tags</span>
            <span v-if="!sideFilter && tags.length > 12" class="tag-count mono">{{ tags.length }}</span>
          </div>
          <template v-if="!collapsedSections.tags">
            <div
              v-for="t in (sideFilter ? fTags : fTags.slice(0, 12))"
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
      <PullRequests
        v-else-if="view === 'prs'"
        :repo-path="repo.path"
        @create="openCreatePr()"
        @pipeline="(sha, title) => sha && openPipeline(sha, title)"
      />

      <!-- Repo-wide pipelines -->
      <PipelinesView
        v-else-if="view === 'pipelines'"
        :repo-path="repo.path"
        @pipeline="(sha, title) => sha && openPipeline(sha, title)"
      />

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

        <div v-else class="hist-list" :style="{ '--graph-gutter': graphGutter, '--hist-cols': histGrid }">
          <div class="hist-head mono">
            <span class="col-graph">GRAPH<span class="col-grip right" title="Resize graph" @pointerdown="startGraphResize"></span></span>
            <span class="hh-col col-commit">COMMIT</span>
            <span v-if="histCols.showAuthor" class="hh-col pad"><span class="col-grip" @pointerdown="startColResize('author', $event)"></span>AUTHOR</span>
            <span v-if="histCols.showHash" class="hh-col pad"><span class="col-grip" @pointerdown="startColResize('hash', $event)"></span>HASH</span>
            <span v-if="histCols.showWhen" class="col-when hh-col pad"><span class="col-grip" @pointerdown="startColResize('when', $event)"></span>WHEN</span>
            <button class="cols-btn" title="Show/hide columns" @click="histColsMenu">⋯</button>
          </div>

          <div ref="histBodyEl" class="hist-body" @scroll="onHistScroll">
            <div v-if="!commitFilter" class="graph-col" :style="{ width: graphColWidth + 'px' }" @wheel="onGraphWheel">
              <CommitGraph :commits="commits" :style="{ transform: `translateX(${-graphScrollX}px)` }" @width="graphWidth = $event" />
            </div>

            <div class="rows" :class="{ filtered: commitFilter }" :style="{ height: histWindow.total * 34 + 'px' }">
            <div class="rows-window" :style="{ transform: `translateY(${histWindow.offset}px)` }">
            <div
              v-for="c in histWindow.items"
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
                  v-if="commitCi(c.id)"
                  class="ci-badge"
                  :class="commitCi(c.id)"
                  :title="`CI: ${commitCi(c.id)} — click for jobs`"
                  @click.stop="openPipeline(c.id, c.short_id)"
                  >{{ ciGlyph(commitCi(c.id)!) }}</span
                >
                <span
                  v-for="r in pillRefs(c)"
                  :key="r"
                  class="ref-pill mono"
                  :class="{ tag: r.startsWith('tag:') }"
                  >{{ r.replace("tag: ", "⌾ ") }}</span
                >
                <span class="summary" :class="{ merge: c.is_merge }">{{ c.summary }}</span>
              </div>
              <div v-if="histCols.showAuthor" class="cell-author">
                <span class="avatar mono">{{ initials(c.author_name) }}</span>
                <span class="ellipsis author-name">{{ c.author_name }}</span>
              </div>
              <span v-if="histCols.showHash" class="cell-hash mono">{{ c.short_id }}</span>
              <span v-if="histCols.showWhen" class="cell-when">{{ relativeTime(c.time) }}</span>
            </div>
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
    <CreatePrDialog
      v-if="repo"
      v-model="createPrOpen"
      :repo-path="repo.path"
      :branches="localBranches.map((b) => b.name)"
      :current-branch="prSourceBranch ?? repo.head_branch"
      @created="() => repo && loadPrCount(repo.path)"
    />
    <ReflogDialog v-if="repo" v-model="reflogOpen" :repo-path="repo.path" @menu="reflogMenu" />
    <CompareDialog
      v-if="repo"
      v-model="compareOpen"
      :repo-path="repo.path"
      :branches="localBranches.map((b) => b.name)"
      :current-branch="repo.head_branch"
      :preset-base="compareBase"
    />
    <PipelineDialog v-if="repo" v-model="pipelineOpen" :repo-path="repo.path" :sha="pipelineSha" :title="pipelineTitle" />
    <RunPipelineDialog
      v-if="repo"
      v-model="runPipelineOpen"
      :repo-path="repo.path"
      :branches="localBranches.map((b) => b.name)"
      :current-branch="pipelineRef ?? repo.head_branch"
      @triggered="() => repo && loadCiMap(repo.path)"
    />
    <RepoSettingsDialog v-if="repo" v-model="repoSettingsOpen" :repo-path="repo.path" :repo-name="repo.name" />
    <RepoInfoDialog v-if="repo" v-model="repoInfoOpen" :repo-path="repo.path" />
    <IntegrateDialog
      v-if="repo"
      v-model="integrateOpen"
      :mode="integrateMode"
      :branches="localBranches.map((b) => b.name)"
      :current-branch="repo.head_branch"
      :preset-branch="integratePreset"
      @confirm="onIntegrate"
    />
    <WorkflowDialog
      v-if="repo"
      v-model="workflowOpen"
      :repo-path="repo.path"
      :branches="localBranches.map((b) => b.name)"
      :current-branch="repo.head_branch"
      @done="refresh"
      @create-pr="openCreatePr"
    />
    <StashSaveDialog v-if="repo" v-model="stashSaveOpen" :repo-path="repo.path" @done="refresh" />
    <StashApplyDialog v-if="repo" v-model="stashApplyOpen" :repo-path="repo.path" :index="stashApplyTarget.index" :label="stashApplyTarget.label" @done="refresh" />
    <SubmodulesDialog v-if="repo" v-model="submodulesOpen" :repo-path="repo.path" @open="loadRepo" />
    <WorktreesDialog v-if="repo" v-model="worktreesOpen" :repo-path="repo.path" :branches="localBranches.map((b) => b.name)" @open="loadRepo" />
    <BisectDialog
      v-if="repo"
      v-model="bisectOpen"
      :repo-path="repo.path"
      :branches="localBranches.map((b) => b.name)"
      @started="() => repo && loadExtras(repo.path)"
    />
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
.spinner.big { width: 22px; height: 22px; border-width: 3px; }

/* First-open placeholder — fills the frame under the tab bar. */
.loading-screen {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  color: var(--text-mid);
}
.loading-name { font-size: 13px; }

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
.repo-actions { display: flex; gap: 2px; }
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
  width: 320px;
  padding: 0 var(--space-3);
  background: var(--bg);
  border: 1px solid var(--line);
}
.search .scope { flex: none; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); font-size: 10.5px; height: 22px; padding: 0 2px; cursor: pointer; }
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
.workspace { flex: 1; display: flex; min-height: 0; overflow: hidden; }

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
.repo-title-row { display: flex; align-items: center; gap: 4px; }
.repo-title-row .repo-title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rh-btn { flex: none; width: 22px; height: 22px; display: inline-grid; place-items: center; background: transparent; border: none; color: var(--accent); font-size: 15px; cursor: pointer; }
.rh-btn:hover { color: var(--accent-strong, var(--accent)); }
.repo-title { font-size: 13px; font-weight: 700; }
.repo-path { font-size: 10.5px; color: var(--text-faint); margin-top: 3px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.side-filter { display: flex; align-items: center; gap: 6px; margin: var(--space-2) var(--space-3) 0; padding: 0 8px; height: 28px; background: var(--bg); border: 1px solid var(--line); }
.side-filter .sf-ico { color: var(--text-faint); font-size: 12px; flex: none; }
.side-filter input { flex: 1; min-width: 0; height: 100%; background: none; border: none; color: var(--text); font-size: 12px; }
.side-filter input:focus { outline: none; }
.side-filter input::placeholder { color: var(--text-faint); }
.side-filter .sf-x { flex: none; width: 16px; height: 16px; background: var(--raised); border: 1px solid var(--line); color: var(--text-dim); font-size: 9px; cursor: pointer; line-height: 1; }
.tag-count { margin-left: auto; font-size: 10px; color: var(--accent); }

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
.side-row .ico { width: 12px; color: var(--accent); }
.side-row .count { margin-left: auto; font-size: 10.5px; font-weight: 700; background: var(--accent); color: var(--accent-on); padding: 1px 5px; }
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
  position: relative;
  height: 28px;
  flex: none;
  display: grid;
  grid-template-columns: var(--hist-cols);
  align-items: center;
  background: var(--subtle);
  border-bottom: 1px solid var(--line);
  font-size: 10px;
  letter-spacing: 0.1em;
  color: var(--text-faint);
  padding-right: 22px;
}
.hist-head .col-graph { position: relative; padding-left: var(--space-3); }
.hist-head .col-when { text-align: right; }
.hist-head .hh-col { position: relative; }
.hist-head .hh-col.pad { padding-left: 14px; }
.col-grip { position: absolute; top: -6px; left: -1px; width: 12px; height: 28px; cursor: col-resize; z-index: 2; }
.col-grip.right { left: auto; right: -7px; }
.col-grip::after { content: ""; position: absolute; left: 5px; top: 0; width: 1px; height: 100%; background: var(--line); }
.col-grip:hover::after { background: var(--accent); width: 2px; }
.col-grip.right::after { left: 6px; }
.cols-btn { position: absolute; top: 3px; right: 4px; width: 18px; height: 20px; background: var(--raised); border: 1px solid var(--line); color: var(--text-dim); font-size: 12px; line-height: 1; cursor: pointer; z-index: 3; }

.hist-body { position: relative; flex: 1; overflow-y: auto; }
.graph-col { position: absolute; left: 12px; top: 0; overflow: hidden; z-index: 1; }
.graph-col :deep(svg) { pointer-events: none; }

.rows { position: relative; z-index: 0; }
.rows-window { position: absolute; top: 0; left: 0; right: 0; }
.commit-row {
  display: grid;
  grid-template-columns: var(--hist-cols);
  height: var(--row-commit);
  align-items: center;
  border-bottom: 1px solid var(--line-soft);
  padding-right: 22px;
}
.commit-row:hover { background: color-mix(in srgb, var(--raised) 55%, transparent); }
.commit-row.selected { background: var(--raised); box-shadow: inset 2px 0 0 var(--accent); }

.cell-author, .cell-hash, .cell-when { padding-left: 14px; }
.hist-head .col-commit { padding-left: 18px; }
.cell-commit { display: flex; align-items: center; gap: var(--space-2); min-width: 0; overflow: hidden; padding-left: 18px; }
.ci-badge { flex: none; width: 15px; height: 15px; display: inline-grid; place-items: center; font-size: 9.5px; font-weight: 800; color: var(--accent-on); cursor: pointer; }
.ci-badge.success { background: var(--lane-3); }
.ci-badge.failure { background: var(--accent); }
.ci-badge.pending { background: var(--lane-2); }
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
