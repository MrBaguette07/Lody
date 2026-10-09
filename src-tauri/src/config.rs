use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub const DEFAULT_PORT: u16 = 47823;
const KEYRING_SERVICE: &str = "Lody";

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ApiProvider {
    pub id: String,
    pub name: String,
    /// "openai" (tout endpoint compatible OpenAI) ou "anthropic"
    pub kind: String,
    pub base_url: String,
    pub model: String,
    #[serde(skip_deserializing)]
    pub has_key: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Sources {
    pub folders: bool,
    pub vscode: bool,
    pub cursor: bool,
    pub github: bool,
}

impl Default for Sources {
    fn default() -> Self {
        Self { folders: true, vscode: true, cursor: true, github: true }
    }
}

/// Source de projets ajoutée par l'utilisateur.
/// kind = "folder" (dossier scanné), "command" (sortie JSON) ou "http" (GET JSON).
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomSource {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub value: String,
    pub enabled: bool,
}

/// Source de projets exposée par un serveur MCP (ex : Nodulz).
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct McpSource {
    pub id: String,
    pub name: String,
    /// Nom du serveur dans ~/.claude.json (url + en-têtes réutilisés), sinon `url`.
    pub server: String,
    pub url: String,
    pub tool: String,
    pub args: Value,
    /// Lien ouvert pour un élément, ex : https://app.nodulz.com/workflows/{id}
    pub link_template: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub hook_port: u16,
    pub claude_path: String,
    pub codex_path: String,
    pub default_mode: String,
    pub approval_timeout_secs: u64,
    pub providers: Vec<ApiProvider>,
    pub system_prompt: String,
    pub project_roots: Vec<String>,
    pub sources: Sources,
    pub custom_sources: Vec<CustomSource>,
    pub mcp_sources: Vec<McpSource>,
    pub sounds: bool,
    pub mascot_position: String,
    pub mascot_x: Option<i32>,
    pub mascot_y: Option<i32>,
    pub shortcut: String,
}

impl Default for Settings {
    fn default() -> Self {
        let p = |id: &str, name: &str, kind: &str, url: &str, model: &str| ApiProvider {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            base_url: url.into(),
            model: model.into(),
            has_key: false,
        };
        Self {
            hook_port: DEFAULT_PORT,
            claude_path: String::new(),
            codex_path: String::new(),
            default_mode: "ask".into(),
            approval_timeout_secs: 600,
            providers: vec![
                p("anthropic", "Claude (API)", "anthropic", "https://api.anthropic.com/v1", "claude-opus-5-5"),
                p("openai", "GPT (OpenAI)", "openai", "https://api.openai.com/v1", ""),
                p("ollama", "Ollama (local)", "openai", "http://localhost:11434/v1", "llama3.2"),
            ],
            system_prompt: "Tu es Lody, un petit assistant de bureau sympa et efficace. Réponds en français par défaut, de façon claire et concise.".into(),
            project_roots: vec![],
            sources: Sources::default(),
            custom_sources: vec![],
            mcp_sources: vec![],
            sounds: true,
            mascot_position: "top".into(),
            mascot_x: None,
            mascot_y: None,
            shortcut: "Ctrl+Alt+L".into(),
        }
    }
}

fn settings_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| crate::util::home().join(".lody"));
    let _ = fs::create_dir_all(&dir);
    dir.join("settings.json")
}

pub fn load(app: &AppHandle) -> Settings {
    let mut s: Settings = fs::read_to_string(settings_path(app))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    refresh_key_flags(&mut s);
    s
}

pub fn save(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let txt = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(settings_path(app), txt).map_err(|e| e.to_string())
}

pub fn refresh_key_flags(s: &mut Settings) {
    for p in s.providers.iter_mut() {
        p.has_key = get_secret(&format!("provider:{}", p.id)).is_some();
    }
}

/// Les clés API restent dans le Gestionnaire d'identification Windows, jamais dans settings.json.
pub fn get_secret(key: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, key)
        .ok()?
        .get_password()
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn set_secret(key: &str, value: &str) -> Result<(), String> {
    let e = keyring::Entry::new(KEYRING_SERVICE, key).map_err(|e| e.to_string())?;
    if value.is_empty() {
        match e.delete_credential() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    } else {
        e.set_password(value).map_err(|e| e.to_string())
    }
}
