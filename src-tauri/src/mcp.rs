//! `plumb mcp` — a Model Context Protocol server over stdio, so AI assistants
//! (Claude Code / Claude Desktop, ChatGPT, Cursor, VS Code Copilot, …) can drive
//! Plumb the same way the editor panels do.
//!
//! This process is deliberately thin: it speaks JSON-RPC on stdin/stdout and
//! forwards every tool call to the running `plumb serve` agent's `/rpc`
//! endpoint (spawning the agent first if none is advertised). That keeps one
//! implementation of every command, and lets the agent's per-session token gate
//! access exactly as it does for the editor plugins. Stdout is reserved for the
//! protocol; anything diagnostic goes to stderr.

use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Protocol revisions this server understands, newest first. We echo the
/// client's version when it's one of these, else offer our newest.
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Cap on a single tool result, so a giant diff can't blow the client's context.
const MAX_RESULT_BYTES: usize = 200 * 1024;

/// A live `plumb serve` agent to forward calls to.
#[derive(Clone)]
struct Agent {
    port: u16,
    token: String,
}

/// Which argument name the underlying RPC command uses for the repository.
#[derive(Clone, Copy)]
enum RepoKey {
    Path,
    RepoPath,
}

/// One MCP tool: its public schema and the serve command it maps onto. Tool
/// argument names are the RPC argument names (camelCase), except the repository
/// which is always exposed as `path` and renamed per `repo_key`.
struct Tool {
    name: &'static str,
    command: &'static str,
    description: &'static str,
    repo_key: RepoKey,
    /// `properties` of the input schema, excluding `path`.
    props: Value,
    required: &'static [&'static str],
    read_only: bool,
    destructive: bool,
}

fn prop(ty: &str, desc: &str) -> Value {
    json!({ "type": ty, "description": desc })
}
fn enum_prop(values: &[&str], desc: &str) -> Value {
    json!({ "type": "string", "enum": values, "description": desc })
}
fn arr_prop(desc: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": desc })
}

/// The curated tool set. Reads are exhaustive; writes cover the everyday
/// workflow; anything that throws work away is flagged destructive so clients
/// can ask before running it.
fn tools() -> Vec<Tool> {
    let t = |name, command, description, repo_key, props, required, read_only, destructive| Tool {
        name,
        command,
        description,
        repo_key,
        props,
        required,
        read_only,
        destructive,
    };
    use RepoKey::*;
    vec![
        // ── Inspect ──
        t("repo_status", "working_status", "List working-tree changes: each file with its staged/unstaged status. Start here to see what's modified before staging or committing.", Path, json!({}), &[], true, false),
        t("repo_state", "repo_state", "Current HEAD, branch, upstream ahead/behind counts, and whether a merge/rebase/cherry-pick/revert is in progress.", Path, json!({}), &[], true, false),
        t("log", "list_commits", "Recent commits on all branches (newest first) with sha, subject, author, date, parents and refs.", Path,
            json!({ "limit": prop("integer", "Max commits to return (default 100)."), "skip": prop("integer", "Commits to skip, for paging.") }), &[], true, false),
        t("branches", "list_branches", "All local and remote branches with their head sha, upstream, and which one is checked out.", Path, json!({}), &[], true, false),
        t("tags", "list_tags", "All tags with the commit they point at.", Path, json!({}), &[], true, false),
        t("stashes", "list_stashes", "Stash list (index, message, sha).", Path, json!({}), &[], true, false),
        t("remotes", "list_remotes", "Configured remotes and their URLs.", Path, json!({}), &[], true, false),
        t("commit_details", "commit_details", "Full details of one commit: message, author/committer, parents, and the list of changed files with stats.", Path,
            json!({ "id": prop("string", "Commit sha or any revspec (branch, tag, HEAD~2).") }), &["id"], true, false),
        t("commit_file_diff", "commit_file_diff", "Unified diff of a single file as changed by one commit.", Path,
            json!({ "id": prop("string", "Commit sha."), "file": prop("string", "Repo-relative file path.") }), &["id", "file"], true, false),
        t("file_diff", "file_diff", "Diff of a working-tree file against the index (unstaged) or the index against HEAD (staged), with hunks and line numbers.", Path,
            json!({ "file": prop("string", "Repo-relative file path."), "staged": prop("boolean", "true for the staged diff, false (default) for unstaged.") }), &["file"], true, false),
        t("file_history", "file_history", "Commits that touched a file, newest first (follows renames).", Path,
            json!({ "file": prop("string", "Repo-relative file path.") }), &["file"], true, false),
        t("blame", "blame_file", "Line-by-line blame for a file: commit, author and date per line.", Path,
            json!({ "file": prop("string", "Repo-relative file path.") }), &["file"], true, false),
        t("search_commits", "search_commits", "Search all history by commit message, or by code change (pickaxe: commits that added/removed the text).", Path,
            json!({ "query": prop("string", "Text to search for."), "mode": enum_prop(&["message", "code"], "'message' greps commit messages (case-insensitive); 'code' finds commits whose diff contains the text."), "limit": prop("integer", "Max results (default 200).") }), &["query", "mode"], true, false),
        t("compare_refs", "compare_refs", "Compare two refs: commits in `compare` not in `base`, plus the changed files between them.", Path,
            json!({ "base": prop("string", "Base ref (branch, tag or sha)."), "compare": prop("string", "Ref to compare against base.") }), &["base", "compare"], true, false),
        t("compare_file_diff", "compare_file_diff", "Diff of one file between two refs.", Path,
            json!({ "base": prop("string", "Base ref."), "compare": prop("string", "Ref to compare."), "file": prop("string", "Repo-relative file path.") }), &["base", "compare", "file"], true, false),
        t("reflog", "reflog", "HEAD reflog — find commits lost to a bad reset or rebase.", Path, json!({}), &[], true, false),
        t("conflicts", "list_conflicts", "Files currently in conflict (during a merge/rebase/cherry-pick).", Path, json!({}), &[], true, false),
        t("conflict_sides", "conflict_sides", "The base, ours and theirs content of a conflicted file, plus the merged text with markers.", Path,
            json!({ "file": prop("string", "Repo-relative file path.") }), &["file"], true, false),
        t("git_identity", "git_identity", "The user.name / user.email that commits will be made with (local overrides global).", Path, json!({}), &[], true, false),
        t("worktrees", "list_worktrees", "Linked worktrees of this repository.", Path, json!({}), &[], true, false),
        t("submodules", "list_submodules", "Submodules and their current commits.", Path, json!({}), &[], true, false),
        t("bisect_status", "bisect_status", "Whether a bisect is in progress and its current state.", Path, json!({}), &[], true, false),
        t("workflow_config", "flow_config", "The repository's configured branching workflow (git-flow prefixes, main/develop branches, environments).", Path, json!({}), &[], true, false),

        // ── Hosting: pull requests / CI (needs an account connected in Plumb) ──
        t("pull_requests", "list_pull_requests", "Open pull / merge requests for this repository's hosting remote (GitHub or GitLab), via the account connected in Plumb.", RepoPath, json!({}), &[], true, false),
        t("ci_statuses", "list_ci_statuses", "CI status (GitHub Checks / GitLab pipelines) per recent commit.", RepoPath, json!({}), &[], true, false),
        t("pipelines", "list_pipelines", "Recent CI pipelines / workflow runs for this repository.", RepoPath, json!({}), &[], true, false),
        t("pipeline_detail", "pipeline_detail", "Jobs and stages of the pipeline for a commit.", RepoPath,
            json!({ "sha": prop("string", "Commit sha.") }), &["sha"], true, false),
        t("job_log", "job_log", "Log output of one CI job.", RepoPath,
            json!({ "jobId": prop("string", "Job id from pipeline_detail.") }), &["jobId"], true, false),
        t("create_pull_request", "create_pull_request", "Open a pull / merge request on GitHub or GitLab from one branch into another. Push the branch first.", RepoPath,
            json!({ "sourceBranch": prop("string", "Head branch."), "targetBranch": prop("string", "Base branch (e.g. main)."), "title": prop("string", "PR title."), "body": prop("string", "PR description (markdown)."), "draft": prop("boolean", "Open as a draft.") }), &["sourceBranch", "targetBranch", "title"], false, false),
        t("trigger_pipeline", "trigger_pipeline", "Run a CI pipeline (GitLab) or dispatch a workflow (GitHub) on a ref.", RepoPath,
            json!({ "gitRef": prop("string", "Branch or tag to run on."), "workflowId": prop("string", "GitHub only: workflow file name or id (see list_workflows in the app).") }), &["gitRef"], false, false),

        // ── Stage / commit ──
        t("stage", "stage_paths", "Stage files (add to the index). Use repo_status first to see candidates.", Path,
            json!({ "paths": arr_prop("Repo-relative file paths to stage.") }), &["paths"], false, false),
        t("unstage", "unstage_paths", "Unstage files (keep their working-tree changes).", Path,
            json!({ "paths": arr_prop("Repo-relative file paths to unstage.") }), &["paths"], false, false),
        t("unstage_all", "unstage_all", "Unstage everything (working-tree changes are kept).", Path, json!({}), &[], false, false),
        t("commit", "commit", "Create a commit from the staged changes. Fails if nothing is staged.", Path,
            json!({ "message": prop("string", "Commit message (subject line, blank line, optional body)."), "amend": prop("boolean", "Amend the previous commit instead of creating a new one."), "signOff": prop("boolean", "Append a Signed-off-by trailer."), "sign": prop("boolean", "GPG/SSH-sign the commit using the repo's signing config.") }), &["message"], false, false),
        t("reword_commit", "reword_commit", "Change the message of a commit (rewrites history from that commit forward — avoid on pushed commits).", Path,
            json!({ "id": prop("string", "Commit sha."), "message": prop("string", "New message.") }), &["id", "message"], false, false),
        t("generate_commit_message", "generate_commit_message", "Ask Plumb's configured AI provider to draft a commit message for the staged diff. Returns the message; it does not commit.", RepoPath,
            json!({ "conventional": prop("boolean", "Use Conventional Commits style (feat:, fix:, …)."), "style": enum_prop(&["default", "shorter", "detailed"], "Length/shape of the message.") }), &[], true, false),
        t("explain_diff", "explain_diff", "Ask Plumb's configured AI provider to explain a commit's diff, or the staged diff when no sha is given.", RepoPath,
            json!({ "sha": prop("string", "Commit sha; omit for the staged changes.") }), &[], true, false),
        t("add_to_gitignore", "add_to_gitignore", "Append a pattern to the repository's .gitignore.", Path,
            json!({ "pattern": prop("string", "Ignore pattern, e.g. `dist/` or `*.log`.") }), &["pattern"], false, false),

        // ── Branches ──
        t("create_branch", "create_branch", "Create a branch at a commit, optionally checking it out.", Path,
            json!({ "name": prop("string", "New branch name."), "id": prop("string", "Start point: sha or ref (default HEAD)."), "checkout": prop("boolean", "Switch to the new branch (default false).") }), &["name"], false, false),
        t("checkout", "checkout_branch", "Switch to a local branch. Fails on conflicting working-tree changes.", Path,
            json!({ "name": prop("string", "Local branch name.") }), &["name"], false, false),
        t("checkout_remote_branch", "checkout_remote_branch", "Create a local tracking branch from a remote branch and switch to it.", Path,
            json!({ "remoteBranch": prop("string", "Remote branch, e.g. origin/feature-x.") }), &["remoteBranch"], false, false),
        t("checkout_commit", "checkout_commit", "Check out a commit (detached HEAD).", Path,
            json!({ "id": prop("string", "Commit sha.") }), &["id"], false, false),
        t("delete_branch", "delete_branch", "Delete a local branch.", Path,
            json!({ "name": prop("string", "Local branch name.") }), &["name"], false, true),
        t("delete_remote_branch", "delete_remote_branch", "Delete a branch on a remote.", Path,
            json!({ "remote": prop("string", "Remote name, e.g. origin."), "branch": prop("string", "Branch name on the remote.") }), &["remote", "branch"], false, true),
        t("delete_tag", "delete_tag", "Delete a local tag.", Path,
            json!({ "name": prop("string", "Tag name.") }), &["name"], false, true),

        // ── Integrate ──
        t("merge", "merge_branch_ex", "Merge a branch into the current branch. On conflicts the merge stays in progress: use conflicts / resolve_conflict then continue_operation.", Path,
            json!({ "name": prop("string", "Branch to merge in."), "squash": prop("boolean", "Squash into one set of staged changes without committing."), "noFf": prop("boolean", "Always create a merge commit."), "noCommit": prop("boolean", "Merge but leave the result uncommitted."), "noVerify": prop("boolean", "Skip commit hooks.") }), &["name"], false, false),
        t("rebase", "rebase_branch_ex", "Rebase the current branch onto another ref.", Path,
            json!({ "onto": prop("string", "Ref to rebase onto."), "autostash": prop("boolean", "Stash uncommitted changes around the rebase."), "noVerify": prop("boolean", "Skip hooks.") }), &["onto"], false, false),
        t("cherry_pick", "cherry_pick", "Apply one commit onto the current branch.", Path,
            json!({ "id": prop("string", "Commit sha.") }), &["id"], false, false),
        t("revert", "revert_commit", "Create a new commit that undoes a commit.", Path,
            json!({ "id": prop("string", "Commit sha.") }), &["id"], false, false),
        t("continue_operation", "op_continue", "Continue the in-progress merge / rebase / cherry-pick / revert after resolving conflicts.", Path, json!({}), &[], false, false),
        t("abort_operation", "op_abort", "Abort the in-progress merge / rebase / cherry-pick / revert and return to the pre-operation state.", Path, json!({}), &[], false, true),
        t("resolve_conflict", "resolve_conflict", "Resolve a conflicted file by taking one side wholesale.", Path,
            json!({ "file": prop("string", "Repo-relative file path."), "side": enum_prop(&["ours", "theirs"], "Which side to keep.") }), &["file", "side"], false, false),
        t("resolve_conflict_content", "resolve_conflict_content", "Resolve a conflicted file by writing the final merged content and marking it resolved.", Path,
            json!({ "file": prop("string", "Repo-relative file path."), "content": prop("string", "Full merged file content, no conflict markers.") }), &["file", "content"], false, false),

        // ── Sync ──
        t("fetch", "fetch", "Fetch all remotes (prunes deleted remote branches).", Path, json!({}), &[], false, false),
        t("pull", "pull_mode", "Pull the current branch from its upstream.", Path,
            json!({ "mode": enum_prop(&["merge", "rebase", "ff-only"], "How to integrate upstream changes (default merge).") }), &[], false, false),
        t("push", "push_advanced", "Push the current branch to its upstream (or a named remote).", Path,
            json!({ "remote": prop("string", "Remote name (default: the branch's upstream remote, else origin)."), "setUpstream": prop("boolean", "Set the upstream (-u) for a new branch."), "pushTags": prop("boolean", "Also push tags."), "forceWithLease": prop("boolean", "Force push, refusing if the remote moved (--force-with-lease).") }), &[], false, false),

        // ── Stash ──
        t("stash_save", "stash_save_ex", "Stash working-tree changes.", Path,
            json!({ "message": prop("string", "Stash message."), "includeUntracked": prop("boolean", "Include untracked files."), "keepIndex": prop("boolean", "Leave staged changes in the index.") }), &[], false, false),
        t("stash_apply", "stash_apply_ex", "Apply a stash, optionally dropping it afterwards (pop).", Path,
            json!({ "index": prop("integer", "Stash index from stashes (0 = newest)."), "pop": prop("boolean", "Drop the stash after applying."), "restoreIndex": prop("boolean", "Also restore the staged state.") }), &["index"], false, false),
        t("stash_drop", "stash_drop", "Delete a stash without applying it.", Path,
            json!({ "index": prop("integer", "Stash index.") }), &["index"], false, true),

        // ── Undo / discard (destructive) ──
        t("discard_changes", "discard_paths", "Discard working-tree changes to files (restores from the index / HEAD; deletes untracked files). Cannot be undone.", Path,
            json!({ "paths": arr_prop("Repo-relative file paths to discard.") }), &["paths"], false, true),
        t("uncommit", "uncommit", "Undo the last commit, keeping its changes staged (git reset --soft HEAD~1).", Path, json!({}), &[], false, true),
        t("reset", "reset", "Reset the current branch to a revision. 'hard' discards all working-tree changes.", Path,
            json!({ "revspec": prop("string", "Target revision, e.g. HEAD~1 or a sha."), "mode": enum_prop(&["soft", "mixed", "hard"], "soft keeps index+worktree, mixed keeps worktree, hard discards both.") }), &["revspec", "mode"], false, true),

        // ── Worktrees / bisect ──
        t("add_worktree", "add_worktree", "Create a linked worktree checked out to a branch (optionally a new one).", Path,
            json!({ "newPath": prop("string", "Absolute directory for the new worktree."), "branch": prop("string", "Branch to check out (or create)."), "newBranch": prop("boolean", "Create the branch.") }), &["newPath", "branch"], false, false),
        t("remove_worktree", "remove_worktree", "Remove a linked worktree.", Path,
            json!({ "worktreePath": prop("string", "Absolute path of the worktree.") }), &["worktreePath"], false, true),
        t("bisect_start", "bisect_start", "Start a bisect between a known-bad and known-good commit.", Path,
            json!({ "bad": prop("string", "Bad revision."), "good": prop("string", "Good revision.") }), &["bad", "good"], false, false),
        t("bisect_mark", "bisect_mark", "Mark the current bisect commit and move to the next one.", Path,
            json!({ "verdict": enum_prop(&["good", "bad", "skip"], "Verdict for the current commit.") }), &["verdict"], false, false),
        t("bisect_reset", "bisect_reset", "End the bisect and return to the original branch.", Path, json!({}), &[], false, false),

        // ── App ──
        t("open_in_plumb", "focus_repo", "Show this repository in the Plumb window (brings the app to the front). Useful after a change so the user can review it visually.", Path, json!({}), &[], false, false),
    ]
}

fn tool_schema(t: &Tool) -> Value {
    let mut props = t.props.clone();
    props["path"] = prop("string", "Absolute path to the repository. Defaults to the repository this server was started for (its working directory).");
    json!({
        "name": t.name,
        "description": t.description,
        "inputSchema": { "type": "object", "properties": props, "required": t.required },
        "annotations": {
            "readOnlyHint": t.read_only,
            "destructiveHint": t.destructive,
            "idempotentHint": t.read_only,
            "openWorldHint": false
        }
    })
}

// ── Agent discovery / spawn ──

fn read_discovery() -> Option<Agent> {
    let text = std::fs::read_to_string(crate::serve::discovery_path()?).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let port = v["port"].as_u64()? as u16;
    let token = v["token"].as_str()?.to_string();
    if port == 0 || token.is_empty() {
        return None;
    }
    Some(Agent { port, token })
}

/// Forward one command to the agent. `Ok(value)` is the command's result;
/// `Err` is either the command's own error or a transport failure.
fn rpc(agent: &Agent, command: &str, args: &Value, timeout: Duration) -> Result<Value, String> {
    let resp = ureq::post(&format!("http://127.0.0.1:{}/rpc", agent.port))
        .set("authorization", &format!("Bearer {}", agent.token))
        .set("content-type", "application/json")
        .timeout(timeout)
        .send_string(&json!({ "command": command, "args": args }).to_string())
        .map_err(|e| format!("could not reach the Plumb agent: {e}"))?;
    let body: Value = resp.into_json().map_err(|e| format!("bad reply from the Plumb agent: {e}"))?;
    if let Some(err) = body.get("error") {
        return Err(err.as_str().map(String::from).unwrap_or_else(|| err.to_string()));
    }
    Ok(body.get("ok").cloned().unwrap_or(Value::Null))
}

/// An advertised agent is only trusted once it answers with our token —
/// otherwise the file is a leftover from a crashed session.
fn live_agent() -> Option<Agent> {
    let a = read_discovery()?;
    rpc(&a, "is_repo", &json!({ "path": "/" }), Duration::from_secs(2)).ok().map(|_| a)
}

/// Find the running agent, or start one (detached, headless) and wait for it.
fn connect(repo: Option<&str>) -> Result<Agent, String> {
    if let Some(a) = live_agent() {
        return Ok(a);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("serve");
    if let Some(r) = repo {
        cmd.arg(r);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map_err(|e| format!("could not start `plumb serve`: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
        if let Some(a) = live_agent() {
            return Ok(a);
        }
    }
    Err("the Plumb agent did not start in time".into())
}

// ── JSON-RPC over stdio ──

struct Server {
    agent: Mutex<Option<Agent>>,
    default_repo: Option<String>,
    tools: Vec<Tool>,
    out: Mutex<std::io::Stdout>,
    /// Clients pipeline calls; libgit2 won't tolerate two writers on one index.
    /// Reads run concurrently, writes one at a time.
    gate: RwLock<()>,
}

impl Server {
    fn send(&self, msg: Value) {
        if let Ok(mut o) = self.out.lock() {
            let _ = writeln!(o, "{msg}");
            let _ = o.flush();
        }
    }
    fn reply(&self, id: &Value, result: Value) {
        self.send(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }
    fn fail(&self, id: &Value, code: i64, message: impl Into<String>) {
        self.send(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message.into() } }));
    }

    /// The agent, reconnecting (or respawning) if the cached one went away.
    fn agent(&self) -> Result<Agent, String> {
        if let Some(a) = self.agent.lock().ok().and_then(|g| g.clone()) {
            if rpc(&a, "is_repo", &json!({ "path": "/" }), Duration::from_secs(2)).is_ok() {
                return Ok(a);
            }
        }
        let a = connect(self.default_repo.as_deref())?;
        if let Ok(mut g) = self.agent.lock() {
            *g = Some(a.clone());
        }
        Ok(a)
    }

    fn call_tool(&self, name: &str, mut args: Value) -> Result<Value, String> {
        let tool = self.tools.iter().find(|t| t.name == name).ok_or_else(|| format!("unknown tool '{name}'"))?;
        if !args.is_object() {
            args = json!({});
        }
        // `path` is the public name; the RPC command may want `repoPath`.
        let path = args["path"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(String::from)
            .or_else(|| self.default_repo.clone())
            .ok_or("no repository: pass `path`, or start `plumb mcp` inside a repository")?;
        let path = std::fs::canonicalize(&path).map(|p| p.to_string_lossy().to_string()).unwrap_or(path);
        let obj = args.as_object_mut().unwrap();
        obj.remove("path");
        let key = match tool.repo_key {
            RepoKey::RepoPath => "repoPath",
            RepoKey::Path => "path",
        };
        obj.insert(key.into(), Value::String(path));
        // Per-tool defaults the RPC layer doesn't supply itself.
        if tool.name == "create_branch" && args["id"].as_str().map(str::is_empty).unwrap_or(true) {
            args["id"] = json!("HEAD");
        }
        // Network / AI calls can legitimately take a while.
        let agent = self.agent()?;
        let _r;
        let _w;
        if tool.read_only {
            _r = self.gate.read().unwrap_or_else(|e| e.into_inner());
        } else {
            _w = self.gate.write().unwrap_or_else(|e| e.into_inner());
        }
        rpc(&agent, tool.command, &args, Duration::from_secs(300))
    }

    fn handle(&self, msg: Value) {
        let id = msg.get("id").cloned();
        let method = msg["method"].as_str().unwrap_or("").to_string();
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        // Notifications (no id) never get a reply.
        let Some(id) = id else { return };
        match method.as_str() {
            "initialize" => {
                let asked = params["protocolVersion"].as_str().unwrap_or("");
                let version = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
                self.reply(&id, json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "plumb", "title": "Plumb", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Plumb is the user's Git client. Tools act on the repository given by `path` (defaults to the one this server was started in). Inspect with repo_status / log / file_diff before changing anything; tools marked destructive throw work away — confirm with the user first. After making changes, open_in_plumb shows them in the app."
                }));
            }
            "ping" => self.reply(&id, json!({})),
            "tools/list" => self.reply(&id, json!({ "tools": self.tools.iter().map(tool_schema).collect::<Vec<_>>() })),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("").to_string();
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let (text, is_error) = match self.call_tool(&name, args) {
                    Ok(Value::Null) => ("ok".to_string(), false),
                    Ok(Value::String(s)) => (s, false),
                    Ok(v) => (serde_json::to_string_pretty(&v).unwrap_or_default(), false),
                    Err(e) => (e, true),
                };
                let text = if text.len() > MAX_RESULT_BYTES {
                    let mut cut = MAX_RESULT_BYTES;
                    while !text.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    format!("{}\n…[truncated: {} bytes total]", &text[..cut], text.len())
                } else {
                    text
                };
                self.reply(&id, json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }));
            }
            // Optional capabilities we don't provide: answer with empty lists so
            // clients that probe unconditionally don't log errors.
            "resources/list" => self.reply(&id, json!({ "resources": [] })),
            "prompts/list" => self.reply(&id, json!({ "prompts": [] })),
            other => self.fail(&id, -32601, format!("method not found: {other}")),
        }
    }
}

/// Entry point for `plumb mcp [repo]`. Blocks until stdin closes.
pub fn run(args: &[String]) {
    // The repository this server is "for": first existing directory argument,
    // else PLUMB_REPO, else the working directory the client launched us in.
    let default_repo = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-') && a.as_str() != "mcp")
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .find(|p| p.is_dir())
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| std::env::var("PLUMB_REPO").ok().filter(|s| !s.is_empty()))
        .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()));

    let server = Arc::new(Server {
        agent: Mutex::new(None),
        default_repo,
        tools: tools(),
        out: Mutex::new(std::io::stdout()),
        gate: RwLock::new(()),
    });

    // Warm the agent connection so the first tool call doesn't pay the spawn
    // cost; failures are reported per call, not fatal here.
    {
        let s = server.clone();
        std::thread::spawn(move || {
            if let Err(e) = s.agent() {
                eprintln!("plumb mcp: {e}");
            }
        });
    }

    let stdin = std::io::stdin();
    let mut in_flight: Vec<std::thread::JoinHandle<()>> = Vec::new();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                server.fail(&Value::Null, -32700, format!("parse error: {e}"));
                continue;
            }
        };
        // Clients pipeline requests; a slow push must not block a tools/list.
        let s = server.clone();
        in_flight.retain(|h| !h.is_finished());
        in_flight.push(std::thread::spawn(move || s.handle(msg)));
    }
    // stdin closed: let outstanding calls answer before exiting.
    for h in in_flight {
        let _ = h.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_are_unique_and_required_keys_exist() {
        let ts = tools();
        let mut seen = std::collections::HashSet::new();
        for t in &ts {
            assert!(seen.insert(t.name), "duplicate tool {}", t.name);
            for r in t.required {
                assert!(t.props.get(*r).is_some(), "tool {} requires unknown prop {r}", t.name);
            }
        }
    }

    #[test]
    fn schema_exposes_path_for_repo_tools() {
        let ts = tools();
        let s = tool_schema(&ts[0]);
        assert_eq!(s["inputSchema"]["properties"]["path"]["type"], "string");
        assert_eq!(s["annotations"]["readOnlyHint"], true);
    }
}
