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

export function listCommits(path: string, limit?: number): Promise<CommitRow[]> {
  return invoke("list_commits", { path, limit });
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
export function stashApply(path: string, index: number): Promise<void> {
  return invoke("stash_apply", { path, index });
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
export function rebaseBranch(path: string, onto: string): Promise<string> {
  return invoke("rebase_branch", { path, onto });
}

export type RebaseAction = "pick" | "reword" | "squash" | "fixup" | "drop";
export interface RebaseStep {
  action: RebaseAction;
  sha: string;
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

export function pullMode(path: string, mode: "merge" | "rebase" | "ff-only"): Promise<string> {
  return invoke("pull_mode", { path, mode });
}

export function deleteRemoteBranch(path: string, remote: string, branch: string): Promise<string> {
  return invoke("delete_remote_branch", { path, remote, branch });
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
