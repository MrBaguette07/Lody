//! Git et GitHub (via `git` et la CLI `gh`) : état d'un dépôt, PR, CI, commits.

use crate::projects::github_slug;
use crate::util::{hide_console, iso_to_ms, str_of};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

fn run(cmd: &str, args: &[&str], cwd: Option<&str>) -> Result<String, String> {
    let mut c = Command::new(cmd);
    c.args(args);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    hide_console(&mut c);
    let o = c.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!("`{cmd}` introuvable")
        } else {
            e.to_string()
        }
    })?;
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    if o.status.success() {
        Ok(out)
    } else {
        let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
        Err(if err.is_empty() { out.trim().to_string() } else { err })
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

/// Résume statusCheckRollup en pass | fail | pending | none.
fn checks_state(rollup: &Value) -> &'static str {
    let Some(items) = rollup.as_array() else { return "none" };
    if items.is_empty() {
        return "none";
    }
    let mut pending = false;
    for c in items {
        let concl = c["conclusion"].as_str().or_else(|| c["state"].as_str()).unwrap_or("").to_uppercase();
        let status = c["status"].as_str().unwrap_or("").to_uppercase();
        if ["FAILURE", "ERROR", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE"].contains(&concl.as_str()) {
            return "fail";
        }
        if concl.is_empty() || concl == "PENDING" || ["IN_PROGRESS", "QUEUED", "PENDING", "WAITING"].contains(&status.as_str()) {
            pending = true;
        }
    }
    if pending { "pending" } else { "pass" }
}

fn map_run(r: &Value) -> Value {
    json!({
        "id": r["databaseId"],
        "title": r["displayTitle"],
        "workflow": r["workflowName"],
        "branch": r["headBranch"],
        "event": r["event"],
        "status": r["status"],
        "conclusion": r["conclusion"],
        "url": r["url"],
        "createdAt": iso_to_ms(str_of(r, "createdAt")),
    })
}

const RUN_FIELDS: &str = "databaseId,displayTitle,status,conclusion,workflowName,headBranch,event,createdAt,url";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub slug: Option<String>,
    pub branch: String,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub files: Vec<Value>,
    pub commits: Vec<Value>,
    pub prs: Vec<Value>,
    pub runs: Vec<Value>,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub async fn repo_status(cwd: String) -> Result<RepoStatus, String> {
    blocking(move || {
        let d = Some(cwd.as_str());
        let branch = run("git", &["rev-parse", "--abbrev-ref", "HEAD"], d)
            .map_err(|e| format!("Pas un dépôt git : {e}"))?
            .trim()
            .to_string();
        let mut warnings = vec![];
        let files = run("git", &["status", "--porcelain=v1", "-uall"], d)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.len() > 3)
            .map(|l| json!({"status": l[..2].trim(), "path": &l[3..]}))
            .collect();
        let (ahead, behind) = run("git", &["rev-list", "--left-right", "--count", "HEAD...@{u}"], d)
            .ok()
            .and_then(|s| {
                let mut it = s.split_whitespace().filter_map(|x| x.parse::<u32>().ok());
                Some((it.next()?, it.next()?))
            })
            .map(|(a, b)| (Some(a), Some(b)))
            .unwrap_or((None, None));
        let commits = run("git", &["log", "-8", "--pretty=format:%h%x1f%s%x1f%an%x1f%ct"], d)
            .unwrap_or_default()
            .lines()
            .map(|l| {
                let p: Vec<&str> = l.split('\x1f').collect();
                json!({
                    "hash": p.first().unwrap_or(&""),
                    "subject": p.get(1).unwrap_or(&""),
                    "author": p.get(2).unwrap_or(&""),
                    "date": p.get(3).and_then(|t| t.parse::<i64>().ok()).map(|t| t * 1000),
                })
            })
            .collect();

        let slug = github_slug(&Path::new(&cwd).join(".git"));
        let mut prs = vec![];
        let mut runs = vec![];
        if let Some(s) = &slug {
            match run("gh", &["pr", "list", "-R", s, "--limit", "20", "--json",
                "number,title,headRefName,author,isDraft,reviewDecision,url,updatedAt,statusCheckRollup"], d)
            {
                Ok(t) => {
                    let v: Value = serde_json::from_str(&t).unwrap_or(json!([]));
                    prs = v.as_array().into_iter().flatten().map(|p| json!({
                        "number": p["number"],
                        "title": p["title"],
                        "branch": p["headRefName"],
                        "author": p["author"]["login"],
                        "draft": p["isDraft"],
                        "review": p["reviewDecision"],
                        "url": p["url"],
                        "updatedAt": iso_to_ms(str_of(p, "updatedAt")),
                        "checks": checks_state(&p["statusCheckRollup"]),
                    })).collect();
                }
                Err(e) => warnings.push(format!("PR : {e}")),
            }
            match run("gh", &["run", "list", "-R", s, "--limit", "10", "--json", RUN_FIELDS], d) {
                Ok(t) => {
                    let v: Value = serde_json::from_str(&t).unwrap_or(json!([]));
                    runs = v.as_array().into_iter().flatten().map(map_run).collect();
                }
                Err(e) => warnings.push(format!("CI : {e}")),
            }
        }
        Ok(RepoStatus { slug, branch, ahead, behind, files, commits, prs, runs, warnings })
    })
    .await?
}

/// Vue d'ensemble GitHub : mes PR, reviews demandées, dernier run CI des dépôts donnés.
#[tauri::command]
pub async fn gh_overview(slugs: Vec<String>) -> Result<Value, String> {
    blocking(move || {
        let fields = "number,title,url,repository,updatedAt,isDraft";
        let search = |filter: &str| -> Result<Vec<Value>, String> {
            let t = run("gh", &["search", "prs", filter, "--state=open", "--limit", "20", "--json", fields], None)?;
            let v: Value = serde_json::from_str(&t).map_err(|e| e.to_string())?;
            Ok(v.as_array().into_iter().flatten().map(|p| json!({
                "number": p["number"],
                "title": p["title"],
                "url": p["url"],
                "repo": p["repository"]["nameWithOwner"],
                "draft": p["isDraft"],
                "updatedAt": iso_to_ms(str_of(p, "updatedAt")),
            })).collect())
        };
        let mine = std::thread::spawn(move || search("--author=@me"));
        let reviews = std::thread::spawn(move || search("--review-requested=@me"));
        let ci_threads: Vec<_> = slugs
            .into_iter()
            .take(8)
            .map(|s| {
                std::thread::spawn(move || {
                    run("gh", &["run", "list", "-R", &s, "--limit", "1", "--json", RUN_FIELDS], None)
                        .ok()
                        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                        .and_then(|v| v.get(0).cloned())
                        .map(|r| {
                            let mut m = map_run(&r);
                            m["repo"] = json!(s);
                            m
                        })
                })
            })
            .collect();
        let mut warnings = vec![];
        let mut take = |h: std::thread::JoinHandle<Result<Vec<Value>, String>>| match h.join() {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                warnings.push(e);
                vec![]
            }
            Err(_) => vec![],
        };
        let my_prs = take(mine);
        let review_prs = take(reviews);
        let ci: Vec<Value> = ci_threads.into_iter().filter_map(|h| h.join().ok().flatten()).collect();
        json!({ "myPrs": my_prs, "reviews": review_prs, "ci": ci, "warnings": warnings })
    })
    .await
}

#[tauri::command]
pub async fn git_commit(cwd: String, message: String) -> Result<String, String> {
    blocking(move || {
        if message.trim().is_empty() {
            return Err("Message de commit vide".to_string());
        }
        run("git", &["add", "-A"], Some(&cwd))?;
        run("git", &["commit", "-m", message.trim()], Some(&cwd))
    })
    .await?
}

#[tauri::command]
pub async fn git_sync(cwd: String, action: String) -> Result<String, String> {
    blocking(move || {
        let d = Some(cwd.as_str());
        match action.as_str() {
            "pull" => run("git", &["pull", "--ff-only"], d),
            "push" => {
                let has_upstream = run("git", &["rev-parse", "--abbrev-ref", "@{u}"], d).is_ok();
                if has_upstream {
                    run("git", &["push"], d)
                } else {
                    run("git", &["push", "-u", "origin", "HEAD"], d)
                }
            }
            "fetch" => run("git", &["fetch", "--prune"], d),
            a => Err(format!("Action inconnue : {a}")),
        }
    })
    .await?
}

#[tauri::command]
pub async fn gh_pr_action(cwd: String, number: u64, action: String) -> Result<String, String> {
    blocking(move || {
        let n = number.to_string();
        let args: Vec<&str> = match action.as_str() {
            "merge" => vec!["pr", "merge", &n, "--squash", "--delete-branch"],
            "checkout" => vec!["pr", "checkout", &n],
            "ready" => vec!["pr", "ready", &n],
            "close" => vec!["pr", "close", &n],
            a => return Err(format!("Action inconnue : {a}")),
        };
        run("gh", &args, Some(&cwd))
    })
    .await?
}

#[tauri::command]
pub async fn gh_pr_create(cwd: String, draft: bool) -> Result<String, String> {
    blocking(move || {
        let d = Some(cwd.as_str());
        if run("git", &["rev-parse", "--abbrev-ref", "@{u}"], d).is_err() {
            run("git", &["push", "-u", "origin", "HEAD"], d)?;
        }
        let mut args = vec!["pr", "create", "--fill"];
        if draft {
            args.push("--draft");
        }
        run("gh", &args, d).map(|o| o.lines().last().unwrap_or("").to_string())
    })
    .await?
}

#[tauri::command]
pub async fn gh_run_rerun(cwd: String, id: u64) -> Result<String, String> {
    blocking(move || run("gh", &["run", "rerun", &id.to_string(), "--failed"], Some(&cwd))).await?
}
