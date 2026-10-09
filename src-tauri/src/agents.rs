use crate::util::now_ms;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// Un agent suivi par Lody : session Claude Code détectée par hook, ou run lancé depuis Lody.
#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    /// "hook" (session externe) ou "lody" (lancé depuis le chat)
    pub source: String,
    pub provider: String,
    pub title: String,
    pub cwd: Option<String>,
    pub project: Option<String>,
    /// idle | thinking | working | waiting | done | error | ended
    pub status: String,
    pub activity: String,
    pub last_message: String,
    pub started_at: i64,
    pub updated_at: i64,
    pub subagents: u32,
    pub tools: u32,
    pub run_id: Option<String>,
    pub session_id: Option<String>,
    pub transcript: Option<String>,
}

#[derive(Default)]
pub struct AgentStore {
    inner: Mutex<HashMap<String, Agent>>,
}

impl AgentStore {
    pub fn upsert(
        &self,
        app: &AppHandle,
        id: &str,
        init: impl FnOnce() -> Agent,
        f: impl FnOnce(&mut Agent),
    ) {
        {
            let mut m = self.inner.lock().unwrap();
            let a = m.entry(id.to_string()).or_insert_with(init);
            f(a);
            a.updated_at = now_ms();
        }
        self.emit(app);
    }

    /// Met à jour un agent seulement s'il existe déjà.
    pub fn touch(&self, app: &AppHandle, id: &str, f: impl FnOnce(&mut Agent)) {
        let found = {
            let mut m = self.inner.lock().unwrap();
            match m.get_mut(id) {
                Some(a) => {
                    f(a);
                    a.updated_at = now_ms();
                    true
                }
                None => false,
            }
        };
        if found {
            self.emit(app);
        }
    }

    pub fn list(&self) -> Vec<Agent> {
        let mut v: Vec<Agent> = self.inner.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        v
    }

    pub fn remove(&self, app: &AppHandle, id: &str) {
        self.inner.lock().unwrap().remove(id);
        self.emit(app);
    }

    pub fn emit(&self, app: &AppHandle) {
        let _ = app.emit("agents", self.list());
    }

    /// Nettoie les sessions terminées et celles silencieuses depuis longtemps.
    pub fn prune(&self, app: &AppHandle) {
        let now = now_ms();
        let changed = {
            let mut m = self.inner.lock().unwrap();
            let before = m.len();
            m.retain(|_, a| {
                let age = now - a.updated_at;
                match a.status.as_str() {
                    "ended" => age < 2 * 60_000,
                    "working" | "thinking" | "waiting" => age < 6 * 3_600_000,
                    _ => age < 3 * 3_600_000,
                }
            });
            m.len() != before
        };
        if changed {
            self.emit(app);
        }
    }
}
