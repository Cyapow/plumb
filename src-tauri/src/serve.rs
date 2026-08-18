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

use crate::{accounts, git, watcher};

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
    let s = |k: &str| args[k].as_str().unwrap_or_default().to_string();
    let sopt = |k: &str| args[k].as_str().map(String::from);
    let b = |k: &str| args[k].as_bool().unwrap_or(false);
    let uz = |k: &str| args[k].as_u64().unwrap_or(0) as usize;
    let vs = |k: &str| {
        args[k]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect::<Vec<_>>())
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

        other => Err(format!("command '{other}' is not exposed over plumb serve yet")),
    }
}
