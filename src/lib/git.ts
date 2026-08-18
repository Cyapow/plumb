// Typed wrappers over the Rust `git` commands. The shapes here mirror the
// serde structs in `src-tauri/src/git.rs` exactly.

import { invoke } from "@tauri-apps/api/core";

export interface RepoInfo {
  path: string;
  name: string;
  head_branch: string | null;
  detached: boolean;
  empty: boolean;
}

export interface CommitRow {
  id: string;
  short_id: string;
  summary: string;
  body: string;
  author_name: string;
  author_email: string;
  time: number; // unix seconds
  parents: string[];
  refs: string[];
  is_merge: boolean;
}

export interface BranchInfo {
  name: string;
  is_head: boolean;
  is_remote: boolean;
  target: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
}

export interface StatusEntry {
  path: string;
  code: string; // M A D R C ? ! U
  staged: boolean;
  unstaged: boolean;
}

export function openRepo(path: string): Promise<RepoInfo> {
  return invoke("open_repo", { path });
}

export function isRepo(path: string): Promise<boolean> {
  return invoke("is_repo", { path });
}
export function initRepo(path: string, branch?: string): Promise<void> {
  return invoke("init_repo", { path, branch: branch ?? null });
}
export function openInTerminal(path: string): Promise<void> {
  return invoke("open_in_terminal", { path });
}
export function openInEditor(path: string): Promise<void> {
  return invoke("open_in_editor", { path });
}
export function addToGitignore(path: string, pattern: string): Promise<void> {
  return invoke("add_to_gitignore", { path, pattern });
}
export function rewordCommit(path: string, id: string, message: string): Promise<string> {
  return invoke("reword_commit", { path, id, message });
}
export function setDiffIgnoreWs(ignore: boolean): Promise<void> {
  return invoke("set_diff_ignore_ws", { ignore });
}
export function listSystemFonts(): Promise<string[]> {
  return invoke("list_system_fonts");
}

export interface RemoteInfo {
  name: string;
  url: string;
}
export function listRemotes(path: string): Promise<RemoteInfo[]> {
  return invoke("list_remotes", { path });
}
export function addRemote(path: string, name: string, url: string): Promise<void> {
  return invoke("add_remote", { path, name, url });
}

export function listCommits(path: string, limit?: number, skip?: number): Promise<CommitRow[]> {
  return invoke("list_commits", { path, limit, skip: skip ?? 0 });
}

export function listBranches(path: string): Promise<BranchInfo[]> {
  return invoke("list_branches", { path });
}

export function workingStatus(path: string): Promise<StatusEntry[]> {
  return invoke("working_status", { path });
}

export interface DiffLine {
  origin: string; // ' ' '+' '-'
  old_lineno: number | null;
  new_lineno: number | null;
  content: string;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  staged: boolean;
  binary: boolean;
  hunks: DiffHunk[];
}

export interface CommitResult {
  id: string;
  short_id: string;
}

export function stagePaths(path: string, paths: string[]): Promise<void> {
  return invoke("stage_paths", { path, paths });
}

export function unstagePaths(path: string, paths: string[]): Promise<void> {
  return invoke("unstage_paths", { path, paths });
}

export function stageHunk(path: string, file: string, hunkIndex: number): Promise<void> {
  return invoke("stage_hunk", { path, file, hunkIndex });
}

export function unstageHunk(path: string, file: string, hunkIndex: number): Promise<void> {
  return invoke("unstage_hunk", { path, file, hunkIndex });
}

export function stageLines(path: string, file: string, hunkIndex: number, lines: number[]): Promise<void> {
  return invoke("stage_lines", { path, file, hunkIndex, lines });
}

export function unstageLines(path: string, file: string, hunkIndex: number, lines: number[]): Promise<void> {
  return invoke("unstage_lines", { path, file, hunkIndex, lines });
}

export function cloneRepo(url: string, parentDir: string): Promise<string> {
  return invoke("clone_repo", { url, parentDir });
}

export interface StashEntry {
  index: number;
  message: string;
  id: string;
}
export function listStashes(path: string): Promise<StashEntry[]> {
  return invoke("list_stashes", { path });
}
export function stashSave(path: string, message?: string): Promise<void> {
  return invoke("stash_save", { path, message: message ?? null });
}
export function stashSaveEx(path: string, message: string | null, includeUntracked: boolean, keepIndex: boolean): Promise<void> {
  return invoke("stash_save_ex", { path, message, includeUntracked, keepIndex });
}
export function stashApply(path: string, index: number): Promise<void> {
  return invoke("stash_apply", { path, index });
}
export function stashApplyEx(path: string, index: number, pop: boolean, restoreIndex: boolean): Promise<void> {
  return invoke("stash_apply_ex", { path, index, pop, restoreIndex });
}
export function stashPop(path: string, index: number): Promise<void> {
  return invoke("stash_pop", { path, index });
}
export function stashDrop(path: string, index: number): Promise<void> {
  return invoke("stash_drop", { path, index });
}

export interface TagInfo {
  name: string;
  target: string | null;
}
export function listTags(path: string): Promise<TagInfo[]> {
  return invoke("list_tags", { path });
}

export function listFiles(path: string): Promise<string[]> {
  return invoke("list_files", { path });
}

export interface FileCommit {
  id: string;
  short_id: string;
  summary: string;
  author_name: string;
  time: number;
}
export function fileHistory(path: string, file: string): Promise<FileCommit[]> {
  return invoke("file_history", { path, file });
}

export interface BlameLine {
  line: number;
  content: string;
  short_id: string;
  author: string;
  time: number;
}
export function blameFile(path: string, file: string): Promise<BlameLine[]> {
  return invoke("blame_file", { path, file });
}

export function mergeBranch(path: string, name: string): Promise<string> {
  return invoke("merge_branch", { path, name });
}
export interface MergeOpts {
  squash?: boolean;
  noFf?: boolean;
  noCommit?: boolean;
  verifySignatures?: boolean;
  noVerify?: boolean;
}
export function mergeBranchEx(path: string, name: string, o: MergeOpts = {}): Promise<string> {
  return invoke("merge_branch_ex", {
    path,
    name,
    squash: !!o.squash,
    noFf: !!o.noFf,
    noCommit: !!o.noCommit,
    verifySignatures: !!o.verifySignatures,
    noVerify: !!o.noVerify,
  });
}
export function rebaseBranch(path: string, onto: string): Promise<string> {
  return invoke("rebase_branch", { path, onto });
}
export function rebaseBranchEx(path: string, onto: string, autostash: boolean, noVerify: boolean): Promise<string> {
  return invoke("rebase_branch_ex", { path, onto, autostash, noVerify });
}

export type RebaseAction = "pick" | "reword" | "squash" | "fixup" | "drop";
export interface RebaseStep {
  action: RebaseAction;
  sha: string;
  message?: string;
}
export function rebaseInteractive(
  path: string,
  base: string | null,
  steps: RebaseStep[],
): Promise<string> {
  return invoke("rebase_interactive", { path, base, steps });
}
export function cherryPick(path: string, id: string): Promise<string> {
  return invoke("cherry_pick", { path, id });
}
export function revertCommit(path: string, id: string): Promise<string> {
  return invoke("revert_commit", { path, id });
}
export function opAbort(path: string): Promise<string> {
  return invoke("op_abort", { path });
}
export function opContinue(path: string): Promise<string> {
  return invoke("op_continue", { path });
}

export interface RepoState {
  state: string; // "clean" | "merge" | "rebase" | "cherrypick" | "revert"
  conflicts: boolean;
}
export function repoState(path: string): Promise<RepoState> {
  return invoke("repo_state", { path });
}

export interface ReflogEntry {
  index: number;
  id: string;
  short_id: string;
  action: string;
  message: string;
  time: number;
}
export function reflog(path: string): Promise<ReflogEntry[]> {
  return invoke("reflog", { path });
}

export interface ConflictSides {
  base: string | null;
  ours: string | null;
  theirs: string | null;
  merged: string;
}
export function listConflicts(path: string): Promise<string[]> {
  return invoke("list_conflicts", { path });
}
export function conflictSides(path: string, file: string): Promise<ConflictSides> {
  return invoke("conflict_sides", { path, file });
}
export function resolveConflict(path: string, file: string, side: "ours" | "theirs"): Promise<void> {
  return invoke("resolve_conflict", { path, file, side });
}
export function resolveConflictContent(path: string, file: string, content: string): Promise<void> {
  return invoke("resolve_conflict_content", { path, file, content });
}

export function watchRepo(path: string): Promise<void> {
  return invoke("watch_repo", { path });
}

export function fileDiff(path: string, file: string, staged: boolean): Promise<FileDiff> {
  return invoke("file_diff", { path, file, staged });
}

export function commit(
  path: string,
  message: string,
  amend: boolean,
  signOff: boolean,
  sign = false,
): Promise<CommitResult> {
  return invoke("commit", { path, message, amend, signOff, sign });
}

export function initialCommit(path: string, message: string): Promise<string> {
  return invoke("initial_commit", { path, message });
}

export function listRemoteBranches(url: string): Promise<string[]> {
  return invoke("list_remote_branches", { url });
}
export function connectRemoteBranch(path: string, url: string, branch: string): Promise<string> {
  return invoke("connect_remote_branch", { path, url, branch });
}

export interface GitIdentity {
  name: string | null;
  email: string | null;
  signing: boolean;
}
export function gitIdentity(path: string): Promise<GitIdentity> {
  return invoke("git_identity", { path });
}
export function setGitIdentity(
  path: string,
  name: string,
  email: string,
  global: boolean,
): Promise<void> {
  return invoke("set_git_identity", { path, name, email, global });
}

export function getConfig(path: string, keys: string[]): Promise<Record<string, string>> {
  return invoke("get_config", { path, keys });
}
export function setConfig(path: string, key: string, value: string, global = false): Promise<void> {
  return invoke("set_config", { path, key, value, global });
}
export function unsetConfig(path: string, key: string, global = false): Promise<void> {
  return invoke("unset_config", { path, key, global });
}
export function getRepoDescription(path: string): Promise<string> {
  return invoke("get_repo_description", { path });
}
export function setRepoDescription(path: string, text: string): Promise<void> {
  return invoke("set_repo_description", { path, text });
}
export function getGitignore(path: string): Promise<string> {
  return invoke("get_gitignore", { path });
}
export function setGitignore(path: string, text: string): Promise<void> {
  return invoke("set_gitignore", { path, text });
}

export interface ChangedFile {
  path: string;
  code: string;
}

export interface CommitDetail {
  id: string;
  short_id: string;
  summary: string;
  body: string;
  author_name: string;
  author_email: string;
  time: number;
  parents: string[];
  files: ChangedFile[];
}

export function commitDetails(path: string, id: string): Promise<CommitDetail> {
  return invoke("commit_details", { path, id });
}

export function commitFileDiff(path: string, id: string, file: string): Promise<FileDiff> {
  return invoke("commit_file_diff", { path, id, file });
}

export interface CompareSummary {
  ahead: number;
  behind: number;
  files: ChangedFile[];
}
export function compareRefs(path: string, base: string, compare: string): Promise<CompareSummary> {
  return invoke("compare_refs", { path, base, compare });
}
export function compareFileDiff(path: string, base: string, compare: string, file: string): Promise<FileDiff> {
  return invoke("compare_file_diff", { path, base, compare, file });
}
export function searchCommits(
  path: string,
  query: string,
  mode: "message" | "code",
  limit?: number,
): Promise<CommitRow[]> {
  return invoke("search_commits", { path, query, mode, limit });
}

export function checkoutBranch(path: string, name: string): Promise<void> {
  return invoke("checkout_branch", { path, name });
}

export function checkoutCommit(path: string, id: string): Promise<void> {
  return invoke("checkout_commit", { path, id });
}

export function checkoutRemoteBranch(path: string, remoteBranch: string): Promise<string> {
  return invoke("checkout_remote_branch", { path, remoteBranch });
}

export function createBranch(
  path: string,
  name: string,
  id: string,
  checkout: boolean,
): Promise<void> {
  return invoke("create_branch", { path, name, id, checkout });
}

export function reset(path: string, revspec: string, mode: "soft" | "mixed" | "hard"): Promise<void> {
  return invoke("reset", { path, revspec, mode });
}

export function unstageAll(path: string): Promise<void> {
  return invoke("unstage_all", { path });
}

export function uncommit(path: string): Promise<void> {
  return invoke("uncommit", { path });
}

export function discardPaths(path: string, paths: string[]): Promise<void> {
  return invoke("discard_paths", { path, paths });
}

export function deleteBranch(path: string, name: string): Promise<void> {
  return invoke("delete_branch", { path, name });
}

export function fetch(path: string): Promise<string> {
  return invoke("fetch", { path });
}

export function pull(path: string): Promise<string> {
  return invoke("pull", { path });
}

export function push(path: string): Promise<string> {
  return invoke("push", { path });
}

export function pushAdvanced(
  path: string,
  opts: { remote?: string; forceWithLease?: boolean; pushTags?: boolean; setUpstream?: boolean } = {},
): Promise<string> {
  return invoke("push_advanced", {
    path,
    remote: opts.remote ?? null,
    forceWithLease: opts.forceWithLease ?? false,
    pushTags: opts.pushTags ?? false,
    setUpstream: opts.setUpstream ?? false,
  });
}

export function pushBranch(path: string, branch: string): Promise<string> {
  return invoke("push_branch", { path, branch });
}

export function pullMode(path: string, mode: "merge" | "rebase" | "ff-only"): Promise<string> {
  return invoke("pull_mode", { path, mode });
}

export function deleteRemoteBranch(path: string, remote: string, branch: string): Promise<string> {
  return invoke("delete_remote_branch", { path, remote, branch });
}

/* ── Power tools ── */
export interface SubmoduleInfo {
  name: string;
  path: string;
  url: string;
  pinned_id: string | null;
  wd_id: string | null;
  initialized: boolean;
  modified: boolean;
}
/** Which branching model a repo follows. "" = not yet chosen. */
export type WorkflowType = "" | "gitflow" | "custom" | "github" | "gitlab" | "trunk";
export interface FlowConfig {
  initialized: boolean;
  workflow: WorkflowType;
  main: string;
  develop: string;
  feature: string;
  release: string;
  hotfix: string;
  versiontag: string;
  environments: string[];
}
export function flowConfig(path: string): Promise<FlowConfig> {
  return invoke("flow_config", { path });
}
export function flowInit(path: string, main: string, develop: string, versiontag: string): Promise<string> {
  return invoke("flow_init", { path, main, develop, versiontag });
}
export function flowStart(path: string, kind: string, name: string): Promise<string> {
  return invoke("flow_start", { path, kind, name });
}
export function flowFinish(path: string, kind: string, name: string, version?: string): Promise<string> {
  return invoke("flow_finish", { path, kind, name, version: version ?? null });
}
export function flowSetType(path: string, workflow: WorkflowType): Promise<void> {
  return invoke("flow_set_type", { path, workflow });
}
export function flowSetEnvironments(path: string, csv: string): Promise<void> {
  return invoke("flow_set_environments", { path, csv });
}
/** Check out `target`, merge `source` into it (--no-ff), optionally delete `source`. */
export function mergeInto(path: string, source: string, target: string, deleteSource: boolean): Promise<string> {
  return invoke("merge_into", { path, source, target, deleteSource });
}

export function listSubmodules(path: string): Promise<SubmoduleInfo[]> {
  return invoke("list_submodules", { path });
}
export function updateSubmodules(path: string, init: boolean): Promise<string> {
  return invoke("update_submodules", { path, init });
}

export interface WorktreeInfo {
  path: string;
  head: string;
  branch: string;
  is_main: boolean;
}
export function listWorktrees(path: string): Promise<WorktreeInfo[]> {
  return invoke("list_worktrees", { path });
}
export function addWorktree(path: string, newPath: string, branch: string, newBranch: boolean): Promise<string> {
  return invoke("add_worktree", { path, newPath, branch, newBranch });
}
export function removeWorktree(path: string, worktreePath: string): Promise<string> {
  return invoke("remove_worktree", { path, worktreePath });
}

export interface BisectStatus {
  active: boolean;
  current: string | null;
  current_short: string | null;
}
export function bisectStatus(path: string): Promise<BisectStatus> {
  return invoke("bisect_status", { path });
}
export function bisectStart(path: string, bad: string, good: string): Promise<string> {
  return invoke("bisect_start", { path, bad, good });
}
export function bisectMark(path: string, verdict: "good" | "bad" | "skip"): Promise<string> {
  return invoke("bisect_mark", { path, verdict });
}
export function bisectReset(path: string): Promise<string> {
  return invoke("bisect_reset", { path });
}

export function renameRemote(path: string, from: string, to: string): Promise<void> {
  return invoke("rename_remote", { path, from, to });
}
export function removeRemote(path: string, name: string): Promise<void> {
  return invoke("remove_remote", { path, name });
}
export function setRemoteUrl(path: string, name: string, url: string): Promise<void> {
  return invoke("set_remote_url", { path, name, url });
}
export function pruneRemote(path: string, name: string): Promise<string> {
  return invoke("prune_remote", { path, name });
}
