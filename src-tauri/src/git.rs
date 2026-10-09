use crate::util::hide_console;
use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub status: String,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiff {
    pub is_repo: bool,
    pub branch: String,
    pub files: Vec<FileChange>,
    pub diff: String,
    pub truncated: bool,
}

const MAX_DIFF: usize = 400_000;

fn git(cwd: &str, args: &[&str]) -> Option<String> {
    let mut c = Command::new("git");
    c.arg("-C").arg(cwd).args(args);
    hide_console(&mut c);
    let o = c.output().ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).to_string())
}

#[tauri::command]
pub async fn git_diff(cwd: String) -> Result<GitDiff, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(status) = git(&cwd, &["status", "--porcelain=v1", "-b", "-uall"]) else {
            return GitDiff { is_repo: false, branch: String::new(), files: vec![], diff: String::new(), truncated: false };
        };
        let mut branch = String::new();
        let mut files = vec![];
        for line in status.lines() {
            if let Some(b) = line.strip_prefix("## ") {
                branch = b.split("...").next().unwrap_or(b).to_string();
            } else if line.len() > 3 {
                files.push(FileChange { status: line[..2].trim().to_string(), path: line[3..].to_string() });
            }
        }
        let mut diff = git(&cwd, &["diff", "HEAD", "--no-color"])
            .or_else(|| git(&cwd, &["diff", "--no-color"]))
            .unwrap_or_default();
        let truncated = diff.len() > MAX_DIFF;
        if truncated {
            let mut cut = MAX_DIFF;
            while !diff.is_char_boundary(cut) {
                cut -= 1;
            }
            diff.truncate(cut);
        }
        GitDiff { is_repo: true, branch, files, diff, truncated }
    })
    .await
    .map_err(|e| e.to_string())
}
