mod accounts;
mod actions;
mod ai;
mod git;
mod secrets;
mod serve;
mod watcher;

use tauri::menu::{AboutMetadata, MenuBuilder, MenuItem, SubmenuBuilder};
use tauri::{AppHandle, Runtime};

/// Build the native application menu. Custom items carry ids that the frontend
/// receives via the "menu-action" event; predefined items (copy, quit, …) are
/// handled by the OS. Accelerators live here so they show next to each item and
/// work app-wide — the frontend no longer binds these keys itself.
fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let mi = |id: &str, text: &str, accel: Option<&str>| MenuItem::with_id(app, id, text, true, accel);

    let app_menu = SubmenuBuilder::new(app, "Plumb")
        .about(Some(AboutMetadata {
            name: Some("Plumb".into()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            comments: Some("A straight line through your history.".into()),
            ..Default::default()
        }))
        .separator()
        .item(&mi("settings", "Settings…", Some("CmdOrCtrl+,"))?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        // ⌘Q hides Plumb to the menu bar (keeps the serve agent alive). The
        // predefined .quit() would call native terminate: and hard-quit,
        // bypassing our exit interception — so use custom items instead.
        .item(&mi("hide_to_tray", "Close to Menu Bar", Some("CmdOrCtrl+Q"))?)
        .item(&mi("app_quit", "Quit Plumb", Some("CmdOrCtrl+Shift+Q"))?)
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&mi("new_tab", "New Tab", Some("CmdOrCtrl+T"))?)
        .item(&mi("close_tab", "Close Tab", Some("CmdOrCtrl+W"))?)
        .separator()
        .item(&mi("open_repo", "Open Repository…", Some("CmdOrCtrl+O"))?)
        .item(&mi("clone_repo", "Clone Repository…", Some("CmdOrCtrl+Shift+O"))?)
        .item(&mi("init_repo", "Initialize Repository…", Some("CmdOrCtrl+I"))?)
        .separator()
        .item(&mi("accounts", "Connect an Account…", Some("CmdOrCtrl+N"))?)
        .separator()
        .item(&mi("reveal", "Reveal in Finder", None)?)
        .item(&mi("terminal", "Open in Terminal", None)?)
        .item(&mi("editor", "Open in Editor", None)?)
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&mi("command_palette", "Command Palette…", Some("CmdOrCtrl+K"))?)
        .separator()
        .item(&mi("view_changes", "Changes", Some("CmdOrCtrl+1"))?)
        .item(&mi("view_history", "History", Some("CmdOrCtrl+2"))?)
        .item(&mi("view_prs", "Pull Requests", Some("CmdOrCtrl+3"))?)
        .separator()
        .item(&mi("toggle_theme", "Toggle Theme", Some("CmdOrCtrl+Shift+L"))?)
        .build()?;

    let repo_menu = SubmenuBuilder::new(app, "Repository")
        .item(&mi("fetch", "Fetch", Some("CmdOrCtrl+R"))?)
        .item(&mi("pull", "Pull", Some("CmdOrCtrl+Shift+P"))?)
        .item(&mi("push", "Push", Some("CmdOrCtrl+P"))?)
        .separator()
        .item(&mi("new_branch", "New Branch…", Some("CmdOrCtrl+B"))?)
        .item(&mi("merge", "Merge…", None)?)
        .item(&mi("rebase", "Rebase…", None)?)
        .item(&mi("flow", "Workflows…", None)?)
        .item(&mi("stash", "Stash Changes…", Some("CmdOrCtrl+Shift+S"))?)
        .separator()
        .item(&mi("new_pr", "New Pull / Merge Request…", None)?)
        .item(&mi("run_pipeline", "Run Pipeline…", None)?)
        .item(&mi("compare", "Compare Branches…", None)?)
        .item(&mi("reflog", "History (Reflog)…", None)?)
        .item(&mi("remotes", "Manage Remotes…", None)?)
        .separator()
        .item(&mi("submodules", "Submodules…", None)?)
        .item(&mi("worktrees", "Worktrees…", None)?)
        .item(&mi("bisect", "Bisect…", None)?)
        .separator()
        .item(&mi("repo_info", "Repository Info…", None)?)
        .item(&mi("repo_settings", "Repository Settings…", Some("CmdOrCtrl+;"))?)
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .separator()
        .close_window()
        .build()?;

    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&mi("github", "Plumb on GitHub", None)?)
        .build()?;

    let menu = MenuBuilder::new(app)
        .items(&[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &repo_menu,
            &window_menu,
            &help_menu,
        ])
        .build()?;

    app.set_menu(menu)?;
    Ok(())
}

/// A repository path Plumb was launched with (CLI arg or editor integration),
/// handed to the frontend once on startup.
struct LaunchPath(std::sync::Mutex<Option<String>>);

/// Take (and clear) the path Plumb was opened with, if any.
#[tauri::command]
fn initial_path(state: tauri::State<LaunchPath>) -> Option<String> {
    state.0.lock().ok().and_then(|mut g| g.take())
}

/// Locate a usable VS Code `code` CLI.
fn find_code() -> Option<String> {
    let mut candidates = vec!["code".to_string(), "code-insiders".to_string()];
    #[cfg(target_os = "macos")]
    {
        candidates.push("/usr/local/bin/code".into());
        candidates.push("/opt/homebrew/bin/code".into());
        candidates.push("/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code".into());
    }
    candidates.into_iter().find(|c| {
        std::process::Command::new(c)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Download the packaged VS Code extension from the latest GitHub release and
/// install it via the `code` CLI.
#[tauri::command]
async fn install_vscode_extension() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        use std::io::Read;
        let code = find_code().ok_or_else(|| {
            "VS Code's `code` command isn't on your PATH. In VS Code, run “Shell Command: Install 'code' command in PATH”, then try again.".to_string()
        })?;
        let rel: serde_json::Value = ureq::get("https://api.github.com/repos/Cyapow/plumb/releases/latest")
            .set("user-agent", "Plumb")
            .set("accept", "application/vnd.github+json")
            .timeout(std::time::Duration::from_secs(20))
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let url = rel["assets"]
            .as_array()
            .and_then(|a| a.iter().find(|x| x["name"].as_str().map(|n| n.ends_with(".vsix")).unwrap_or(false)))
            .and_then(|x| x["browser_download_url"].as_str())
            .ok_or_else(|| "The latest release doesn't include a VS Code extension yet.".to_string())?;
        let mut bytes = Vec::new();
        ureq::get(url)
            .timeout(std::time::Duration::from_secs(60))
            .call()
            .map_err(|e| e.to_string())?
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let tmp = std::env::temp_dir().join("plumb-vscode.vsix");
        std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        let out = std::process::Command::new(&code)
            .arg("--install-extension")
            .arg(&tmp)
            .arg("--force")
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok("VS Code extension installed. Reload VS Code, then run “Plumb: Open Plumb Panel”.".to_string())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whether Plumb's background server is set to launch at login.
#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Enable/disable launching the background server (serve mode) at login.
#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let m = app.autolaunch();
    if enabled { m.enable() } else { m.disable() }.map_err(|e| e.to_string())
}

/// The first CLI argument that resolves to an existing directory (a repo to
/// open) — skips flags and the `serve` subcommand.
fn arg_repo_path(args: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-') && a.as_str() != "serve")
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .find(|p| p.is_dir())
        .map(|p| p.to_string_lossy().to_string())
}

/// Promote to a normal windowed app (macOS Dock icon) and show the main window.
/// Used by the tray "Open" item and when a second launch is forwarded to a
/// running (possibly menu-bar-only) instance.
fn show_main<R: Runtime>(app: &AppHandle<R>) {
    use tauri::Manager as _;
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Tray (menu-bar) icon shown in serve mode, with Open / Quit.
fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItem};
    use tauri::tray::TrayIconBuilder;
    #[cfg(not(target_os = "macos"))]
    use tauri::Manager;

    let open = MenuItem::with_id(app, "tray_open", "Open Plumb Window", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray_quit", "Quit Plumb", true, None::<&str>)?;
    let menu = MenuBuilder::new(app).items(&[&open, &quit]).build()?;

    let mut builder = TrayIconBuilder::new().tooltip("Plumb — serving").menu(&menu).on_menu_event(|app, event| {
        match event.id().0.as_str() {
            "tray_quit" => {
                serve::clear_discovery();
                app.exit(0);
            }
            "tray_open" => show_main(app),
            _ => {}
        }
    });
    // macOS: a monochrome template icon so the menu bar tints it black/white to
    // match the wallpaper. Other platforms: the normal colored logo.
    #[cfg(target_os = "macos")]
    {
        if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png")) {
            builder = builder.icon(icon).icon_as_template(true);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
    }
    builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let launched = arg_repo_path(&std::env::args().collect::<Vec<_>>());
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        // Single-instance must be first: a second `plumb <path>` invocation
        // forwards its path to the running window instead of opening anew.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            use tauri::Emitter;
            // A repo path forwarded from `plumb <path>` opens that repo.
            if let Some(path) = arg_repo_path(&argv) {
                let _ = app.emit("open-path", path);
            }
            // A plain relaunch (Spotlight / Raycast / Dock) should surface the
            // window even if this instance is running as a menu-bar agent.
            show_main(app);
        }))
        .manage(LaunchPath(std::sync::Mutex::new(launched)))
        .manage(watcher::WatchState::default())
        // At-login autostart launches the background server (menu-bar agent).
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["serve"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());
    // Window-chrome tweaks (traffic-light inset / snap overlay) — macOS + Windows.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        builder = builder.plugin(tauri_plugin_decorum::init());
    }
    builder
        // Closing the main window hides Plumb to the menu bar instead of quitting:
        // the `serve` agent + tray keep running so editors stay connected. Quit is
        // explicit (tray "Quit Plumb" or ⌘Q, which fire ExitRequested, not this).
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                    #[cfg(target_os = "macos")]
                    {
                        use tauri::Manager as _;
                        let _ = window
                            .app_handle()
                            .set_activation_policy(tauri::ActivationPolicy::Accessory);
                    }
                }
            }
        })
        .setup(|app| {
            // Vertically centre the macOS traffic lights in our 52px header, and
            // keep them there across show/resize (which macOS otherwise resets).
            #[cfg(target_os = "macos")]
            {
                use tauri::Manager;
                use tauri_plugin_decorum::WebviewWindowExt;
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_traffic_lights_inset(16.0, 13.0);
                }
            }

            build_menu(app.handle())?;
            // Menu clicks become "menu-action" events the frontend dispatches.
            app.on_menu_event(|app, event| {
                use tauri::Emitter;
                match event.id().0.as_str() {
                    // ⌘Q → hide to the menu bar; the agent + tray stay alive.
                    "hide_to_tray" => {
                        use tauri::Manager as _;
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.hide();
                        }
                        #[cfg(target_os = "macos")]
                        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                    }
                    // ⌘⇧Q → a genuine quit (matches the tray "Quit Plumb").
                    "app_quit" => {
                        serve::clear_discovery();
                        app.exit(0);
                    }
                    other => {
                        let _ = app.emit("menu-action", other.to_string());
                    }
                }
            });

            // Always run the local server + menu-bar tray, so editors can connect
            // whenever Plumb is open. `serve` mode additionally stays headless (no
            // window, no Dock icon — a menu-bar-only agent).
            {
                use tauri::Manager;
                let serve_mode = std::env::args().any(|a| a == "serve");
                let repo = arg_repo_path(&std::env::args().collect::<Vec<_>>());
                serve::start(app.handle().clone(), repo);
                build_tray(app.handle())?;
                if serve_mode {
                    #[cfg(target_os = "macos")]
                    let _ = app.handle().set_activation_policy(tauri::ActivationPolicy::Accessory);
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.hide();
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initial_path,
            get_autostart,
            set_autostart,
            install_vscode_extension,
            git::open_repo,
            git::is_repo,
            git::init_repo,
            git::open_in_terminal,
            git::open_in_editor,
            git::add_to_gitignore,
            git::reword_commit,
            git::set_diff_ignore_ws,
            git::list_system_fonts,
            git::list_remotes,
            git::add_remote,
            git::list_commits,
            git::list_branches,
            git::working_status,
            git::stage_paths,
            git::unstage_paths,
            git::stage_hunk,
            git::unstage_hunk,
            git::stage_lines,
            git::unstage_lines,
            git::clone_repo,
            git::list_stashes,
            git::stash_save,
            git::stash_save_ex,
            git::stash_apply,
            git::stash_apply_ex,
            git::stash_pop,
            git::stash_drop,
            git::list_tags,
            git::list_files,
            git::file_history,
            git::blame_file,
            git::merge_branch,
            git::merge_branch_ex,
            git::rebase_branch,
            git::rebase_branch_ex,
            git::cherry_pick,
            git::revert_commit,
            git::op_abort,
            git::op_continue,
            git::repo_state,
            git::reflog,
            git::list_conflicts,
            git::conflict_sides,
            git::resolve_conflict,
            git::resolve_conflict_content,
            git::file_diff,
            git::git_identity,
            git::set_git_identity,
            git::get_config,
            git::set_config,
            git::unset_config,
            git::get_repo_description,
            git::set_repo_description,
            git::get_gitignore,
            git::set_gitignore,
            git::initial_commit,
            git::list_remote_branches,
            git::connect_remote_branch,
            git::commit,
            git::unstage_all,
            git::uncommit,
            git::commit_details,
            git::commit_file_diff,
            git::compare_refs,
            git::compare_file_diff,
            git::search_commits,
            git::checkout_branch,
            git::checkout_remote_branch,
            git::checkout_commit,
            git::create_branch,
            git::reset,
            git::discard_paths,
            git::delete_branch,
            git::delete_tag,
            git::fetch,
            git::pull,
            git::push,
            git::push_advanced,
            git::push_branch,
            git::pull_mode,
            git::rebase_interactive,
            git::delete_remote_branch,
            git::flow_config,
            git::flow_init,
            git::flow_start,
            git::flow_finish,
            git::flow_set_type,
            git::flow_set_environments,
            git::merge_into,
            git::list_submodules,
            git::update_submodules,
            git::list_worktrees,
            git::add_worktree,
            git::remove_worktree,
            git::bisect_status,
            git::bisect_start,
            git::bisect_mark,
            git::bisect_reset,
            git::rename_remote,
            git::remove_remote,
            git::set_remote_url,
            git::prune_remote,
            ai::list_ai_providers,
            ai::save_ai_provider,
            ai::remove_ai_provider,
            ai::set_default_ai_provider,
            ai::has_api_key,
            ai::list_ollama_models,
            ai::list_provider_models,
            ai::detect_env_keys,
            ai::save_ai_provider_from_env,
            ai::openrouter_login,
            ai::generate_commit_message,
            ai::explain_diff,
            ai::ai_group_changes,
            ai::test_ai_provider,
            accounts::list_connections,
            accounts::connect_account,
            accounts::remove_connection,
            accounts::test_connection,
            accounts::github_device_start,
            accounts::github_device_poll,
            accounts::gitlab_oauth_login,
            accounts::list_pull_requests,
            accounts::list_ci_statuses,
            accounts::list_workflows,
            accounts::trigger_pipeline,
            accounts::pipeline_detail,
            accounts::pipeline_action,
            accounts::list_pipelines,
            accounts::job_log,
            accounts::pr_target,
            accounts::create_pull_request,
            accounts::list_account_repos,
            accounts::create_remote_repo,
            watcher::watch_repo,
            actions::list_actions,
            actions::save_actions,
            actions::run_action,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| match event {
            // ⌘Q (and last-window-close) request an app exit. Keep the menu-bar
            // agent alive instead so editors stay connected — hide to the tray.
            // A real quit comes from the tray "Quit Plumb" item, which calls
            // app.exit() and fires RunEvent::Exit directly (below), not this.
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
                use tauri::Manager as _;
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
                #[cfg(target_os = "macos")]
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            // Clear the agent's advertisement on a genuine exit so editors don't
            // find a stale server.
            tauri::RunEvent::Exit => serve::clear_discovery(),
            _ => {}
        });
}
