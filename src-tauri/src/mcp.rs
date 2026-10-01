//! Client MCP minimal (transport HTTP « streamable ») pour lire des projets
//! depuis n'importe quel serveur MCP, Nodulz par exemple.

use crate::config::{get_secret, McpSource};
use crate::util::home;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;

pub struct Conn {
    url: String,
    headers: Vec<(String, String)>,
    session: Option<String>,
    client: reqwest::Client,
    next_id: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMcpServer {
    pub name: String,
    pub url: String,
    pub kind: String,
}

fn claude_json() -> Value {
    fs::read_to_string(home().join(".claude.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

/// Serveurs MCP HTTP déclarés dans ~/.claude.json (globaux et par projet).
pub fn claude_servers() -> Vec<ClaudeMcpServer> {
    let root = claude_json();
    let mut out: Vec<ClaudeMcpServer> = vec![];
    let mut push = |map: &Value| {
        if let Some(obj) = map.as_object() {
            for (name, cfg) in obj {
                let url = cfg["url"].as_str().unwrap_or("");
                if url.is_empty() || out.iter().any(|s| &s.name == name) {
                    continue;
                }
                out.push(ClaudeMcpServer {
                    name: name.clone(),
                    url: url.to_string(),
                    kind: cfg["type"].as_str().unwrap_or("http").to_string(),
                });
            }
        }
    };
    push(&root["mcpServers"]);
    if let Some(projects) = root["projects"].as_object() {
        for p in projects.values() {
            push(&p["mcpServers"]);
        }
    }
    out
}

fn resolve(src: &McpSource) -> Result<(String, Vec<(String, String)>), String> {
    if !src.server.is_empty() {
        let root = claude_json();
        let mut cfg = root["mcpServers"][&src.server].clone();
        if cfg.is_null() {
            if let Some(projects) = root["projects"].as_object() {
                cfg = projects
                    .values()
                    .map(|p| p["mcpServers"][&src.server].clone())
                    .find(|c| !c.is_null())
                    .unwrap_or(Value::Null);
            }
        }
        let url = cfg["url"]
            .as_str()
            .ok_or_else(|| format!("Serveur MCP « {} » introuvable dans ~/.claude.json", src.server))?;
        let headers = cfg["headers"]
            .as_object()
            .map(|h| {
                h.iter()
                    .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        return Ok((url.to_string(), headers));
    }
    let headers = get_secret(&format!("mcp:{}", src.id))
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .map(|h| {
            h.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                .collect()
        })
        .unwrap_or_default();
    Ok((src.url.clone(), headers))
}

impl Conn {
    pub async fn open(src: &McpSource) -> Result<Self, String> {
        let (url, headers) = resolve(src)?;
        let mut c = Conn {
            url,
            headers,
            session: None,
            client: reqwest::Client::new(),
            next_id: 1,
        };
        c.rpc(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "lody", "version": env!("CARGO_PKG_VERSION")}}),
        )
        .await?;
        c.notify("notifications/initialized").await;
        Ok(c)
    }

    fn request(&self, body: &Value) -> reqwest::RequestBuilder {
        let mut rb = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(body);
        for (k, v) in &self.headers {
            rb = rb.header(k, v);
        }
        if let Some(s) = &self.session {
            rb = rb.header("Mcp-Session-Id", s);
        }
        rb
    }

    async fn notify(&self, method: &str) {
        let _ = self.request(&json!({"jsonrpc": "2.0", "method": method})).send().await;
    }

    pub async fn rpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let resp = self
            .request(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if let Some(s) = resp.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()) {
            self.session = Some(s.to_string());
        }
        let status = resp.status();
        let sse = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.contains("event-stream"))
            .unwrap_or(false);
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("MCP HTTP {status} : {}", crate::util::truncate(&text, 300)));
        }
        let msg = if sse {
            text.lines()
                .filter_map(|l| l.strip_prefix("data:"))
                .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
                .find(|v| v["id"].as_u64() == Some(id))
                .ok_or("Réponse MCP vide")?
        } else {
            serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())?
        };
        if let Some(err) = msg.get("error") {
            return Err(err["message"].as_str().unwrap_or("Erreur MCP").to_string());
        }
        Ok(msg["result"].clone())
    }
}

/// Appelle l'outil configuré et renvoie son contenu (JSON si possible).
pub async fn call_tool(src: &McpSource) -> Result<Value, String> {
    let mut c = Conn::open(src).await?;
    let args = if src.args.is_object() { src.args.clone() } else { json!({}) };
    let r = c.rpc("tools/call", json!({"name": src.tool, "arguments": args})).await?;
    if let Some(sc) = r.get("structuredContent") {
        return Ok(sc.clone());
    }
    let text = r["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

pub async fn list_tools(src: &McpSource) -> Result<Vec<Value>, String> {
    let mut c = Conn::open(src).await?;
    let r = c.rpc("tools/list", json!({})).await?;
    Ok(r["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| json!({"name": t["name"], "description": t["description"]}))
        .collect())
}
