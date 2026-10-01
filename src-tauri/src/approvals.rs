use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub id: String,
    /// Clé de l'agent concerné (session_id pour un hook externe, run_id pour un run Lody).
    pub agent_key: String,
    pub session_id: String,
    pub tool_use_id: String,
    pub tool_name: String,
    pub detail: String,
    pub input: Value,
    pub cwd: Option<String>,
    pub project: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Decision {
    Allow,
    AllowAlways,
    Deny,
    /// Laisse Claude Code gérer (invite habituelle dans le terminal).
    Pass,
}

impl Decision {
    pub fn parse(s: &str) -> Self {
        match s {
            "allow" => Self::Allow,
            "always" => Self::AllowAlways,
            "deny" => Self::Deny,
            _ => Self::Pass,
        }
    }
}

#[derive(Default)]
pub struct ApprovalStore {
    pending: Mutex<HashMap<String, (Approval, mpsc::Sender<Decision>)>>,
    /// (clé agent, outil) autorisés pour toute la session.
    always: Mutex<HashSet<(String, String)>>,
}

impl ApprovalStore {
    pub fn is_always(&self, key: &str, tool: &str) -> bool {
        self.always
            .lock()
            .unwrap()
            .contains(&(key.to_string(), tool.to_string()))
    }

    /// Bloque jusqu'à la décision de l'utilisateur (ou expiration → Pass).
    pub fn request(&self, app: &AppHandle, a: Approval, timeout_secs: u64) -> Decision {
        let (tx, rx) = mpsc::channel();
        let id = a.id.clone();
        let key = (a.agent_key.clone(), a.tool_name.clone());
        self.pending.lock().unwrap().insert(id.clone(), (a, tx));
        self.emit(app);
        if let Some(w) = app.get_webview_window("mascot") {
            let _ = w.show();
        }
        let d = rx
            .recv_timeout(Duration::from_secs(timeout_secs.max(10)))
            .unwrap_or(Decision::Pass);
        self.pending.lock().unwrap().remove(&id);
        if d == Decision::AllowAlways {
            self.always.lock().unwrap().insert(key);
        }
        self.emit(app);
        d
    }

    pub fn resolve(&self, id: &str, d: Decision) -> bool {
        match self.pending.lock().unwrap().get(id) {
            Some((_, tx)) => tx.send(d).is_ok(),
            None => false,
        }
    }

    /// Libère les demandes d'une session qui vient de se terminer.
    pub fn drop_agent(&self, key: &str) {
        for (a, tx) in self.pending.lock().unwrap().values() {
            if a.agent_key == key {
                let _ = tx.send(Decision::Pass);
            }
        }
    }

    pub fn list(&self) -> Vec<Approval> {
        let mut v: Vec<Approval> = self
            .pending
            .lock()
            .unwrap()
            .values()
            .map(|(a, _)| a.clone())
            .collect();
        v.sort_by_key(|a| a.created_at);
        v
    }

    fn emit(&self, app: &AppHandle) {
        let _ = app.emit("approvals", self.list());
    }
}
