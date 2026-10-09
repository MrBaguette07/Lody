use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Évite l'ouverture d'une console noire pour chaque processus enfant.
pub fn hide_console(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

pub fn one_line(s: &str, max: usize) -> String {
    truncate(&s.split_whitespace().collect::<Vec<_>>().join(" "), max)
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn folder_name(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string())
}

pub fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("")
}

/// Résumé court et lisible d'un appel d'outil (Claude Code / Codex).
pub fn describe_tool(name: &str, input: &Value) -> String {
    let s = |k: &str| str_of(input, k);
    let detail = match name {
        "Bash" | "PowerShell" => one_line(s("command"), 90),
        "Read" | "Write" | "Edit" | "MultiEdit" => folder_name(s("file_path")),
        "NotebookEdit" => folder_name(s("notebook_path")),
        "Grep" | "Glob" => s("pattern").to_string(),
        "WebFetch" => s("url").to_string(),
        "WebSearch" => s("query").to_string(),
        "Task" | "Agent" => s("description").to_string(),
        "TodoWrite" => "liste de tâches".to_string(),
        _ => String::new(),
    };
    if detail.is_empty() {
        name.to_string()
    } else {
        format!("{name} · {detail}")
    }
}

pub fn kill_tree(pid: u32) {
    let mut c = Command::new("taskkill");
    c.args(["/T", "/F", "/PID", &pid.to_string()]);
    hide_console(&mut c);
    let _ = c.status();
}

fn where_exe(name: &str) -> Vec<PathBuf> {
    let mut c = Command::new("where");
    c.arg(name);
    hide_console(&mut c);
    match c.output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| PathBuf::from(l.trim()))
            .filter(|p| p.exists())
            .collect(),
        _ => vec![],
    }
}

fn pick_exe(paths: Vec<PathBuf>) -> Option<String> {
    let ext = |p: &PathBuf, e: &str| {
        p.extension()
            .map(|x| x.eq_ignore_ascii_case(e))
            .unwrap_or(false)
    };
    paths
        .iter()
        .find(|p| ext(p, "exe"))
        .or_else(|| paths.iter().find(|p| ext(p, "cmd")))
        .map(|p| p.display().to_string())
}

/// Cherche le binaire embarqué le plus récent dans les extensions VS Code / Cursor.
fn newest_in_extensions(prefix: &str, rel: &[&str]) -> Option<String> {
    let h = home();
    let mut best: Option<(Vec<u32>, PathBuf)> = None;
    for root in [
        ".vscode/extensions",
        ".vscode-insiders/extensions",
        ".cursor/extensions",
        ".windsurf/extensions",
    ] {
        let Ok(rd) = fs::read_dir(h.join(root)) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with(prefix) {
                continue;
            }
            let mut p = e.path();
            for r in rel {
                p.push(r);
            }
            if !p.exists() {
                continue;
            }
            let ver: Vec<u32> = name[prefix.len()..]
                .split(|c: char| !c.is_ascii_digit())
                .filter_map(|x| x.parse().ok())
                .collect();
            if best.as_ref().map_or(true, |(bv, _)| ver > *bv) {
                best = Some((ver, p));
            }
        }
    }
    best.map(|(_, p)| p.display().to_string())
}

pub fn find_claude(custom: &str) -> Option<String> {
    if !custom.is_empty() && Path::new(custom).exists() {
        return Some(custom.to_string());
    }
    if let Some(p) = pick_exe(where_exe("claude")) {
        return Some(p);
    }
    let h = home();
    for p in [
        h.join(".local").join("bin").join("claude.exe"),
        h.join("AppData").join("Roaming").join("npm").join("claude.cmd"),
    ] {
        if p.exists() {
            return Some(p.display().to_string());
        }
    }
    newest_in_extensions(
        "anthropic.claude-code-",
        &["resources", "native-binary", "claude.exe"],
    )
}

pub fn find_codex(custom: &str) -> Option<String> {
    if !custom.is_empty() && Path::new(custom).exists() {
        return Some(custom.to_string());
    }
    if let Some(p) = pick_exe(where_exe("codex")) {
        return Some(p);
    }
    let npm = home()
        .join("AppData")
        .join("Roaming")
        .join("npm")
        .join("codex.cmd");
    if npm.exists() {
        return Some(npm.display().to_string());
    }
    newest_in_extensions("openai.chatgpt-", &["bin", "windows-x86_64", "codex.exe"])
}

/// Convertit une date ISO 8601 (UTC) en millisecondes, sans dépendance externe.
pub fn iso_to_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (hh, mm, ss) = (n(11..13)?, n(14..16)?, n(17..19)?);
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400) + hh * 3600 + mm * 60 + ss) * 1000)
}
