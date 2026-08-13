//! Multi-account connections to GitHub, GitLab, and Azure DevOps (incl.
//! self-managed/enterprise).
//!
//! Connections are a *list* — the headline differentiator over GitKraken's
//! one-account-per-provider model. Each connection's token lives in the OS
//! keychain; only non-secret metadata (provider, host, username) is persisted.
//! GitHub/GitLab authenticate a token as `Bearer`; Azure DevOps sends a PAT as
//! HTTP Basic and is organisation-scoped (the org lives in the base URL).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Serialize for AccountError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

type Result<T> = std::result::Result<T, AccountError>;

/// A configured account. `provider` is "github" | "gitlab". `base_url` is the
/// API base (api.github.com, gitlab.com, or a self-managed host).
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub base_url: String,
    pub username: String,
    pub avatar_url: String,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    pub connections: Vec<Connection>,
}

const TOKEN_SERVICE: &str = "app.plumb.desktop.git";

fn token_entry(id: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(TOKEN_SERVICE, id).map_err(|e| AccountError::Msg(format!("Keychain: {e}")))
}
fn store_token(id: &str, token: &str) -> Result<()> {
    token_entry(id)?
        .set_password(token)
        .map_err(|e| AccountError::Msg(format!("Couldn't save token: {e}")))
}
fn read_token(id: &str) -> Result<String> {
    token_entry(id)?
        .get_password()
        .map_err(|_| AccountError::Msg("No token in the Keychain for this account.".into()))
}
fn delete_token(id: &str) {
    if let Ok(e) = token_entry(id) {
        let _ = e.delete_credential();
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AccountError::Msg(format!("No config dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("connections.json"))
}
fn load(app: &AppHandle) -> Result<ConnectionConfig> {
    let p = config_path(app)?;
    if !p.exists() {
        return Ok(ConnectionConfig::default());
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(p)?).unwrap_or_default())
}
fn store(app: &AppHandle, cfg: &ConnectionConfig) -> Result<()> {
    std::fs::write(config_path(app)?, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

fn new_id(prefix: &str) -> String {
    let mut b = [0u8; 8];
    let _ = getrandom::getrandom(&mut b);
    format!("{prefix}-{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b))
}

#[tauri::command]
pub fn list_connections(app: AppHandle) -> Result<ConnectionConfig> {
    load(&app)
}

#[tauri::command]
pub fn remove_connection(app: AppHandle, id: String) -> Result<ConnectionConfig> {
    delete_token(&id);
    let mut cfg = load(&app)?;
    cfg.connections.retain(|c| c.id != id);
    store(&app, &cfg)?;
    Ok(cfg)
}

/// Validate a token by fetching the authenticated user, then save the account.
#[tauri::command]
pub async fn connect_account(
    app: AppHandle,
    provider: String,
    base_url: String,
    token: String,
    label: Option<String>,
) -> Result<Connection> {
    let base = normalize_base(&provider, &base_url);
    let (username, avatar_url) = {
        let (p, b, t) = (provider.clone(), base.clone(), token.clone());
        tauri::async_runtime::spawn_blocking(move || fetch_user(&p, &b, &t))
            .await
            .map_err(|e| AccountError::Msg(e.to_string()))??
    };

    let id = new_id(&provider);
    store_token(&id, &token)?;
    let conn = Connection {
        id: id.clone(),
        provider,
        label: label.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| username.clone()),
        base_url: base,
        username,
        avatar_url,
    };
    let mut cfg = load(&app)?;
    cfg.connections.push(conn.clone());
    store(&app, &cfg)?;
    Ok(conn)
}

/// Re-validate a saved connection's token.
#[tauri::command]
pub async fn test_connection(app: AppHandle, id: String) -> Result<String> {
    let cfg = load(&app)?;
    let conn = cfg
        .connections
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or_else(|| AccountError::Msg("Connection not found.".into()))?;
    let token = read_token(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (user, _) = fetch_user(&conn.provider, &conn.base_url, &token)?;
        Ok(format!("Signed in as {user}."))
    })
    .await
    .map_err(|e| AccountError::Msg(e.to_string()))?
}

/// Default API base per provider when the user leaves it blank.
fn normalize_base(provider: &str, base_url: &str) -> String {
    let b = base_url.trim().trim_end_matches('/');
    // Azure DevOps is organisation-scoped: accept a full org URL, or just the
    // organisation name (→ https://dev.azure.com/{org}).
    if provider == "azure" {
        return if b.is_empty() {
            String::new()
        } else if b.starts_with("http") {
            b.to_string()
        } else {
            format!("https://dev.azure.com/{b}")
        };
    }
    if !b.is_empty() {
        return b.to_string();
    }
    match provider {
        "github" => "https://api.github.com".to_string(),
        _ => "https://gitlab.com".to_string(),
    }
}

/// Azure DevOps authenticates a PAT via HTTP Basic with an empty username and
/// the token as the password.
fn azure_basic(token: &str) -> String {
    let raw = format!(":{token}");
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(raw))
}

/// The organisation name a connection's base URL points at
/// (dev.azure.com/{org} or {org}.visualstudio.com).
fn azure_org_of(base_url: &str) -> String {
    let no_scheme = base_url.trim_end_matches('/').split("://").last().unwrap_or("");
    let host = no_scheme.split('/').next().unwrap_or("");
    if host == "dev.azure.com" {
        return no_scheme.split('/').nth(1).unwrap_or("").to_string();
    }
    if let Some(org) = host.strip_suffix(".visualstudio.com") {
        return org.to_string();
    }
    no_scheme.split('/').last().unwrap_or("").to_string()
}

/// Extract (org, project, repo) from an Azure DevOps remote URL's (host, path).
/// Handles dev.azure.com (HTTPS + ssh.dev.azure.com) and {org}.visualstudio.com.
fn azure_ids(host: &str, path: &str) -> Option<(String, String, String)> {
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if host == "dev.azure.com" {
        // {org}/{project}/_git/{repo}
        let git = segs.iter().position(|s| *s == "_git")?;
        let org = segs.first()?.to_string();
        let project = segs.get(git.checked_sub(1)?)?.to_string();
        let repo = segs.get(git + 1)?.to_string();
        return Some((org, project, repo));
    }
    if host == "ssh.dev.azure.com" {
        // v3/{org}/{project}/{repo}
        let i = segs.iter().position(|s| *s == "v3").map(|p| p + 1).unwrap_or(0);
        return Some((segs.get(i)?.to_string(), segs.get(i + 1)?.to_string(), segs.get(i + 2)?.to_string()));
    }
    if let Some(sub) = host.strip_suffix(".visualstudio.com") {
        // HTTPS: {project}/_git/{repo}
        if let Some(git) = segs.iter().position(|s| *s == "_git") {
            return Some((sub.to_string(), segs.get(git.checked_sub(1)?)?.to_string(), segs.get(git + 1)?.to_string()));
        }
        // SSH (vs-ssh.visualstudio.com): v3/{org}/{project}/{repo}
        let i = segs.iter().position(|s| *s == "v3").map(|p| p + 1).unwrap_or(0);
        return Some((segs.get(i)?.to_string(), segs.get(i + 1)?.to_string(), segs.get(i + 2)?.to_string()));
    }
    None
}

fn fetch_user(provider: &str, base: &str, token: &str) -> Result<(String, String)> {
    match provider {
        "github" => {
            let url = format!("{}/user", base.trim_end_matches('/'));
            let json: serde_json::Value = ureq::get(&url)
                .set("authorization", &format!("Bearer {token}"))
                .set("user-agent", "Plumb")
                .set("accept", "application/vnd.github+json")
                .timeout(Duration::from_secs(15))
                .call()
                .map_err(|e| http_err("GitHub sign-in failed", e))?
                .into_json()?;
            Ok((
                json["login"].as_str().unwrap_or("").to_string(),
                json["avatar_url"].as_str().unwrap_or("").to_string(),
            ))
        }
        "gitlab" => {
            // Bearer works for both OAuth access tokens and personal access
            // tokens; PRIVATE-TOKEN only works for PATs (OAuth tokens 401).
            let url = format!("{}/api/v4/user", base.trim_end_matches('/'));
            let json: serde_json::Value = ureq::get(&url)
                .set("authorization", &format!("Bearer {token}"))
                .timeout(Duration::from_secs(15))
                .call()
                .map_err(|e| http_err("GitLab sign-in failed", e))?
                .into_json()?;
            Ok((
                json["username"].as_str().unwrap_or("").to_string(),
                json["avatar_url"].as_str().unwrap_or("").to_string(),
            ))
        }
        "azure" => {
            // PAT via Basic auth; validate against the org's connectionData.
            if base.trim().is_empty() {
                return Err(AccountError::Msg("Enter your Azure DevOps organisation.".into()));
            }
            let url = format!(
                "{}/_apis/connectionData?connectOptions=none&api-version=7.1-preview.1",
                base.trim_end_matches('/')
            );
            let json: serde_json::Value = ureq::get(&url)
                .set("authorization", &azure_basic(token))
                .set("accept", "application/json")
                .timeout(Duration::from_secs(15))
                .call()
                .map_err(|e| http_err("Azure DevOps sign-in failed", e))?
                .into_json()?;
            let u = &json["authenticatedUser"];
            let name = u["providerDisplayName"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| u["customDisplayName"].as_str())
                .unwrap_or("")
                .to_string();
            Ok((name, String::new()))
        }
        other => Err(AccountError::Msg(format!("Unknown provider '{other}'."))),
    }
}

/// Find the connected account serving this remote (host, path), returning it
/// with the provider-specific repo id the API expects: "owner/repo" for GitHub,
/// "group/project" for GitLab, "project/repo" for Azure (the org is already in
/// the connection's base URL).
fn match_conn<'a>(cfg: &'a ConnectionConfig, host: &str, path: &str) -> Option<(&'a Connection, String)> {
    for c in &cfg.connections {
        if c.provider == "azure" {
            if let Some((org, project, repo)) = azure_ids(host, path) {
                if azure_org_of(&c.base_url).eq_ignore_ascii_case(&org) {
                    return Some((c, format!("{project}/{repo}")));
                }
            }
        } else if conn_web_host(c) == host {
            return Some((c, path.to_string()));
        }
    }
    None
}

/* ── OAuth (no central server) ────────────────────────────────────── */
// GitHub uses the Device Flow (enter a code in the browser — no redirect URI).
// GitLab uses Authorization Code + PKCE with a fixed loopback redirect.
// Both need only a public client_id from an OAuth app the user registers.

/// Fixed loopback port for the GitLab redirect (must match the OAuth app's
/// registered redirect URI: http://127.0.0.1:47823/callback).
const GITLAB_REDIRECT_PORT: u16 = 47823;

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}
fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
fn open_browser(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

fn persist_connection(
    app: &AppHandle,
    provider: &str,
    base_url: &str,
    token: &str,
    username: String,
    avatar: String,
) -> Result<Connection> {
    let id = new_id(provider);
    store_token(&id, token)?;
    let conn = Connection {
        id,
        provider: provider.to_string(),
        label: username.clone(),
        base_url: base_url.to_string(),
        username,
        avatar_url: avatar,
    };
    let mut cfg = load(app)?;
    cfg.connections.push(conn.clone());
    store(app, &cfg)?;
    Ok(conn)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
}

/// Begin GitHub Device Flow: returns the code the user types in the browser.
#[tauri::command]
pub async fn github_device_start(client_id: String) -> Result<DeviceCode> {
    tauri::async_runtime::spawn_blocking(move || {
        let json: serde_json::Value = ureq::post("https://github.com/login/device/code")
            .set("accept", "application/json")
            .send_form(&[("client_id", client_id.as_str()), ("scope", "repo read:user")])
            .map_err(|e| http_err("GitHub device code request failed", e))?
            .into_json()?;
        Ok(DeviceCode {
            device_code: json["device_code"].as_str().unwrap_or("").to_string(),
            user_code: json["user_code"].as_str().unwrap_or("").to_string(),
            verification_uri: json["verification_uri"]
                .as_str()
                .unwrap_or("https://github.com/login/device")
                .to_string(),
            interval: json["interval"].as_u64().unwrap_or(5),
        })
    })
    .await
    .map_err(|e| AccountError::Msg(e.to_string()))?
}

/// Poll GitHub until the user authorises, then validate + save the account.
#[tauri::command]
pub async fn github_device_poll(
    app: AppHandle,
    client_id: String,
    device_code: String,
    interval: u64,
) -> Result<Connection> {
    let (token, username, avatar) =
        tauri::async_runtime::spawn_blocking(move || -> Result<(String, String, String)> {
            let token = poll_github(&client_id, &device_code, interval)?;
            let (u, a) = fetch_user("github", "https://api.github.com", &token)?;
            Ok((token, u, a))
        })
        .await
        .map_err(|e| AccountError::Msg(e.to_string()))??;
    persist_connection(&app, "github", "https://api.github.com", &token, username, avatar)
}

fn poll_github(client_id: &str, device_code: &str, interval: u64) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(900);
    let mut wait = interval.max(5);
    loop {
        if Instant::now() > deadline {
            return Err(AccountError::Msg("GitHub login timed out.".into()));
        }
        std::thread::sleep(Duration::from_secs(wait));
        let json: serde_json::Value = ureq::post("https://github.com/login/oauth/access_token")
            .set("accept", "application/json")
            .send_form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .map_err(|e| http_err("GitHub token request failed", e))?
            .into_json()?;
        if let Some(tok) = json["access_token"].as_str() {
            return Ok(tok.to_string());
        }
        match json["error"].as_str() {
            Some("authorization_pending") => continue,
            Some("slow_down") => wait += 5,
            Some(other) => return Err(AccountError::Msg(format!("GitHub: {other}"))),
            None => return Err(AccountError::Msg("GitHub returned no token.".into())),
        }
    }
}

/// GitLab OAuth (Authorization Code + PKCE via loopback), then validate + save.
#[tauri::command]
pub async fn gitlab_oauth_login(app: AppHandle, client_id: String) -> Result<Connection> {
    let base = "https://gitlab.com";
    let (token, username, avatar) =
        tauri::async_runtime::spawn_blocking(move || -> Result<(String, String, String)> {
            let token = gitlab_pkce(&client_id, base)?;
            let (u, a) = fetch_user("gitlab", base, &token)?;
            Ok((token, u, a))
        })
        .await
        .map_err(|e| AccountError::Msg(e.to_string()))??;
    persist_connection(&app, "gitlab", base, &token, username, avatar)
}

fn gitlab_pkce(client_id: &str, base: &str) -> Result<String> {
    let mut vb = [0u8; 32];
    getrandom::getrandom(&mut vb).map_err(|e| AccountError::Msg(format!("rng: {e}")))?;
    let verifier = b64url(&vb);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let mut sb = [0u8; 12];
    let _ = getrandom::getrandom(&mut sb);
    let state = b64url(&sb);

    let listener = std::net::TcpListener::bind(("127.0.0.1", GITLAB_REDIRECT_PORT)).map_err(|e| {
        AccountError::Msg(format!(
            "Couldn't open the callback port {GITLAB_REDIRECT_PORT}: {e}"
        ))
    })?;
    let redirect = format!("http://127.0.0.1:{GITLAB_REDIRECT_PORT}/callback");
    eprintln!("[plumb oauth] gitlab: listening on {redirect}");

    let auth_url = format!(
        "{base}/oauth/authorize?client_id={cid}&redirect_uri={ru}&response_type=code&scope=api&state={st}&code_challenge={ch}&code_challenge_method=S256",
        cid = urlencode(client_id),
        ru = urlencode(&redirect),
        st = state,
        ch = challenge,
    );
    open_browser(&auth_url);

    let code = wait_for_code(listener, &state)?;
    eprintln!("[plumb oauth] gitlab: received code ({} chars), exchanging…", code.len());

    let json: serde_json::Value = ureq::post(&format!("{base}/oauth/token"))
        .set("accept", "application/json")
        .send_form(&[
            ("client_id", client_id),
            ("code", &code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", &redirect),
            ("code_verifier", &verifier),
        ])
        .map_err(|e| {
            eprintln!("[plumb oauth] gitlab: token exchange error: {e:?}");
            http_err("GitLab token exchange failed", e)
        })?
        .into_json()?;
    eprintln!("[plumb oauth] gitlab: token response keys: {:?}", json.as_object().map(|o| o.keys().collect::<Vec<_>>()));
    json["access_token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| AccountError::Msg("GitLab returned no token.".into()))
}

fn wait_for_code(listener: std::net::TcpListener, expected_state: &str) -> Result<String> {
    use std::io::{Read, Write};
    listener
        .set_nonblocking(true)
        .map_err(|e| AccountError::Msg(e.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let request_line = req.lines().next().unwrap_or("");
                eprintln!("[plumb oauth] callback request: {request_line}");
                let query = req
                    .split_whitespace()
                    .nth(1)
                    .and_then(|path| path.split('?').nth(1))
                    .unwrap_or("");
                let mut code = None;
                let mut state = None;
                for kv in query.split('&') {
                    if let Some(v) = kv.strip_prefix("code=") {
                        code = Some(v.to_string());
                    } else if let Some(v) = kv.strip_prefix("state=") {
                        state = Some(v.to_string());
                    }
                }
                let body = "<html><body style='font-family:-apple-system,sans-serif;padding:48px;text-align:center'><h2>Connected to Plumb</h2><p>You can close this tab and return to the app.</p></body></html>";
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.flush();
                if state.as_deref() != Some(expected_state) {
                    return Err(AccountError::Msg("OAuth state mismatch — try again.".into()));
                }
                return code.ok_or_else(|| AccountError::Msg("No authorization code returned.".into()));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err(AccountError::Msg("GitLab login timed out.".into()));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(AccountError::Msg(format!("Callback error: {e}"))),
        }
    }
}

/* ── Pull / merge requests ────────────────────────────────────────── */

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub author_avatar: String,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub url: String,
    pub updated_at: String,
    pub provider: String,
    pub assignees: Vec<String>,
    pub reviewers: Vec<String>,
    /// Rolled-up CI status of the head commit: "success" | "failure" |
    /// "pending" | "" (none/unknown).
    pub ci_status: String,
    pub head_sha: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrList {
    /// "ok" | "no_remote" | "no_account"
    pub status: String,
    pub provider: Option<String>,
    pub host: Option<String>,
    /// The authenticated username of the matched account (for the "you" filters).
    pub username: Option<String>,
    pub items: Vec<PullRequest>,
}

fn usernames(v: &serde_json::Value, key: &str, field: &str) -> Vec<String> {
    v[key]
        .as_array()
        .map(|a| a.iter().filter_map(|u| u[field].as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// (name, url) for each remote of a repo.
fn repo_remotes(path: &str) -> Vec<(String, String)> {
    let repo = match git2::Repository::open(path) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    if let Ok(names) = repo.remotes() {
        for name in names.iter().flatten() {
            if let Ok(remote) = repo.find_remote(name) {
                if let Some(url) = remote.url() {
                    out.push((name.to_string(), url.to_string()));
                }
            }
        }
    }
    out
}

/// Extract (host, "owner/repo" or "group/subgroup/project") from a remote URL.
fn parse_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let clean = |p: &str| p.trim_end_matches(".git").trim_end_matches('/').to_string();

    // scp-like: git@host:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            return Some((host.to_string(), clean(path)));
        }
    }
    // https://host/owner/repo.git  or  ssh://git@host:port/owner/repo.git
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"));
    if let Some(s) = stripped {
        let s = s.split_once('@').map(|(_, h)| h).unwrap_or(s);
        if let Some((host_port, path)) = s.split_once('/') {
            let host = host_port.split(':').next().unwrap_or(host_port);
            return Some((host.to_string(), clean(path)));
        }
    }
    None
}

/// The web host a connection serves (github.com for api.github.com, else its host).
fn conn_web_host(c: &Connection) -> String {
    let base_host = c
        .base_url
        .replace("https://", "")
        .replace("http://", "")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();
    if c.provider == "github" && base_host == "api.github.com" {
        "github.com".to_string()
    } else {
        base_host
    }
}

/// Open pull/merge requests for the repo's remote, via a matching account.
#[tauri::command]
pub async fn list_pull_requests(app: AppHandle, repo_path: String) -> Result<PrList> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    if remotes.is_empty() {
        return Ok(PrList {
            status: "no_remote".into(),
            provider: None,
            host: None,
            username: None,
            items: vec![],
        });
    }

    let mut matched_host: Option<String> = None;
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            matched_host.get_or_insert(host.clone());
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                let token = read_token(&conn.id)?;
                let provider = conn.provider.clone();
                let base = conn.base_url.clone();
                let items = tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
                    "github" => github_prs(&base, &token, &repo_id),
                    "gitlab" => gitlab_mrs(&base, &token, &repo_id),
                    "azure" => azure_prs(&base, &token, &repo_id),
                    _ => Ok(Vec::new()),
                })
                .await
                .map_err(|e| AccountError::Msg(e.to_string()))??;
                return Ok(PrList {
                    status: "ok".into(),
                    provider: Some(conn.provider.clone()),
                    host: Some(host),
                    username: Some(conn.username.clone()),
                    items,
                });
            }
        }
    }
    Ok(PrList {
        status: "no_account".into(),
        provider: None,
        host: matched_host,
        username: None,
        items: vec![],
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiStatus {
    pub sha: String,
    pub status: String, // "success" | "failure" | "pending"
}

/// Recent CI results keyed by commit SHA, for badges in the graph. One batched
/// call per provider (workflow runs / pipelines), not one per commit.
#[tauri::command]
pub async fn list_ci_statuses(app: AppHandle, repo_path: String) -> Result<Vec<CiStatus>> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                let token = read_token(&conn.id)?;
                let (provider, base) = (conn.provider.clone(), conn.base_url.clone());
                return tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
                    "github" => Ok(github_ci_map(&base, &token, &repo_id)),
                    "gitlab" => Ok(gitlab_ci_map(&base, &token, &urlencode(&repo_id))),
                    "azure" => Ok(azure_ci_map(&base, &token, &repo_id)),
                    _ => Ok(Vec::new()),
                })
                .await
                .map_err(|e| AccountError::Msg(e.to_string()))?;
            }
        }
    }
    Ok(Vec::new())
}

fn roll(pending: bool, fail: bool, success: bool) -> Option<String> {
    if pending {
        Some("pending".into())
    } else if fail {
        Some("failure".into())
    } else if success {
        Some("success".into())
    } else {
        None
    }
}

fn github_ci_map(base: &str, token: &str, owner_repo: &str) -> Vec<CiStatus> {
    // (pending, fail, success) accumulated per head_sha across workflow runs.
    let mut acc: HashMap<String, (bool, bool, bool)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for page in 1..=2 {
        let url = format!(
            "{}/repos/{}/actions/runs?per_page=100&page={page}",
            base.trim_end_matches('/'),
            owner_repo
        );
        let json: serde_json::Value = match ureq::get(&url)
            .set("authorization", &format!("Bearer {token}"))
            .set("user-agent", "Plumb")
            .set("accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(20))
            .call()
        {
            Ok(r) => match r.into_json() {
                Ok(j) => j,
                Err(_) => break,
            },
            Err(_) => break,
        };
        let runs = json["workflow_runs"].as_array().cloned().unwrap_or_default();
        if runs.is_empty() {
            break;
        }
        for r in &runs {
            let sha = r["head_sha"].as_str().unwrap_or("").to_string();
            if sha.is_empty() {
                continue;
            }
            let e = acc.entry(sha.clone()).or_insert_with(|| {
                order.push(sha.clone());
                (false, false, false)
            });
            if r["status"].as_str().unwrap_or("") != "completed" {
                e.0 = true;
            } else {
                match r["conclusion"].as_str().unwrap_or("") {
                    "failure" | "timed_out" | "cancelled" | "action_required" | "startup_failure" => e.1 = true,
                    "success" => e.2 = true,
                    _ => {}
                }
            }
        }
        if runs.len() < 100 {
            break;
        }
    }
    order
        .into_iter()
        .filter_map(|sha| acc.get(&sha).and_then(|&(p, f, s)| roll(p, f, s)).map(|status| CiStatus { sha, status }))
        .collect()
}

fn gitlab_ci_map(base: &str, token: &str, project_enc: &str) -> Vec<CiStatus> {
    let mut out = Vec::new();
    let mut seen: HashMap<String, ()> = HashMap::new();
    for page in 1..=2 {
        let url = format!(
            "{}/api/v4/projects/{}/pipelines?per_page=100&page={page}",
            base.trim_end_matches('/'),
            project_enc
        );
        let arr: Vec<serde_json::Value> = match ureq::get(&url)
            .set("authorization", &format!("Bearer {token}"))
            .timeout(Duration::from_secs(20))
            .call()
        {
            Ok(r) => match r.into_json() {
                Ok(j) => j,
                Err(_) => break,
            },
            Err(_) => break,
        };
        if arr.is_empty() {
            break;
        }
        for p in &arr {
            let sha = p["sha"].as_str().unwrap_or("").to_string();
            if sha.is_empty() || seen.contains_key(&sha) {
                continue; // pipelines are newest-first, so keep the first per sha
            }
            let status = match p["status"].as_str().unwrap_or("") {
                "success" => "success",
                "failed" => "failure",
                "running" | "pending" | "created" | "waiting_for_resource" | "preparing" | "scheduled" => "pending",
                _ => continue,
            };
            seen.insert(sha.clone(), ());
            out.push(CiStatus { sha, status: status.into() });
        }
        if arr.len() < 100 {
            break;
        }
    }
    out
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineJob {
    pub id: String,
    pub name: String,
    pub stage: String,
    pub status: String, // success | failed | running | pending | canceled | skipped | manual | other
    pub web_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineDetail {
    pub id: String,
    pub name: String,
    pub status: String,
    pub web_url: String,
    pub jobs: Vec<PipelineJob>,
}

/// Pipeline(s) for a commit, with their jobs — for the detail view. GitHub can
/// have several workflow runs per commit; GitLab has one latest pipeline.
#[tauri::command]
pub async fn pipeline_detail(app: AppHandle, repo_path: String, sha: String) -> Result<Vec<PipelineDetail>> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                let token = read_token(&conn.id)?;
                let (provider, base) = (conn.provider.clone(), conn.base_url.clone());
                return tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
                    "github" => Ok(github_pipeline_detail(&base, &token, &repo_id, &sha)),
                    "gitlab" => Ok(gitlab_pipeline_detail(&base, &token, &urlencode(&repo_id), &sha)),
                    "azure" => Ok(azure_pipeline_detail(&base, &token, &repo_id, &sha)),
                    _ => Ok(Vec::new()),
                })
                .await
                .map_err(|e| AccountError::Msg(e.to_string()))?;
            }
        }
    }
    Ok(Vec::new())
}

/// Fetch a job's log (tail-truncated), for inline viewing.
#[tauri::command]
pub async fn job_log(app: AppHandle, repo_path: String, job_id: String) -> Result<String> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                let token = read_token(&conn.id)?;
                let (provider, base) = (conn.provider.clone(), conn.base_url.clone());
                return tauri::async_runtime::spawn_blocking(move || {
                    if provider == "azure" {
                        return azure_job_log(&base, &token, &repo_id, &job_id);
                    }
                    let (url, gh) = match provider.as_str() {
                        "github" => (
                            format!("{}/repos/{}/actions/jobs/{}/logs", base.trim_end_matches('/'), repo_id, job_id),
                            true,
                        ),
                        "gitlab" => (
                            format!("{}/api/v4/projects/{}/jobs/{}/trace", base.trim_end_matches('/'), urlencode(&repo_id), job_id),
                            false,
                        ),
                        _ => return Err(AccountError::Msg("Unsupported provider.".into())),
                    };
                    let mut req = ureq::get(&url).set("authorization", &format!("Bearer {token}")).timeout(Duration::from_secs(25));
                    if gh {
                        req = req.set("user-agent", "Plumb").set("accept", "application/vnd.github+json");
                    }
                    let text = req
                        .call()
                        .map_err(|e| http_err("Couldn't fetch the log", e))?
                        .into_string()
                        .map_err(|e| AccountError::Msg(e.to_string()))?;
                    // Keep the tail so huge logs stay light.
                    const MAX: usize = 200_000;
                    if text.len() > MAX {
                        let tail = &text[text.len() - MAX..];
                        Ok(format!("… (log truncated; showing last {MAX} chars) …\n{tail}"))
                    } else {
                        Ok(text)
                    }
                })
                .await
                .map_err(|e| AccountError::Msg(e.to_string()))?;
            }
        }
    }
    Err(AccountError::Msg("No connected account matches this repository's remote.".into()))
}

/// Retry or cancel a pipeline/run by id. `action` = "retry" | "cancel".
#[tauri::command]
pub async fn pipeline_action(app: AppHandle, repo_path: String, id: String, action: String) -> Result<String> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                let token = read_token(&conn.id)?;
                let (provider, base) = (conn.provider.clone(), conn.base_url.clone());
                return tauri::async_runtime::spawn_blocking(move || {
                    let (verb, ok) = match action.as_str() {
                        "retry" => ("retry", "Retrying"),
                        "cancel" => ("cancel", "Cancelling"),
                        _ => return Err(AccountError::Msg("action must be retry/cancel".into())),
                    };
                    if provider == "azure" {
                        return azure_pipeline_action(&base, &token, &repo_id, &id, verb).map(|_| format!("{ok} pipeline"));
                    }
                    let url = match provider.as_str() {
                        "github" => format!(
                            "{}/repos/{}/actions/runs/{}/{}",
                            base.trim_end_matches('/'),
                            repo_id,
                            id,
                            if verb == "retry" { "rerun" } else { "cancel" }
                        ),
                        "gitlab" => format!(
                            "{}/api/v4/projects/{}/pipelines/{}/{}",
                            base.trim_end_matches('/'),
                            urlencode(&repo_id),
                            id,
                            verb
                        ),
                        _ => return Err(AccountError::Msg("Unsupported provider.".into())),
                    };
                    let mut req = ureq::post(&url).set("authorization", &format!("Bearer {token}")).timeout(Duration::from_secs(20));
                    if provider == "github" {
                        req = req.set("user-agent", "Plumb").set("accept", "application/vnd.github+json");
                    }
                    req.call().map_err(|e| http_err("Pipeline action failed", e))?;
                    Ok(format!("{ok} pipeline"))
                })
                .await
                .map_err(|e| AccountError::Msg(e.to_string()))?;
            }
        }
    }
    Err(AccountError::Msg("No connected account matches this repository's remote.".into()))
}

fn gh_job_status(status: &str, conclusion: &str) -> String {
    if status != "completed" {
        return match status {
            "in_progress" => "running",
            _ => "pending",
        }
        .into();
    }
    match conclusion {
        "success" => "success",
        "failure" | "timed_out" | "startup_failure" => "failed",
        "cancelled" => "canceled",
        "skipped" | "neutral" => "skipped",
        "action_required" => "pending",
        _ => "other",
    }
    .into()
}

fn github_pipeline_detail(base: &str, token: &str, owner_repo: &str, sha: &str) -> Vec<PipelineDetail> {
    let get = |url: &str| -> Option<serde_json::Value> {
        ureq::get(url)
            .set("authorization", &format!("Bearer {token}"))
            .set("user-agent", "Plumb")
            .set("accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(20))
            .call()
            .ok()
            .and_then(|r| r.into_json().ok())
    };
    let runs_url = format!(
        "{}/repos/{}/actions/runs?head_sha={}&per_page=20",
        base.trim_end_matches('/'),
        owner_repo,
        sha
    );
    let runs = match get(&runs_url) {
        Some(j) => j["workflow_runs"].as_array().cloned().unwrap_or_default(),
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for r in runs {
        let run_id = r["id"].as_u64().unwrap_or(0);
        let jobs_url = format!("{}/repos/{}/actions/runs/{}/jobs?per_page=100", base.trim_end_matches('/'), owner_repo, run_id);
        let jobs = get(&jobs_url)
            .and_then(|j| j["jobs"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(|j| PipelineJob {
                id: j["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
                name: j["name"].as_str().unwrap_or("").to_string(),
                stage: String::new(),
                status: gh_job_status(j["status"].as_str().unwrap_or(""), j["conclusion"].as_str().unwrap_or("")),
                web_url: j["html_url"].as_str().unwrap_or("").to_string(),
            })
            .collect();
        out.push(PipelineDetail {
            id: run_id.to_string(),
            name: r["name"].as_str().unwrap_or("Workflow").to_string(),
            status: gh_job_status(r["status"].as_str().unwrap_or(""), r["conclusion"].as_str().unwrap_or("")),
            web_url: r["html_url"].as_str().unwrap_or("").to_string(),
            jobs,
        });
    }
    out
}

fn gitlab_pipeline_detail(base: &str, token: &str, project_enc: &str, sha: &str) -> Vec<PipelineDetail> {
    let get = |url: &str| -> Option<serde_json::Value> {
        ureq::get(url)
            .set("authorization", &format!("Bearer {token}"))
            .timeout(Duration::from_secs(20))
            .call()
            .ok()
            .and_then(|r| r.into_json().ok())
    };
    let list_url = format!("{}/api/v4/projects/{}/pipelines?sha={}&per_page=1", base.trim_end_matches('/'), project_enc, sha);
    let pipeline = match get(&list_url).and_then(|j| j.as_array().and_then(|a| a.first().cloned())) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let pid = pipeline["id"].as_u64().unwrap_or(0);
    let jobs_url = format!("{}/api/v4/projects/{}/pipelines/{}/jobs?per_page=100", base.trim_end_matches('/'), project_enc, pid);
    let jobs = get(&jobs_url)
        .and_then(|j| j.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|j| PipelineJob {
            id: j["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
            name: j["name"].as_str().unwrap_or("").to_string(),
            stage: j["stage"].as_str().unwrap_or("").to_string(),
            status: match j["status"].as_str().unwrap_or("") {
                "created" | "waiting_for_resource" | "preparing" | "scheduled" => "pending".into(),
                s => s.to_string(),
            },
            web_url: j["web_url"].as_str().unwrap_or("").to_string(),
        })
        .collect();
    vec![PipelineDetail {
        id: pid.to_string(),
        name: format!("Pipeline #{pid}"),
        status: pipeline["status"].as_str().unwrap_or("").to_string(),
        web_url: pipeline["web_url"].as_str().unwrap_or("").to_string(),
        jobs,
    }]
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRef {
    pub id: String,
    pub name: String,
}

/// GitHub Actions workflows (so the user can pick one to dispatch). Empty for
/// GitLab, where a pipeline is defined by .gitlab-ci.yml and needs no choice.
#[tauri::command]
pub async fn list_workflows(app: AppHandle, repo_path: String) -> Result<Vec<WorkflowRef>> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                let (provider, base) = (conn.provider.clone(), conn.base_url.clone());
                if provider != "github" && provider != "azure" {
                    return Ok(Vec::new());
                }
                let token = read_token(&conn.id)?;
                return tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
                    "github" => github_workflows(&base, &token, &repo_id),
                    "azure" => azure_definitions(&base, &token, &repo_id),
                    _ => Ok(Vec::new()),
                })
                .await
                .map_err(|e| AccountError::Msg(e.to_string()))?;
            }
        }
    }
    Ok(Vec::new())
}

fn github_workflows(base: &str, token: &str, owner_repo: &str) -> Result<Vec<WorkflowRef>> {
    let url = format!("{}/repos/{}/actions/workflows?per_page=100", base.trim_end_matches('/'), owner_repo);
    let json: serde_json::Value = ureq::get(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("user-agent", "Plumb")
        .set("accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| http_err("Couldn't list workflows", e))?
        .into_json()?;
    Ok(json["workflows"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|w| w["state"].as_str().unwrap_or("") == "active")
                .map(|w| WorkflowRef {
                    id: w["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
                    name: w["name"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Kick off a pipeline. GitHub dispatches `workflow_id` on `git_ref` (the
/// workflow must declare `workflow_dispatch`); GitLab runs a pipeline on `git_ref`.
#[tauri::command]
pub async fn trigger_pipeline(
    app: AppHandle,
    repo_path: String,
    git_ref: String,
    workflow_id: Option<String>,
) -> Result<String> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                let token = read_token(&conn.id)?;
                let (provider, base) = (conn.provider.clone(), conn.base_url.clone());
                return tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
                    "github" => github_dispatch(&base, &token, &repo_id, &git_ref, workflow_id.as_deref()),
                    "gitlab" => gitlab_trigger(&base, &token, &urlencode(&repo_id), &git_ref),
                    "azure" => azure_trigger(&base, &token, &repo_id, &git_ref, workflow_id.as_deref()),
                    _ => Err(AccountError::Msg("Unsupported provider.".into())),
                })
                .await
                .map_err(|e| AccountError::Msg(e.to_string()))?;
            }
        }
    }
    Err(AccountError::Msg("No connected account matches this repository's remote.".into()))
}

fn github_dispatch(base: &str, token: &str, owner_repo: &str, git_ref: &str, workflow_id: Option<&str>) -> Result<String> {
    let wid = workflow_id.ok_or_else(|| AccountError::Msg("Pick a workflow to run.".into()))?;
    let url = format!(
        "{}/repos/{}/actions/workflows/{}/dispatches",
        base.trim_end_matches('/'),
        owner_repo,
        wid
    );
    // 204 No Content on success — don't parse a body.
    ureq::post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("user-agent", "Plumb")
        .set("accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({ "ref": git_ref }))
        .map_err(|e| http_err("Couldn't start the workflow (does it allow manual runs?)", e))?;
    Ok(format!("Workflow dispatched on {git_ref}"))
}

fn gitlab_trigger(base: &str, token: &str, project_enc: &str, git_ref: &str) -> Result<String> {
    let url = format!("{}/api/v4/projects/{}/pipeline", base.trim_end_matches('/'), project_enc);
    let json: serde_json::Value = ureq::post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({ "ref": git_ref }))
        .map_err(|e| http_err("Couldn't start the pipeline", e))?
        .into_json()?;
    Ok(format!("Pipeline #{} started", json["id"].as_u64().unwrap_or(0)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedPr {
    pub url: String,
    pub number: u64,
    pub provider: String,
}

/// Which provider (and label) a repo's remote maps to, so the UI can say
/// "pull request" vs "merge request" and default the target branch.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrTarget {
    /// "github" | "gitlab" | "" (no matching account)
    pub provider: String,
    pub host: String,
    pub repo: String,
}

/// Resolve the repo's origin to a connected provider (for the create-PR dialog).
#[tauri::command]
pub fn pr_target(app: AppHandle, repo_path: String) -> Result<PrTarget> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                return Ok(PrTarget { provider: conn.provider.clone(), host, repo: repo_id });
            }
        }
    }
    Ok(PrTarget { provider: String::new(), host: String::new(), repo: String::new() })
}

/// Open a pull request (GitHub) or merge request (GitLab) for the repo's remote.
#[tauri::command]
pub async fn create_pull_request(
    app: AppHandle,
    repo_path: String,
    source_branch: String,
    target_branch: String,
    title: String,
    body: String,
    draft: bool,
) -> Result<CreatedPr> {
    let cfg = load(&app)?;
    let mut remotes = repo_remotes(&repo_path);
    remotes.sort_by_key(|(n, _)| if n == "origin" { 0 } else { 1 });
    for (_, url) in &remotes {
        if let Some((host, path)) = parse_remote(url) {
            if let Some((conn, repo_id)) = match_conn(&cfg, &host, &path) {
                let token = read_token(&conn.id)?;
                let (provider, base) = (conn.provider.clone(), conn.base_url.clone());
                return tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
                    "github" => github_create_pr(&base, &token, &repo_id, &source_branch, &target_branch, &title, &body, draft),
                    "gitlab" => gitlab_create_mr(&base, &token, &repo_id, &source_branch, &target_branch, &title, &body, draft),
                    "azure" => azure_create_pr(&base, &token, &repo_id, &source_branch, &target_branch, &title, &body, draft),
                    _ => Err(AccountError::Msg("Unsupported provider.".into())),
                })
                .await
                .map_err(|e| AccountError::Msg(e.to_string()))?;
            }
        }
    }
    Err(AccountError::Msg("No connected account matches this repository's remote.".into()))
}

fn github_create_pr(
    base: &str, token: &str, owner_repo: &str,
    head: &str, base_branch: &str, title: &str, body: &str, draft: bool,
) -> Result<CreatedPr> {
    let url = format!("{}/repos/{}/pulls", base.trim_end_matches('/'), owner_repo);
    let json: serde_json::Value = ureq::post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("user-agent", "Plumb")
        .set("accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({
            "title": title, "head": head, "base": base_branch, "body": body, "draft": draft
        }))
        .map_err(|e| http_err("Couldn't create pull request", e))?
        .into_json()?;
    Ok(CreatedPr {
        url: json["html_url"].as_str().unwrap_or("").to_string(),
        number: json["number"].as_u64().unwrap_or(0),
        provider: "github".into(),
    })
}

fn gitlab_create_mr(
    base: &str, token: &str, project_path: &str,
    source: &str, target: &str, title: &str, body: &str, draft: bool,
) -> Result<CreatedPr> {
    // GitLab wants the project path URL-encoded; drafts are marked by title prefix.
    let enc = project_path.replace('/', "%2F");
    let url = format!("{}/api/v4/projects/{}/merge_requests", base.trim_end_matches('/'), enc);
    let title = if draft { format!("Draft: {title}") } else { title.to_string() };
    let json: serde_json::Value = ureq::post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({
            "source_branch": source, "target_branch": target, "title": title, "description": body
        }))
        .map_err(|e| http_err("Couldn't create merge request", e))?
        .into_json()?;
    Ok(CreatedPr {
        url: json["web_url"].as_str().unwrap_or("").to_string(),
        number: json["iid"].as_u64().unwrap_or(0),
        provider: "gitlab".into(),
    })
}

fn github_prs(base: &str, token: &str, owner_repo: &str) -> Result<Vec<PullRequest>> {
    let url = format!(
        "{}/repos/{}/pulls?state=open&per_page=50&sort=updated&direction=desc",
        base.trim_end_matches('/'),
        owner_repo
    );
    let json: serde_json::Value = ureq::get(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("user-agent", "Plumb")
        .set("accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| http_err("Couldn't load pull requests", e))?
        .into_json()?;
    let mut prs: Vec<PullRequest> = json
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|p| PullRequest {
                    number: p["number"].as_u64().unwrap_or(0),
                    title: p["title"].as_str().unwrap_or("").to_string(),
                    author: p["user"]["login"].as_str().unwrap_or("").to_string(),
                    author_avatar: p["user"]["avatar_url"].as_str().unwrap_or("").to_string(),
                    draft: p["draft"].as_bool().unwrap_or(false),
                    source_branch: p["head"]["ref"].as_str().unwrap_or("").to_string(),
                    target_branch: p["base"]["ref"].as_str().unwrap_or("").to_string(),
                    url: p["html_url"].as_str().unwrap_or("").to_string(),
                    updated_at: p["updated_at"].as_str().unwrap_or("").to_string(),
                    provider: "github".to_string(),
                    assignees: usernames(p, "assignees", "login"),
                    reviewers: usernames(p, "requested_reviewers", "login"),
                    ci_status: String::new(),
                    head_sha: p["head"]["sha"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    // Fetch each head commit's check-run rollup concurrently (best-effort).
    std::thread::scope(|s| {
        let handles: Vec<_> = prs
            .iter()
            .map(|pr| {
                let sha = pr.head_sha.clone();
                s.spawn(move || github_check_rollup(base, token, owner_repo, &sha))
            })
            .collect();
        for (pr, h) in prs.iter_mut().zip(handles) {
            pr.ci_status = h.join().unwrap_or_default();
        }
    });
    Ok(prs)
}

/// Roll up a commit's GitHub check runs to one status word.
fn github_check_rollup(base: &str, token: &str, owner_repo: &str, sha: &str) -> String {
    if sha.is_empty() {
        return String::new();
    }
    let url = format!(
        "{}/repos/{}/commits/{}/check-runs",
        base.trim_end_matches('/'),
        owner_repo,
        sha
    );
    let json: serde_json::Value = match ureq::get(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("user-agent", "Plumb")
        .set("accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .call()
    {
        Ok(r) => match r.into_json() {
            Ok(j) => j,
            Err(_) => return String::new(),
        },
        Err(_) => return String::new(),
    };
    let runs = match json["check_runs"].as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return String::new(),
    };
    let (mut pending, mut fail, mut success) = (false, false, false);
    for r in runs {
        if r["status"].as_str().unwrap_or("") != "completed" {
            pending = true;
        } else {
            match r["conclusion"].as_str().unwrap_or("") {
                "failure" | "timed_out" | "cancelled" | "action_required" | "startup_failure" => fail = true,
                "success" => success = true,
                _ => {}
            }
        }
    }
    if pending {
        "pending".into()
    } else if fail {
        "failure".into()
    } else if success {
        "success".into()
    } else {
        String::new()
    }
}

fn gitlab_mrs(base: &str, token: &str, project: &str) -> Result<Vec<PullRequest>> {
    // GitLab wants the project path URL-encoded (slashes become %2F).
    let url = format!(
        "{}/api/v4/projects/{}/merge_requests?state=opened&per_page=50&order_by=updated_at",
        base.trim_end_matches('/'),
        urlencode(project)
    );
    let json: serde_json::Value = ureq::get(&url)
        .set("authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| http_err("Couldn't load merge requests", e))?
        .into_json()?;
    let mut mrs: Vec<PullRequest> = json
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|m| PullRequest {
                    number: m["iid"].as_u64().unwrap_or(0),
                    title: m["title"].as_str().unwrap_or("").to_string(),
                    author: m["author"]["username"].as_str().unwrap_or("").to_string(),
                    author_avatar: m["author"]["avatar_url"].as_str().unwrap_or("").to_string(),
                    draft: m["draft"].as_bool().or_else(|| m["work_in_progress"].as_bool()).unwrap_or(false),
                    source_branch: m["source_branch"].as_str().unwrap_or("").to_string(),
                    target_branch: m["target_branch"].as_str().unwrap_or("").to_string(),
                    url: m["web_url"].as_str().unwrap_or("").to_string(),
                    updated_at: m["updated_at"].as_str().unwrap_or("").to_string(),
                    provider: "gitlab".to_string(),
                    assignees: usernames(m, "assignees", "username"),
                    reviewers: usernames(m, "reviewers", "username"),
                    ci_status: String::new(),
                    head_sha: m["sha"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let enc = urlencode(project);
    std::thread::scope(|s| {
        let handles: Vec<_> = mrs
            .iter()
            .map(|mr| {
                let sha = mr.head_sha.clone();
                let enc = enc.clone();
                s.spawn(move || gitlab_pipeline_status(base, token, &enc, &sha))
            })
            .collect();
        for (mr, h) in mrs.iter_mut().zip(handles) {
            mr.ci_status = h.join().unwrap_or_default();
        }
    });
    Ok(mrs)
}

/// Latest pipeline status for a commit on GitLab, mapped to one status word.
fn gitlab_pipeline_status(base: &str, token: &str, project_enc: &str, sha: &str) -> String {
    if sha.is_empty() {
        return String::new();
    }
    let url = format!(
        "{}/api/v4/projects/{}/pipelines?sha={}&per_page=1",
        base.trim_end_matches('/'),
        project_enc,
        sha
    );
    let json: serde_json::Value = match ureq::get(&url)
        .set("authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(15))
        .call()
    {
        Ok(r) => match r.into_json() {
            Ok(j) => j,
            Err(_) => return String::new(),
        },
        Err(_) => return String::new(),
    };
    match json.as_array().and_then(|a| a.first()).and_then(|p| p["status"].as_str()).unwrap_or("") {
        "success" => "success".into(),
        "failed" => "failure".into(),
        "running" | "pending" | "created" | "waiting_for_resource" | "preparing" | "scheduled" => "pending".into(),
        _ => String::new(),
    }
}

/* ── Repository listing (for Clone) ───────────────────────────────── */

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRef {
    pub name: String,
    pub ssh_url: String,
    pub http_url: String,
    pub description: String,
}

/// List repositories the connected account can access (for the Clone browser).
#[tauri::command]
pub async fn list_account_repos(app: AppHandle, connection_id: String) -> Result<Vec<RepoRef>> {
    let cfg = load(&app)?;
    let conn = cfg
        .connections
        .iter()
        .find(|c| c.id == connection_id)
        .cloned()
        .ok_or_else(|| AccountError::Msg("Connection not found.".into()))?;
    let token = read_token(&conn.id)?;
    tauri::async_runtime::spawn_blocking(move || match conn.provider.as_str() {
        "github" => github_repos(&conn.base_url, &token),
        "gitlab" => gitlab_repos(&conn.base_url, &token),
        "azure" => azure_repos(&conn.base_url, &token),
        _ => Ok(Vec::new()),
    })
    .await
    .map_err(|e| AccountError::Msg(e.to_string()))?
}

/// Create a new repository on a connected account and return its URLs.
#[tauri::command]
pub async fn create_remote_repo(
    app: AppHandle,
    connection_id: String,
    name: String,
    private: bool,
) -> Result<RepoRef> {
    let cfg = load(&app)?;
    let conn = cfg
        .connections
        .iter()
        .find(|c| c.id == connection_id)
        .cloned()
        .ok_or_else(|| AccountError::Msg("Connection not found.".into()))?;
    let token = read_token(&conn.id)?;
    tauri::async_runtime::spawn_blocking(move || match conn.provider.as_str() {
        "github" => github_create(&conn.base_url, &token, &name, private),
        "gitlab" => gitlab_create(&conn.base_url, &token, &name, private),
        "azure" => azure_create(&conn.base_url, &token, &name, private),
        _ => Err(AccountError::Msg("Unsupported provider.".into())),
    })
    .await
    .map_err(|e| AccountError::Msg(e.to_string()))?
}

fn github_create(base: &str, token: &str, name: &str, private: bool) -> Result<RepoRef> {
    let url = format!("{}/user/repos", base.trim_end_matches('/'));
    let json: serde_json::Value = ureq::post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("user-agent", "Plumb")
        .set("accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({ "name": name, "private": private }))
        .map_err(|e| http_err("Couldn't create repository", e))?
        .into_json()?;
    Ok(RepoRef {
        name: json["full_name"].as_str().unwrap_or("").to_string(),
        ssh_url: json["ssh_url"].as_str().unwrap_or("").to_string(),
        http_url: json["clone_url"].as_str().unwrap_or("").to_string(),
        description: String::new(),
    })
}

fn gitlab_create(base: &str, token: &str, name: &str, private: bool) -> Result<RepoRef> {
    let url = format!("{}/api/v4/projects", base.trim_end_matches('/'));
    let visibility = if private { "private" } else { "public" };
    let json: serde_json::Value = ureq::post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({ "name": name, "visibility": visibility }))
        .map_err(|e| http_err("Couldn't create project", e))?
        .into_json()?;
    Ok(RepoRef {
        name: json["path_with_namespace"].as_str().unwrap_or("").to_string(),
        ssh_url: json["ssh_url_to_repo"].as_str().unwrap_or("").to_string(),
        http_url: json["http_url_to_repo"].as_str().unwrap_or("").to_string(),
        description: String::new(),
    })
}

fn github_repos(base: &str, token: &str) -> Result<Vec<RepoRef>> {
    let mut out = Vec::new();
    for page in 1..=3 {
        let url = format!(
            "{}/user/repos?per_page=100&page={page}&sort=updated&affiliation=owner,collaborator,organization_member",
            base.trim_end_matches('/')
        );
        let arr: Vec<serde_json::Value> = ureq::get(&url)
            .set("authorization", &format!("Bearer {token}"))
            .set("user-agent", "Plumb")
            .set("accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(20))
            .call()
            .map_err(|e| http_err("Couldn't list repositories", e))?
            .into_json()?;
        let n = arr.len();
        for r in &arr {
            out.push(RepoRef {
                name: r["full_name"].as_str().unwrap_or("").to_string(),
                ssh_url: r["ssh_url"].as_str().unwrap_or("").to_string(),
                http_url: r["clone_url"].as_str().unwrap_or("").to_string(),
                description: r["description"].as_str().unwrap_or("").to_string(),
            });
        }
        if n < 100 {
            break;
        }
    }
    Ok(out)
}

fn gitlab_repos(base: &str, token: &str) -> Result<Vec<RepoRef>> {
    let mut out = Vec::new();
    for page in 1..=3 {
        let url = format!(
            "{}/api/v4/projects?membership=true&per_page=100&page={page}&order_by=last_activity_at&simple=true",
            base.trim_end_matches('/')
        );
        let arr: Vec<serde_json::Value> = ureq::get(&url)
            .set("authorization", &format!("Bearer {token}"))
            .timeout(Duration::from_secs(20))
            .call()
            .map_err(|e| http_err("Couldn't list projects", e))?
            .into_json()?;
        let n = arr.len();
        for r in &arr {
            out.push(RepoRef {
                name: r["path_with_namespace"].as_str().unwrap_or("").to_string(),
                ssh_url: r["ssh_url_to_repo"].as_str().unwrap_or("").to_string(),
                http_url: r["http_url_to_repo"].as_str().unwrap_or("").to_string(),
                description: r["description"].as_str().unwrap_or("").to_string(),
            });
        }
        if n < 100 {
            break;
        }
    }
    Ok(out)
}

/* ── Azure DevOps ─────────────────────────────────────────────────────
   Auth is a PAT sent as HTTP Basic (empty username). Repos live under
   {org}/{project}/_git/{repo}; the org is baked into the connection base URL,
   so these helpers receive "project/repo". Pipelines map onto the Build API. */

fn azure_get(url: &str, token: &str) -> Option<serde_json::Value> {
    ureq::get(url)
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .call()
        .ok()
        .and_then(|r| r.into_json().ok())
}

fn azure_split(project_repo: &str) -> Result<(&str, &str)> {
    project_repo
        .split_once('/')
        .ok_or_else(|| AccountError::Msg("Expected an Azure DevOps project/repo.".into()))
}

fn strip_ref(r: &str) -> String {
    r.trim_start_matches("refs/heads/").to_string()
}

/// Roll an Azure build's (status, result) to a badge word.
fn azure_build_status(status: &str, result: &str) -> String {
    if status != "completed" {
        return "pending".into();
    }
    match result {
        "succeeded" | "partiallySucceeded" => "success".into(),
        _ => "failure".into(),
    }
}

/// Top-level pipeline status for the detail view.
fn azure_run_status(status: &str, result: &str) -> String {
    match status {
        "completed" => match result {
            "succeeded" | "partiallySucceeded" => "success",
            "canceled" => "canceled",
            "failed" => "failed",
            _ => "other",
        },
        "inProgress" | "cancelling" => "running",
        _ => "pending",
    }
    .into()
}

/// Per-job status for a timeline record.
fn azure_job_state(state: &str, result: &str) -> String {
    if state != "completed" {
        return if state == "inProgress" { "running" } else { "pending" }.into();
    }
    match result {
        "succeeded" | "partiallySucceeded" => "success",
        "failed" => "failed",
        "canceled" | "abandoned" => "canceled",
        "skipped" => "skipped",
        _ => "other",
    }
    .into()
}

fn azure_prs(base: &str, token: &str, project_repo: &str) -> Result<Vec<PullRequest>> {
    let (project, repo) = azure_split(project_repo)?;
    let root = base.trim_end_matches('/');
    let url = format!(
        "{root}/{}/_apis/git/repositories/{}/pullrequests?searchCriteria.status=active&$top=50&api-version=7.1",
        urlencode(project),
        urlencode(repo)
    );
    let json: serde_json::Value = ureq::get(&url)
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| http_err("Couldn't load pull requests", e))?
        .into_json()?;
    Ok(json["value"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|p| {
                    let id = p["pullRequestId"].as_u64().unwrap_or(0);
                    PullRequest {
                        number: id,
                        title: p["title"].as_str().unwrap_or("").to_string(),
                        author: p["createdBy"]["displayName"].as_str().unwrap_or("").to_string(),
                        author_avatar: p["createdBy"]["imageUrl"].as_str().unwrap_or("").to_string(),
                        draft: p["isDraft"].as_bool().unwrap_or(false),
                        source_branch: strip_ref(p["sourceRefName"].as_str().unwrap_or("")),
                        target_branch: strip_ref(p["targetRefName"].as_str().unwrap_or("")),
                        url: format!("{root}/{}/_git/{}/pullrequest/{id}", urlencode(project), urlencode(repo)),
                        updated_at: p["creationDate"].as_str().unwrap_or("").to_string(),
                        provider: "azure".to_string(),
                        assignees: Vec::new(),
                        reviewers: p["reviewers"]
                            .as_array()
                            .map(|a| a.iter().filter_map(|r| r["displayName"].as_str().map(String::from)).collect())
                            .unwrap_or_default(),
                        ci_status: String::new(),
                        head_sha: p["lastMergeSourceCommit"]["commitId"].as_str().unwrap_or("").to_string(),
                    }
                })
                .collect()
        })
        .unwrap_or_default())
}

#[allow(clippy::too_many_arguments)]
fn azure_create_pr(
    base: &str, token: &str, project_repo: &str,
    source: &str, target: &str, title: &str, body: &str, draft: bool,
) -> Result<CreatedPr> {
    let (project, repo) = azure_split(project_repo)?;
    let root = base.trim_end_matches('/');
    let url = format!(
        "{root}/{}/_apis/git/repositories/{}/pullrequests?api-version=7.1",
        urlencode(project),
        urlencode(repo)
    );
    let json: serde_json::Value = ureq::post(&url)
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({
            "sourceRefName": format!("refs/heads/{source}"),
            "targetRefName": format!("refs/heads/{target}"),
            "title": title, "description": body, "isDraft": draft
        }))
        .map_err(|e| http_err("Couldn't create pull request", e))?
        .into_json()?;
    let id = json["pullRequestId"].as_u64().unwrap_or(0);
    Ok(CreatedPr {
        url: format!("{root}/{}/_git/{}/pullrequest/{id}", urlencode(project), urlencode(repo)),
        number: id,
        provider: "azure".into(),
    })
}

fn azure_repos(base: &str, token: &str) -> Result<Vec<RepoRef>> {
    let url = format!("{}/_apis/git/repositories?api-version=7.1", base.trim_end_matches('/'));
    let json: serde_json::Value = ureq::get(&url)
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| http_err("Couldn't list repositories", e))?
        .into_json()?;
    Ok(json["value"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|r| {
                    let project = r["project"]["name"].as_str().unwrap_or("");
                    let name = r["name"].as_str().unwrap_or("");
                    RepoRef {
                        name: format!("{project}/{name}"),
                        ssh_url: r["sshUrl"].as_str().unwrap_or("").to_string(),
                        http_url: r["remoteUrl"].as_str().unwrap_or("").to_string(),
                        description: String::new(),
                    }
                })
                .collect()
        })
        .unwrap_or_default())
}

fn azure_create(base: &str, token: &str, name: &str, private: bool) -> Result<RepoRef> {
    // Azure repo visibility follows its project; `private` isn't a repo-level knob.
    let _ = private;
    let (project, repo) = name
        .split_once('/')
        .ok_or_else(|| AccountError::Msg("Name a new Azure repo as project/repo.".into()))?;
    let root = base.trim_end_matches('/');
    let proj: serde_json::Value = ureq::get(&format!("{root}/_apis/projects/{}?api-version=7.1", urlencode(project)))
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| http_err("Project not found", e))?
        .into_json()?;
    let pid = proj["id"].as_str().unwrap_or("");
    let json: serde_json::Value = ureq::post(&format!("{root}/_apis/git/repositories?api-version=7.1"))
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({ "name": repo, "project": { "id": pid } }))
        .map_err(|e| http_err("Couldn't create repository", e))?
        .into_json()?;
    Ok(RepoRef {
        name: format!("{project}/{}", json["name"].as_str().unwrap_or(repo)),
        ssh_url: json["sshUrl"].as_str().unwrap_or("").to_string(),
        http_url: json["remoteUrl"].as_str().unwrap_or("").to_string(),
        description: String::new(),
    })
}

fn azure_ci_map(base: &str, token: &str, project_repo: &str) -> Vec<CiStatus> {
    let (project, repo) = match project_repo.split_once('/') {
        Some(x) => x,
        None => return Vec::new(),
    };
    let url = format!(
        "{}/{}/_apis/build/builds?$top=100&queryOrder=finishTimeDescending&api-version=7.1",
        base.trim_end_matches('/'),
        urlencode(project)
    );
    let json = match azure_get(&url, token) {
        Some(j) => j,
        None => return Vec::new(),
    };
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut out = Vec::new();
    if let Some(arr) = json["value"].as_array() {
        for b in arr {
            if let Some(rn) = b["repository"]["name"].as_str() {
                if !rn.is_empty() && !rn.eq_ignore_ascii_case(repo) {
                    continue;
                }
            }
            let sha = b["sourceVersion"].as_str().unwrap_or("");
            if sha.is_empty() || seen.insert(sha.to_string(), ()).is_some() {
                continue;
            }
            out.push(CiStatus {
                sha: sha.to_string(),
                status: azure_build_status(b["status"].as_str().unwrap_or(""), b["result"].as_str().unwrap_or("")),
            });
        }
    }
    out
}

fn azure_pipeline_detail(base: &str, token: &str, project_repo: &str, sha: &str) -> Vec<PipelineDetail> {
    let (project, repo) = match project_repo.split_once('/') {
        Some(x) => x,
        None => return Vec::new(),
    };
    let root = base.trim_end_matches('/');
    let url = format!(
        "{root}/{}/_apis/build/builds?$top=50&queryOrder=finishTimeDescending&api-version=7.1",
        urlencode(project)
    );
    let json = match azure_get(&url, token) {
        Some(j) => j,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    if let Some(arr) = json["value"].as_array() {
        for b in arr {
            if b["sourceVersion"].as_str().unwrap_or("") != sha {
                continue;
            }
            if let Some(rn) = b["repository"]["name"].as_str() {
                if !rn.is_empty() && !rn.eq_ignore_ascii_case(repo) {
                    continue;
                }
            }
            let build_id = b["id"].as_u64().unwrap_or(0);
            let web = b["_links"]["web"]["href"].as_str().unwrap_or("").to_string();
            let tl = azure_get(
                &format!("{root}/{}/_apis/build/builds/{build_id}/timeline?api-version=7.1", urlencode(project)),
                token,
            );
            let jobs = tl
                .as_ref()
                .and_then(|t| t["records"].as_array())
                .map(|recs| {
                    recs.iter()
                        .filter(|r| r["type"].as_str() == Some("Job"))
                        .map(|r| {
                            let jid = match r["log"]["id"].as_u64() {
                                Some(l) => format!("{build_id}:{l}"),
                                None => r["id"].as_str().unwrap_or("").to_string(),
                            };
                            PipelineJob {
                                id: jid,
                                name: r["name"].as_str().unwrap_or("").to_string(),
                                stage: String::new(),
                                status: azure_job_state(r["state"].as_str().unwrap_or(""), r["result"].as_str().unwrap_or("")),
                                web_url: web.clone(),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push(PipelineDetail {
                id: build_id.to_string(),
                name: b["definition"]["name"].as_str().unwrap_or("").to_string(),
                status: azure_run_status(b["status"].as_str().unwrap_or(""), b["result"].as_str().unwrap_or("")),
                web_url: web,
                jobs,
            });
        }
    }
    out
}

fn azure_job_log(base: &str, token: &str, project_repo: &str, job_id: &str) -> Result<String> {
    let (project, _repo) = azure_split(project_repo)?;
    let (build_id, log_id) = job_id
        .split_once(':')
        .ok_or_else(|| AccountError::Msg("No log is available for this job yet.".into()))?;
    let url = format!(
        "{}/{}/_apis/build/builds/{build_id}/logs/{log_id}?api-version=7.1",
        base.trim_end_matches('/'),
        urlencode(project)
    );
    let text = ureq::get(&url)
        .set("authorization", &azure_basic(token))
        .timeout(Duration::from_secs(25))
        .call()
        .map_err(|e| http_err("Couldn't fetch the log", e))?
        .into_string()
        .map_err(|e| AccountError::Msg(e.to_string()))?;
    const MAX: usize = 200_000;
    Ok(if text.len() > MAX {
        format!("… (log truncated; showing last {MAX} chars) …\n{}", &text[text.len() - MAX..])
    } else {
        text
    })
}

fn azure_pipeline_action(base: &str, token: &str, project_repo: &str, build_id: &str, verb: &str) -> Result<()> {
    let (project, _repo) = azure_split(project_repo)?;
    let root = base.trim_end_matches('/');
    if verb == "cancel" {
        ureq::request("PATCH", &format!("{root}/{}/_apis/build/builds/{build_id}?api-version=7.1", urlencode(project)))
            .set("authorization", &azure_basic(token))
            .set("accept", "application/json")
            .timeout(Duration::from_secs(20))
            .send_json(serde_json::json!({ "status": "cancelling" }))
            .map_err(|e| http_err("Couldn't cancel the pipeline", e))?;
        return Ok(());
    }
    // Retry = queue a fresh build from the same definition and branch.
    let b = azure_get(
        &format!("{root}/{}/_apis/build/builds/{build_id}?api-version=7.1", urlencode(project)),
        token,
    )
    .ok_or_else(|| AccountError::Msg("Build not found.".into()))?;
    let did = b["definition"]["id"].as_u64().ok_or_else(|| AccountError::Msg("No pipeline definition for this build.".into()))?;
    let branch = b["sourceBranch"].as_str().unwrap_or("refs/heads/main").to_string();
    ureq::post(&format!("{root}/{}/_apis/build/builds?api-version=7.1", urlencode(project)))
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({ "definition": { "id": did }, "sourceBranch": branch }))
        .map_err(|e| http_err("Couldn't retry the pipeline", e))?;
    Ok(())
}

fn azure_definitions(base: &str, token: &str, project_repo: &str) -> Result<Vec<WorkflowRef>> {
    let (project, _repo) = azure_split(project_repo)?;
    let url = format!(
        "{}/{}/_apis/build/definitions?$top=100&api-version=7.1",
        base.trim_end_matches('/'),
        urlencode(project)
    );
    let json: serde_json::Value = ureq::get(&url)
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| http_err("Couldn't list pipelines", e))?
        .into_json()?;
    Ok(json["value"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|d| WorkflowRef {
                    id: d["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
                    name: d["name"].as_str().unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default())
}

fn azure_trigger(base: &str, token: &str, project_repo: &str, git_ref: &str, definition_id: Option<&str>) -> Result<String> {
    let (project, _repo) = azure_split(project_repo)?;
    let did: u64 = definition_id
        .ok_or_else(|| AccountError::Msg("Pick a pipeline to run.".into()))?
        .parse()
        .map_err(|_| AccountError::Msg("Bad pipeline id.".into()))?;
    let url = format!("{}/{}/_apis/build/builds?api-version=7.1", base.trim_end_matches('/'), urlencode(project));
    ureq::post(&url)
        .set("authorization", &azure_basic(token))
        .set("accept", "application/json")
        .timeout(Duration::from_secs(20))
        .send_json(serde_json::json!({ "definition": { "id": did }, "sourceBranch": format!("refs/heads/{git_ref}") }))
        .map_err(|e| http_err("Couldn't queue the pipeline", e))?;
    Ok(format!("Pipeline queued on {git_ref}"))
}

pub(crate) fn http_err(context: &str, e: ureq::Error) -> AccountError {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let detail = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|j| {
                    j["message"]
                        .as_str()
                        .or_else(|| j["error"].as_str())
                        .or_else(|| j["error_description"].as_str())
                        .map(String::from)
                })
                .unwrap_or_else(|| body.chars().take(160).collect());
            AccountError::Msg(format!("{context}: HTTP {code} — {detail}"))
        }
        ureq::Error::Transport(t) => AccountError::Msg(format!("{context}: {t}")),
    }
}
