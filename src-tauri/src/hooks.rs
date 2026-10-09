//! Pont avec les hooks Claude Code.
//!
//! Claude Code exécute `lody.exe hook <port>` pour chaque événement ; ce relais
//! transmet le JSON au serveur local de Lody et renvoie sa réponse (décision
//! d'autorisation). Si Lody est fermé, le relais sort silencieusement.

use crate::agents::Agent;
use crate::app::AppState;
use crate::approvals::{Approval, Decision};
use crate::config::DEFAULT_PORT;
use crate::util::{describe_tool, folder_name, now_ms, one_line, str_of, truncate};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const EVENTS: [&str; 10] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Notification",
    "Stop",
    "SubagentStart",
    "SubagentStop",
];

/// Outils soumis à validation pour un run Lody en mode « demander ».
pub const GATED_TOOLS_ASK: &str = "Bash|PowerShell|Write|Edit|MultiEdit|NotebookEdit|WebFetch|mcp__lody__send_keys";
pub const GATED_TOOLS_EDITS: &str = "Bash|PowerShell|WebFetch|mcp__lody__send_keys";

// ---------------------------------------------------------------- relais (client)

pub fn client_main(args: &[String]) {
    let port = args
        .iter()
        .find_map(|a| a.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);
    let gate = args.iter().any(|a| a == "gate");
    let mut body = String::new();
    let _ = std::io::stdin().read_to_string(&mut body);
    let run = std::env::var("LODY_RUN_ID").unwrap_or_default();

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else {
        return; // Lody n'est pas lancé : on ne gêne pas Claude Code.
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(1800)));
    let path = if gate { "/hook?gate=1" } else { "/hook" };
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nX-Lody-Run: {run}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    if s.write_all(head.as_bytes()).is_err() || s.write_all(body.as_bytes()).is_err() {
        return;
    }
    let mut resp = Vec::new();
    let _ = s.read_to_end(&mut resp);
    let text = String::from_utf8_lossy(&resp);
    if let Some(idx) = text.find("\r\n\r\n") {
        let out = text[idx + 4..].trim();
        if out.starts_with('{') && out != "{}" {
            print!("{out}");
        }
    }
}

// ---------------------------------------------------------------- serveur

pub fn start_server(app: AppHandle, port: u16) {
    std::thread::spawn(move || {
        let server = match tiny_http::Server::http(("127.0.0.1", port)) {
            Ok(s) => s,
            Err(e) => {
                let _ = app.emit(
                    "toast",
                    format!("Port {port} indisponible pour les hooks : {e}"),
                );
                return;
            }
        };
        for req in server.incoming_requests() {
            let app = app.clone();
            std::thread::spawn(move || handle(app, req));
        }
    });
}

fn handle(app: AppHandle, mut req: tiny_http::Request) {
    if *req.method() == tiny_http::Method::Post && req.url().starts_with("/notify") {
        let mut body = String::new();
        let _ = req.as_reader().read_to_string(&mut body);
        if let Ok(v) = serde_json::from_str::<Value>(&body) {
            let _ = app.emit("say", v);
        }
        let _ = req.respond(tiny_http::Response::from_string("{}"));
        return;
    }
    if *req.method() != tiny_http::Method::Post || !req.url().starts_with("/hook") {
        let _ = req.respond(tiny_http::Response::from_string("Lody").with_status_code(404));
        return;
    }
    let gate = req.url().contains("gate=1");
    let run = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("X-Lody-Run"))
        .map(|h| h.value.as_str().trim().to_string())
        .unwrap_or_default();
    let mut body = String::new();
    let _ = req.as_reader().read_to_string(&mut body);
    let out = match serde_json::from_str::<Value>(&body) {
        Ok(v) => process(&app, &v, &run, gate),
        Err(_) => "{}".to_string(),
    };
    let header =
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    let _ = req.respond(tiny_http::Response::from_string(out).with_header(header));
}

fn process(app: &AppHandle, b: &Value, run: &str, gate: bool) -> String {
    let st = app.state::<AppState>();
    let ev = str_of(b, "hook_event_name");
    let is_lody = !run.is_empty();
    let sid = str_of(b, "session_id").to_string();

    if ev == "PermissionRequest" || (ev == "PreToolUse" && gate) {
        let key = if is_lody { run.to_string() } else { sid.clone() };
        let tool = str_of(b, "tool_name").to_string();
        if !is_lody {
            track(app, ev, b);
        }
        let decision = if st.approvals.is_always(&key, &tool) {
            Decision::Allow
        } else {
            let cwd = b.get("cwd").and_then(|v| v.as_str()).map(String::from);
            let input = b.get("tool_input").cloned().unwrap_or(Value::Null);
            st.agents.touch(app, &key, |a| {
                a.status = "waiting".into();
                a.activity = format!("Attend ton accord · {}", describe_tool(&tool, &input));
            });
            let approval = Approval {
                id: uuid::Uuid::new_v4().to_string(),
                agent_key: key.clone(),
                session_id: sid.clone(),
                tool_use_id: str_of(b, "tool_use_id").to_string(),
                detail: describe_tool(&tool, &input),
                tool_name: tool.clone(),
                input,
                project: cwd.as_deref().map(folder_name),
                cwd,
                created_at: now_ms(),
            };
            let timeout = st.settings.lock().unwrap().approval_timeout_secs;
            let d = st.approvals.request(app, approval, timeout);
            st.agents.touch(app, &key, |a| {
                a.status = if d == Decision::Deny { "thinking" } else { "working" }.into();
                a.activity = describe_tool(&tool, b.get("tool_input").unwrap_or(&Value::Null));
            });
            d
        };
        return if ev == "PermissionRequest" {
            permission_output(decision)
        } else {
            pretool_output(decision)
        };
    }

    // Les runs lancés par Lody sont déjà suivis via leur flux JSON.
    if !is_lody {
        track(app, ev, b);
    }
    "{}".to_string()
}

fn permission_output(d: Decision) -> String {
    match d {
        Decision::Allow | Decision::AllowAlways => json!({
            "hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}
        })
        .to_string(),
        Decision::Deny => json!({
            "hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "deny", "message": "Refusé depuis Lody."}}
        })
        .to_string(),
        Decision::Pass => "{}".to_string(),
    }
}

fn pretool_output(d: Decision) -> String {
    match d {
        Decision::Allow | Decision::AllowAlways => json!({
            "hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "allow", "permissionDecisionReason": "Autorisé depuis Lody"}
        })
        .to_string(),
        Decision::Deny => json!({
            "hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": "L'utilisateur a refusé depuis Lody."}
        })
        .to_string(),
        Decision::Pass => "{}".to_string(),
    }
}

/// Retire les blocs `<balise>…</balise>` injectés par les IDE (fichier ouvert, sélection…).
fn clean_prompt(s: &str) -> String {
    let mut out = s.to_string();
    while let Some(start) = out.find('<') {
        let Some(end_name) = out[start + 1..].find(|c: char| c == '>' || c.is_whitespace()) else { break };
        let name = out[start + 1..start + 1 + end_name].to_string();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            break;
        }
        let close = format!("</{name}>");
        match out[start..].find(&close) {
            Some(rel) => out.replace_range(start..start + rel + close.len(), " "),
            None => break,
        }
    }
    one_line(out.trim(), 90)
}

/// Dernier prompt utilisateur d'un transcript (pour les sessions ouvertes avant Lody).
fn last_prompt(transcript: &str) -> Option<String> {
    use std::io::{Seek, SeekFrom};
    let mut f = fs::File::open(transcript).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(512 * 1024))).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    String::from_utf8_lossy(&buf).lines().rev().find_map(|l| {
        let v: Value = serde_json::from_str(l).ok()?;
        if str_of(&v, "type") != "user" || v.get("isMeta").and_then(|m| m.as_bool()) == Some(true) {
            return None;
        }
        let c = &v["message"]["content"];
        let text = match c {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter(|p| str_of(p, "type") == "text")
                .map(|p| str_of(p, "text"))
                .collect::<Vec<_>>()
                .join(" "),
            _ => return None,
        };
        let t = clean_prompt(&text);
        (!t.is_empty() && !t.starts_with('<')).then_some(t)
    })
}

fn track(app: &AppHandle, ev: &str, b: &Value) {
    let st = app.state::<AppState>();
    let sid = str_of(b, "session_id").to_string();
    if sid.is_empty() {
        return;
    }
    let cwd = b.get("cwd").and_then(|v| v.as_str()).map(String::from);
    let transcript = b
        .get("transcript_path")
        .and_then(|v| v.as_str())
        .map(String::from);
    let init = || Agent {
        id: sid.clone(),
        source: "hook".into(),
        provider: "claude".into(),
        title: transcript
            .as_deref()
            .and_then(last_prompt)
            .unwrap_or_else(|| "Session Claude Code".into()),
        project: cwd.as_deref().map(folder_name),
        cwd: cwd.clone(),
        status: "idle".into(),
        started_at: now_ms(),
        session_id: Some(sid.clone()),
        transcript,
        ..Default::default()
    };
    let tool_name = str_of(b, "tool_name");
    let input = b.get("tool_input").unwrap_or(&Value::Null);
    let sub = b.get("agent_id").and_then(|v| v.as_str()).is_some();

    if ev == "SessionEnd" {
        st.approvals.drop_agent(&sid);
        st.agents.upsert(app, &sid, init, |a| {
            a.status = "ended".into();
            a.activity = "Session fermée".into();
        });
        return;
    }
    if ev == "Stop" {
        st.approvals.drop_agent(&sid);
    }
    st.agents.upsert(app, &sid, init, |a| {
        if let Some(c) = &cwd {
            a.project = Some(folder_name(c));
            a.cwd = Some(c.clone());
        }
        match ev {
            "SessionStart" => {
                a.status = "idle".into();
                a.activity = "Session ouverte".into();
            }
            "UserPromptSubmit" => {
                let p = clean_prompt(str_of(b, "prompt"));
                if !p.is_empty() {
                    a.title = p;
                }
                a.status = "thinking".into();
                a.activity = "Réfléchit…".into();
            }
            "PreToolUse" => {
                a.status = "working".into();
                let d = describe_tool(tool_name, input);
                a.activity = if sub { format!("↳ {d}") } else { d };
            }
            "PostToolUse" => {
                a.tools += 1;
                if a.status != "waiting" {
                    a.status = "thinking".into();
                }
            }
            "PermissionRequest" => {
                a.status = "waiting".into();
                a.activity = format!("Attend ton accord · {}", describe_tool(tool_name, input));
            }
            "Notification" => {
                match str_of(b, "notification_type") {
                    "permission_prompt" => a.status = "waiting".into(),
                    "idle_prompt" => a.status = "idle".into(),
                    _ => {}
                }
                let m = str_of(b, "message");
                if !m.is_empty() {
                    a.activity = one_line(m, 90);
                }
            }
            "Stop" => {
                a.status = "done".into();
                a.activity = "Terminé".into();
                let m = str_of(b, "last_assistant_message");
                if !m.is_empty() {
                    a.last_message = truncate(m, 600);
                }
            }
            "SubagentStart" => a.subagents += 1,
            "SubagentStop" => a.subagents = a.subagents.saturating_sub(1),
            _ => {}
        }
    });
}

// ---------------------------------------------------------------- installation

fn claude_settings_path() -> PathBuf {
    crate::util::home().join(".claude").join("settings.json")
}

fn is_lody_hook(h: &Value) -> bool {
    let cmd = str_of(h, "command").to_ascii_lowercase();
    let first_arg = h
        .get("args")
        .and_then(|a| a.get(0))
        .and_then(|a| a.as_str())
        .unwrap_or("");
    (cmd.ends_with("lody.exe") || cmd.ends_with("lody")) && first_arg == "hook"
}

fn strip_lody(root: &mut Value) {
    let Some(hooks) = root.get_mut("hooks").and_then(|h| h.as_object_mut()) else {
        return;
    };
    for ev in EVENTS {
        let Some(arr) = hooks.get_mut(ev).and_then(|a| a.as_array_mut()) else {
            continue;
        };
        arr.retain(|group| {
            !group
                .get("hooks")
                .and_then(|h| h.as_array())
                .map(|hs| hs.iter().any(is_lody_hook))
                .unwrap_or(false)
        });
        if arr.is_empty() {
            hooks.remove(ev);
        }
    }
}

fn read_claude_settings() -> Result<Value, String> {
    match fs::read_to_string(claude_settings_path()) {
        Ok(t) if t.trim().is_empty() => Ok(json!({})),
        Ok(t) => serde_json::from_str(&t)
            .map_err(|e| format!("~/.claude/settings.json illisible, rien n'a été modifié : {e}")),
        Err(_) => Ok(json!({})),
    }
}

fn write_claude_settings(v: &Value) -> Result<(), String> {
    let path = claude_settings_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if path.exists() {
        let _ = fs::copy(&path, path.with_extension("json.lody-backup"));
    }
    let txt = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    fs::write(&path, txt).map_err(|e| e.to_string())
}

fn exe_path() -> Result<String, String> {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .map_err(|e| e.to_string())
}

fn hook_entry(exe: &str, port: u16, ev: &str) -> Value {
    let timeout = if ev == "PermissionRequest" { 1800 } else { 10 };
    json!({ "hooks": [{
        "type": "command",
        "command": exe,
        "args": ["hook", port.to_string()],
        "timeout": timeout
    }]})
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    pub installed: bool,
    pub up_to_date: bool,
    pub settings_path: String,
}

pub fn status(port: u16) -> HookStatus {
    let path = claude_settings_path();
    let exe = exe_path().unwrap_or_default();
    let mut installed = false;
    let mut up_to_date = false;
    if let Ok(v) = read_claude_settings() {
        if let Some(hooks) = v.get("hooks").and_then(|h| h.as_object()) {
            let mut all_current = true;
            for ev in EVENTS {
                let found = hooks
                    .get(ev)
                    .and_then(|a| a.as_array())
                    .into_iter()
                    .flatten()
                    .flat_map(|g| g.get("hooks").and_then(|h| h.as_array()).cloned().unwrap_or_default())
                    .find(is_lody_hook);
                match found {
                    Some(h) => {
                        installed = true;
                        let same_exe = str_of(&h, "command").eq_ignore_ascii_case(&exe);
                        let same_port = h
                            .get("args")
                            .and_then(|a| a.get(1))
                            .and_then(|a| a.as_str())
                            == Some(&port.to_string());
                        all_current &= same_exe && same_port;
                    }
                    None => all_current = false,
                }
            }
            up_to_date = installed && all_current;
        }
    }
    HookStatus { installed, up_to_date, settings_path: path.display().to_string() }
}

pub fn install(port: u16) -> Result<(), String> {
    let mut root = read_claude_settings()?;
    if !root.is_object() {
        return Err("~/.claude/settings.json n'est pas un objet JSON".into());
    }
    strip_lody(&mut root);
    let exe = exe_path()?;
    let obj = root.as_object_mut().unwrap();
    let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks = hooks.as_object_mut().unwrap();
    for ev in EVENTS {
        let arr = hooks.entry(ev).or_insert_with(|| json!([]));
        if !arr.is_array() {
            *arr = json!([]);
        }
        arr.as_array_mut().unwrap().push(hook_entry(&exe, port, ev));
    }
    write_claude_settings(&root)
}

pub fn uninstall() -> Result<(), String> {
    let mut root = read_claude_settings()?;
    strip_lody(&mut root);
    if root.get("hooks").and_then(|h| h.as_object()).map(|h| h.is_empty()) == Some(true) {
        root.as_object_mut().unwrap().remove("hooks");
    }
    write_claude_settings(&root)
}

/// Fichier de réglages passé à `claude -p --settings` pour valider les outils depuis Lody.
pub fn gate_settings_file(run_id: &str, port: u16, matcher: &str) -> Result<String, String> {
    let exe = exe_path()?;
    let v = json!({ "hooks": { "PreToolUse": [{
        "matcher": matcher,
        "hooks": [{ "type": "command", "command": exe, "args": ["hook", port.to_string(), "gate"], "timeout": 1800 }]
    }]}});
    let dir = std::env::temp_dir().join("lody");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("run-{run_id}.json"));
    fs::write(&path, v.to_string()).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}
