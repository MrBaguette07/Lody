//! Agrège les projets de l'utilisateur : dossiers git locaux, récents de
//! VS Code / Cursor, dépôts GitHub (via `gh`), serveurs MCP (Nodulz…) et
//! sources personnalisées.

use crate::app::AppState;
use crate::config::{CustomSource, McpSource, Settings};
use crate::util::{folder_name, hide_console, home, iso_to_ms, str_of};
use percent_encoding::percent_decode_str;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    pub url: Option<String>,
    pub sources: Vec<String>,
    pub description: Option<String>,
    pub updated_at: Option<i64>,
    /// local | remote | item
    pub kind: String,
    pub github: Option<String>,
    pub private: bool,
}

#[derive(Serialize)]
pub struct ProjectList {
    pub projects: Vec<Project>,
    pub warnings: Vec<String>,
}

fn key_of(p: &str) -> String {
    p.trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase()
}

#[derive(Default)]
struct Collector {
    by_path: HashMap<String, Project>,
    others: Vec<Project>,
    warnings: Vec<String>,
}

impl Collector {
    fn add_local(&mut self, path: &Path, source: &str) {
        if !path.is_dir() {
            return;
        }
        let p = path.display().to_string();
        let entry = self.by_path.entry(key_of(&p)).or_insert_with(|| {
            let git = path.join(".git");
            Project {
                id: format!("local:{}", key_of(&p)),
                name: folder_name(&p),
                path: Some(p.clone()),
                kind: "local".into(),
                github: github_slug(&git),
                updated_at: activity_ms(path),
                ..Default::default()
            }
        });
        if !entry.sources.iter().any(|s| s == source) {
            entry.sources.push(source.to_string());
        }
    }
}

fn mtime_ms(p: &Path) -> Option<i64> {
    fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as i64)
}

fn activity_ms(dir: &Path) -> Option<i64> {
    [dir.join(".git").join("index"), dir.join(".git").join("HEAD"), dir.to_path_buf()]
        .iter()
        .filter_map(|p| mtime_ms(p))
        .max()
}

pub fn github_slug(git_dir: &Path) -> Option<String> {
    let cfg = fs::read_to_string(git_dir.join("config")).ok()?;
    cfg.lines().find_map(|l| {
        let l = l.trim();
        let url = l.strip_prefix("url")?.trim_start().strip_prefix('=')?.trim();
        let idx = url.find("github.com")?;
        let rest = url[idx + "github.com".len()..].trim_start_matches([':', '/']);
        Some(rest.trim_end_matches(".git").trim_end_matches('/').to_string())
    })
}

pub fn default_roots() -> Vec<PathBuf> {
    let h = home();
    [
        h.join("Documents").join("GitHub"),
        h.join("source").join("repos"),
        h.join("Projects"),
        h.join("projects"),
        h.join("dev"),
        h.join("code"),
        h.join("Code"),
    ]
    .into_iter()
    .filter(|p| p.is_dir())
    .collect()
}

fn scan_root(c: &mut Collector, root: &Path, source: &str) {
    let Ok(rd) = fs::read_dir(root) else {
        c.warnings.push(format!("Dossier illisible : {}", root.display()));
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if !p.is_dir() || name.starts_with('.') || name == "node_modules" {
            continue;
        }
        if p.join(".git").exists() {
            c.add_local(&p, source);
            continue;
        }
        // Un niveau de plus pour les dossiers de regroupement (ex : ESGI/projet).
        if let Ok(sub) = fs::read_dir(&p) {
            for s in sub.flatten() {
                let sp = s.path();
                if sp.is_dir() && sp.join(".git").exists() {
                    c.add_local(&sp, source);
                }
            }
        }
    }
}

fn file_uri_to_path(u: &str) -> Option<String> {
    let rest = u.strip_prefix("file:///")?;
    let decoded = percent_decode_str(rest).decode_utf8().ok()?.to_string();
    let mut p = decoded.replace('/', "\\");
    if p.len() > 1 && p.as_bytes()[1] == b':' {
        p.replace_range(0..1, &p[0..1].to_uppercase());
    }
    Some(p)
}

/// Lit `history.recentlyOpenedPathsList` dans state.vscdb (copie pour éviter les verrous).
fn vscode_recent(app_dir: &str) -> Result<Vec<String>, String> {
    let Some(cfg) = dirs::config_dir() else { return Ok(vec![]) };
    let db = cfg.join(app_dir).join("User").join("globalStorage").join("state.vscdb");
    if !db.exists() {
        return Ok(vec![]);
    }
    let tmp = std::env::temp_dir().join("lody");
    let _ = fs::create_dir_all(&tmp);
    let copy = tmp.join(format!("{}-state.vscdb", app_dir.replace(' ', "_")));
    fs::copy(&db, &copy).map_err(|e| e.to_string())?;
    let wal = db.with_extension("vscdb-wal");
    let wal_copy = copy.with_extension("vscdb-wal");
    let _ = fs::remove_file(&wal_copy);
    if wal.exists() {
        let _ = fs::copy(&wal, &wal_copy);
    }
    let conn = rusqlite::Connection::open(&copy).map_err(|e| e.to_string())?;
    let raw: rusqlite::types::Value = match conn.query_row(
        "SELECT value FROM ItemTable WHERE key = 'history.recentlyOpenedPathsList'",
        [],
        |r| r.get(0),
    ) {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(vec![]),
        Err(e) => return Err(e.to_string()),
    };
    let text = match raw {
        rusqlite::types::Value::Text(t) => t,
        rusqlite::types::Value::Blob(b) => String::from_utf8_lossy(&b).to_string(),
        _ => return Ok(vec![]),
    };
    let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(v["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["folderUri"].as_str().and_then(file_uri_to_path))
        .collect())
}

fn github_repos(c: &mut Collector) {
    let mut cmd = Command::new("gh");
    cmd.args([
        "repo",
        "list",
        "--limit",
        "200",
        "--json",
        "name,nameWithOwner,url,description,updatedAt,isPrivate",
    ]);
    hide_console(&mut cmd);
    let out = match cmd.output() {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => {
            c.warnings.push(format!(
                "GitHub : {}",
                String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("gh a échoué")
            ));
            return;
        }
        Err(_) => {
            c.warnings
                .push("GitHub : la CLI `gh` est introuvable (winget install GitHub.cli).".into());
            return;
        }
    };
    let Ok(repos) = serde_json::from_slice::<Vec<Value>>(&out) else { return };
    for r in repos {
        let slug = str_of(&r, "nameWithOwner").to_string();
        let local = c.by_path.values_mut().find(|p| {
            p.github
                .as_deref()
                .map(|g| g.eq_ignore_ascii_case(&slug))
                .unwrap_or(false)
        });
        if let Some(p) = local {
            if !p.sources.iter().any(|s| s == "github") {
                p.sources.push("github".into());
            }
            p.url = Some(str_of(&r, "url").to_string());
            p.private = r["isPrivate"].as_bool().unwrap_or(false);
            if p.description.is_none() {
                p.description = r["description"].as_str().filter(|d| !d.is_empty()).map(String::from);
            }
            continue;
        }
        c.others.push(Project {
            id: format!("gh:{slug}"),
            name: str_of(&r, "name").to_string(),
            url: Some(str_of(&r, "url").to_string()),
            sources: vec!["github".into()],
            description: r["description"].as_str().filter(|d| !d.is_empty()).map(String::from),
            updated_at: iso_to_ms(str_of(&r, "updatedAt")),
            kind: "remote".into(),
            github: Some(slug),
            private: r["isPrivate"].as_bool().unwrap_or(false),
            path: None,
        });
    }
}

fn from_json_list(v: &Value, source: &str, c: &mut Collector) {
    for item in v.as_array().into_iter().flatten() {
        let name = str_of(item, "name");
        let path = str_of(item, "path");
        if !path.is_empty() && Path::new(path).is_dir() {
            c.add_local(Path::new(path), source);
            continue;
        }
        if name.is_empty() {
            continue;
        }
        c.others.push(Project {
            id: format!("{source}:{name}"),
            name: name.to_string(),
            url: item["url"].as_str().map(String::from),
            description: item["description"].as_str().map(String::from),
            sources: vec![source.to_string()],
            kind: "item".into(),
            ..Default::default()
        });
    }
}

fn custom_blocking(src: &CustomSource, c: &mut Collector) {
    match src.kind.as_str() {
        "folder" => scan_root(c, Path::new(&src.value), &src.name),
        "command" => {
            let mut cmd = Command::new("cmd");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.raw_arg(format!("/C {}", src.value));
            }
            cmd.current_dir(home());
            hide_console(&mut cmd);
            match cmd.output() {
                Ok(o) => match serde_json::from_slice::<Value>(&o.stdout) {
                    Ok(v) => from_json_list(&v, &src.name, c),
                    Err(_) => c.warnings.push(format!("{} : la commande doit renvoyer un tableau JSON", src.name)),
                },
                Err(e) => c.warnings.push(format!("{} : {e}", src.name)),
            }
        }
        _ => {}
    }
}

fn collect_blocking(s: &Settings) -> Collector {
    let mut c = Collector::default();
    if s.sources.folders {
        let roots: Vec<PathBuf> = if s.project_roots.is_empty() {
            default_roots()
        } else {
            s.project_roots.iter().map(PathBuf::from).collect()
        };
        for r in roots {
            scan_root(&mut c, &r, "dossier");
        }
    }
    let editors = [("Code", "vscode", s.sources.vscode), ("Code - Insiders", "vscode", s.sources.vscode), ("Cursor", "cursor", s.sources.cursor)];
    for (dir, label, on) in editors {
        if !on {
            continue;
        }
        match vscode_recent(dir) {
            Ok(paths) => {
                for p in paths {
                    c.add_local(Path::new(&p), label);
                }
            }
            Err(e) => c.warnings.push(format!("{dir} : {e}")),
        }
    }
    for src in s.custom_sources.iter().filter(|x| x.enabled) {
        custom_blocking(src, &mut c);
    }
    if s.sources.github {
        github_repos(&mut c);
    }
    c
}

/// Aplatit une arborescence JSON quelconque en éléments {id, name}.
fn flatten_items(v: &Value, src: &McpSource, crumb: &str, out: &mut Vec<Project>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| flatten_items(x, src, crumb, out)),
        Value::Object(o) => {
            let name = o.get("name").or_else(|| o.get("title")).and_then(|n| n.as_str());
            let id = o.get("id").map(|i| match i {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            });
            let kind = o
                .get("type")
                .or_else(|| o.get("kind"))
                .and_then(|k| k.as_str())
                .unwrap_or("")
                .to_lowercase();
            let mut next_crumb = crumb.to_string();
            if let (Some(name), Some(id)) = (name, id.clone()) {
                if kind.contains("folder") || kind.contains("dossier") {
                    next_crumb = if crumb.is_empty() { name.to_string() } else { format!("{crumb} / {name}") };
                } else {
                    out.push(Project {
                        id: format!("mcp:{}:{id}", src.id),
                        name: name.to_string(),
                        url: (!src.link_template.is_empty()).then(|| src.link_template.replace("{id}", &id)),
                        description: (!crumb.is_empty()).then(|| crumb.to_string()),
                        sources: vec![src.name.clone()],
                        kind: "item".into(),
                        updated_at: o
                            .get("updatedAt")
                            .or_else(|| o.get("updated_at"))
                            .and_then(|u| u.as_str())
                            .and_then(iso_to_ms),
                        ..Default::default()
                    });
                }
            }
            for (k, child) in o {
                if child.is_array() || (child.is_object() && k != "definition" && k != "graph") {
                    flatten_items(child, src, &next_crumb, out);
                }
            }
        }
        _ => {}
    }
}

#[tauri::command]
pub async fn list_projects(app: AppHandle) -> Result<ProjectList, String> {
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    let s2 = settings.clone();
    let mut c = tauri::async_runtime::spawn_blocking(move || collect_blocking(&s2))
        .await
        .map_err(|e| e.to_string())?;

    for src in settings.custom_sources.iter().filter(|x| x.enabled && x.kind == "http") {
        match reqwest::get(&src.value).await {
            Ok(r) => match r.json::<Value>().await {
                Ok(v) => from_json_list(&v, &src.name, &mut c),
                Err(e) => c.warnings.push(format!("{} : {e}", src.name)),
            },
            Err(e) => c.warnings.push(format!("{} : {e}", src.name)),
        }
    }
    for src in settings.mcp_sources.iter().filter(|x| x.enabled) {
        match crate::mcp::call_tool(src).await {
            Ok(v) => {
                let mut items = vec![];
                flatten_items(&v, src, "", &mut items);
                c.others.extend(items);
            }
            Err(e) => c.warnings.push(format!("{} : {e}", src.name)),
        }
    }

    let mut projects: Vec<Project> = c.by_path.into_values().collect();
    projects.extend(c.others);
    projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(ProjectList { projects, warnings: c.warnings })
}

#[tauri::command]
pub async fn clone_repo(slug: String, dest_root: Option<String>) -> Result<String, String> {
    let root = dest_root
        .map(PathBuf::from)
        .or_else(|| default_roots().into_iter().next())
        .unwrap_or_else(|| home().join("Documents").join("GitHub"));
    let name = slug.rsplit('/').next().unwrap_or(&slug).to_string();
    let dest = root.join(&name);
    if dest.exists() {
        return Ok(dest.display().to_string());
    }
    let dest2 = dest.clone();
    let out = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("gh");
        cmd.args(["repo", "clone", &slug]).arg(&dest2);
        hide_console(&mut cmd);
        cmd.output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    Ok(dest.display().to_string())
}

#[tauri::command]
pub fn claude_mcp_servers() -> Vec<crate::mcp::ClaudeMcpServer> {
    crate::mcp::claude_servers()
}

#[tauri::command]
pub async fn mcp_list_tools(source: McpSource) -> Result<Vec<Value>, String> {
    crate::mcp::list_tools(&source).await
}
