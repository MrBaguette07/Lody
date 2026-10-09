use crate::agents::{Agent, AgentStore};
use crate::approvals::{Approval, ApprovalStore, Decision};
use crate::config::{self, Settings};
use crate::devproc::ProcRegistry;
use crate::runner::RunRegistry;
use crate::{devproc, git, hooks, projects, providers, runner, util, window};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub agents: AgentStore,
    pub approvals: ApprovalStore,
    pub runs: RunRegistry,
    pub procs: ProcRegistry,
}

// ---------------------------------------------------------------- commandes

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    let mut s = state.settings.lock().unwrap().clone();
    config::refresh_key_flags(&mut s);
    s
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    let mut s = settings;
    {
        let cur = state.settings.lock().unwrap();
        // La position de la mascotte est gérée à part.
        s.mascot_x = cur.mascot_x;
        s.mascot_y = cur.mascot_y;
    }
    config::refresh_key_flags(&mut s);
    config::save(&app, &s)?;
    *state.settings.lock().unwrap() = s.clone();
    let _ = app.emit("settings", &s);
    Ok(s)
}

#[tauri::command]
fn set_secret(app: AppHandle, state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    config::set_secret(&key, &value)?;
    let mut s = state.settings.lock().unwrap();
    config::refresh_key_flags(&mut s);
    let _ = app.emit("settings", &*s);
    Ok(())
}

#[tauri::command]
fn detect_bins(state: State<'_, AppState>) -> Value {
    let s = state.settings.lock().unwrap().clone();
    json!({
        "claude": util::find_claude(&s.claude_path),
        "codex": util::find_codex(&s.codex_path),
    })
}

#[tauri::command]
fn hooks_status(state: State<'_, AppState>) -> hooks::HookStatus {
    hooks::status(state.settings.lock().unwrap().hook_port)
}

#[tauri::command]
fn hooks_install(state: State<'_, AppState>) -> Result<hooks::HookStatus, String> {
    let port = state.settings.lock().unwrap().hook_port;
    hooks::install(port)?;
    Ok(hooks::status(port))
}

#[tauri::command]
fn hooks_uninstall(state: State<'_, AppState>) -> Result<hooks::HookStatus, String> {
    hooks::uninstall()?;
    Ok(hooks::status(state.settings.lock().unwrap().hook_port))
}

#[tauri::command]
fn list_agents(state: State<'_, AppState>) -> Vec<Agent> {
    state.agents.list()
}

#[tauri::command]
fn dismiss_agent(app: AppHandle, state: State<'_, AppState>, id: String) {
    state.agents.remove(&app, &id);
}

#[tauri::command]
fn list_approvals(state: State<'_, AppState>) -> Vec<Approval> {
    state.approvals.list()
}

#[tauri::command]
fn resolve_approval(state: State<'_, AppState>, id: String, decision: String) -> bool {
    state.approvals.resolve(&id, Decision::parse(&decision))
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Ouvre un dossier dans un éditeur, l'explorateur ou un terminal.
#[tauri::command]
fn open_in(app: AppHandle, path: String, target: String) -> Result<(), String> {
    use std::process::Command;
    let shell = |line: String| -> Result<(), String> {
        let mut c = Command::new("cmd");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.raw_arg(format!("/C {line}"));
        }
        util::hide_console(&mut c);
        c.spawn().map(|_| ()).map_err(|e| e.to_string())
    };
    match target.as_str() {
        "code" => shell(format!("code \"{path}\"")),
        "cursor" => shell(format!("cursor \"{path}\"")),
        "terminal" => {
            if Command::new("wt").arg("-d").arg(&path).spawn().is_ok() {
                Ok(())
            } else {
                shell(format!("start \"\" cmd /K \"cd /d {path}\""))
            }
        }
        _ => app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string()),
    }
}

/// Derniers fichiers du dossier Téléchargements (contexte pour l'orchestrateur).
#[tauri::command]
fn recent_downloads(limit: Option<usize>) -> Vec<Value> {
    let Some(dir) = dirs::download_dir() else { return vec![] };
    let mut files: Vec<(i64, Value)> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let m = e.metadata().ok()?;
                    if !m.is_file() {
                        return None;
                    }
                    let t = m
                        .modified()
                        .ok()?
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()?
                        .as_millis() as i64;
                    Some((t, json!({
                        "path": e.path().display().to_string(),
                        "name": e.file_name().to_string_lossy(),
                        "modifiedAt": t,
                        "size": m.len(),
                    })))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().take(limit.unwrap_or(10)).map(|(_, v)| v).collect()
}

/// Lit un fichier texte joint (pour les fournisseurs API qui n'ont pas accès au disque).
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    const MAX: usize = 200_000;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return Err("fichier binaire".into());
    }
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX)]).to_string();
    Ok(if bytes.len() > MAX { format!("{text}\n…(tronqué)") } else { text })
}

#[tauri::command]
fn app_info(app: AppHandle) -> Value {
    json!({
        "version": app.package_info().version.to_string(),
        "exe": std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default(),
    })
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    devproc::stop_all(&app.state::<AppState>());
    app.exit(0);
}

// ---------------------------------------------------------------- démarrage

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Ouvrir Lody", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "mascot_show", "Afficher la mascotte", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "mascot_hide", "Masquer la mascotte", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &show, &hide, &quit])?;
    let mut tray = TrayIconBuilder::with_id("lody")
        .tooltip("Lody")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => window::open_island(app, None),
            "mascot_show" => window::place_mascot(app),
            "mascot_hide" => {
                if let Some(w) = app.get_webview_window("mascot") {
                    let _ = w.hide();
                }
            }
            "quit" => {
                devproc::stop_all(&app.state::<AppState>());
                app.exit(0)
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                window::toggle_island(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // `lody.exe --tab projects` ouvre directement un onglet.
            let tab = args
                .iter()
                .position(|a| a == "--tab")
                .and_then(|i| args.get(i + 1).cloned());
            window::open_island(app, tab);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, ev| {
                    if ev.state() == ShortcutState::Pressed {
                        window::toggle_island(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = config::load(&handle);
            let port = settings.hook_port;
            let shortcut = settings.shortcut.clone();
            app.manage(AppState {
                settings: Mutex::new(settings),
                agents: AgentStore::default(),
                approvals: ApprovalStore::default(),
                runs: RunRegistry::default(),
                procs: ProcRegistry::default(),
            });

            // Fenêtres créées après `manage()` : leurs premières commandes trouvent l'état.
            let windows = app.config().app.windows.clone();
            for cfg in &windows {
                tauri::WebviewWindowBuilder::from_config(&handle, cfg)?.build()?;
            }

            hooks::start_server(handle.clone(), port);
            setup_tray(app)?;
            if !shortcut.is_empty() {
                if let Err(e) = app.global_shortcut().register(shortcut.as_str()) {
                    eprintln!("Raccourci {shortcut} indisponible : {e}");
                }
            }
            window::place_mascot(&handle);
            window::spawn_cursor_tracker(handle.clone());

            let h2 = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(30));
                h2.state::<AppState>().agents.prune(&h2);
            });
            Ok(())
        })
        .on_window_event(|w, e| {
            // Alt+F4 sur l'island la masque au lieu de quitter Lody.
            if let WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            set_secret,
            detect_bins,
            hooks_status,
            hooks_install,
            hooks_uninstall,
            list_agents,
            dismiss_agent,
            list_approvals,
            resolve_approval,
            open_url,
            open_in,
            read_text_file,
            recent_downloads,
            app_info,
            crate::github::repo_status,
            crate::github::gh_overview,
            crate::github::git_commit,
            crate::github::git_sync,
            crate::github::gh_pr_action,
            crate::github::gh_pr_create,
            crate::github::gh_run_rerun,
            quit_app,
            runner::run_cli,
            runner::stop_run,
            providers::chat_api,
            providers::list_models,
            projects::list_projects,
            projects::clone_repo,
            projects::claude_mcp_servers,
            projects::mcp_list_tools,
            git::git_diff,
            devproc::start_process,
            devproc::stop_process,
            devproc::suggest_command,
            window::focus_mascot,
            window::save_mascot_pos,
            window::reset_mascot_pos,
            window::set_mascot_visible,
        ])
        .run(tauri::generate_context!())
        .expect("Lody n'a pas pu démarrer");
}
