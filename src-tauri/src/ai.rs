//! AI commit-message generation with a pluggable, multi-provider model.
//!
//! Providers are stored as a *list* (mirroring Plumb's multi-account design):
//! the user can configure several and mark one default. This first cut ships
//! the **Local (Ollama)** provider — private, on-device, no key — with the
//! config shape ready for cloud/custom/MCP providers to slot in later.
//!
//! Generation is never automatic: the UI calls `generate_commit_message`
//! explicitly, we read the *staged* diff, and hand back a plain, editable draft.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Git(#[from] git2::Error),
}

impl Serialize for AiError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

type Result<T> = std::result::Result<T, AiError>;

/// A configured AI provider. `kind` is "local" (Ollama) or "cloud". For cloud,
/// `vendor` is anthropic | openai | openai-compatible, and the API key lives in
/// the macOS Keychain — never in this struct or the on-disk config.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub vendor: String,
    pub label: String,
    pub model: String,
    pub endpoint: String,
}

impl AiProvider {
    fn is_local(&self) -> bool {
        self.kind == "local"
    }
    /// Host shown in the privacy indicator for cloud providers.
    fn host(&self) -> String {
        self.endpoint
            .replace("https://", "")
            .replace("http://", "")
            .split('/')
            .next()
            .unwrap_or("")
            .to_string()
    }
}

/// Keychain service under which provider API keys are stored.
const KEY_SERVICE: &str = "app.plumb.desktop.ai";

fn key_entry(provider_id: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(KEY_SERVICE, provider_id)
        .map_err(|e| AiError::Msg(format!("Keychain error: {e}")))
}

fn store_key(provider_id: &str, key: &str) -> Result<()> {
    key_entry(provider_id)?
        .set_password(key)
        .map_err(|e| AiError::Msg(format!("Couldn't save key to Keychain: {e}")))
}

fn read_key(provider_id: &str) -> Result<String> {
    key_entry(provider_id)?.get_password().map_err(|_| {
        AiError::Msg("No API key found in the Keychain for this provider.".into())
    })
}

fn delete_key(provider_id: &str) {
    if let Ok(entry) = key_entry(provider_id) {
        let _ = entry.delete_credential();
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub providers: Vec<AiProvider>,
    pub default_id: Option<String>,
}

fn config_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AiError::Msg(format!("No config dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("ai_providers.json"))
}

fn load(app: &AppHandle) -> Result<AiConfig> {
    let p = config_path(app)?;
    if !p.exists() {
        return Ok(AiConfig::default());
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(p)?).unwrap_or_default())
}

fn store(app: &AppHandle, cfg: &AiConfig) -> Result<()> {
    std::fs::write(config_path(app)?, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

#[tauri::command]
pub fn list_ai_providers(app: AppHandle) -> Result<AiConfig> {
    load(&app)
}

/// Insert or update a provider. If `api_key` is provided (cloud), it's stored in
/// the Keychain, not in the config. Becomes default if asked, or if it's first.
#[tauri::command]
pub fn save_ai_provider(
    app: AppHandle,
    provider: AiProvider,
    make_default: bool,
    api_key: Option<String>,
) -> Result<AiConfig> {
    if let Some(key) = api_key {
        if !key.trim().is_empty() {
            store_key(&provider.id, key.trim())?;
        }
    }
    let mut cfg = load(&app)?;
    match cfg.providers.iter_mut().find(|p| p.id == provider.id) {
        Some(existing) => *existing = provider.clone(),
        None => cfg.providers.push(provider.clone()),
    }
    if make_default || cfg.default_id.is_none() {
        cfg.default_id = Some(provider.id);
    }
    store(&app, &cfg)?;
    Ok(cfg)
}

#[tauri::command]
pub fn remove_ai_provider(app: AppHandle, id: String) -> Result<AiConfig> {
    delete_key(&id);
    let mut cfg = load(&app)?;
    cfg.providers.retain(|p| p.id != id);
    if cfg.default_id.as_deref() == Some(id.as_str()) {
        cfg.default_id = cfg.providers.first().map(|p| p.id.clone());
    }
    store(&app, &cfg)?;
    Ok(cfg)
}

/// Whether an API key is stored for this provider (never returns the key).
#[tauri::command]
pub fn has_api_key(id: String) -> bool {
    read_key(&id).is_ok()
}

#[tauri::command]
pub fn set_default_ai_provider(app: AppHandle, id: String) -> Result<AiConfig> {
    let mut cfg = load(&app)?;
    if cfg.providers.iter().any(|p| p.id == id) {
        cfg.default_id = Some(id);
    }
    store(&app, &cfg)?;
    Ok(cfg)
}

/// List models installed in a local Ollama instance (for the add-provider flow).
#[tauri::command]
pub async fn list_ollama_models(endpoint: String) -> Result<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let url = format!("{}/api/tags", endpoint.trim_end_matches('/'));
        let resp = ureq::get(&url)
            .timeout(Duration::from_secs(5))
            .call()
            .map_err(|e| AiError::Msg(format!("Couldn't reach Ollama at {endpoint} — is it running? ({e})")))?;
        let json: serde_json::Value = resp.into_json()?;
        let models = json["models"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    })
    .await
    .map_err(|e| AiError::Msg(e.to_string()))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedMessage {
    pub message: String,
    pub provider_label: String,
    pub model: String,
    pub is_local: bool,
    pub host: String,
    pub files: usize,
    pub added: usize,
    pub removed: usize,
    pub ms: u128,
}

/// Generate a commit message from the staged diff using the chosen (or default)
/// provider. Returns the draft plus what it read, for the "AI DRAFT" chip.
#[tauri::command]
pub async fn generate_commit_message(
    app: AppHandle,
    repo_path: String,
    provider_id: Option<String>,
    conventional: bool,
    style: String,
) -> Result<GeneratedMessage> {
    let cfg = load(&app)?;
    let provider = provider_id
        .and_then(|id| cfg.providers.iter().find(|p| p.id == id).cloned())
        .or_else(|| {
            cfg.default_id
                .as_ref()
                .and_then(|d| cfg.providers.iter().find(|p| &p.id == d).cloned())
        })
        .ok_or_else(|| AiError::Msg("No AI provider configured.".into()))?;

    let (diff_text, files, added, removed) = staged_diff(&repo_path)?;
    if diff_text.trim().is_empty() {
        return Err(AiError::Msg("Nothing staged to summarise.".into()));
    }
    let prompt = build_prompt(&diff_text, conventional, &style);

    let start = Instant::now();
    let label = provider.label.clone();
    let model = provider.model.clone();
    let is_local = provider.is_local();
    let host = provider.host();
    let message = tauri::async_runtime::spawn_blocking(move || run_provider(&provider, &prompt))
        .await
        .map_err(|e| AiError::Msg(e.to_string()))??;

    Ok(GeneratedMessage {
        message: clean_message(&message),
        provider_label: label,
        model,
        is_local,
        host,
        files,
        added,
        removed,
        ms: start.elapsed().as_millis(),
    })
}

/// Send a minimal request to verify a provider is reachable and authorised.
#[tauri::command]
pub async fn test_ai_provider(app: AppHandle, id: String) -> Result<String> {
    let cfg = load(&app)?;
    let provider = cfg
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| AiError::Msg("Provider not found.".into()))?;
    tauri::async_runtime::spawn_blocking(move || {
        run_provider(&provider, "Reply with the single word: OK")?;
        Ok("Provider reachable and authorised.".to_string())
    })
    .await
    .map_err(|e| AiError::Msg(e.to_string()))?
}

/// Dispatch a prompt to whichever backend a provider uses.
fn run_provider(provider: &AiProvider, prompt: &str) -> Result<String> {
    match provider.kind.as_str() {
        "local" => ollama_generate(&provider.endpoint, &provider.model, prompt),
        "cloud" => {
            let key = read_key(&provider.id)?;
            match provider.vendor.as_str() {
                "anthropic" => anthropic_generate(&provider.endpoint, &provider.model, &key, prompt),
                // openai + any openai-compatible endpoint share the chat schema
                _ => openai_generate(&provider.endpoint, &provider.model, &key, prompt),
            }
        }
        other => Err(AiError::Msg(format!("Unknown provider type '{other}'."))),
    }
}

fn pick_provider(cfg: &AiConfig, provider_id: Option<String>) -> Result<AiProvider> {
    provider_id
        .and_then(|id| cfg.providers.iter().find(|p| p.id == id).cloned())
        .or_else(|| cfg.default_id.as_ref().and_then(|d| cfg.providers.iter().find(|p| &p.id == d).cloned()))
        .ok_or_else(|| AiError::Msg("No AI provider configured.".into()))
}

/// The subject + patch text for a commit (truncated for the model).
fn commit_diff(path: &str, sha: &str) -> Result<(String, String)> {
    let subj = std::process::Command::new("git")
        .current_dir(path)
        .args(["log", "-1", "--format=%s", sha])
        .output()
        .map_err(|e| AiError::Msg(format!("git log: {e}")))?;
    let subject = String::from_utf8_lossy(&subj.stdout).trim().to_string();
    let out = std::process::Command::new("git")
        .current_dir(path)
        .args(["show", "--no-color", "--format=", "-p", sha])
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| AiError::Msg(format!("git show: {e}")))?;
    let diff: String = String::from_utf8_lossy(&out.stdout).chars().take(16000).collect();
    Ok((subject, diff))
}

fn build_explain_prompt(subject: &str, diff: &str) -> String {
    format!(
        "You are a senior engineer explaining a code change to a teammate reviewing it.\n\
         Change subject: {subject}\n\n\
         Diff (unified, possibly truncated):\n{diff}\n\n\
         Explain what this change does and why. Begin with a one-sentence summary, then a short \
         bullet list of the key changes and their impact, and finally note any risks or things to \
         review. Be concise and concrete; do not restate the diff or quote large code blocks."
    )
}

/// Explain a commit's changes (by sha), or the working diff when sha is None.
#[tauri::command]
pub async fn explain_diff(
    app: AppHandle,
    repo_path: String,
    provider_id: Option<String>,
    sha: Option<String>,
) -> Result<String> {
    let cfg = load(&app)?;
    let provider = pick_provider(&cfg, provider_id)?;
    let (subject, diff) = match &sha {
        Some(s) => commit_diff(&repo_path, s)?,
        None => ("Working changes".to_string(), worktree_diff(&repo_path)?),
    };
    if diff.trim().is_empty() {
        return Err(AiError::Msg("No changes to explain.".into()));
    }
    let prompt = build_explain_prompt(&subject, &diff);
    let out = tauri::async_runtime::spawn_blocking(move || run_provider(&provider, &prompt))
        .await
        .map_err(|e| AiError::Msg(e.to_string()))??;
    Ok(out.trim().to_string())
}

/* ── AI: split a working tree into several commits (design plate 13) ── */

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitGroup {
    pub message: String,
    pub files: Vec<String>,
}

/// Ask the provider to group the working-tree changes into focused commits.
#[tauri::command]
pub async fn ai_group_changes(
    app: AppHandle,
    repo_path: String,
    provider_id: Option<String>,
    conventional: bool,
) -> Result<Vec<CommitGroup>> {
    let cfg = load(&app)?;
    let provider = provider_id
        .and_then(|id| cfg.providers.iter().find(|p| p.id == id).cloned())
        .or_else(|| {
            cfg.default_id
                .as_ref()
                .and_then(|d| cfg.providers.iter().find(|p| &p.id == d).cloned())
        })
        .ok_or_else(|| AiError::Msg("No AI provider configured.".into()))?;

    let files = changed_files(&repo_path)?;
    if files.is_empty() {
        return Err(AiError::Msg("No changes to split.".into()));
    }
    let diff = worktree_diff(&repo_path)?;
    let prompt = build_group_prompt(&files, &diff, conventional);

    let raw = tauri::async_runtime::spawn_blocking(move || run_provider(&provider, &prompt))
        .await
        .map_err(|e| AiError::Msg(e.to_string()))??;

    parse_groups(&raw, &files)
}

/// Every changed path (staged, unstaged, untracked).
fn changed_files(path: &str) -> Result<Vec<String>> {
    let repo = git2::Repository::open(path).map_err(|e| AiError::Msg(e.to_string()))?;
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| AiError::Msg(e.to_string()))?;
    let mut out = Vec::new();
    for e in statuses.iter() {
        if let Some(p) = e.path() {
            if !e.status().is_ignored() {
                out.push(p.to_string());
            }
        }
    }
    Ok(out)
}

fn worktree_diff(path: &str) -> Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(path)
        .args(["diff", "HEAD"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| AiError::Msg(format!("git diff: {e}")))?;
    Ok(String::from_utf8_lossy(&output.stdout).chars().take(16000).collect())
}

fn build_group_prompt(files: &[String], diff: &str, conventional: bool) -> String {
    let list = files.join("\n");
    let conv = if conventional {
        " Use Conventional Commits (type(scope): subject)."
    } else {
        ""
    };
    format!(
        "You are grouping a messy git working tree into several focused commits.\n\
         Changed files:\n{list}\n\n\
         Combined diff (truncated):\n{diff}\n\n\
         Group these files into 1–5 logical commits. Every file must appear in exactly one group, \
         and only files from the list above may be used. Write a concise commit message for each.{conv}\n\
         Respond with ONLY a JSON array, no prose, no code fences, like:\n\
         [{{\"message\":\"...\",\"files\":[\"path/one\",\"path/two\"]}}]"
    )
}

/// Parse the model's JSON, keep only known files, and sweep any leftover files
/// into a final catch-all group so nothing is silently dropped.
fn parse_groups(raw: &str, files: &[String]) -> Result<Vec<CommitGroup>> {
    let start = raw.find('[');
    let end = raw.rfind(']');
    let json = match (start, end) {
        (Some(s), Some(e)) if e > s => &raw[s..=e],
        _ => return Err(AiError::Msg("The model didn't return a usable grouping.".into())),
    };
    let mut groups: Vec<CommitGroup> =
        serde_json::from_str(json).map_err(|e| AiError::Msg(format!("Couldn't parse grouping: {e}")))?;

    let known: std::collections::HashSet<&str> = files.iter().map(|s| s.as_str()).collect();
    let mut assigned: std::collections::HashSet<String> = std::collections::HashSet::new();
    for g in groups.iter_mut() {
        g.files.retain(|f| known.contains(f.as_str()) && assigned.insert(f.clone()));
    }
    groups.retain(|g| !g.files.is_empty());

    let leftover: Vec<String> = files
        .iter()
        .filter(|f| !assigned.contains(*f))
        .cloned()
        .collect();
    if !leftover.is_empty() {
        groups.push(CommitGroup {
            message: "chore: remaining changes".into(),
            files: leftover,
        });
    }
    Ok(groups)
}

/// The staged diff as patch text, plus (files, additions, deletions).
fn staged_diff(path: &str) -> Result<(String, usize, usize, usize)> {
    let repo = git2::Repository::open(path)?;
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3);
    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?;

    let stats = diff.stats()?;
    let (files, added, removed) = (
        stats.files_changed(),
        stats.insertions(),
        stats.deletions(),
    );

    let mut buf = String::new();
    diff.print(git2::DiffFormat::Patch, |_d, _h, line| {
        let o = line.origin();
        if o == '+' || o == '-' || o == ' ' {
            buf.push(o);
        }
        buf.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;

    Ok((buf, files, added, removed))
}

fn build_prompt(diff: &str, conventional: bool, style: &str) -> String {
    // Keep the prompt bounded so a huge diff doesn't overwhelm a local model.
    let truncated: String = diff.chars().take(16000).collect();

    let mut rules = vec![
        "You write git commit messages. Summarise the staged changes in the diff below.".to_string(),
        "Output ONLY the commit message — no preamble, no code fences, no quotes, no explanation.".to_string(),
        "Use an imperative subject line of at most ~72 characters.".to_string(),
    ];
    if conventional {
        rules.push(
            "Follow Conventional Commits: `type(scope): subject` (feat, fix, chore, refactor, docs, test)."
                .to_string(),
        );
    }
    match style {
        "shorter" => rules.push("Return only a single concise subject line, with no body.".to_string()),
        "detailed" => rules.push(
            "After the subject line, add a blank line and a short body explaining what changed and why."
                .to_string(),
        ),
        _ => {}
    }
    format!("{}\n\nStaged diff:\n{}", rules.join(" "), truncated)
}

/// Strip stray code fences / surrounding quotes some models add.
fn clean_message(msg: &str) -> String {
    let mut s = msg.trim();
    if s.starts_with("```") {
        s = s.trim_start_matches("```");
        if let Some(nl) = s.find('\n') {
            s = &s[nl + 1..];
        }
        s = s.trim_end_matches("```").trim();
    }
    s.trim().trim_matches('"').trim().to_string()
}

/// Turn a ureq error into a readable message, including any API error body.
fn ureq_err(context: &str, e: ureq::Error) -> AiError {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let detail = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|j| {
                    j["error"]["message"]
                        .as_str()
                        .or_else(|| j["error"].as_str())
                        .map(String::from)
                })
                .unwrap_or_else(|| body.chars().take(200).collect());
            AiError::Msg(format!("{context}: HTTP {code} — {detail}"))
        }
        ureq::Error::Transport(t) => AiError::Msg(format!("{context}: {t}")),
    }
}

fn anthropic_generate(endpoint: &str, model: &str, key: &str, prompt: &str) -> Result<String> {
    let url = format!("{}/messages", endpoint.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": [{ "role": "user", "content": prompt }],
    });
    let resp = ureq::post(&url)
        .set("x-api-key", key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .timeout(Duration::from_secs(120))
        .send_json(body)
        .map_err(|e| ureq_err("Anthropic request failed", e))?;
    let json: serde_json::Value = resp.into_json()?;
    let text = json["content"][0]["text"].as_str().unwrap_or("").trim().to_string();
    if text.is_empty() {
        return Err(AiError::Msg("Anthropic returned an empty message.".into()));
    }
    Ok(text)
}

fn openai_generate(endpoint: &str, model: &str, key: &str, prompt: &str) -> Result<String> {
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.2,
    });
    let resp = ureq::post(&url)
        .set("authorization", &format!("Bearer {key}"))
        .set("content-type", "application/json")
        .timeout(Duration::from_secs(120))
        .send_json(body)
        .map_err(|e| ureq_err("OpenAI request failed", e))?;
    let json: serde_json::Value = resp.into_json()?;
    let text = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(AiError::Msg("The API returned an empty message.".into()));
    }
    Ok(text)
}

/* ── Model discovery ──────────────────────────────────────────────── */

/// List models for a provider draft (add flow) or an existing one (edit flow).
/// If `api_key` is blank, falls back to the stored Keychain key for `provider_id`.
#[tauri::command]
pub async fn list_provider_models(
    kind: String,
    vendor: String,
    endpoint: String,
    api_key: Option<String>,
    provider_id: Option<String>,
) -> Result<Vec<String>> {
    let key = match api_key {
        Some(k) if !k.trim().is_empty() => k,
        _ => provider_id.and_then(|id| read_key(&id).ok()).unwrap_or_default(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let base = endpoint.trim_end_matches('/');
        match kind.as_str() {
            "local" => {
                let json: serde_json::Value = ureq::get(&format!("{base}/api/tags"))
                    .timeout(Duration::from_secs(5))
                    .call()
                    .map_err(|e| ureq_err("Ollama", e))?
                    .into_json()?;
                Ok(json["models"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
                    .unwrap_or_default())
            }
            "cloud" if vendor == "gemini" => {
                // Use Gemini's native endpoint so we can keep only models that
                // support chat (`generateContent`); the OpenAI-compat list mixes
                // in embedding/imagen/interactions-only models that 400 on chat.
                let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={key}");
                let json: serde_json::Value = ureq::get(&url)
                    .timeout(Duration::from_secs(15))
                    .call()
                    .map_err(|e| ureq_err("Gemini models", e))?
                    .into_json()?;
                let mut ids: Vec<String> = json["models"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|m| {
                                m["supportedGenerationMethods"]
                                    .as_array()
                                    .map(|meth| meth.iter().any(|x| x.as_str() == Some("generateContent")))
                                    .unwrap_or(false)
                            })
                            .filter_map(|m| {
                                m["name"].as_str().map(|n| n.trim_start_matches("models/").to_string())
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                ids.sort();
                Ok(ids)
            }
            "cloud" => {
                let req = if vendor == "anthropic" {
                    ureq::get(&format!("{base}/models"))
                        .set("x-api-key", &key)
                        .set("anthropic-version", "2023-06-01")
                } else {
                    ureq::get(&format!("{base}/models")).set("authorization", &format!("Bearer {key}"))
                };
                let json: serde_json::Value = req
                    .timeout(Duration::from_secs(15))
                    .call()
                    .map_err(|e| ureq_err("Models request failed", e))?
                    .into_json()?;
                let mut ids: Vec<String> = json["data"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
                    .unwrap_or_default();
                ids.sort();
                Ok(ids)
            }
            other => Err(AiError::Msg(format!("Can't list models for '{other}'."))),
        }
    })
    .await
    .map_err(|e| AiError::Msg(e.to_string()))?
}

/* ── Auto-detect keys already on the machine ──────────────────────── */

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvKey {
    pub var: String,
    pub vendor: String,
    pub masked: String,
}

/// Read an env var, falling back to a login+interactive shell so keys exported
/// from the user's profile are visible even when launched from Finder.
fn read_shell_var(var: &str) -> Option<String> {
    if let Ok(v) = std::env::var(var) {
        if !v.is_empty() {
            return Some(v);
        }
    }
    let out = std::process::Command::new("zsh")
        .args(["-lic", &format!("printf %s \"${{{var}}}\"")])
        .output()
        .ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn mask(key: &str) -> String {
    let n = key.len();
    if n <= 8 {
        "•".repeat(n.max(3))
    } else {
        format!("{}…{}", &key[..4], &key[n - 4..])
    }
}

#[tauri::command]
pub async fn detect_env_keys() -> Vec<EnvKey> {
    // Off the main thread, and one shell for all vars — spawning login shells
    // is slow, so doing it per-var on the UI thread froze the panel.
    tauri::async_runtime::spawn_blocking(|| {
        let vars = [
            ("ANTHROPIC_API_KEY", "anthropic"),
            ("OPENAI_API_KEY", "openai"),
            ("OPENROUTER_API_KEY", "openai-compatible"),
        ];
        let script = vars
            .iter()
            .map(|(v, _)| format!("printf '%s\\n' \"${{{v}}}\""))
            .collect::<Vec<_>>()
            .join("; ");
        let shell_lines: Vec<String> = std::process::Command::new("zsh")
            .args(["-lic", &script])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).collect())
            .unwrap_or_default();

        vars.iter()
            .enumerate()
            .filter_map(|(i, (var, vendor))| {
                let v = std::env::var(var)
                    .ok()
                    .filter(|s| !s.is_empty())
                    .or_else(|| shell_lines.get(i).cloned().filter(|s| !s.is_empty()))?;
                Some(EnvKey {
                    var: var.to_string(),
                    vendor: vendor.to_string(),
                    masked: mask(&v),
                })
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Save a provider whose key comes from an environment variable (read
/// server-side and written straight to the Keychain — never via the frontend).
#[tauri::command]
pub fn save_ai_provider_from_env(
    app: AppHandle,
    provider: AiProvider,
    env_var: String,
    make_default: bool,
) -> Result<AiConfig> {
    let key = read_shell_var(&env_var).ok_or_else(|| AiError::Msg(format!("{env_var} is no longer set.")))?;
    store_key(&provider.id, &key)?;
    let mut cfg = load(&app)?;
    match cfg.providers.iter_mut().find(|p| p.id == provider.id) {
        Some(existing) => *existing = provider.clone(),
        None => cfg.providers.push(provider.clone()),
    }
    if make_default || cfg.default_id.is_none() {
        cfg.default_id = Some(provider.id);
    }
    store(&app, &cfg)?;
    Ok(cfg)
}

/* ── OpenRouter one-click login (OAuth PKCE, no central server) ────── */

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRouterResult {
    pub provider_id: String,
    pub models: Vec<String>,
}

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

#[tauri::command]
pub async fn openrouter_login(app: AppHandle) -> Result<OpenRouterResult> {
    // PKCE verifier + challenge
    let mut vb = [0u8; 32];
    getrandom::getrandom(&mut vb).map_err(|e| AiError::Msg(format!("rng: {e}")))?;
    let verifier = b64url(&vb);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));

    // Loopback listener to catch the redirect
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let callback = format!("http://127.0.0.1:{port}/callback");

    let auth_url = format!(
        "https://openrouter.ai/auth?callback_url={}&code_challenge={}&code_challenge_method=S256",
        urlencode(&callback),
        challenge
    );
    let _ = std::process::Command::new("open").arg(&auth_url).spawn();

    let code = tauri::async_runtime::spawn_blocking(move || wait_for_code(listener))
        .await
        .map_err(|e| AiError::Msg(e.to_string()))??;

    let key = tauri::async_runtime::spawn_blocking(move || exchange_code(&code, &verifier))
        .await
        .map_err(|e| AiError::Msg(e.to_string()))??;

    let mut idb = [0u8; 8];
    let _ = getrandom::getrandom(&mut idb);
    let id = format!("or-{}", b64url(&idb));
    store_key(&id, &key)?;

    let mut cfg = load(&app)?;
    cfg.providers.push(AiProvider {
        id: id.clone(),
        kind: "cloud".into(),
        vendor: "openai-compatible".into(),
        label: "OpenRouter".into(),
        model: String::new(),
        endpoint: "https://openrouter.ai/api/v1".into(),
    });
    if cfg.default_id.is_none() {
        cfg.default_id = Some(id.clone());
    }
    store(&app, &cfg)?;

    let models = tauri::async_runtime::spawn_blocking(|| {
        let json: serde_json::Value = ureq::get("https://openrouter.ai/api/v1/models")
            .timeout(Duration::from_secs(15))
            .call()
            .map_err(|e| ureq_err("OpenRouter models", e))?
            .into_json()?;
        let mut ids: Vec<String> = json["data"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
            .unwrap_or_default();
        ids.sort();
        Ok::<Vec<String>, AiError>(ids)
    })
    .await
    .map_err(|e| AiError::Msg(e.to_string()))??;

    Ok(OpenRouterResult { provider_id: id, models })
}

fn wait_for_code(listener: std::net::TcpListener) -> Result<String> {
    use std::io::{Read, Write};
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let code = req
                    .split_whitespace()
                    .nth(1)
                    .and_then(|path| path.split('?').nth(1))
                    .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("code=")))
                    .map(|c| c.to_string());
                let body = "<html><body style='font-family:-apple-system,sans-serif;padding:48px;text-align:center'><h2>Connected to Plumb</h2><p>You can close this tab and return to the app.</p></body></html>";
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.flush();
                return code.ok_or_else(|| AiError::Msg("No authorization code in the callback.".into()));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err(AiError::Msg("Login timed out — no response from the browser.".into()));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(AiError::Msg(format!("Callback error: {e}"))),
        }
    }
}

fn exchange_code(code: &str, verifier: &str) -> Result<String> {
    let body = serde_json::json!({
        "code": code,
        "code_verifier": verifier,
        "code_challenge_method": "S256",
    });
    let json: serde_json::Value = ureq::post("https://openrouter.ai/api/v1/auth/keys")
        .timeout(Duration::from_secs(30))
        .send_json(body)
        .map_err(|e| ureq_err("OpenRouter token exchange failed", e))?
        .into_json()?;
    json["key"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| AiError::Msg("OpenRouter did not return a key.".into()))
}

fn ollama_generate(endpoint: &str, model: &str, prompt: &str) -> Result<String> {
    let url = format!("{}/api/chat", endpoint.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "stream": false,
        "options": { "temperature": 0.2 }
    });
    let resp = ureq::post(&url)
        .timeout(Duration::from_secs(120))
        .send_json(body)
        .map_err(|e| AiError::Msg(format!("Ollama request failed: {e}")))?;
    let json: serde_json::Value = resp.into_json()?;
    let content = json["message"]["content"].as_str().unwrap_or("").trim().to_string();
    if content.is_empty() {
        return Err(AiError::Msg("Ollama returned an empty message.".into()));
    }
    Ok(content)
}
