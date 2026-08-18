//! `plumb serve` — a loopback HTTP RPC server exposing the same commands the
//! desktop app calls over Tauri IPC, so an embedded webview (VS Code / JetBrains)
//! can drive the real Plumb frontend.
//!
//! Local-only and token-gated by construction: bound to 127.0.0.1 on an
//! ephemeral port, every request must carry the per-session token, and the Host
//! header must be loopback (defeats DNS-rebinding). This is the Phase-2
//! foundation: request/response `POST /rpc` with a command dispatch that grows
//! toward full parity; event streaming over WebSocket lands next.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::git;

/// Random hex token minted per server session.
fn gen_token() -> String {
    let mut b = [0u8; 24];
    let _ = getrandom::getrandom(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Start the RPC server on a background thread. Prints a machine-readable
/// `PLUMB_SERVE port=<port> token=<token>` line the launching editor parses.
pub fn start(app: AppHandle) {
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

    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            handle(&app, &token, req);
        }
    });
}

fn header<'a>(req: &'a tiny_http::Request, name: &'static str) -> Option<&'a str> {
    req.headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str())
}

fn respond(req: tiny_http::Request, status: u16, body: Value) {
    let ct = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    let resp = tiny_http::Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(ct);
    let _ = req.respond(resp);
}

fn handle(app: &AppHandle, token: &str, mut req: tiny_http::Request) {
    // Only the RPC endpoint; a bare GET / is a liveness probe.
    if req.url() == "/" {
        return respond(req, 200, json!({ "service": "plumb", "ok": true }));
    }
    if req.method() != &tiny_http::Method::Post || req.url() != "/rpc" {
        return respond(req, 404, json!({ "error": "not found" }));
    }
    // Loopback Host only — blocks DNS-rebinding from a foreign page.
    if let Some(h) = header(&req, "host") {
        if !(h.starts_with("127.0.0.1") || h.starts_with("localhost")) {
            return respond(req, 403, json!({ "error": "non-loopback host rejected" }));
        }
    }
    // Per-session token, as a Bearer header or x-plumb-token.
    let authed = header(&req, "authorization").map(|v| v == format!("Bearer {token}")).unwrap_or(false)
        || header(&req, "x-plumb-token").map(|v| v == token).unwrap_or(false);
    if !authed {
        return respond(req, 401, json!({ "error": "unauthorized" }));
    }

    let mut body = String::new();
    if req.as_reader().read_to_string(&mut body).is_err() {
        return respond(req, 400, json!({ "error": "could not read body" }));
    }
    let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let command = parsed["command"].as_str().unwrap_or("").to_string();
    let args = parsed.get("args").cloned().unwrap_or_else(|| json!({}));

    // Mirror `invoke`: 200 with {ok} on success, {error} on failure; the JS
    // transport resolves the first and rejects the second.
    match dispatch(app, &command, &args) {
        Ok(v) => respond(req, 200, json!({ "ok": v })),
        Err(e) => respond(req, 200, json!({ "error": e })),
    }
}

/// Serialize a command's `Result` for the wire.
fn ok<T: serde::Serialize>(r: git::Result<T>) -> Result<Value, String> {
    r.map_err(|x| x.to_string())
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
}
/// Serialize a plain (non-Result) value.
fn okv<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// Route a command name + args to the same core the desktop app calls.
/// Coverage grows toward the full 135; unexposed commands return a clear error.
fn dispatch(app: &AppHandle, command: &str, args: &Value) -> Result<Value, String> {
    let _ = app; // reserved for accounts/ai commands that need AppHandle
    let s = |k: &str| args[k].as_str().unwrap_or_default().to_string();

    match command {
        "is_repo" => okv(git::is_repo(s("path"))),
        "open_repo" => ok(git::open_repo(s("path"))),
        "list_commits" => {
            let limit = args["limit"].as_u64().map(|n| n as usize);
            let skip = args["skip"].as_u64().map(|n| n as usize);
            ok(git::list_commits(s("path"), limit, skip))
        }
        "list_branches" => ok(git::list_branches(s("path"))),
        "working_status" => ok(git::working_status(s("path"))),
        "list_tags" => ok(git::list_tags(s("path"))),
        "list_stashes" => ok(git::list_stashes(s("path"))),
        "list_remotes" => ok(git::list_remotes(s("path"))),
        "list_files" => ok(tauri::async_runtime::block_on(git::list_files(s("path")))),
        "commit_details" => ok(git::commit_details(s("path"), s("id"))),
        "commit_file_diff" => ok(git::commit_file_diff(s("path"), s("id"), s("file"))),
        "file_diff" => ok(git::file_diff(s("path"), s("file"), args["staged"].as_bool().unwrap_or(false))),
        other => Err(format!("command '{other}' is not exposed over plumb serve yet")),
    }
}
