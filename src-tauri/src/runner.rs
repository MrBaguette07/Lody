//! Lance Claude Code / Codex en mode non interactif et normalise leur flux JSON
//! en événements `run` pour l'interface.

use crate::agents::Agent;
use crate::app::AppState;
use crate::hooks;
use crate::util::{describe_tool, folder_name, hide_console, kill_tree, now_ms, one_line, str_of, truncate};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub enum RunHandle {
    Pid(u32),
    Cancel(Arc<AtomicBool>),
}

#[derive(Default)]
pub struct RunRegistry {
    inner: Mutex<HashMap<String, RunHandle>>,
}

impl RunRegistry {
    pub fn insert(&self, id: &str, h: RunHandle) {
        self.inner.lock().unwrap().insert(id.to_string(), h);
    }
    pub fn remove(&self, id: &str) {
        self.inner.lock().unwrap().remove(id);
    }
    pub fn stop(&self, id: &str) -> bool {
        match self.inner.lock().unwrap().get(id) {
            Some(RunHandle::Pid(pid)) => {
                kill_tree(*pid);
                true
            }
            Some(RunHandle::Cancel(flag)) => {
                flag.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }
}

pub fn emit_run(app: &AppHandle, run_id: &str, mut ev: Value) {
    ev["runId"] = json!(run_id);
    let _ = app.emit("run", ev);
}

/// Applique un événement normalisé au suivi d'agent.
pub fn apply_to_agent(app: &AppHandle, run_id: &str, ev: &Value) {
    let st = app.state::<AppState>();
    let kind = str_of(ev, "kind");
    st.agents.touch(app, run_id, |a| match kind {
        "session" => a.session_id = ev["sessionId"].as_str().map(String::from),
        "text" => {
            a.status = "thinking".into();
            a.last_message = truncate(str_of(ev, "text"), 600);
        }
        "text_delta" if a.status != "thinking" => a.status = "thinking".into(),
        "tool" => {
            a.status = "working".into();
            a.activity = str_of(ev, "detail").to_string();
            a.tools += 1;
        }
        "done" => {
            let err = ev["isError"].as_bool().unwrap_or(false);
            a.status = if err { "error" } else { "done" }.into();
            a.activity = if err { "Erreur" } else { "Terminé" }.into();
            let r = str_of(ev, "result");
            if !r.is_empty() {
                a.last_message = truncate(r, 600);
            }
        }
        "error" => {
            a.status = "error".into();
            a.activity = one_line(str_of(ev, "message"), 90);
        }
        _ => {}
    });
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliRunArgs {
    pub run_id: String,
    pub provider: String,
    pub prompt: String,
    pub cwd: String,
    pub resume: Option<String>,
    pub mode: String,
    pub model: Option<String>,
    pub title: Option<String>,
    /// Lody « généraliste » : travaille depuis son espace et peut agir sur tous les projets.
    #[serde(default)]
    pub orchestrator: bool,
}

/// Dossier de travail de l'orchestrateur, avec ses consignes pour Claude Code.
fn workspace(app: &AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("workspace");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let claude_md = dir.join("CLAUDE.md");
    if !claude_md.exists() {
        let _ = std::fs::write(
            &claude_md,
            "# Lody\n\nCe dossier est l'espace de travail de Lody, l'assistant de bureau.\n\
             Le contexte de chaque demande (projets, téléchargements récents) est donné dans le message.\n\
             Travaille directement dans le dossier du projet concerné (chemins absolus).\n",
        );
    }
    Ok(dir.display().to_string())
}

/// Dossiers accessibles à l'orchestrateur en plus de son espace.
fn extra_dirs(settings: &crate::config::Settings) -> Vec<String> {
    let mut dirs: Vec<std::path::PathBuf> = if settings.project_roots.is_empty() {
        crate::projects::default_roots()
    } else {
        settings.project_roots.iter().map(std::path::PathBuf::from).collect()
    };
    if let Some(d) = dirs::download_dir() {
        dirs.push(d);
    }
    if let Some(d) = dirs::document_dir() {
        dirs.push(d);
    }
    dirs.into_iter().filter(|d| d.is_dir()).map(|d| d.display().to_string()).collect()
}

#[tauri::command]
pub fn run_cli(app: AppHandle, state: State<'_, AppState>, args: CliRunArgs) -> Result<(), String> {
    let settings = state.settings.lock().unwrap().clone();
    let mut args = args;
    if args.orchestrator {
        args.cwd = workspace(&app)?;
    }
    if !std::path::Path::new(&args.cwd).is_dir() {
        return Err(format!("Dossier introuvable : {}", args.cwd));
    }
    let model = args.model.clone().filter(|m| !m.trim().is_empty());
    let resume = args.resume.clone().filter(|r| !r.trim().is_empty());

    let (bin, argv): (String, Vec<String>) = match args.provider.as_str() {
        "claude" => {
            let bin = crate::util::find_claude(&settings.claude_path)
                .ok_or("Claude Code introuvable. Installe-le ou indique son chemin dans Réglages.")?;
            let mut a: Vec<String> = ["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages"]
                .iter()
                .map(|s| s.to_string())
                .collect();
            let (perm, gate) = match args.mode.as_str() {
                "yolo" => ("bypassPermissions", None),
                "edits" => ("acceptEdits", Some(hooks::GATED_TOOLS_EDITS)),
                _ => ("default", Some(hooks::GATED_TOOLS_ASK)),
            };
            a.extend(["--permission-mode".into(), perm.into()]);
            if let Some(matcher) = gate {
                let f = hooks::gate_settings_file(&args.run_id, settings.hook_port, matcher)?;
                a.extend(["--settings".into(), f]);
            }
            if let Some(r) = &resume {
                a.extend(["--resume".into(), r.clone()]);
            }
            if let Some(m) = &model {
                a.extend(["--model".into(), m.clone()]);
            }
            // Outils « bureau » de Lody (applis, musique, presse-papiers…).
            let mcp = crate::desktop::mcp_config_file(&args.run_id, settings.hook_port)?;
            a.extend(["--mcp-config".into(), mcp, "--allowedTools".into(), crate::desktop::SAFE_TOOLS.into()]);
            if args.orchestrator {
                for d in extra_dirs(&settings) {
                    a.extend(["--add-dir".into(), d]);
                }
            }
            (bin, a)
        }
        "codex" => {
            let bin = crate::util::find_codex(&settings.codex_path)
                .ok_or("Codex CLI introuvable. Installe-le (npm i -g @openai/codex) ou indique son chemin.")?;
            let mut a: Vec<String> = vec!["exec".into(), "--json".into(), "--skip-git-repo-check".into()];
            if args.mode == "yolo" {
                a.push("--dangerously-bypass-approvals-and-sandbox".into());
            } else {
                a.push("--full-auto".into());
            }
            if let Some(m) = &model {
                a.extend(["-m".into(), m.clone()]);
            }
            if let Some(r) = &resume {
                a.extend(["resume".into(), r.clone()]);
            }
            a.push("-".into());
            (bin, a)
        }
        p => return Err(format!("Fournisseur CLI inconnu : {p}")),
    };

    let mut cmd = Command::new(&bin);
    cmd.args(&argv)
        .current_dir(&args.cwd)
        .env("LODY_RUN_ID", &args.run_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("Lancement impossible ({bin}) : {e}"))?;

    let run_id = args.run_id.clone();
    let provider = args.provider.clone();
    state.runs.insert(&run_id, RunHandle::Pid(child.id()));
    state.agents.upsert(
        &app,
        &run_id,
        || Agent {
            id: run_id.clone(),
            source: "lody".into(),
            provider: provider.clone(),
            title: args.title.clone().unwrap_or_else(|| one_line(&args.prompt, 90)),
            project: Some(if args.orchestrator { "Lody".to_string() } else { folder_name(&args.cwd) }),
            cwd: Some(args.cwd.clone()),
            status: "thinking".into(),
            activity: "Démarre…".into(),
            started_at: now_ms(),
            run_id: Some(run_id.clone()),
            ..Default::default()
        },
        |a| {
            a.status = "thinking".into();
            a.title = args.title.clone().unwrap_or_else(|| one_line(&args.prompt, 90));
        },
    );

    if let Some(mut stdin) = child.stdin.take() {
        let prompt = args.prompt.clone();
        std::thread::spawn(move || {
            let _ = stdin.write_all(prompt.as_bytes());
        });
    }

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let err_tail = Arc::new(Mutex::new(Vec::<String>::new()));
    let tail2 = err_tail.clone();
    let stderr_thread = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let mut t = tail2.lock().unwrap();
            t.push(line);
            if t.len() > 40 {
                t.remove(0);
            }
        }
    });

    std::thread::spawn(move || {
        let mut finished = false;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            let mut evs = Vec::new();
            if provider == "claude" {
                parse_claude(&v, &mut evs);
            } else {
                parse_codex(&v, &mut evs);
            }
            for ev in evs {
                if str_of(&ev, "kind") == "done" {
                    finished = true;
                }
                apply_to_agent(&app, &run_id, &ev);
                emit_run(&app, &run_id, ev);
            }
        }
        let _ = stderr_thread.join();
        let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
        if !finished {
            let tail = err_tail.lock().unwrap().join("\n");
            let ev = if code == 0 {
                json!({"kind": "done", "result": "", "isError": false})
            } else {
                json!({"kind": "error", "message": if tail.is_empty() { format!("Processus terminé (code {code})") } else { truncate(&tail, 3000) }})
            };
            apply_to_agent(&app, &run_id, &ev);
            emit_run(&app, &run_id, ev);
        }
        emit_run(&app, &run_id, json!({"kind": "exit", "code": code}));
        let st = app.state::<AppState>();
        st.runs.remove(&run_id);
        let tmp = std::env::temp_dir().join("lody");
        let _ = std::fs::remove_file(tmp.join(format!("run-{run_id}.json")));
        let _ = std::fs::remove_file(tmp.join(format!("mcp-{run_id}.json")));
    });
    Ok(())
}

#[tauri::command]
pub fn stop_run(app: AppHandle, state: State<'_, AppState>, run_id: String) -> bool {
    let ok = state.runs.stop(&run_id);
    if ok {
        state.agents.touch(&app, &run_id, |a| {
            a.status = "done".into();
            a.activity = "Arrêté".into();
        });
        emit_run(&app, &run_id, json!({"kind": "stopped"}));
    }
    ok
}

fn tool_result_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Format `claude -p --output-format stream-json --verbose --include-partial-messages`.
fn parse_claude(v: &Value, out: &mut Vec<Value>) {
    let sub = v.get("parent_tool_use_id").map(|p| !p.is_null()).unwrap_or(false);
    match str_of(v, "type") {
        "system" if str_of(v, "subtype") == "init" => {
            out.push(json!({"kind": "session", "sessionId": v["session_id"], "model": v["model"]}));
        }
        "stream_event" if !sub => {
            let e = &v["event"];
            if str_of(e, "type") == "content_block_delta" && str_of(&e["delta"], "type") == "text_delta" {
                out.push(json!({"kind": "text_delta", "text": e["delta"]["text"]}));
            }
        }
        "assistant" => {
            let Some(blocks) = v["message"]["content"].as_array() else { return };
            for b in blocks {
                match str_of(b, "type") {
                    "text" if !sub => out.push(json!({"kind": "text", "text": b["text"]})),
                    "tool_use" => {
                        let name = str_of(b, "name");
                        out.push(json!({
                            "kind": "tool", "id": b["id"], "name": name, "sub": sub,
                            "detail": describe_tool(name, &b["input"]),
                            "input": truncate(&b["input"].to_string(), 4000),
                        }));
                    }
                    _ => {}
                }
            }
        }
        "user" => {
            let Some(blocks) = v["message"]["content"].as_array() else { return };
            for b in blocks.iter().filter(|b| str_of(b, "type") == "tool_result") {
                out.push(json!({
                    "kind": "tool_result", "id": b["tool_use_id"],
                    "isError": b["is_error"].as_bool().unwrap_or(false),
                    "output": truncate(&tool_result_text(&b["content"]), 4000),
                }));
            }
        }
        "result" => out.push(json!({
            "kind": "done", "result": v["result"], "isError": v["is_error"].as_bool().unwrap_or(false),
            "costUsd": v["total_cost_usd"], "durationMs": v["duration_ms"], "sessionId": v["session_id"],
        })),
        _ => {}
    }
}

/// Format `codex exec --json` (événements thread/turn/item).
fn parse_codex(v: &Value, out: &mut Vec<Value>) {
    let t = str_of(v, "type");
    match t {
        "thread.started" => out.push(json!({"kind": "session", "sessionId": v["thread_id"]})),
        "item.started" | "item.completed" => {
            let it = &v["item"];
            let id = it["id"].clone();
            let done = t == "item.completed";
            let item_type = if it.get("type").is_some() { str_of(it, "type") } else { str_of(it, "item_type") };
            match item_type {
                "agent_message" if done => out.push(json!({"kind": "text", "text": it["text"]})),
                "command_execution" => {
                    if done {
                        let code = it["exit_code"].as_i64().unwrap_or(0);
                        out.push(json!({"kind": "tool_result", "id": id, "isError": code != 0,
                            "output": truncate(str_of(it, "aggregated_output"), 4000)}));
                    } else {
                        out.push(json!({"kind": "tool", "id": id, "name": "Shell",
                            "detail": format!("Shell · {}", one_line(str_of(it, "command"), 90))}));
                    }
                }
                "file_change" if done => {
                    let files: Vec<String> = it["changes"].as_array().into_iter().flatten()
                        .map(|c| folder_name(str_of(c, "path"))).collect();
                    out.push(json!({"kind": "tool", "id": id, "name": "Edit", "detail": format!("Edit · {}", files.join(", "))}));
                    out.push(json!({"kind": "tool_result", "id": id, "isError": str_of(it, "status") == "failed", "output": ""}));
                }
                "mcp_tool_call" => {
                    let name = format!("{}.{}", str_of(it, "server"), str_of(it, "tool"));
                    if done {
                        out.push(json!({"kind": "tool_result", "id": id, "isError": str_of(it, "status") == "failed", "output": ""}));
                    } else {
                        out.push(json!({"kind": "tool", "id": id, "name": name, "detail": name}));
                    }
                }
                "web_search" if !done => out.push(json!({"kind": "tool", "id": id, "name": "WebSearch",
                    "detail": format!("WebSearch · {}", str_of(it, "query"))})),
                "error" => out.push(json!({"kind": "error", "message": it["message"]})),
                _ => {}
            }
        }
        "turn.completed" => out.push(json!({"kind": "done", "result": "", "isError": false, "usage": v["usage"]})),
        "turn.failed" => out.push(json!({"kind": "error", "message": v["error"]["message"]})),
        "error" => out.push(json!({"kind": "error", "message": v["message"]})),
        _ => {}
    }
}
