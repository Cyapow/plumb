//! `plumb serve` — a loopback HTTP RPC server exposing the same commands the
//! desktop app calls over Tauri IPC, so an embedded webview (VS Code / JetBrains)
//! or a plain browser tab can drive the real Plumb frontend.
//!
//! Local-only and token-gated by construction: bound to 127.0.0.1 on an
//! ephemeral port, every request must carry the per-session token, and the Host
//! header must be loopback (defeats DNS-rebinding). CORS is granted only to
//! loopback / webview origins. Event streaming over WebSocket lands next.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Listener, Manager};

use crate::{accounts, ai, git, watcher};

/// One long-poll subscriber. Events queue in the channel between polls so none
/// are lost; `last` drives idle cleanup.
struct ClientState {
    tx: mpsc::Sender<Value>,
    rx: Mutex<mpsc::Receiver<Value>>,
    last: Mutex<Instant>,
}
type Clients = Arc<Mutex<HashMap<String, Arc<ClientState>>>>;

/// The built frontend, embedded so `plumb serve` can host the app itself — the
/// browser tab / editor webview loads it straight from the server, no dev server.
static DIST: include_dir::Dir<'_> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/../dist");

/// Per-session server context shared with every request handler.
struct Ctx {
    token: String,
    port: u16,
    repo: Option<String>,
    clients: Clients,
}

/// Where the running agent advertises itself so editors can find and reuse it
/// instead of spawning their own server.
fn discovery_path() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        return Some(std::path::PathBuf::from(home).join("Library/Application Support/plumb/serve.json"));
    }
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("APPDATA").ok()?;
        return Some(std::path::PathBuf::from(base).join("plumb").join("serve.json"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.config")))?;
        return Some(std::path::PathBuf::from(base).join("plumb").join("serve.json"));
    }
    #[allow(unreachable_code)]
    None
}

fn write_discovery(port: u16, token: &str) {
    if let Some(p) = discovery_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let body = json!({ "port": port, "token": token, "pid": std::process::id() }).to_string();
        if std::fs::write(&p, body.as_bytes()).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
}

/// Remove the advertisement (best effort) when the agent shuts down cleanly.
pub fn clear_discovery() {
    if let Some(p) = discovery_path() {
        let _ = std::fs::remove_file(p);
    }
}

/// Random hex token minted per server session.
fn gen_token() -> String {
    let mut b = [0u8; 24];
    let _ = getrandom::getrandom(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Start the RPC server on a background thread. Prints a machine-readable
/// `PLUMB_SERVE port=<port> token=<token>` line the launching editor parses.
pub fn start(app: AppHandle, repo: Option<String>) {
    let token = std::env::var("PLUMB_SERVE_TOKEN").unwrap_or_else(|_| gen_token());
    let server = match tiny_http::Server::http("127.0.0.1:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("plumb serve: could not bind 127.0.0.1: {e}");
            return;
        }
    };
    let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(0);
    println!("PLUMB_SERVE port={port} token={token}");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    write_discovery(port, &token);

    // Forward the events the app emits into every subscriber's queue.
    let clients: Clients = Default::default();
    for name in ["repo-changed", "ai-explain-chunk", "menu-action"] {
        let subs = clients.clone();
        app.listen(name, move |event| {
            let payload: Value = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
            let msg = json!({ "event": name, "payload": payload });
            if let Ok(map) = subs.lock() {
                for c in map.values() {
                    let _ = c.tx.send(msg.clone());
                }
            }
        });
    }
    // Prune subscribers that stopped polling.
    {
        let subs = clients.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            if let Ok(mut map) = subs.lock() {
                map.retain(|_, c| c.last.lock().map(|t| t.elapsed() < Duration::from_secs(90)).unwrap_or(false));
            }
        });
    }

    let ctx = std::sync::Arc::new(Ctx { token, port, repo, clients });
    // One thread per request: the frontend fires many commands at once, and a
    // single slow one (e.g. a network call) must not stall the others.
    let server = std::sync::Arc::new(server);
    std::thread::spawn(move || loop {
        match server.recv() {
            Ok(req) => {
                let (app, ctx) = (app.clone(), ctx.clone());
                std::thread::spawn(move || handle(&app, &ctx, req));
            }
            Err(_) => break,
        }
    });
}

fn header<'a>(req: &'a tiny_http::Request, name: &'static str) -> Option<&'a str> {
    req.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str())
}

/// Only loopback dev servers and editor webviews may make cross-origin calls.
fn allowed_origin(origin: &str) -> bool {
    origin == "null"
        || origin.starts_with("http://localhost")
        || origin.starts_with("http://127.0.0.1")
        || origin.starts_with("https://localhost")
        || origin.starts_with("vscode-webview://")
        || origin.starts_with("jar:")
}

fn hdr(name: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn respond(req: tiny_http::Request, status: u16, body: Value, origin: Option<&str>) {
    let mut resp = tiny_http::Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(hdr("Content-Type", "application/json"));
    if let Some(o) = origin {
        resp = resp
            .with_header(hdr("Access-Control-Allow-Origin", o))
            .with_header(hdr("Access-Control-Allow-Headers", "authorization, x-plumb-token, content-type"))
            .with_header(hdr("Access-Control-Allow-Methods", "POST, OPTIONS"))
            .with_header(hdr("Vary", "Origin"));
    }
    let _ = req.respond(resp);
}

fn respond_bytes(req: tiny_http::Request, status: u16, bytes: Vec<u8>, content_type: &str, origin: Option<&str>) {
    let mut resp = tiny_http::Response::from_data(bytes)
        .with_status_code(status)
        .with_header(hdr("Content-Type", content_type));
    if let Some(o) = origin {
        resp = resp.with_header(hdr("Access-Control-Allow-Origin", o)).with_header(hdr("Vary", "Origin"));
    }
    let _ = req.respond(resp);
}

fn mime_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Serve the embedded frontend. The index page gets the serve config injected so
/// the app's transport talks to this server (token never travels in the URL).
fn serve_static(req: tiny_http::Request, ctx: &Ctx, cors: Option<&str>) {
    let raw = req.url().split('?').next().unwrap_or("/").to_string();
    let rel = if raw == "/" || raw.is_empty() { "index.html".to_string() } else { raw.trim_start_matches('/').to_string() };
    match DIST.get_file(&rel) {
        Some(f) if rel == "index.html" => {
            let html = String::from_utf8_lossy(f.contents());
            let repo_json = ctx
                .repo
                .as_deref()
                .and_then(|r| serde_json::to_string(r).ok())
                .unwrap_or_else(|| "null".into());
            let inject = format!(
                "<script>window.__PLUMB__={{port:{},token:\"{}\",repo:{}}};</script>",
                ctx.port, ctx.token, repo_json
            );
            let out = if html.contains("<head>") {
                html.replacen("<head>", &format!("<head>{inject}"), 1)
            } else {
                format!("{inject}{html}")
            };
            respond_bytes(req, 200, out.into_bytes(), "text/html; charset=utf-8", cors);
        }
        Some(f) => respond_bytes(req, 200, f.contents().to_vec(), mime_for(&rel), cors),
        None => respond(req, 404, json!({ "error": "not found" }), cors),
    }
}

fn handle(app: &AppHandle, ctx: &Ctx, mut req: tiny_http::Request) {
    // Grant CORS back only to an allowed origin; capture it before consuming req.
    let cors = header(&req, "origin").filter(|o| allowed_origin(o)).map(|o| o.to_string());
    let cors = cors.as_deref();

    // Preflight.
    if req.method() == &tiny_http::Method::Options {
        return respond(req, 204, json!({}), cors);
    }
    // Event stream via long-poll: block up to 25s for queued events, return
    // them, and the client immediately re-polls. Token + client id come as query
    // params (fetch could set headers, but this keeps the client trivial;
    // loopback + Host checks keep it local-only).
    if req.method() == &tiny_http::Method::Get && req.url().split('?').next() == Some("/events") {
        let query = req.url().splitn(2, '?').nth(1).unwrap_or("").to_string();
        let param = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(&format!("{k}="))).unwrap_or("").to_string();
        if param("token") != ctx.token {
            return respond(req, 401, json!({ "error": "unauthorized" }), cors);
        }
        let id = param("id");
        if id.is_empty() {
            return respond(req, 400, json!({ "error": "missing client id" }), cors);
        }
        let state = {
            let mut map = match ctx.clients.lock() {
                Ok(m) => m,
                Err(_) => return respond(req, 500, json!({ "error": "server busy" }), cors),
            };
            map.entry(id)
                .or_insert_with(|| {
                    let (tx, rx) = mpsc::channel::<Value>();
                    Arc::new(ClientState { tx, rx: Mutex::new(rx), last: Mutex::new(Instant::now()) })
                })
                .clone()
        };
        if let Ok(mut t) = state.last.lock() {
            *t = Instant::now();
        }
        let mut events: Vec<Value> = Vec::new();
        if let Ok(rx) = state.rx.lock() {
            if let Ok(first) = rx.recv_timeout(Duration::from_secs(25)) {
                events.push(first);
                while let Ok(e) = rx.try_recv() {
                    events.push(e);
                }
            }
        }
        return respond(req, 200, json!({ "events": events }), cors);
    }
    // Any other GET is a request for the embedded frontend.
    if req.method() == &tiny_http::Method::Get {
        return serve_static(req, ctx, cors);
    }
    if req.method() != &tiny_http::Method::Post || req.url() != "/rpc" {
        return respond(req, 404, json!({ "error": "not found" }), cors);
    }
    // Loopback Host only — blocks DNS-rebinding from a foreign page.
    if let Some(h) = header(&req, "host") {
        if !(h.starts_with("127.0.0.1") || h.starts_with("localhost")) {
            return respond(req, 403, json!({ "error": "non-loopback host rejected" }), cors);
        }
    }
    // Per-session token, as a Bearer header or x-plumb-token.
    let authed = header(&req, "authorization").map(|v| v == format!("Bearer {}", ctx.token)).unwrap_or(false)
        || header(&req, "x-plumb-token").map(|v| v == ctx.token).unwrap_or(false);
    if !authed {
        return respond(req, 401, json!({ "error": "unauthorized" }), cors);
    }

    let mut body = String::new();
    if req.as_reader().read_to_string(&mut body).is_err() {
        return respond(req, 400, json!({ "error": "could not read body" }), cors);
    }
    let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let command = parsed["command"].as_str().unwrap_or("").to_string();
    let args = parsed.get("args").cloned().unwrap_or_else(|| json!({}));

    // Mirror `invoke`: 200 {ok} on success, {error} on failure.
    match dispatch(app, &command, &args) {
        Ok(v) => respond(req, 200, json!({ "ok": v }), cors),
        Err(e) => respond(req, 200, json!({ "error": e }), cors),
    }
}

/// Open a URL/path in the OS default handler, or reveal a file in the file
/// manager — run on the agent's (i.e. the user's) machine, so served-mode
/// "open link" / "reveal in Finder" work like the desktop app.
fn os_open(target: &str, reveal: bool) {
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        if reveal {
            c.arg("-R");
        }
        let _ = c.arg(target).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        if reveal {
            let _ = std::process::Command::new("explorer").arg(format!("/select,{target}")).spawn();
        } else {
            let _ = std::process::Command::new("cmd").args(["/C", "start", "", target]).spawn();
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let t = if reveal {
            std::path::Path::new(target).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| target.to_string())
        } else {
            target.to_string()
        };
        let _ = std::process::Command::new("xdg-open").arg(t).spawn();
    }
}

/// Serialize any command `Result` for the wire.
fn ok<T: serde::Serialize, E: std::fmt::Display>(r: std::result::Result<T, E>) -> Result<Value, String> {
    r.map_err(|x| x.to_string())
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
}
fn okv<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// Route a command name + (camelCase, as the frontend sends) args to the same
/// core the desktop app calls. Coverage grows toward the full 135.
fn dispatch(app: &AppHandle, command: &str, args: &Value) -> Result<Value, String> {
    use tauri::async_runtime::block_on;
    let s = |k: &str| args[k].as_str().unwrap_or_default().to_string();
    let sopt = |k: &str| args[k].as_str().map(String::from);
    let b = |k: &str| args[k].as_bool().unwrap_or(false);
    let uz = |k: &str| args[k].as_u64().unwrap_or(0) as usize;
    let su64 = |k: &str| args[k].as_u64().unwrap_or(0);
    let vs = |k: &str| {
        args[k]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let uzs = |k: &str| {
        args[k]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as usize)).collect::<Vec<usize>>())
            .unwrap_or_default()
    };

    match command {
        // ── Read / render path ──
        "is_repo" => okv(git::is_repo(s("path"))),
        "open_repo" => ok(git::open_repo(s("path"))),
        "list_commits" => ok(git::list_commits(
            s("path"),
            args["limit"].as_u64().map(|n| n as usize),
            args["skip"].as_u64().map(|n| n as usize),
        )),
        "list_branches" => ok(git::list_branches(s("path"))),
        "working_status" => ok(git::working_status(s("path"))),
        "list_tags" => ok(git::list_tags(s("path"))),
        "list_stashes" => ok(git::list_stashes(s("path"))),
        "list_remotes" => ok(git::list_remotes(s("path"))),
        "list_files" => ok(tauri::async_runtime::block_on(git::list_files(s("path")))),
        "commit_details" => ok(git::commit_details(s("path"), s("id"))),
        "commit_file_diff" => ok(git::commit_file_diff(s("path"), s("id"), s("file"))),
        "file_diff" => ok(git::file_diff(s("path"), s("file"), b("staged"))),
        "repo_state" => ok(git::repo_state(s("path"))),
        "bisect_status" => ok(git::bisect_status(s("path"))),
        "git_identity" => ok(git::git_identity(s("path"))),
        "list_conflicts" => ok(git::list_conflicts(s("path"))),
        "reflog" => ok(git::reflog(s("path"))),
        "file_history" => ok(tauri::async_runtime::block_on(git::file_history(s("path"), s("file")))),
        "blame_file" => ok(tauri::async_runtime::block_on(git::blame_file(s("path"), s("file")))),
        "search_commits" => ok(tauri::async_runtime::block_on(git::search_commits(
            s("path"),
            s("query"),
            s("mode"),
            args["limit"].as_u64().map(|n| n as usize),
        ))),
        "get_config" => ok(git::get_config(s("path"), vs("keys"))),
        "compare_refs" => ok(git::compare_refs(s("path"), s("base"), s("compare"))),
        "compare_file_diff" => ok(git::compare_file_diff(s("path"), s("base"), s("compare"), s("file"))),

        // ── Sync / network actions ──
        "fetch" => ok(tauri::async_runtime::block_on(git::fetch(s("path")))),
        "pull" => ok(tauri::async_runtime::block_on(git::pull(s("path")))),
        "push" => ok(tauri::async_runtime::block_on(git::push(s("path")))),
        "commit" => ok(git::commit(s("path"), s("message"), b("amend"), b("signOff"), b("sign"))),
        "stage_paths" => ok(git::stage_paths(s("path"), vs("paths"))),
        "unstage_paths" => ok(git::unstage_paths(s("path"), vs("paths"))),
        "unstage_all" => ok(tauri::async_runtime::block_on(git::unstage_all(s("path")))),
        "discard_paths" => ok(tauri::async_runtime::block_on(git::discard_paths(s("path"), vs("paths")))),
        "uncommit" => ok(tauri::async_runtime::block_on(git::uncommit(s("path")))),
        "reset" => ok(tauri::async_runtime::block_on(git::reset(s("path"), s("revspec"), s("mode")))),

        // ── Branch / integrate ──
        "checkout_branch" => ok(tauri::async_runtime::block_on(git::checkout_branch(s("path"), s("name")))),
        "checkout_commit" => ok(tauri::async_runtime::block_on(git::checkout_commit(s("path"), s("id")))),
        "create_branch" => ok(tauri::async_runtime::block_on(git::create_branch(s("path"), s("name"), s("id"), b("checkout")))),
        "delete_branch" => ok(git::delete_branch(s("path"), s("name"))),
        "merge_branch_ex" => ok(tauri::async_runtime::block_on(git::merge_branch_ex(
            s("path"), s("name"), b("squash"), b("noFf"), b("noCommit"), b("verifySignatures"), b("noVerify"),
        ))),
        "rebase_branch_ex" => ok(tauri::async_runtime::block_on(git::rebase_branch_ex(s("path"), s("onto"), b("autostash"), b("noVerify")))),
        "cherry_pick" => ok(tauri::async_runtime::block_on(git::cherry_pick(s("path"), s("id")))),
        "revert_commit" => ok(tauri::async_runtime::block_on(git::revert_commit(s("path"), s("id")))),
        "op_abort" => ok(tauri::async_runtime::block_on(git::op_abort(s("path")))),
        "op_continue" => ok(tauri::async_runtime::block_on(git::op_continue(s("path")))),

        // ── Stash ──
        "stash_save_ex" => ok(tauri::async_runtime::block_on(git::stash_save_ex(s("path"), sopt("message"), b("includeUntracked"), b("keepIndex")))),
        "stash_apply_ex" => ok(tauri::async_runtime::block_on(git::stash_apply_ex(s("path"), uz("index"), b("pop"), b("restoreIndex")))),
        "stash_pop" => ok(tauri::async_runtime::block_on(git::stash_pop(s("path"), uz("index")))),
        "stash_drop" => ok(git::stash_drop(s("path"), uz("index"))),

        // ── Watcher (drives repo-changed → SSE auto-refresh) ──
        "watch_repo" => ok(watcher::watch_repo(app.clone(), app.state(), s("path"))),

        // ── Accounts (need AppHandle) ──
        "list_connections" => ok(accounts::list_connections(app.clone())),
        "pr_target" => ok(accounts::pr_target(app.clone(), s("path"))),
        "list_pull_requests" => ok(tauri::async_runtime::block_on(accounts::list_pull_requests(app.clone(), s("repoPath")))),
        "list_ci_statuses" => ok(tauri::async_runtime::block_on(accounts::list_ci_statuses(app.clone(), s("repoPath")))),

        // ── Git: config / identity / ignore / description / misc ──
        "set_config" => ok(git::set_config(s("path"), s("key"), s("value"), b("global"))),
        "unset_config" => ok(git::unset_config(s("path"), s("key"), b("global"))),
        "set_git_identity" => ok(git::set_git_identity(s("path"), s("name"), s("email"), b("global"))),
        "get_repo_description" => ok(git::get_repo_description(s("path"))),
        "set_repo_description" => ok(git::set_repo_description(s("path"), s("text"))),
        "get_gitignore" => ok(git::get_gitignore(s("path"))),
        "set_gitignore" => ok(git::set_gitignore(s("path"), s("text"))),
        "add_to_gitignore" => ok(git::add_to_gitignore(s("path"), s("pattern"))),
        "reword_commit" => ok(block_on(git::reword_commit(s("path"), s("id"), s("message")))),
        "set_diff_ignore_ws" => {
            git::set_diff_ignore_ws(b("ignore"));
            okv(Value::Null)
        }
        "list_system_fonts" => okv(git::list_system_fonts()),
        "init_repo" => ok(git::init_repo(s("path"), sopt("branch"))),
        "initial_commit" => ok(block_on(git::initial_commit(s("path"), s("message")))),
        "clone_repo" => ok(block_on(git::clone_repo(s("url"), s("parentDir")))),
        "open_in_editor" => ok(git::open_in_editor(s("path"))),
        "open_in_terminal" => ok(git::open_in_terminal(s("path"))),

        // ── Git: staging (hunk / line) ──
        "stage_hunk" => ok(block_on(git::stage_hunk(s("path"), s("file"), uz("hunkIndex")))),
        "unstage_hunk" => ok(block_on(git::unstage_hunk(s("path"), s("file"), uz("hunkIndex")))),
        "stage_lines" => ok(block_on(git::stage_lines(s("path"), s("file"), uz("hunkIndex"), uzs("lines")))),
        "unstage_lines" => ok(block_on(git::unstage_lines(s("path"), s("file"), uz("hunkIndex"), uzs("lines")))),

        // ── Git: stash (legacy variants) ──
        "stash_save" => ok(block_on(git::stash_save(s("path"), sopt("message")))),
        "stash_apply" => ok(block_on(git::stash_apply(s("path"), uz("index")))),

        // ── Git: branches / merge / rebase / remotes ──
        "merge_branch" => ok(block_on(git::merge_branch(s("path"), s("name")))),
        "rebase_branch" => ok(block_on(git::rebase_branch(s("path"), s("onto")))),
        "merge_into" => ok(block_on(git::merge_into(s("path"), s("source"), s("target"), b("deleteSource")))),
        "checkout_remote_branch" => ok(block_on(git::checkout_remote_branch(s("path"), s("remoteBranch")))),
        "connect_remote_branch" => ok(block_on(git::connect_remote_branch(s("path"), s("url"), s("branch")))),
        "delete_remote_branch" => ok(block_on(git::delete_remote_branch(s("path"), s("remote"), s("branch")))),
        "list_remote_branches" => ok(block_on(git::list_remote_branches(s("url")))),
        "push_branch" => ok(block_on(git::push_branch(s("path"), s("branch")))),
        "pull_mode" => ok(block_on(git::pull_mode(s("path"), s("mode")))),
        "push_advanced" => ok(block_on(git::push_advanced(s("path"), sopt("remote"), b("forceWithLease"), b("pushTags"), b("setUpstream")))),
        "add_remote" => ok(git::add_remote(s("path"), s("name"), s("url"))),
        "rename_remote" => ok(git::rename_remote(s("path"), s("from"), s("to"))),
        "remove_remote" => ok(git::remove_remote(s("path"), s("name"))),
        "set_remote_url" => ok(git::set_remote_url(s("path"), s("name"), s("url"))),
        "prune_remote" => ok(block_on(git::prune_remote(s("path"), s("name")))),
        "rebase_interactive" => {
            let steps: Vec<git::RebaseStep> = serde_json::from_value(args["steps"].clone()).map_err(|e| e.to_string())?;
            ok(block_on(git::rebase_interactive(s("path"), sopt("base"), steps)))
        }

        // ── Git: conflicts ──
        "conflict_sides" => ok(git::conflict_sides(s("path"), s("file"))),
        "resolve_conflict" => ok(block_on(git::resolve_conflict(s("path"), s("file"), s("side")))),
        "resolve_conflict_content" => ok(block_on(git::resolve_conflict_content(s("path"), s("file"), s("content")))),

        // ── Git: bisect / submodules / worktrees ──
        "bisect_start" => ok(block_on(git::bisect_start(s("path"), s("bad"), s("good")))),
        "bisect_mark" => ok(block_on(git::bisect_mark(s("path"), s("verdict")))),
        "bisect_reset" => ok(block_on(git::bisect_reset(s("path")))),
        "list_submodules" => ok(git::list_submodules(s("path"))),
        "update_submodules" => ok(block_on(git::update_submodules(s("path"), b("init")))),
        "list_worktrees" => ok(block_on(git::list_worktrees(s("path")))),
        "add_worktree" => ok(block_on(git::add_worktree(s("path"), s("newPath"), s("branch"), b("newBranch")))),
        "remove_worktree" => ok(block_on(git::remove_worktree(s("path"), s("worktreePath")))),

        // ── Git Flow ──
        "flow_config" => ok(git::flow_config(s("path"))),
        "flow_init" => ok(block_on(git::flow_init(s("path"), s("main"), s("develop"), s("versiontag")))),
        "flow_start" => ok(block_on(git::flow_start(s("path"), s("kind"), s("name")))),
        "flow_finish" => ok(block_on(git::flow_finish(s("path"), s("kind"), s("name"), sopt("version")))),
        "flow_set_type" => ok(git::flow_set_type(s("path"), s("workflow"))),
        "flow_set_environments" => ok(git::flow_set_environments(s("path"), s("csv"))),

        // ── AI ──
        "list_ai_providers" => ok(ai::list_ai_providers(app.clone())),
        "save_ai_provider" => {
            let provider: ai::AiProvider = serde_json::from_value(args["provider"].clone()).map_err(|e| e.to_string())?;
            ok(ai::save_ai_provider(app.clone(), provider, b("makeDefault"), sopt("apiKey")))
        }
        "save_ai_provider_from_env" => {
            let provider: ai::AiProvider = serde_json::from_value(args["provider"].clone()).map_err(|e| e.to_string())?;
            ok(ai::save_ai_provider_from_env(app.clone(), provider, s("envVar"), b("makeDefault")))
        }
        "remove_ai_provider" => ok(ai::remove_ai_provider(app.clone(), s("id"))),
        "set_default_ai_provider" => ok(ai::set_default_ai_provider(app.clone(), s("id"))),
        "has_api_key" => okv(ai::has_api_key(s("id"))),
        "list_ollama_models" => ok(block_on(ai::list_ollama_models(s("endpoint")))),
        "list_provider_models" => ok(block_on(ai::list_provider_models(s("kind"), s("vendor"), s("endpoint"), sopt("apiKey"), sopt("providerId")))),
        "detect_env_keys" => okv(block_on(ai::detect_env_keys())),
        "openrouter_login" => ok(block_on(ai::openrouter_login(app.clone()))),
        "generate_commit_message" => ok(block_on(ai::generate_commit_message(app.clone(), s("repoPath"), sopt("providerId"), b("conventional"), s("style")))),
        "explain_diff" => ok(block_on(ai::explain_diff(app.clone(), s("repoPath"), sopt("providerId"), sopt("sha")))),
        "ai_group_changes" => ok(block_on(ai::ai_group_changes(app.clone(), s("repoPath"), sopt("providerId"), b("conventional")))),
        "test_ai_provider" => ok(block_on(ai::test_ai_provider(app.clone(), s("id")))),

        // ── Accounts ──
        "connect_account" => ok(block_on(accounts::connect_account(app.clone(), s("provider"), s("baseUrl"), s("token"), sopt("label"), sopt("username")))),
        "remove_connection" => ok(accounts::remove_connection(app.clone(), s("id"))),
        "test_connection" => ok(block_on(accounts::test_connection(app.clone(), s("id")))),
        "github_device_start" => ok(block_on(accounts::github_device_start(s("clientId")))),
        "github_device_poll" => ok(block_on(accounts::github_device_poll(app.clone(), s("clientId"), s("deviceCode"), su64("interval")))),
        "gitlab_oauth_login" => ok(block_on(accounts::gitlab_oauth_login(app.clone(), s("clientId")))),
        "create_pull_request" => ok(block_on(accounts::create_pull_request(app.clone(), s("repoPath"), s("sourceBranch"), s("targetBranch"), s("title"), s("body"), b("draft")))),
        "create_remote_repo" => ok(block_on(accounts::create_remote_repo(app.clone(), s("connectionId"), s("name"), b("private")))),
        "list_account_repos" => ok(block_on(accounts::list_account_repos(app.clone(), s("connectionId")))),
        "list_workflows" => ok(block_on(accounts::list_workflows(app.clone(), s("repoPath")))),
        "trigger_pipeline" => ok(block_on(accounts::trigger_pipeline(app.clone(), s("repoPath"), s("gitRef"), sopt("workflowId")))),
        "pipeline_detail" => ok(block_on(accounts::pipeline_detail(app.clone(), s("repoPath"), s("sha")))),
        "pipeline_action" => ok(block_on(accounts::pipeline_action(app.clone(), s("repoPath"), s("id"), s("action")))),
        "list_pipelines" => ok(block_on(accounts::list_pipelines(app.clone(), s("repoPath")))),
        "job_log" => ok(block_on(accounts::job_log(app.clone(), s("repoPath"), s("jobId")))),

        // ── Native capability bridge (served-mode only) ──
        "open_url" => {
            os_open(&s("url"), false);
            okv(Value::Null)
        }
        "reveal_path" => {
            os_open(&s("path"), true);
            okv(Value::Null)
        }

        other => Err(format!("command '{other}' is not exposed over plumb serve yet")),
    }
}
