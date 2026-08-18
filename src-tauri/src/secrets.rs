//! Secret storage. By default secrets live in the OS keychain (via `keyring`).
//!
//! Set `PLUMB_DEV_SECRETS=1` to store them in a `0600` JSON file under the home
//! directory instead. This exists purely to avoid the repeated macOS Keychain
//! authorization prompts you get while debugging — each rebuild re-signs the
//! binary, which the Keychain treats as a new app, so "Always Allow" never
//! sticks. It is a plaintext store and must NEVER be enabled in a distributed
//! build; it is off unless the env var is explicitly set.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

fn dev_mode() -> bool {
    matches!(std::env::var("PLUMB_DEV_SECRETS").as_deref(), Ok("1") | Ok("true"))
}

fn dev_file() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".plumb-dev-secrets.json")
}

fn dev_load() -> HashMap<String, String> {
    std::fs::read_to_string(dev_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn dev_save(map: &HashMap<String, String>) {
    if let Ok(s) = serde_json::to_string_pretty(map) {
        if let Ok(mut f) = std::fs::File::create(dev_file()) {
            let _ = f.write_all(s.as_bytes());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dev_file(), std::fs::Permissions::from_mode(0o600));
            }
        }
    }
}

fn compound(service: &str, id: &str) -> String {
    format!("{service}\u{1}{id}")
}

/// Store `secret` for (service, id).
pub fn store(service: &str, id: &str, secret: &str) -> Result<(), String> {
    if dev_mode() {
        let mut m = dev_load();
        m.insert(compound(service, id), secret.to_string());
        dev_save(&m);
        return Ok(());
    }
    keyring::Entry::new(service, id)
        .map_err(|e| e.to_string())?
        .set_password(secret)
        .map_err(|e| e.to_string())
}

/// Read the secret for (service, id).
pub fn read(service: &str, id: &str) -> Result<String, String> {
    if dev_mode() {
        return dev_load()
            .get(&compound(service, id))
            .cloned()
            .ok_or_else(|| "not found".into());
    }
    keyring::Entry::new(service, id)
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|e| e.to_string())
}

/// Delete the secret for (service, id) — best effort.
pub fn delete(service: &str, id: &str) {
    if dev_mode() {
        let mut m = dev_load();
        m.remove(&compound(service, id));
        dev_save(&m);
        return;
    }
    if let Ok(e) = keyring::Entry::new(service, id) {
        let _ = e.delete_credential();
    }
}
