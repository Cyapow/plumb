//! Git backend for Plumb, built on `git2` (libgit2).
//!
//! Everything the UI needs about a repository is exposed here as Tauri
//! commands that return plain, serde-serialisable structs. Keeping the
//! libgit2 surface in one module means the rest of the app never touches a
//! `git2` type directly.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use git2::{BranchType, DiffOptions, ObjectType, Oid, Patch, Repository, Sort, StatusOptions};
use serde::{Deserialize, Serialize};

/// When set, diffs ignore whitespace-only changes (a view option).
static IGNORE_WS: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn set_diff_ignore_ws(ignore: bool) {
    IGNORE_WS.store(ignore, Ordering::Relaxed);
}

fn apply_ignore_ws(opts: &mut DiffOptions) {
    if IGNORE_WS.load(Ordering::Relaxed) {
        opts.ignore_whitespace(true);
    }
}

/// Error type that crosses the Tauri boundary as a plain string message.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("{0}")]
    Git(#[from] git2::Error),
    #[error("no repository is open at {0}")]
    NotARepo(String),
    #[error("{0}")]
    Message(String),
}

impl Serialize for GitError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

type Result<T> = std::result::Result<T, GitError>;

/// Summary of the repository itself.
#[derive(Serialize)]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub head_branch: Option<String>,
    pub detached: bool,
    pub empty: bool,
}

/// A single row in the history / commit graph.
#[derive(Serialize)]
pub struct CommitRow {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub body: String,
    pub author_name: String,
    pub author_email: String,
    /// Author time, seconds since the Unix epoch (UTC).
    pub time: i64,
    pub parents: Vec<String>,
    /// Branch/tag refs whose tip is exactly this commit (e.g. "main",
    /// "origin/main", "tag: v2.4.0", "HEAD").
    pub refs: Vec<String>,
    pub is_merge: bool,
}

/// A branch, local or remote-tracking.
#[derive(Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
    pub target: Option<String>,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
}

/// One changed path in the working tree / index.
#[derive(Serialize)]
pub struct StatusEntry {
    pub path: String,
    /// Single-letter code: M, A, D, R, C, ? (untracked), ! (ignored), or U (conflict).
    pub code: String,
    pub staged: bool,
    pub unstaged: bool,
}

/// One line inside a diff hunk.
#[derive(Serialize)]
pub struct DiffLine {
    /// ' ' context, '+' addition, '-' deletion (git2 line origin).
    pub origin: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

#[derive(Serialize)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// A single file's diff, either staged (HEAD↔index) or unstaged (index↔workdir).
#[derive(Serialize)]
pub struct FileDiff {
    pub path: String,
    pub staged: bool,
    pub binary: bool,
    pub hunks: Vec<DiffHunk>,
}

/// The result of creating a commit.
#[derive(Serialize)]
pub struct CommitResult {
    pub id: String,
    pub short_id: String,
}

/// One file changed by a commit.
#[derive(Serialize)]
pub struct ChangedFile {
    pub path: String,
    pub code: String,
}

/// Full detail for one commit, including its changed files.
#[derive(Serialize)]
pub struct CommitDetail {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub body: String,
    pub author_name: String,
    pub author_email: String,
    pub time: i64,
    pub parents: Vec<String>,
    pub files: Vec<ChangedFile>,
}

fn open(path: &str) -> Result<Repository> {
    Repository::open(path).map_err(|_| GitError::NotARepo(path.to_string()))
}

/// Whether `path` is inside a Git repository.
#[tauri::command]
pub fn is_repo(path: String) -> bool {
    Repository::discover(&path).is_ok()
}

/// Initialize a new Git repository at `path`. `branch` names the initial
/// branch (unborn until the first commit); defaults to "main".
#[tauri::command]
pub fn init_repo(path: String, branch: Option<String>) -> Result<()> {
    let mut opts = git2::RepositoryInitOptions::new();
    let name = branch.as_deref().map(str::trim).filter(|b| !b.is_empty()).unwrap_or("main");
    opts.initial_head(name);
    Repository::init_opts(&path, &opts)?;
    Ok(())
}

#[derive(Serialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
}

#[tauri::command]
pub fn list_remotes(path: String) -> Result<Vec<RemoteInfo>> {
    let repo = open(&path)?;
    let mut out = Vec::new();
    if let Ok(names) = repo.remotes() {
        for n in names.iter().flatten() {
            if let Ok(r) = repo.find_remote(n) {
                out.push(RemoteInfo {
                    name: n.to_string(),
                    url: r.url().unwrap_or("").to_string(),
                });
            }
        }
    }
    Ok(out)
}

/// Add a remote (e.g. origin) pointing at a URL.
#[tauri::command]
pub fn add_remote(path: String, name: String, url: String) -> Result<()> {
    let repo = open(&path)?;
    repo.remote(&name, &url)?;
    Ok(())
}

/// Open a repository (or discover one from a path inside it) and return a summary.
#[tauri::command]
pub fn open_repo(path: String) -> Result<RepoInfo> {
    let repo = Repository::discover(&path).map_err(|_| GitError::NotARepo(path.clone()))?;
    let workdir = repo
        .workdir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let name = Path::new(workdir.trim_end_matches('/'))
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repository".into());

    let empty = repo.is_empty().unwrap_or(false);
    let (head_branch, detached) = match repo.head() {
        Ok(head) => {
            let detached = repo.head_detached().unwrap_or(false);
            let name = if detached {
                head.target().map(|o| short(&o))
            } else {
                head.shorthand().map(|s| s.to_string())
            };
            (name, detached)
        }
        Err(_) => {
            // Unborn branch (freshly initialised, no commits yet): HEAD is a
            // symbolic ref to refs/heads/<branch>. Report it so the UI shows
            // the branch you're about to commit onto.
            let name = repo
                .find_reference("HEAD")
                .ok()
                .and_then(|r| r.symbolic_target().map(|t| t.to_string()))
                .map(|t| t.trim_start_matches("refs/heads/").to_string());
            (name, false)
        }
    };

    Ok(RepoInfo {
        path: workdir,
        name,
        head_branch,
        detached,
        empty,
    })
}

/// Walk history across all local and remote branches, newest first.
#[tauri::command]
pub fn list_commits(path: String, limit: Option<usize>) -> Result<Vec<CommitRow>> {
    let repo = open(&path)?;
    let limit = limit.unwrap_or(500);

    let ref_map = build_ref_map(&repo);

    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)?;
    // Seed the walk from every branch tip plus HEAD so the graph shows all lanes.
    if walk.push_glob("refs/heads/*").is_err() {
        // fall through — an empty repo simply yields nothing
    }
    let _ = walk.push_glob("refs/remotes/*");
    let _ = walk.push_head();

    let mut rows = Vec::new();
    for oid in walk.flatten().take(limit) {
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let author = commit.author();
        let parents: Vec<String> = commit.parent_ids().map(|p| p.to_string()).collect();
        rows.push(CommitRow {
            id: oid.to_string(),
            short_id: short(&oid),
            summary: commit.summary().unwrap_or("").to_string(),
            body: commit.body().unwrap_or("").to_string(),
            author_name: author.name().unwrap_or("").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            time: author.when().seconds(),
            is_merge: parents.len() > 1,
            parents,
            refs: ref_map.get(&oid).cloned().unwrap_or_default(),
        });
    }
    Ok(rows)
}

/// List local and remote-tracking branches with ahead/behind vs their upstream.
#[tauri::command]
pub fn list_branches(path: String) -> Result<Vec<BranchInfo>> {
    let repo = open(&path)?;
    let mut out = Vec::new();

    for kind in [BranchType::Local, BranchType::Remote] {
        let branches = repo.branches(Some(kind))?;
        for item in branches {
            let (branch, _) = item?;
            let is_head = branch.is_head();
            let name = branch
                .name()?
                .map(|s| s.to_string())
                .unwrap_or_else(|| "<invalid utf-8>".into());
            let target = branch.get().target();

            let mut upstream = None;
            let mut ahead = 0;
            let mut behind = 0;
            if kind == BranchType::Local {
                if let Ok(up) = branch.upstream() {
                    upstream = up.name()?.map(|s| s.to_string());
                    if let (Some(local_oid), Some(up_oid)) = (target, up.get().target()) {
                        if let Ok((a, b)) = repo.graph_ahead_behind(local_oid, up_oid) {
                            ahead = a;
                            behind = b;
                        }
                    }
                }
            }

            out.push(BranchInfo {
                name,
                is_head,
                is_remote: kind == BranchType::Remote,
                target: target.map(|o| o.to_string()),
                upstream,
                ahead,
                behind,
            });
        }
    }

    // Unborn HEAD (fresh/connected repo with no commits yet): the branch has no
    // ref to iterate, so surface it explicitly as the current branch.
    if repo.head().is_err() && !out.iter().any(|b| b.is_head && !b.is_remote) {
        if let Some(name) = repo
            .find_reference("HEAD")
            .ok()
            .and_then(|r| r.symbolic_target().map(|t| t.trim_start_matches("refs/heads/").to_string()))
        {
            out.push(BranchInfo {
                name,
                is_head: true,
                is_remote: false,
                target: None,
                upstream: None,
                ahead: 0,
                behind: 0,
            });
        }
    }
    Ok(out)
}

/// Working-tree + index status, one entry per changed path.
#[tauri::command]
pub fn working_status(path: String) -> Result<Vec<StatusEntry>> {
    let repo = open(&path)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut out = Vec::new();
    for entry in statuses.iter() {
        let s = entry.status();
        let path = entry.path().unwrap_or("").to_string();

        let staged = s.is_index_new()
            || s.is_index_modified()
            || s.is_index_deleted()
            || s.is_index_renamed()
            || s.is_index_typechange();
        let unstaged = s.is_wt_new()
            || s.is_wt_modified()
            || s.is_wt_deleted()
            || s.is_wt_renamed()
            || s.is_wt_typechange();

        let code = if s.is_conflicted() {
            "U"
        } else if s.is_wt_new() && !staged {
            "?"
        } else if s.is_index_new() {
            "A"
        } else if s.is_index_deleted() || s.is_wt_deleted() {
            "D"
        } else if s.is_index_renamed() || s.is_wt_renamed() {
            "R"
        } else if s.is_ignored() {
            "!"
        } else {
            "M"
        };

        out.push(StatusEntry {
            path,
            code: code.to_string(),
            staged,
            unstaged,
        });
    }
    Ok(out)
}

/// Stage the given paths (add for created/modified, remove for deletions).
#[tauri::command]
pub fn stage_paths(path: String, paths: Vec<String>) -> Result<()> {
    let repo = open(&path)?;
    let workdir = repo.workdir().map(|w| w.to_path_buf());
    let mut index = repo.index()?;
    for p in &paths {
        let exists = workdir
            .as_ref()
            .map(|w| w.join(p).exists())
            .unwrap_or(false);
        if exists {
            index.add_path(Path::new(p))?;
        } else {
            index.remove_path(Path::new(p))?;
        }
    }
    index.write()?;
    Ok(())
}

/// Unstage the given paths (reset their index entry back to HEAD).
#[tauri::command]
pub fn unstage_paths(path: String, paths: Vec<String>) -> Result<()> {
    let repo = open(&path)?;
    match repo.head() {
        Ok(head) => {
            let obj = head.peel(ObjectType::Commit)?;
            repo.reset_default(Some(&obj), paths.iter().map(|s| s.as_str()))?;
        }
        Err(_) => {
            // Unborn HEAD (no commits yet): just drop them from the index.
            let mut index = repo.index()?;
            for p in &paths {
                let _ = index.remove_path(Path::new(p));
            }
            index.write()?;
        }
    }
    Ok(())
}

/// Read a single file's diff. `staged` selects HEAD↔index vs index↔workdir.
#[tauri::command]
pub fn file_diff(path: String, file: String, staged: bool) -> Result<FileDiff> {
    let repo = open(&path)?;
    let mut opts = DiffOptions::new();
    opts.pathspec(&file);
    opts.context_lines(3);
        apply_ignore_ws(&mut opts);
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    let diff = if staged {
        let head_tree = match repo.head() {
            Ok(h) => Some(h.peel_to_tree()?),
            Err(_) => None,
        };
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };

    collect_file_diff(&diff, &file, staged)
}

/// Turn a libgit2 diff into a single file's hunks/lines.
fn collect_file_diff(diff: &git2::Diff, file: &str, staged: bool) -> Result<FileDiff> {
    let mut result = FileDiff {
        path: file.to_string(),
        staged,
        binary: false,
        hunks: Vec::new(),
    };

    for i in 0..diff.deltas().len() {
        let delta = diff.get_delta(i);
        let matches = delta
            .and_then(|d| d.new_file().path().or_else(|| d.old_file().path()))
            .map(|p| p.to_string_lossy() == file)
            .unwrap_or(false);
        if !matches {
            continue;
        }

        match Patch::from_diff(diff, i)? {
            None => result.binary = true,
            Some(patch) => {
                for h in 0..patch.num_hunks() {
                    let (hunk, _) = patch.hunk(h)?;
                    let header = String::from_utf8_lossy(hunk.header()).trim_end().to_string();
                    let mut lines = Vec::new();
                    for l in 0..patch.num_lines_in_hunk(h)? {
                        let dl = patch.line_in_hunk(h, l)?;
                        lines.push(DiffLine {
                            origin: dl.origin().to_string(),
                            old_lineno: dl.old_lineno(),
                            new_lineno: dl.new_lineno(),
                            content: String::from_utf8_lossy(dl.content())
                                .trim_end_matches('\n')
                                .to_string(),
                        });
                    }
                    result.hunks.push(DiffHunk { header, lines });
                }
            }
        }
        break;
    }
    Ok(result)
}

/* ── Hunk-level staging ───────────────────────────────────────────── */

/// The full unified-diff text for one file, so we can carve out single hunks.
fn file_patch_text(repo: &Repository, file: &str, staged: bool) -> Result<String> {
    let mut opts = DiffOptions::new();
    opts.pathspec(file);
    opts.context_lines(3);
        apply_ignore_ws(&mut opts);
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    let diff = if staged {
        let head_tree = match repo.head() {
            Ok(h) => Some(h.peel_to_tree()?),
            Err(_) => None,
        };
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };

    for i in 0..diff.deltas().len() {
        let delta = diff.get_delta(i);
        let matches = delta
            .and_then(|d| d.new_file().path().or_else(|| d.old_file().path()))
            .map(|p| p.to_string_lossy() == file)
            .unwrap_or(false);
        if matches {
            if let Some(mut patch) = Patch::from_diff(&diff, i)? {
                let buf = patch.to_buf()?;
                return Ok(buf.as_str().unwrap_or("").to_string());
            }
        }
    }
    Err(GitError::Message("No textual diff for that file.".into()))
}

/// Split a file patch into its header and each `@@` hunk block.
fn split_patch(text: &str) -> (String, Vec<String>) {
    let mut header = String::new();
    let mut hunks: Vec<String> = Vec::new();
    let mut cur: Option<String> = None;
    for line in text.split_inclusive('\n') {
        if line.starts_with("@@") {
            if let Some(h) = cur.take() {
                hunks.push(h);
            }
            cur = Some(line.to_string());
        } else if let Some(h) = cur.as_mut() {
            h.push_str(line);
        } else {
            header.push_str(line);
        }
    }
    if let Some(h) = cur.take() {
        hunks.push(h);
    }
    (header, hunks)
}

/// Apply a patch to the index via the system git (reliable partial staging).
fn git_apply_cached(dir: &str, patch: &str, reverse: bool) -> Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("git");
    cmd.current_dir(dir)
        .arg("apply")
        .arg("--cached")
        .arg("--whitespace=nowarn");
    if reverse {
        cmd.arg("--reverse");
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| GitError::Message(format!("git apply: {e}")))?;
    {
        let mut si = child
            .stdin
            .take()
            .ok_or_else(|| GitError::Message("git apply: no stdin".into()))?;
        si.write_all(patch.as_bytes())
            .map_err(|e| GitError::Message(e.to_string()))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| GitError::Message(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(GitError::Message(if err.is_empty() {
            "Couldn't apply that hunk.".into()
        } else {
            err
        }))
    }
}

/// Stage a single hunk (index of the unstaged diff) into the index.
#[tauri::command]
pub async fn stage_hunk(path: String, file: String, hunk_index: usize) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let (header, hunks) = split_patch(&file_patch_text(&repo, &file, false)?);
        let hunk = hunks
            .get(hunk_index)
            .ok_or_else(|| GitError::Message("Hunk no longer exists — refresh.".into()))?;
        git_apply_cached(&path, &format!("{header}{hunk}"), false)
    })
    .await
}

/// Unstage a single hunk (index of the staged diff) from the index.
#[tauri::command]
pub async fn unstage_hunk(path: String, file: String, hunk_index: usize) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let (header, hunks) = split_patch(&file_patch_text(&repo, &file, true)?);
        let hunk = hunks
            .get(hunk_index)
            .ok_or_else(|| GitError::Message("Hunk no longer exists — refresh.".into()))?;
        git_apply_cached(&path, &format!("{header}{hunk}"), true)
    })
    .await
}

/* ── Line-level staging ───────────────────────────────────────────── */

/// Build a patch containing only the selected lines of one hunk. Unselected
/// additions are dropped; unselected deletions become context — the standard
/// `git add -p` line-selection transform.
fn build_line_patch(
    repo: &Repository,
    file: &str,
    staged: bool,
    hunk_index: usize,
    selected: &HashSet<usize>,
) -> Result<String> {
    let mut opts = DiffOptions::new();
    opts.pathspec(file);
    opts.context_lines(3);
        apply_ignore_ws(&mut opts);

    let diff = if staged {
        let head_tree = match repo.head() {
            Ok(h) => Some(h.peel_to_tree()?),
            Err(_) => None,
        };
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
    } else {
        opts.include_untracked(true);
        opts.recurse_untracked_dirs(true);
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };

    for i in 0..diff.deltas().len() {
        let delta = diff.get_delta(i);
        let matches = delta
            .and_then(|d| d.new_file().path().or_else(|| d.old_file().path()))
            .map(|p| p.to_string_lossy() == file)
            .unwrap_or(false);
        if !matches {
            continue;
        }
        let mut patch = match Patch::from_diff(&diff, i)? {
            Some(p) => p,
            None => return Err(GitError::Message("Can't stage lines of a binary file.".into())),
        };
        if hunk_index >= patch.num_hunks() {
            return Err(GitError::Message("Hunk no longer exists — refresh.".into()));
        }
        let (old_start, new_start) = {
            let (h, _) = patch.hunk(hunk_index)?;
            (h.old_start(), h.new_start())
        };

        let mut body = String::new();
        let (mut old_count, mut new_count) = (0u32, 0u32);
        for l in 0..patch.num_lines_in_hunk(hunk_index)? {
            let dl = patch.line_in_hunk(hunk_index, l)?;
            let content = String::from_utf8_lossy(dl.content()).to_string();
            let picked = selected.contains(&l);
            match dl.origin() {
                ' ' => {
                    body.push(' ');
                    body.push_str(&content);
                    old_count += 1;
                    new_count += 1;
                }
                '+' => {
                    if picked {
                        body.push('+');
                        body.push_str(&content);
                        new_count += 1;
                    }
                }
                '-' => {
                    if picked {
                        body.push('-');
                        body.push_str(&content);
                        old_count += 1;
                    } else {
                        body.push(' ');
                        body.push_str(&content);
                        old_count += 1;
                        new_count += 1;
                    }
                }
                _ => body.push_str("\\ No newline at end of file\n"),
            }
        }

        let buf = patch.to_buf()?;
        let (file_header, _) = split_patch(buf.as_str().unwrap_or(""));
        let header = format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@\n");
        return Ok(format!("{file_header}{header}{body}"));
    }
    Err(GitError::Message("No textual diff for that file.".into()))
}

/// Stage only the selected lines (indices within the unstaged hunk).
#[tauri::command]
pub async fn stage_lines(path: String, file: String, hunk_index: usize, lines: Vec<usize>) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let sel: HashSet<usize> = lines.into_iter().collect();
        let patch = build_line_patch(&repo, &file, false, hunk_index, &sel)?;
        git_apply_cached(&path, &patch, false)
    })
    .await
}

/// Unstage only the selected lines (indices within the staged hunk).
#[tauri::command]
pub async fn unstage_lines(path: String, file: String, hunk_index: usize, lines: Vec<usize>) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let sel: HashSet<usize> = lines.into_iter().collect();
        let patch = build_line_patch(&repo, &file, true, hunk_index, &sel)?;
        git_apply_cached(&path, &patch, true)
    })
    .await
}

/* ── Clone ────────────────────────────────────────────────────────── */

/// Clone a repository URL into `parent_dir`; returns the cloned repo path.
#[tauri::command]
pub async fn clone_repo(url: String, parent_dir: String) -> Result<String> {
    spawn(move || {
        run_git(&parent_dir, &["clone", "--progress", &url])?;
        let name = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("repository")
            .trim_end_matches(".git");
        Ok(Path::new(&parent_dir).join(name).to_string_lossy().to_string())
    })
    .await
}

/* ── Stashes ──────────────────────────────────────────────────────── */

#[derive(Serialize)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
    pub id: String,
}

#[tauri::command]
pub fn list_stashes(path: String) -> Result<Vec<StashEntry>> {
    let mut repo = open(&path)?;
    let mut out = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        out.push(StashEntry {
            index,
            message: message.to_string(),
            id: oid.to_string(),
        });
        true
    })?;
    Ok(out)
}

#[tauri::command]
pub async fn stash_save(path: String, message: Option<String>) -> Result<()> {
    spawn(move || {
        let mut repo = open(&path)?;
        let sig = repo.signature()?;
        let msg = message.unwrap_or_default();
        repo.stash_save(&sig, &msg, Some(git2::StashFlags::INCLUDE_UNTRACKED))?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn stash_apply(path: String, index: usize) -> Result<()> {
    spawn(move || {
        let mut repo = open(&path)?;
        repo.stash_apply(index, None)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn stash_pop(path: String, index: usize) -> Result<()> {
    spawn(move || {
        let mut repo = open(&path)?;
        repo.stash_pop(index, None)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn stash_drop(path: String, index: usize) -> Result<()> {
    let mut repo = open(&path)?;
    repo.stash_drop(index)?;
    Ok(())
}

/* ── Tags & file list ─────────────────────────────────────────────── */

#[derive(Serialize)]
pub struct TagInfo {
    pub name: String,
    pub target: Option<String>,
}

#[tauri::command]
pub fn list_tags(path: String) -> Result<Vec<TagInfo>> {
    let repo = open(&path)?;
    let mut out = Vec::new();
    repo.tag_foreach(|oid, name| {
        let full = String::from_utf8_lossy(name);
        let short = full.strip_prefix("refs/tags/").unwrap_or(&full).to_string();
        out.push(TagInfo { name: short, target: Some(oid.to_string()) });
        true
    })?;
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/* ── File history & blame ─────────────────────────────────────────── */

#[derive(Serialize)]
pub struct FileCommit {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub time: i64,
}

/// Commits that touched a file (follows renames).
#[tauri::command]
pub async fn file_history(path: String, file: String) -> Result<Vec<FileCommit>> {
    spawn(move || {
        let out = run_git(
            &path,
            &[
                "log",
                "--follow",
                "-n",
                "200",
                "--pretty=format:%H\x1f%h\x1f%an\x1f%at\x1f%s",
                "--",
                &file,
            ],
        )?;
        let mut v = Vec::new();
        for line in out.lines() {
            let p: Vec<&str> = line.split('\u{1f}').collect();
            if p.len() >= 5 {
                v.push(FileCommit {
                    id: p[0].to_string(),
                    short_id: p[1].to_string(),
                    author_name: p[2].to_string(),
                    time: p[3].parse().unwrap_or(0),
                    summary: p[4].to_string(),
                });
            }
        }
        Ok(v)
    })
    .await
}

#[derive(Serialize)]
pub struct BlameLine {
    pub line: usize,
    pub content: String,
    pub short_id: String,
    pub author: String,
    pub time: i64,
}

/// Per-line blame for a file in the working tree.
#[tauri::command]
pub async fn blame_file(path: String, file: String) -> Result<Vec<BlameLine>> {
    spawn(move || {
        let repo = open(&path)?;
        let blame = repo.blame_file(Path::new(&file), None)?;
        let workdir = repo
            .workdir()
            .ok_or_else(|| GitError::Message("Bare repository.".into()))?;
        let content = std::fs::read_to_string(workdir.join(&file))
            .map_err(|e| GitError::Message(format!("Couldn't read file: {e}")))?;

        let mut out = Vec::new();
        for (i, text) in content.lines().enumerate() {
            let lineno = i + 1;
            match blame.get_line(lineno) {
                Some(hunk) => {
                    let oid = hunk.final_commit_id();
                    let sig = hunk.final_signature();
                    out.push(BlameLine {
                        line: lineno,
                        content: text.to_string(),
                        short_id: short(&oid),
                        author: sig.name().unwrap_or("").to_string(),
                        time: sig.when().seconds(),
                    });
                }
                None => out.push(BlameLine {
                    line: lineno,
                    content: text.to_string(),
                    short_id: String::new(),
                    author: String::new(),
                    time: 0,
                }),
            }
        }
        Ok(out)
    })
    .await
}

/// Tracked files in the repo (for the command palette).
#[tauri::command]
pub async fn list_files(path: String) -> Result<Vec<String>> {
    spawn(move || {
        let out = run_git(&path, &["ls-files"])?;
        Ok(out.lines().map(|l| l.to_string()).collect())
    })
    .await
}

/* ── Merge / rebase / cherry-pick / revert ────────────────────────── */

/// Run a merge-like op; treat conflicts as a soft "resolve then continue"
/// state rather than a hard error, so the UI can show a banner.
fn run_merge_like(path: &str, args: &[&str]) -> Result<String> {
    match run_git(path, args) {
        Ok(out) => Ok(if out.trim().is_empty() { "Done".into() } else { out.trim().to_string() }),
        Err(GitError::Message(m)) => {
            let low = m.to_lowercase();
            if low.contains("conflict")
                || low.contains("automatic merge failed")
                || low.contains("could not apply")
                || low.contains("after resolving the conflicts")
            {
                Ok("Stopped with conflicts — resolve them, then Continue.".into())
            } else {
                Err(GitError::Message(m))
            }
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn merge_branch(path: String, name: String) -> Result<String> {
    spawn(move || run_merge_like(&path, &["merge", "--no-edit", &name])).await
}

#[tauri::command]
pub async fn rebase_branch(path: String, onto: String) -> Result<String> {
    spawn(move || run_merge_like(&path, &["rebase", &onto])).await
}

#[tauri::command]
pub async fn cherry_pick(path: String, id: String) -> Result<String> {
    spawn(move || run_merge_like(&path, &["cherry-pick", &id])).await
}

#[tauri::command]
pub async fn revert_commit(path: String, id: String) -> Result<String> {
    spawn(move || run_merge_like(&path, &["revert", "--no-edit", &id])).await
}

#[tauri::command]
pub async fn op_abort(path: String) -> Result<String> {
    spawn(move || {
        let repo = open(&path)?;
        let args: &[&str] = match repo.state() {
            git2::RepositoryState::Rebase
            | git2::RepositoryState::RebaseInteractive
            | git2::RepositoryState::RebaseMerge => &["rebase", "--abort"],
            git2::RepositoryState::CherryPick | git2::RepositoryState::CherryPickSequence => {
                &["cherry-pick", "--abort"]
            }
            git2::RepositoryState::Revert | git2::RepositoryState::RevertSequence => {
                &["revert", "--abort"]
            }
            git2::RepositoryState::Merge => &["merge", "--abort"],
            _ => return Ok("Nothing to abort.".into()),
        };
        drop(repo);
        run_git(&path, args)?;
        Ok("Aborted.".into())
    })
    .await
}

#[tauri::command]
pub async fn op_continue(path: String) -> Result<String> {
    spawn(move || {
        let repo = open(&path)?;
        let state = repo.state();
        drop(repo);
        // core.editor=true skips the editor for continue/commit steps.
        let res = match state {
            git2::RepositoryState::Rebase
            | git2::RepositoryState::RebaseInteractive
            | git2::RepositoryState::RebaseMerge => {
                run_merge_like(&path, &["-c", "core.editor=true", "rebase", "--continue"])
            }
            git2::RepositoryState::CherryPick | git2::RepositoryState::CherryPickSequence => {
                run_merge_like(&path, &["-c", "core.editor=true", "cherry-pick", "--continue"])
            }
            git2::RepositoryState::Revert | git2::RepositoryState::RevertSequence => {
                run_merge_like(&path, &["-c", "core.editor=true", "revert", "--continue"])
            }
            git2::RepositoryState::Merge => run_git(&path, &["commit", "--no-edit"]),
            _ => return Ok("Nothing to continue.".into()),
        };
        res
    })
    .await
}

#[derive(Serialize)]
pub struct RepoState {
    /// "clean" | "merge" | "rebase" | "cherrypick" | "revert"
    pub state: String,
    pub conflicts: bool,
}

#[tauri::command]
pub fn repo_state(path: String) -> Result<RepoState> {
    let repo = open(&path)?;
    let state = match repo.state() {
        git2::RepositoryState::Merge => "merge",
        git2::RepositoryState::Rebase
        | git2::RepositoryState::RebaseInteractive
        | git2::RepositoryState::RebaseMerge => "rebase",
        git2::RepositoryState::CherryPick | git2::RepositoryState::CherryPickSequence => "cherrypick",
        git2::RepositoryState::Revert | git2::RepositoryState::RevertSequence => "revert",
        _ => "clean",
    }
    .to_string();

    let mut opts = StatusOptions::new();
    opts.include_untracked(false);
    let conflicts = repo
        .statuses(Some(&mut opts))
        .map(|st| st.iter().any(|e| e.status().is_conflicted()))
        .unwrap_or(false);

    Ok(RepoState { state, conflicts })
}

/// One HEAD reflog entry — a point HEAD has been, so lost commits after a bad
/// reset/rebase can be recovered.
#[derive(Serialize)]
pub struct ReflogEntry {
    pub index: usize,
    pub id: String,
    pub short_id: String,
    pub action: String,
    pub message: String,
    pub time: i64,
}

/// Read the HEAD reflog (most recent first), capped so it stays snappy.
#[tauri::command]
pub fn reflog(path: String) -> Result<Vec<ReflogEntry>> {
    let repo = open(&path)?;
    let rl = repo.reflog("HEAD")?;
    let mut out = Vec::new();
    for i in 0..rl.len().min(250) {
        if let Some(e) = rl.get(i) {
            let oid = e.id_new();
            let message = e.message().unwrap_or("").to_string();
            let action = message.split(':').next().unwrap_or("").trim().to_string();
            out.push(ReflogEntry {
                index: i,
                id: oid.to_string(),
                short_id: short(&oid),
                action,
                message,
                time: e.committer().when().seconds(),
            });
        }
    }
    Ok(out)
}

/// A file with unresolved merge conflicts, and whether each side still exists
/// (a side is absent for add/add or delete/modify conflicts).
#[derive(Serialize)]
pub struct ConflictSides {
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    pub merged: String,
}

fn blob_text(repo: &Repository, oid: Oid) -> Option<String> {
    repo.find_blob(oid).ok().map(|b| String::from_utf8_lossy(b.content()).to_string())
}

fn entry_path(e: &git2::IndexEntry) -> String {
    String::from_utf8_lossy(&e.path).to_string()
}

/// List paths that currently have merge conflicts (index has multiple stages).
#[tauri::command]
pub fn list_conflicts(path: String) -> Result<Vec<String>> {
    let repo = open(&path)?;
    let index = repo.index()?;
    let mut out = Vec::new();
    if index.has_conflicts() {
        for c in index.conflicts()? {
            let c = c?;
            if let Some(e) = c.our.as_ref().or(c.their.as_ref()).or(c.ancestor.as_ref()) {
                out.push(entry_path(e));
            }
        }
    }
    Ok(out)
}

/// The three sides of a conflicted file plus the current working-tree contents
/// (which still has the conflict markers until resolved).
#[tauri::command]
pub fn conflict_sides(path: String, file: String) -> Result<ConflictSides> {
    let repo = open(&path)?;
    let index = repo.index()?;
    let (mut base, mut ours, mut theirs) = (None, None, None);
    for c in index.conflicts()? {
        let c = c?;
        let matches = [c.ancestor.as_ref(), c.our.as_ref(), c.their.as_ref()]
            .into_iter()
            .flatten()
            .any(|e| entry_path(e) == file);
        if matches {
            base = c.ancestor.and_then(|e| blob_text(&repo, e.id));
            ours = c.our.and_then(|e| blob_text(&repo, e.id));
            theirs = c.their.and_then(|e| blob_text(&repo, e.id));
            break;
        }
    }
    let merged = repo
        .workdir()
        .map(|w| w.join(&file))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    Ok(ConflictSides { base, ours, theirs, merged })
}

/// Resolve a conflict by taking one whole side ("ours" | "theirs"), then stage.
#[tauri::command]
pub async fn resolve_conflict(path: String, file: String, side: String) -> Result<()> {
    spawn(move || {
        let flag = match side.as_str() {
            "ours" => "--ours",
            "theirs" => "--theirs",
            _ => return Err(GitError::Message("side must be 'ours' or 'theirs'".into())),
        };
        run_git(&path, &["checkout", flag, "--", &file])?;
        run_git(&path, &["add", "--", &file])?;
        Ok(())
    })
    .await
}

/// Resolve a conflict with hand-edited content, then stage it.
#[tauri::command]
pub async fn resolve_conflict_content(path: String, file: String, content: String) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let full = repo
            .workdir()
            .ok_or_else(|| GitError::Message("Repository has no working directory.".into()))?
            .join(&file);
        std::fs::write(&full, content.as_bytes())
            .map_err(|e| GitError::Message(format!("Couldn't write {file}: {e}")))?;
        run_git(&path, &["add", "--", &file])?;
        Ok(())
    })
    .await
}

fn delta_code(status: git2::Delta) -> &'static str {
    use git2::Delta::*;
    match status {
        Added => "A",
        Deleted => "D",
        Modified => "M",
        Renamed => "R",
        Copied => "C",
        Untracked => "?",
        Typechange => "T",
        Conflicted => "U",
        _ => "M",
    }
}

/// Full metadata and changed-file list for a single commit.
#[tauri::command]
pub fn commit_details(path: String, id: String) -> Result<CommitDetail> {
    let repo = open(&path)?;
    let oid = Oid::from_str(&id)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
    let mut files = Vec::new();
    for delta in diff.deltas() {
        let p = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        files.push(ChangedFile {
            path: p,
            code: delta_code(delta.status()).to_string(),
        });
    }

    let author = commit.author();
    Ok(CommitDetail {
        id: oid.to_string(),
        short_id: short(&oid),
        summary: commit.summary().unwrap_or("").to_string(),
        body: commit.body().unwrap_or("").to_string(),
        author_name: author.name().unwrap_or("").to_string(),
        author_email: author.email().unwrap_or("").to_string(),
        time: author.when().seconds(),
        parents: commit.parent_ids().map(|p| p.to_string()).collect(),
        files,
    })
}

/// Summary of comparing two refs (branches or commits): commit lead/lag and the
/// files that differ between their trees.
#[derive(Serialize)]
pub struct CompareSummary {
    pub ahead: usize,
    pub behind: usize,
    pub files: Vec<ChangedFile>,
}

/// Compare `base` with `compare` (each a branch name or revspec).
#[tauri::command]
pub fn compare_refs(path: String, base: String, compare: String) -> Result<CompareSummary> {
    let repo = open(&path)?;
    let base_commit = repo.revparse_single(&base)?.peel_to_commit()?;
    let comp_commit = repo.revparse_single(&compare)?.peel_to_commit()?;
    let (ahead, behind) = repo.graph_ahead_behind(comp_commit.id(), base_commit.id())?;
    let diff = repo.diff_tree_to_tree(Some(&base_commit.tree()?), Some(&comp_commit.tree()?), None)?;
    let mut files = Vec::new();
    for delta in diff.deltas() {
        let p = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        files.push(ChangedFile { path: p, code: delta_code(delta.status()).to_string() });
    }
    Ok(CompareSummary { ahead, behind, files })
}

/// Diff of one file between two refs.
#[tauri::command]
pub fn compare_file_diff(path: String, base: String, compare: String, file: String) -> Result<FileDiff> {
    let repo = open(&path)?;
    let base_tree = repo.revparse_single(&base)?.peel_to_commit()?.tree()?;
    let comp_tree = repo.revparse_single(&compare)?.peel_to_commit()?.tree()?;
    let mut opts = DiffOptions::new();
    opts.pathspec(&file);
    opts.context_lines(3);
        apply_ignore_ws(&mut opts);
    let diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&comp_tree), Some(&mut opts))?;
    collect_file_diff(&diff, &file, false)
}

/// Search all history. `mode` = "message" (grep commit message) or "code"
/// (pickaxe -G: commits whose diff matches the query). Delegated to `git log`.
#[tauri::command]
pub async fn search_commits(
    path: String,
    query: String,
    mode: String,
    limit: Option<usize>,
) -> Result<Vec<CommitRow>> {
    spawn(move || {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let n = limit.unwrap_or(200).to_string();
        let fmt = "--pretty=format:%H\u{1f}%h\u{1f}%s\u{1f}%an\u{1f}%ae\u{1f}%at\u{1f}%P";
        let mut args: Vec<String> = vec!["log".into(), "--all".into(), "-n".into(), n];
        match mode.as_str() {
            "code" => args.push(format!("-G{query}")),
            _ => {
                args.push("-i".into());
                args.push(format!("--grep={query}"));
            }
        }
        args.push(fmt.into());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = run_git(&path, &refs)?;
        let mut rows = Vec::new();
        for line in out.lines() {
            let f: Vec<&str> = line.split('\u{1f}').collect();
            if f.len() < 7 {
                continue;
            }
            let parents: Vec<String> = f[6].split_whitespace().map(String::from).collect();
            rows.push(CommitRow {
                id: f[0].to_string(),
                short_id: f[1].to_string(),
                summary: f[2].to_string(),
                body: String::new(),
                author_name: f[3].to_string(),
                author_email: f[4].to_string(),
                time: f[5].parse().unwrap_or(0),
                is_merge: parents.len() > 1,
                parents,
                refs: Vec::new(),
            });
        }
        Ok(rows)
    })
    .await
}

/// Diff of one file within a commit (against its first parent).
#[tauri::command]
pub fn commit_file_diff(path: String, id: String, file: String) -> Result<FileDiff> {
    let repo = open(&path)?;
    let oid = Oid::from_str(&id)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };

    let mut opts = DiffOptions::new();
    opts.pathspec(&file);
    opts.context_lines(3);
        apply_ignore_ws(&mut opts);
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;
    collect_file_diff(&diff, &file, false)
}

/// Check out a specific commit (detached HEAD).
#[tauri::command]
pub async fn checkout_commit(path: String, id: String) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let oid = Oid::from_str(&id)?;
        let obj = repo.find_object(oid, None)?;
        repo.checkout_tree(&obj, None)?;
        repo.set_head_detached(oid)?;
        Ok(())
    })
    .await
}

/// Create a branch at a commit, optionally checking it out.
#[tauri::command]
pub async fn create_branch(path: String, name: String, id: String, checkout: bool) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        // Accept a raw SHA or a revspec like "HEAD" so we can branch off the
        // current tip without the caller resolving it first.
        let commit = match Oid::from_str(&id) {
            Ok(oid) => repo.find_commit(oid).ok(),
            Err(_) => repo.revparse_single(&id).ok().and_then(|o| o.peel_to_commit().ok()),
        };
        let full = format!("refs/heads/{name}");
        match commit {
            Some(commit) => {
                repo.branch(&name, &commit, false)?;
                if checkout {
                    let obj = repo.revparse_single(&full)?;
                    repo.checkout_tree(&obj, None)?;
                    repo.set_head(&full)?;
                }
            }
            // Unborn HEAD (no commits yet): there's no commit to point at, so
            // just move the unborn branch — matches `git switch -c <name>` on a
            // fresh repo. The branch is born on the first commit.
            None => repo.set_head(&full)?,
        }
        Ok(())
    })
    .await
}

/// Reset the current branch to a revision. `mode` is soft | mixed | hard.
/// Accepts a sha or a revspec like "HEAD~1".
#[tauri::command]
pub async fn reset(path: String, revspec: String, mode: String) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let obj = repo.revparse_single(&revspec)?;
        let kind = match mode.as_str() {
            "soft" => git2::ResetType::Soft,
            "hard" => git2::ResetType::Hard,
            _ => git2::ResetType::Mixed,
        };
        repo.reset(&obj, kind, None)?;
        Ok(())
    })
    .await
}

/// Discard working-tree changes for the given paths (restore from HEAD;
/// untracked files matching are removed). Destructive — confirm in the UI.
#[tauri::command]
pub async fn discard_paths(path: String, paths: Vec<String>) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.force().remove_untracked(true);
        for p in &paths {
            cb.path(p.as_str());
        }
        repo.checkout_head(Some(&mut cb))?;
        Ok(())
    })
    .await
}

/// Delete a local branch by name.
#[tauri::command]
pub fn delete_branch(path: String, name: String) -> Result<()> {
    let repo = open(&path)?;
    let mut branch = repo.find_branch(&name, BranchType::Local)?;
    branch.delete()?;
    Ok(())
}

/// Check out a local branch by name, moving HEAD to it.
#[tauri::command]
pub async fn checkout_branch(path: String, name: String) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let (obj, reference) = repo.revparse_ext(&name)?;
        repo.checkout_tree(&obj, None)?;
        match reference {
            Some(r) => {
                let ref_name = r
                    .name()
                    .ok_or_else(|| GitError::Message("branch has no valid name".into()))?;
                repo.set_head(ref_name)?;
            }
            None => repo.set_head_detached(obj.id())?,
        }
        Ok(())
    })
    .await
}

/// Check out a remote-tracking branch (e.g. "origin/main") by creating a local
/// branch of the same short name that tracks it — or switching to that local
/// branch if it already exists. Mirrors `git checkout main` when only the
/// remote branch is present.
#[tauri::command]
pub async fn checkout_remote_branch(path: String, remote_branch: String) -> Result<String> {
    spawn(move || {
        let repo = open(&path)?;
        // Strip the remote name ("origin/") to get the local branch name.
        let local = remote_branch
            .split_once('/')
            .map(|(_, b)| b.to_string())
            .unwrap_or_else(|| remote_branch.clone());
        if repo.find_branch(&local, BranchType::Local).is_ok() {
            run_git(&path, &["checkout", &local])?;
        } else {
            run_git(&path, &["checkout", "-b", &local, "--track", &remote_branch])?;
        }
        Ok(format!("Checked out {local}"))
    })
    .await
}

/// Create an empty first commit on an unborn branch, so the branch is born and
/// history/branching become available. Deliberately does NOT stage working
/// files — the user stages and commits those normally afterwards. Routes
/// through the user's `git` so their identity and any signing config apply.
#[tauri::command]
pub async fn initial_commit(path: String, message: String) -> Result<String> {
    spawn(move || {
        run_git(&path, &["commit", "--allow-empty", "-m", &message])?;
        Ok("Initial commit created".into())
    })
    .await
}

/// Open the repo folder in the system terminal (macOS: Terminal.app).
#[tauri::command]
pub fn open_in_terminal(path: String) -> Result<()> {
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").args(["-a", "Terminal", &path]).status();
    #[cfg(not(target_os = "macos"))]
    let status = std::process::Command::new("x-terminal-emulator").current_dir(&path).status();
    status
        .map_err(|e| GitError::Message(format!("Couldn't open terminal: {e}")))
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err(GitError::Message("Terminal exited with an error.".into()))
            }
        })
}

/// Open the repo (or a file) in an editor — VS Code if present, else the
/// default app.
#[tauri::command]
pub fn open_in_editor(path: String) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let vscode = std::process::Command::new("open")
            .args(["-a", "Visual Studio Code", &path])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !vscode {
            std::process::Command::new("open")
                .arg(&path)
                .status()
                .map_err(|e| GitError::Message(format!("Couldn't open: {e}")))?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = path;
    Ok(())
}

/// Append a pattern to the repo's .gitignore (deduplicated).
#[tauri::command]
pub fn add_to_gitignore(path: String, pattern: String) -> Result<()> {
    let repo = open(&path)?;
    let root = repo.workdir().ok_or_else(|| GitError::Message("No working directory.".into()))?;
    let gi = root.join(".gitignore");
    let mut content = std::fs::read_to_string(&gi).unwrap_or_default();
    let pat = pattern.trim();
    if pat.is_empty() || content.lines().any(|l| l.trim() == pat) {
        return Ok(());
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(pat);
    content.push('\n');
    std::fs::write(&gi, content).map_err(|e| GitError::Message(format!("Couldn't write .gitignore: {e}")))?;
    Ok(())
}

/// Change a commit's message. HEAD is a plain amend; an older commit is reworded
/// via a non-interactive rebase (aborts cleanly if it would conflict).
#[tauri::command]
pub async fn reword_commit(path: String, id: String, message: String) -> Result<String> {
    spawn(move || {
        let repo = open(&path)?;
        let head = repo.head().ok().and_then(|h| h.target());
        let target = Oid::from_str(&id).ok();

        if head.is_some() && head == target {
            let commit = repo.find_commit(target.unwrap())?;
            commit.amend(Some("HEAD"), None, None, None, Some(message.trim_end()), None)?;
            return Ok("Message updated".into());
        }

        // Older commit: rebase with a sed that flips its `pick` to `reword`, and
        // a GIT_EDITOR that drops in our message file.
        let short = &id[..id.len().min(8)];
        let tmp = std::env::temp_dir().join(format!("plumb-reword-{}.txt", std::process::id()));
        std::fs::write(&tmp, message.trim_end())
            .map_err(|e| GitError::Message(format!("Couldn't stage message: {e}")))?;
        let seq_editor = format!("sed -i '' -e 's/^pick {short}/reword {short}/'");
        let msg_editor = format!("cp {}", tmp.display());
        let base = format!("{id}^");

        let output = std::process::Command::new("git")
            .current_dir(&path)
            .args(["rebase", "-i", &base])
            .env("GIT_SEQUENCE_EDITOR", &seq_editor)
            .env("GIT_EDITOR", &msg_editor)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| GitError::Message(format!("Couldn't run git: {e}")))?;
        let _ = std::fs::remove_file(&tmp);

        if output.status.success() {
            return Ok("Message updated".into());
        }
        // Non-zero: if it paused (conflict), abort so we don't strand the repo.
        if open(&path)?.state() != git2::RepositoryState::Clean {
            let _ = std::process::Command::new("git").current_dir(&path).args(["rebase", "--abort"]).output();
            return Err(GitError::Message(
                "Rewording that commit would require resolving a rebase conflict — use interactive rebase instead.".into(),
            ));
        }
        Err(GitError::Message(String::from_utf8_lossy(&output.stderr).trim().to_string()))
    })
    .await
}

/// List installed font family names (macOS CoreText), sorted, with Apple's
/// hidden system fonts (leading ".") filtered out.
#[tauri::command]
pub fn list_system_fonts() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        let names = core_text::font_manager::copy_available_font_family_names();
        let mut out: Vec<String> = names
            .iter()
            .map(|n| n.to_string())
            .filter(|s| !s.starts_with('.'))
            .collect();
        out.sort_by_key(|s| s.to_lowercase());
        out.dedup();
        out
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

/// List branch names on a remote URL without cloning (`git ls-remote --heads`).
/// Best-effort: fails fast if the remote needs credentials we can't supply.
#[tauri::command]
pub async fn list_remote_branches(url: String) -> Result<Vec<String>> {
    spawn(move || {
        let output = std::process::Command::new("git")
            .current_dir(std::env::temp_dir())
            .args(["ls-remote", "--heads", &url])
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| GitError::Message(format!("Couldn't run git: {e}")))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::Message(if err.trim().is_empty() {
                "Couldn't reach that remote.".into()
            } else {
                err.trim().to_string()
            }));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let branches = stdout
            .lines()
            .filter_map(|l| l.split_once('\t').map(|(_, r)| r))
            .filter_map(|r| r.strip_prefix("refs/heads/"))
            .map(|s| s.to_string())
            .collect();
        Ok(branches)
    })
    .await
}

/// Attach an unborn repo to an existing remote branch: wire up `origin`, fetch
/// the branch, and check it out — leaving the working-tree files in place as
/// uncommitted changes on top of that branch's history.
#[tauri::command]
pub async fn connect_remote_branch(path: String, url: String, branch: String) -> Result<String> {
    spawn(move || {
        {
            let repo = open(&path)?;
            if repo.find_remote("origin").is_ok() {
                run_git(&path, &["remote", "set-url", "origin", &url])?;
            } else {
                run_git(&path, &["remote", "add", "origin", &url])?;
            }
        }
        // Fetch the branch if it exists; tolerate an empty remote (no refs) by
        // falling back to a full fetch, which succeeds with nothing.
        if run_git(&path, &["fetch", "origin", &branch]).is_err() {
            let _ = run_git(&path, &["fetch", "origin"]);
        }

        // Re-open so libgit2 sees any refs the fetch just wrote.
        let repo = open(&path)?;
        if repo.find_reference(&format!("refs/remotes/origin/{branch}")).is_ok() {
            // Remote branch exists — base our work on its tip, keeping working files.
            run_git(&path, &["checkout", "-B", &branch, &format!("origin/{branch}")])?;
            Ok(format!("Now on {branch}, tracking origin/{branch}"))
        } else if repo.head().is_ok() {
            // Remote is empty but we already have commits: just name our branch.
            run_git(&path, &["checkout", "-B", &branch])?;
            Ok(format!("Connected to origin. On {branch} — push to create it there."))
        } else {
            // Empty remote and unborn local: point HEAD at the branch. The first
            // commit + push (which sets upstream) creates it on the remote.
            repo.set_head(&format!("refs/heads/{branch}"))?;
            Ok(format!("Connected to origin. On {branch} — commit and push to create it."))
        }
    })
    .await
}

/// The effective commit identity for a repo (local config falling back to
/// global), plus whether commit signing is enabled in config.
#[derive(Serialize)]
pub struct GitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
    pub signing: bool,
}

/// Read the effective commit identity and signing setting for a repo.
#[tauri::command]
pub fn git_identity(path: String) -> Result<GitIdentity> {
    let repo = open(&path)?;
    let cfg = repo.config()?;
    let clean = |s: String| Some(s).filter(|s| !s.trim().is_empty());
    Ok(GitIdentity {
        name: cfg.get_string("user.name").ok().and_then(clean),
        email: cfg.get_string("user.email").ok().and_then(clean),
        signing: cfg.get_bool("commit.gpgsign").unwrap_or(false),
    })
}

/// Set user.name / user.email, either in this repo's config or globally.
#[tauri::command]
pub fn set_git_identity(path: String, name: String, email: String, global: bool) -> Result<()> {
    let mut cfg = if global {
        git2::Config::open_default()?
    } else {
        open(&path)?.config()?
    };
    cfg.set_str("user.name", name.trim())?;
    cfg.set_str("user.email", email.trim())?;
    Ok(())
}

/// Create a commit from the current index. Returns the new commit id. When
/// `sign` is set we route through the user's `git` so their GPG/SSH signing
/// config (and any signing key) is honoured — libgit2 can't sign on its own.
#[tauri::command]
pub fn commit(
    path: String,
    message: String,
    amend: bool,
    sign_off: bool,
    sign: bool,
) -> Result<CommitResult> {
    let repo = open(&path)?;

    let msg_trimmed = message.trim_end().to_string();
    if msg_trimmed.is_empty() {
        return Err(GitError::Message("Commit message is empty.".into()));
    }

    if sign {
        let mut args: Vec<String> = vec!["commit".into(), "-S".into(), "-m".into(), msg_trimmed];
        if amend {
            args.push("--amend".into());
        }
        if sign_off {
            args.push("-s".into());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_git(&path, &refs)?;
        let oid = repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .ok_or_else(|| GitError::Message("Commit created but HEAD is unreadable.".into()))?;
        return Ok(CommitResult { id: oid.to_string(), short_id: short(&oid) });
    }

    let sig = repo
        .signature()
        .map_err(|_| GitError::Message(
            "No commit identity configured. Set your name and email first.".into(),
        ))?;

    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let mut msg = msg_trimmed;
    if sign_off {
        msg.push_str(&format!(
            "\n\nSigned-off-by: {} <{}>",
            sig.name().unwrap_or(""),
            sig.email().unwrap_or("")
        ));
    }

    let head_commit = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());

    let new_oid = if amend {
        let commit = head_commit
            .ok_or_else(|| GitError::Message("Nothing to amend — no HEAD commit.".into()))?;
        commit.amend(Some("HEAD"), Some(&sig), Some(&sig), None, Some(&msg), Some(&tree))?
    } else {
        let parents: Vec<&git2::Commit> = head_commit.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &parents)?
    };

    Ok(CommitResult {
        id: new_oid.to_string(),
        short_id: short(&new_oid),
    })
}

/// Unstage everything. With a HEAD, that's a mixed reset to HEAD; on an unborn
/// branch (no commit to reset to) we simply clear the index — files become
/// untracked again, working tree untouched.
#[tauri::command]
pub async fn unstage_all(path: String) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        match repo.head().ok().and_then(|h| h.peel_to_commit().ok()) {
            Some(commit) => {
                repo.reset(commit.as_object(), git2::ResetType::Mixed, None)?;
            }
            None => {
                let mut index = repo.index()?;
                index.clear()?;
                index.write()?;
            }
        }
        Ok(())
    })
    .await
}

/// Undo the last commit, keeping its changes staged (a soft reset). Handles the
/// root commit — which has no parent — by moving the branch back to unborn.
#[tauri::command]
pub async fn uncommit(path: String) -> Result<()> {
    spawn(move || {
        let repo = open(&path)?;
        let head = repo
            .head()
            .map_err(|_| GitError::Message("No commit to undo.".into()))?;
        let commit = head.peel_to_commit()?;
        if commit.parent_count() > 0 {
            let parent = commit.parent(0)?;
            repo.reset(parent.as_object(), git2::ResetType::Soft, None)?;
        } else {
            // Root commit: delete the branch ref so HEAD is unborn again. The
            // index keeps the committed tree, so the changes stay staged.
            let branch = head.shorthand().unwrap_or("main").to_string();
            let full = format!("refs/heads/{branch}");
            repo.set_head(&full)?;
            if let Ok(mut r) = repo.find_reference(&full) {
                r.delete()?;
            }
        }
        Ok(())
    })
    .await
}

/* ── Remote sync (delegated to the system `git`) ──────────────────── */
//
// libgit2's bundled libssh2 authenticates against SSH agents and key files
// inconsistently (passphrase-protected keys, ed25519 via agent, etc.), so a
// remote that works perfectly from the terminal can still fail here. Rather
// than reimplement the user's SSH stack, we shell out to their own `git` for
// network operations — it already honours the SSH agent, `~/.ssh/config`,
// `known_hosts`, and any credential helpers exactly as their terminal does.
// All local reads/writes stay on libgit2.

fn run_git(dir: &str, args: &[&str]) -> Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        // Never block on an interactive credential prompt — fail fast instead.
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| GitError::Message(format!("Couldn't run git: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let msg = if stdout.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        Ok(msg.to_string())
    } else {
        let msg = stderr.trim();
        Err(GitError::Message(if msg.is_empty() {
            "git command failed".into()
        } else {
            msg.to_string()
        }))
    }
}

/// Run a blocking closure off the main thread so the UI stays responsive.
async fn spawn<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| GitError::Message(format!("Task failed: {e}")))?
}

/// Fetch all remotes (pruning deleted branches).
#[tauri::command]
pub async fn fetch(path: String) -> Result<String> {
    spawn(move || {
        let out = run_git(&path, &["fetch", "--all", "--prune"])?;
        Ok(if out.is_empty() { "Fetched".into() } else { out })
    })
    .await
}

/// Pull the current branch's upstream (fast-forward or merge, no editor).
#[tauri::command]
pub async fn pull(path: String) -> Result<String> {
    spawn(move || {
        let out = run_git(&path, &["pull", "--no-edit"])?;
        Ok(if out.is_empty() { "Pulled".into() } else { out })
    })
    .await
}

/// Push the current branch. If it has no upstream yet, set one to origin.
#[tauri::command]
pub async fn push(path: String) -> Result<String> {
    spawn(move || match run_git(&path, &["push"]) {
        Ok(out) => Ok(if out.is_empty() { "Pushed".into() } else { out }),
        Err(GitError::Message(msg))
            if msg.contains("no upstream") || msg.contains("set-upstream") =>
        {
            // First push of a new branch — publish it to origin and track it.
            let out = run_git(&path, &["push", "--set-upstream", "origin", "HEAD"])?;
            Ok(if out.is_empty() {
                "Pushed & set upstream".into()
            } else {
                out
            })
        }
        Err(e) => Err(e),
    })
    .await
}

/// Push with explicit options. `remote` defaults to the tracked upstream (or
/// origin when setting upstream). Covers force-with-lease, tags, and first-push.
#[tauri::command]
pub async fn push_advanced(
    path: String,
    remote: Option<String>,
    force_with_lease: bool,
    push_tags: bool,
    set_upstream: bool,
) -> Result<String> {
    spawn(move || {
        let mut args: Vec<String> = vec!["push".into()];
        if force_with_lease {
            args.push("--force-with-lease".into());
        }
        if push_tags {
            args.push("--tags".into());
        }
        if set_upstream {
            args.push("--set-upstream".into());
            args.push(remote.clone().unwrap_or_else(|| "origin".into()));
            args.push("HEAD".into());
        } else if let Some(r) = &remote {
            args.push(r.clone());
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = run_git(&path, &refs)?;
        Ok(if out.is_empty() { "Pushed".into() } else { out })
    })
    .await
}

/// Push a specific local branch to origin and set upstream (works even if the
/// branch isn't the current HEAD). Used before opening a PR/MR.
#[tauri::command]
pub async fn push_branch(path: String, branch: String) -> Result<String> {
    spawn(move || {
        let out = run_git(&path, &["push", "--set-upstream", "origin", &branch])?;
        Ok(if out.is_empty() { format!("Pushed {branch}") } else { out })
    })
    .await
}

/// Pull with an explicit integration mode: "merge", "rebase", or "ff-only".
#[tauri::command]
pub async fn pull_mode(path: String, mode: String) -> Result<String> {
    spawn(move || {
        let args: &[&str] = match mode.as_str() {
            "rebase" => &["pull", "--rebase"],
            "ff-only" => &["pull", "--ff-only"],
            _ => &["pull", "--no-edit"],
        };
        let out = run_git(&path, args)?;
        Ok(if out.is_empty() { "Pulled".into() } else { out })
    })
    .await
}

/// One line of a rebase plan: an action and the commit it applies to.
#[derive(Deserialize)]
pub struct RebaseStep {
    pub action: String, // pick | reword | squash | fixup | drop
    pub sha: String,
}

/// Run an interactive rebase from `base` (a revspec; None → --root) applying
/// the given plan. We drive the real `git rebase -i` by feeding it our todo
/// list through GIT_SEQUENCE_EDITOR, so reorder/squash/fixup/drop all work
/// exactly as git implements them. Messages are kept as-is (GIT_EDITOR=true).
/// If the rebase pauses on a conflict we report it rather than erroring, so the
/// in-progress banner + conflict resolver can take over.
#[tauri::command]
pub async fn rebase_interactive(
    path: String,
    base: Option<String>,
    steps: Vec<RebaseStep>,
) -> Result<String> {
    spawn(move || {
        let mut body = String::new();
        for s in &steps {
            let action = match s.action.as_str() {
                "reword" | "squash" | "fixup" | "drop" | "edit" | "pick" => s.action.as_str(),
                _ => "pick",
            };
            // `drop` lines are honoured by git; keep the sha for readability.
            body.push_str(&format!("{action} {}\n", s.sha));
        }

        let todo = std::env::temp_dir().join(format!("plumb-rebase-{}.txt", std::process::id()));
        std::fs::write(&todo, &body)
            .map_err(|e| GitError::Message(format!("Couldn't write rebase plan: {e}")))?;

        // GIT_SEQUENCE_EDITOR is invoked as `sh -c "<value> <todofile>"`, so
        // `cp <ourtodo>` overwrites git's generated todo with our plan.
        let seq_editor = format!("cp {}", todo.display());

        let base_arg = base.clone();
        let mut args: Vec<&str> = vec!["rebase", "-i"];
        match base_arg.as_deref() {
            Some(b) if !b.is_empty() => args.push(b),
            _ => args.push("--root"),
        }

        let output = std::process::Command::new("git")
            .current_dir(&path)
            .args(&args)
            .env("GIT_SEQUENCE_EDITOR", &seq_editor)
            .env("GIT_EDITOR", "true")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| GitError::Message(format!("Couldn't run git: {e}")))?;

        let _ = std::fs::remove_file(&todo);

        if output.status.success() {
            return Ok("Rebased".into());
        }

        // Non-zero exit: either a real failure or a normal conflict pause.
        let repo = open(&path)?;
        if repo.state() != git2::RepositoryState::Clean {
            return Ok("Rebase paused — resolve conflicts, then Continue.".into());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(GitError::Message(if stderr.trim().is_empty() {
            "Interactive rebase failed.".into()
        } else {
            stderr.trim().to_string()
        }))
    })
    .await
}

/* ── Submodules ───────────────────────────────────────────────────── */

#[derive(Serialize)]
pub struct SubmoduleInfo {
    pub name: String,
    pub path: String,
    pub url: String,
    /// Commit the superproject pins vs. what's actually checked out.
    pub pinned_id: Option<String>,
    pub wd_id: Option<String>,
    pub initialized: bool,
    pub modified: bool,
}

#[tauri::command]
pub fn list_submodules(path: String) -> Result<Vec<SubmoduleInfo>> {
    let repo = open(&path)?;
    let mut out = Vec::new();
    for sm in repo.submodules()? {
        let name = sm.name().unwrap_or("").to_string();
        let status = repo.submodule_status(&name, git2::SubmoduleIgnore::None).ok();
        let modified = status
            .map(|s| {
                s.intersects(
                    git2::SubmoduleStatus::WD_MODIFIED
                        | git2::SubmoduleStatus::WD_INDEX_MODIFIED
                        | git2::SubmoduleStatus::WD_WD_MODIFIED
                        | git2::SubmoduleStatus::WD_UNTRACKED,
                )
            })
            .unwrap_or(false);
        out.push(SubmoduleInfo {
            name,
            path: sm.path().to_string_lossy().to_string(),
            url: sm.url().unwrap_or("").to_string(),
            pinned_id: sm.head_id().map(|o| o.to_string()),
            wd_id: sm.workdir_id().map(|o| o.to_string()),
            initialized: sm.open().is_ok(),
            modified,
        });
    }
    Ok(out)
}

/// Init + update submodules (optionally recursively) to their pinned commits.
#[tauri::command]
pub async fn update_submodules(path: String, init: bool) -> Result<String> {
    spawn(move || {
        let mut args: Vec<&str> = vec!["submodule", "update"];
        if init {
            args.push("--init");
        }
        args.push("--recursive");
        run_git(&path, &args)?;
        Ok("Submodules updated".into())
    })
    .await
}

/* ── Worktrees ────────────────────────────────────────────────────── */

#[derive(Serialize)]
pub struct WorktreeInfo {
    pub path: String,
    pub head: String,
    pub branch: String,
    pub is_main: bool,
}

#[tauri::command]
pub async fn list_worktrees(path: String) -> Result<Vec<WorktreeInfo>> {
    spawn(move || {
        let out = run_git(&path, &["worktree", "list", "--porcelain"])?;
        let mut trees: Vec<WorktreeInfo> = Vec::new();
        let mut first = true;
        for line in out.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                trees.push(WorktreeInfo {
                    path: p.to_string(),
                    head: String::new(),
                    branch: "(detached)".into(),
                    is_main: first,
                });
                first = false;
            } else if let Some(h) = line.strip_prefix("HEAD ") {
                if let Some(t) = trees.last_mut() {
                    t.head = short(&Oid::from_str(h).unwrap_or_else(|_| Oid::zero()));
                }
            } else if let Some(b) = line.strip_prefix("branch ") {
                if let Some(t) = trees.last_mut() {
                    t.branch = b.trim_start_matches("refs/heads/").to_string();
                }
            }
        }
        Ok(trees)
    })
    .await
}

/// Add a worktree at `new_path`. If `new_branch`, create `branch` there.
#[tauri::command]
pub async fn add_worktree(path: String, new_path: String, branch: String, new_branch: bool) -> Result<String> {
    spawn(move || {
        let mut args: Vec<&str> = vec!["worktree", "add"];
        if new_branch {
            args.push("-b");
            args.push(&branch);
            args.push(&new_path);
        } else {
            args.push(&new_path);
            args.push(&branch);
        }
        run_git(&path, &args)?;
        Ok(format!("Worktree added at {new_path}"))
    })
    .await
}

#[tauri::command]
pub async fn remove_worktree(path: String, worktree_path: String) -> Result<String> {
    spawn(move || {
        run_git(&path, &["worktree", "remove", "--force", &worktree_path])?;
        Ok("Worktree removed".into())
    })
    .await
}

/* ── Bisect ───────────────────────────────────────────────────────── */

#[derive(Serialize)]
pub struct BisectStatus {
    pub active: bool,
    pub current: Option<String>,
    pub current_short: Option<String>,
}

#[tauri::command]
pub fn bisect_status(path: String) -> Result<BisectStatus> {
    let repo = open(&path)?;
    let active = repo.path().join("BISECT_LOG").exists();
    let (current, current_short) = match repo.head().ok().and_then(|h| h.target()) {
        Some(o) => (Some(o.to_string()), Some(short(&o))),
        None => (None, None),
    };
    Ok(BisectStatus { active, current, current_short })
}

/// Begin a bisect between a known-bad and known-good commit; returns git's hint.
#[tauri::command]
pub async fn bisect_start(path: String, bad: String, good: String) -> Result<String> {
    spawn(move || {
        run_git(&path, &["bisect", "start"])?;
        run_git(&path, &["bisect", "bad", &bad])?;
        run_git(&path, &["bisect", "good", &good])
    })
    .await
}

/// Mark the current commit good/bad/skip; returns git's next step or the result.
#[tauri::command]
pub async fn bisect_mark(path: String, verdict: String) -> Result<String> {
    spawn(move || {
        let v = match verdict.as_str() {
            "good" => "good",
            "bad" => "bad",
            "skip" => "skip",
            _ => return Err(GitError::Message("verdict must be good/bad/skip".into())),
        };
        run_git(&path, &["bisect", v])
    })
    .await
}

#[tauri::command]
pub async fn bisect_reset(path: String) -> Result<String> {
    spawn(move || {
        run_git(&path, &["bisect", "reset"])?;
        Ok("Bisect ended".into())
    })
    .await
}

/// Delete a branch on a remote (`git push <remote> --delete <branch>`).
#[tauri::command]
pub async fn delete_remote_branch(path: String, remote: String, branch: String) -> Result<String> {
    spawn(move || {
        run_git(&path, &["push", &remote, "--delete", &branch])?;
        Ok(format!("Deleted {remote}/{branch}"))
    })
    .await
}

/// Rename a remote.
#[tauri::command]
pub fn rename_remote(path: String, from: String, to: String) -> Result<()> {
    let repo = open(&path)?;
    repo.remote_rename(&from, &to)?;
    Ok(())
}

/// Remove a remote.
#[tauri::command]
pub fn remove_remote(path: String, name: String) -> Result<()> {
    let repo = open(&path)?;
    repo.remote_delete(&name)?;
    Ok(())
}

/// Change a remote's URL.
#[tauri::command]
pub fn set_remote_url(path: String, name: String, url: String) -> Result<()> {
    let repo = open(&path)?;
    repo.remote_set_url(&name, &url)?;
    Ok(())
}

/// Prune remote-tracking refs that no longer exist on the remote.
#[tauri::command]
pub async fn prune_remote(path: String, name: String) -> Result<String> {
    spawn(move || {
        run_git(&path, &["remote", "prune", &name])?;
        Ok(format!("Pruned {name}"))
    })
    .await
}

/// Build a map of commit oid -> ref labels pointing at it.
fn build_ref_map(repo: &Repository) -> HashMap<Oid, Vec<String>> {
    let mut map: HashMap<Oid, Vec<String>> = HashMap::new();

    // HEAD marker on whatever it resolves to.
    if let Ok(head) = repo.head() {
        if let Some(oid) = head.target() {
            let label = head
                .shorthand()
                .map(|s| format!("HEAD → {s}"))
                .unwrap_or_else(|| "HEAD".into());
            map.entry(oid).or_default().push(label);
        }
    }

    if let Ok(refs) = repo.references() {
        for r in refs.flatten() {
            let Some(oid) = r.target() else { continue };
            if r.is_branch() {
                if let Some(name) = r.shorthand() {
                    map.entry(oid).or_default().push(name.to_string());
                }
            } else if r.is_remote() {
                if let Some(name) = r.shorthand() {
                    map.entry(oid).or_default().push(name.to_string());
                }
            } else if r.is_tag() {
                if let Some(name) = r.shorthand() {
                    map.entry(oid).or_default().push(format!("tag: {name}"));
                }
            }
        }
    }
    map
}

fn short(oid: &Oid) -> String {
    oid.to_string().chars().take(7).collect()
}
