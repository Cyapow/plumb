//! User-defined "Custom Actions": small, safe, out-of-the-box automation.
//!
//! An action is a program plus a list of argument templates. Placeholders
//! ({repo} {sha} {shortSha} {branch} {file}) are substituted per-argument as
//! literal argv entries — never through a shell — so a crafted sha or branch
//! name can't inject extra commands. The definitions live in the app config
//! dir; the frontend only ever triggers an action by id, so a served (browser)
//! session can run the user's own actions but cannot invent new commands.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomAction {
    pub id: String,
    pub label: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Where the action shows up: "toolbar" | "commit" | "branch" | "file".
    #[serde(default)]
    pub context: String,
    /// Ask for confirmation before running.
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActionConfig {
    #[serde(default)]
    pub actions: Vec<CustomAction>,
}

/// Values a placeholder can expand to, gathered at call time.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActionCtx {
    #[serde(default)]
    pub sha: String,
    #[serde(default)]
    pub short_sha: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub file: String,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| format!("No config dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("actions.json"))
}

fn load(app: &AppHandle) -> Result<ActionConfig, String> {
    let p = config_path(app)?;
    if !p.exists() {
        return Ok(ActionConfig::default());
    }
    let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

#[tauri::command]
pub fn list_actions(app: AppHandle) -> Result<Vec<CustomAction>, String> {
    Ok(load(&app)?.actions)
}

#[tauri::command]
pub fn save_actions(app: AppHandle, actions: Vec<CustomAction>) -> Result<(), String> {
    let cfg = ActionConfig { actions };
    std::fs::write(config_path(&app)?, serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Substitute placeholders into one argument as a literal value.
fn expand(arg: &str, repo: &str, ctx: &ActionCtx) -> String {
    arg.replace("{repo}", repo)
        .replace("{sha}", &ctx.sha)
        .replace("{shortSha}", &ctx.short_sha)
        .replace("{branch}", &ctx.branch)
        .replace("{file}", &ctx.file)
}

/// Run an action by id in the given repo. Returns a short output summary.
#[tauri::command]
pub async fn run_action(app: AppHandle, repo_path: String, id: String, ctx: ActionCtx) -> Result<String, String> {
    let action = load(&app)?
        .actions
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| "No such action.".to_string())?;
    if action.program.trim().is_empty() {
        return Err("This action has no program set.".into());
    }
    let args: Vec<String> = action.args.iter().map(|a| expand(a, &repo_path, &ctx)).collect();

    tauri::async_runtime::spawn_blocking(move || {
        let out = std::process::Command::new(&action.program)
            .args(&args)
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("Couldn't run \"{}\": {e}", action.program))?;
        let mut msg = String::new();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !stdout.trim().is_empty() {
            msg.push_str(stdout.trim());
        }
        if !stderr.trim().is_empty() {
            if !msg.is_empty() {
                msg.push('\n');
            }
            msg.push_str(stderr.trim());
        }
        // Keep the toast light.
        const MAX: usize = 2000;
        if msg.len() > MAX {
            msg = format!("{}…", &msg[..MAX]);
        }
        if out.status.success() {
            Ok(if msg.is_empty() { format!("{} finished.", action.label) } else { msg })
        } else {
            Err(if msg.is_empty() {
                format!("{} exited with an error.", action.label)
            } else {
                msg
            })
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
