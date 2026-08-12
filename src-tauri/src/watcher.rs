//! Filesystem watcher: emits "repo-changed" when the open repo's files or refs
//! change (your editor, terminal git, branch switches), so the UI auto-refreshes.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use tauri::{AppHandle, Emitter, State};

/// Holds the active watcher; replaced when the repo changes.
pub struct WatchState(pub Mutex<Option<Debouncer<RecommendedWatcher>>>);

impl Default for WatchState {
    fn default() -> Self {
        WatchState(Mutex::new(None))
    }
}

/// Skip noisy paths that don't reflect meaningful repo state.
fn interesting(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("/node_modules/")
        || s.contains("/target/")
        || s.contains("/.git/objects/")
        || s.contains("/dist/")
        || s.contains("/.git/logs/")
    {
        return false;
    }
    !(s.ends_with(".lock") || s.ends_with('~') || s.ends_with(".swp") || s.ends_with(".tmp"))
}

#[tauri::command]
pub fn watch_repo(app: AppHandle, state: State<WatchState>, path: String) -> Result<(), String> {
    let handle = app.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        move |res: DebounceEventResult| {
            if let Ok(events) = res {
                if events.iter().any(|e| interesting(&e.path)) {
                    let _ = handle.emit("repo-changed", ());
                }
            }
        },
    )
    .map_err(|e| e.to_string())?;

    debouncer
        .watcher()
        .watch(Path::new(&path), RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    *state.0.lock().unwrap() = Some(debouncer);
    Ok(())
}
