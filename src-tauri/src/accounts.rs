//! Multi-account connections to GitHub and GitLab (incl. self-managed/enterprise).
//!
//! Connections are a *list* — the headline differentiator over GitKraken's
//! one-account-per-provider model. Each connection's token lives in the macOS
//! Keychain; only non-secret metadata (provider, host, username) is persisted.

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
    if !b.is_empty() {
        return b.to_string();
    }
    match provider {
        "github" => "https://api.github.com".to_string(),
        _ => "https://gitlab.com".to_string(),
    }
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
        other => Err(AccountError::Msg(format!("Unknown provider '{other}'."))),
    }
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
            if let Some(conn) = cfg.connections.iter().find(|c| conn_web_host(c) == host) {
                let token = read_token(&conn.id)?;
                let provider = conn.provider.clone();
                let base = conn.base_url.clone();
                let items = tauri::async_runtime::spawn_blocking(move || match provider.as_str() {
                    "github" => github_prs(&base, &token, &path),
                    "gitlab" => gitlab_mrs(&base, &token, &path),
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
    Ok(json
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
                })
                .collect()
        })
        .unwrap_or_default())
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
    Ok(json
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
                })
                .collect()
        })
        .unwrap_or_default())
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
