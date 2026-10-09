//! Lance un serveur de dev (npm run dev…) pour l'onglet Aperçu.

use crate::app::AppState;
use crate::util::{hide_console, kill_tree};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
pub struct ProcRegistry {
    inner: Mutex<HashMap<String, u32>>,
}

fn pump(app: AppHandle, id: String, r: impl Read + Send + 'static, err: bool) {
    std::thread::spawn(move || {
        for line in BufReader::new(r).lines().map_while(Result::ok) {
            let _ = app.emit("proc", json!({"id": id, "line": line, "err": err}));
        }
    });
}

#[tauri::command]
pub fn start_process(app: AppHandle, state: State<'_, AppState>, id: String, cwd: String, command: String) -> Result<(), String> {
    if let Some(pid) = state.procs.inner.lock().unwrap().remove(&id) {
        kill_tree(pid);
    }
    let mut cmd = Command::new("cmd");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(format!("/C {command}"));
    }
    cmd.current_dir(&cwd)
        .env("FORCE_COLOR", "0")
        .env("NO_COLOR", "1")
        .env("BROWSER", "none")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    state.procs.inner.lock().unwrap().insert(id.clone(), child.id());
    pump(app.clone(), id.clone(), child.stdout.take().unwrap(), false);
    pump(app.clone(), id.clone(), child.stderr.take().unwrap(), true);
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code());
        let st = app.state::<AppState>();
        let mut m = st.procs.inner.lock().unwrap();
        if m.get(&id) == Some(&child.id()) {
            m.remove(&id);
        }
        drop(m);
        let _ = app.emit("proc", json!({"id": id, "exit": code}));
    });
    Ok(())
}

#[tauri::command]
pub fn stop_process(state: State<'_, AppState>, id: String) -> bool {
    match state.procs.inner.lock().unwrap().remove(&id) {
        Some(pid) => {
            kill_tree(pid);
            true
        }
        None => false,
    }
}

pub fn stop_all(state: &AppState) {
    for (_, pid) in state.procs.inner.lock().unwrap().drain() {
        kill_tree(pid);
    }
}

/// Devine la commande de lancement du projet.
#[tauri::command]
pub fn suggest_command(cwd: String) -> Option<String> {
    let dir = Path::new(&cwd);
    if let Ok(txt) = std::fs::read_to_string(dir.join("package.json")) {
        let pkg: Value = serde_json::from_str(&txt).unwrap_or(Value::Null);
        let script = ["dev", "start", "serve", "preview"]
            .into_iter()
            .find(|s| pkg["scripts"][*s].is_string())?;
        let pm = if dir.join("bun.lock").exists() || dir.join("bun.lockb").exists() {
            "bun run"
        } else if dir.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if dir.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm run"
        };
        return Some(format!("{pm} {script}"));
    }
    if dir.join("manage.py").exists() {
        return Some("python manage.py runserver".into());
    }
    if dir.join("Cargo.toml").exists() {
        return Some("cargo run".into());
    }
    if dir.join("go.mod").exists() {
        return Some("go run .".into());
    }
    if dir.join("index.html").exists() {
        return Some("npx --yes serve -l 4173 .".into());
    }
    None
}
