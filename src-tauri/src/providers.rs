//! Chat direct avec les API : Anthropic (Messages API) et tout endpoint compatible
//! OpenAI (OpenAI, Ollama, OpenRouter, Mistral, Groq, Gemini, LM Studio…).

use crate::agents::Agent;
use crate::app::AppState;
use crate::config::{get_secret, ApiProvider};
use crate::runner::{apply_to_agent, emit_run, RunHandle};
use crate::util::{now_ms, one_line, str_of};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Modèles pour lesquels on active le repli serveur en cas de refus.
const FALLBACK_MODELS: [&str; 4] = ["claude-opus-5-5", "claude-fable-5-1", "claude-opus-5", "claude-sonnet-5-5"];

#[derive(Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiChatArgs {
    pub run_id: String,
    pub provider_id: String,
    pub messages: Vec<ChatMessage>,
    pub model: Option<String>,
    pub title: Option<String>,
}

fn provider(app: &AppHandle, id: &str) -> Result<(ApiProvider, String, String), String> {
    let st = app.state::<AppState>();
    let s = st.settings.lock().unwrap();
    let p = s
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| format!("Fournisseur inconnu : {id}"))?;
    let key = get_secret(&format!("provider:{id}")).unwrap_or_default();
    Ok((p, key, s.system_prompt.clone()))
}

#[tauri::command]
pub async fn chat_api(app: AppHandle, args: ApiChatArgs) -> Result<(), String> {
    let (p, key, system) = provider(&app, &args.provider_id)?;
    let model = args
        .model
        .clone()
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| p.model.clone());
    if model.trim().is_empty() {
        return Err(format!("Aucun modèle choisi pour « {} » (Réglages → Fournisseurs).", p.name));
    }
    if p.kind == "anthropic" && key.is_empty() {
        return Err("Clé API Anthropic manquante (Réglages → Fournisseurs).".into());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let run_id = args.run_id.clone();
    {
        let st = app.state::<AppState>();
        st.runs.insert(&run_id, RunHandle::Cancel(cancel.clone()));
        let last = args.messages.last().map(|m| m.content.clone()).unwrap_or_default();
        st.agents.upsert(
            &app,
            &run_id,
            || Agent {
                id: run_id.clone(),
                source: "lody".into(),
                provider: p.name.clone(),
                title: args.title.clone().unwrap_or_else(|| one_line(&last, 90)),
                status: "thinking".into(),
                activity: format!("{} · {}", p.name, model),
                started_at: now_ms(),
                run_id: Some(run_id.clone()),
                ..Default::default()
            },
            |a| a.status = "thinking".into(),
        );
    }

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let started = now_ms();
        let res = if p.kind == "anthropic" {
            stream_anthropic(&app2, &run_id, &p, &key, &model, &system, &args.messages, &cancel).await
        } else {
            stream_openai(&app2, &run_id, &p, &key, &model, &system, &args.messages, &cancel).await
        };
        let ev = match res {
            Ok(text) => json!({"kind": "done", "result": text, "isError": false, "durationMs": now_ms() - started}),
            Err(e) => json!({"kind": "error", "message": e}),
        };
        apply_to_agent(&app2, &run_id, &ev);
        emit_run(&app2, &run_id, ev);
        emit_run(&app2, &run_id, json!({"kind": "exit", "code": 0}));
        app2.state::<AppState>().runs.remove(&run_id);
    });
    Ok(())
}

/// Lit un flux SSE ligne par ligne et appelle `on_data` pour chaque `data:`.
async fn read_sse(
    resp: reqwest::Response,
    cancel: &AtomicBool,
    mut on_data: impl FnMut(&str) -> Result<bool, String>,
) -> Result<(), String> {
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            return Err("Arrêté".into());
        }
        let chunk = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find('\n') {
            let line: String = buf.drain(..=pos).collect();
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data:") {
                if !on_data(data.trim())? {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

async fn check(resp: reqwest::Response) -> Result<reqwest::Response, String> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let code = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let msg = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(String::from))
        .unwrap_or(body);
    Err(format!("HTTP {code} : {msg}"))
}

#[allow(clippy::too_many_arguments)]
async fn stream_openai(
    app: &AppHandle,
    run_id: &str,
    p: &ApiProvider,
    key: &str,
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    cancel: &AtomicBool,
) -> Result<String, String> {
    let mut msgs = vec![];
    if !system.is_empty() {
        msgs.push(json!({"role": "system", "content": system}));
    }
    msgs.extend(messages.iter().map(|m| json!({"role": m.role, "content": m.content})));
    let url = format!("{}/chat/completions", p.base_url.trim_end_matches('/'));
    let mut req = reqwest::Client::new()
        .post(url)
        .json(&json!({"model": model, "messages": msgs, "stream": true}));
    if !key.is_empty() {
        req = req.bearer_auth(key);
    }
    let resp = check(req.send().await.map_err(|e| e.to_string())?).await?;
    let mut full = String::new();
    read_sse(resp, cancel, |data| {
        if data == "[DONE]" {
            return Ok(false);
        }
        if let Ok(v) = serde_json::from_str::<Value>(data) {
            if let Some(err) = v.get("error") {
                return Err(err["message"].as_str().unwrap_or("Erreur du fournisseur").to_string());
            }
            if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                full.push_str(t);
                emit_run(app, run_id, json!({"kind": "text_delta", "text": t}));
            }
        }
        Ok(true)
    })
    .await?;
    Ok(full)
}

#[allow(clippy::too_many_arguments)]
async fn stream_anthropic(
    app: &AppHandle,
    run_id: &str,
    p: &ApiProvider,
    key: &str,
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    cancel: &AtomicBool,
) -> Result<String, String> {
    let msgs: Vec<Value> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect();
    let mut body = json!({"model": model, "max_tokens": 64000, "stream": true, "messages": msgs});
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    let base = if p.base_url.is_empty() { "https://api.anthropic.com/v1" } else { p.base_url.trim_end_matches('/') };
    let mut req = reqwest::Client::new()
        .post(format!("{base}/messages"))
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01");
    if FALLBACK_MODELS.contains(&model) {
        // Repli automatique sur un autre modèle si la requête est refusée.
        req = req.header("anthropic-beta", "server-side-fallback-2026-07-01");
        body["fallbacks"] = json!("default");
    }
    let resp = check(req.json(&body).send().await.map_err(|e| e.to_string())?).await?;
    let mut full = String::new();
    read_sse(resp, cancel, |data| {
        let Ok(v) = serde_json::from_str::<Value>(data) else { return Ok(true) };
        match str_of(&v, "type") {
            "content_block_delta" if str_of(&v["delta"], "type") == "text_delta" => {
                let t = str_of(&v["delta"], "text");
                full.push_str(t);
                emit_run(app, run_id, json!({"kind": "text_delta", "text": t}));
            }
            "message_delta" if str_of(&v["delta"], "stop_reason") == "refusal" => {
                let note = "\n\n_(Le modèle a refusé de répondre à cette demande.)_";
                full.push_str(note);
                emit_run(app, run_id, json!({"kind": "text_delta", "text": note}));
            }
            "error" => return Err(v["error"]["message"].as_str().unwrap_or("Erreur API").to_string()),
            "message_stop" => return Ok(false),
            _ => {}
        }
        Ok(true)
    })
    .await?;
    Ok(full)
}

#[tauri::command]
pub async fn list_models(app: AppHandle, provider_id: String) -> Result<Vec<String>, String> {
    let (p, key, _) = provider(&app, &provider_id)?;
    let base = p.base_url.trim_end_matches('/').to_string();
    let mut req = reqwest::Client::new().get(format!("{base}/models"));
    if p.kind == "anthropic" {
        req = req.header("x-api-key", &key).header("anthropic-version", "2023-06-01");
    } else if !key.is_empty() {
        req = req.bearer_auth(&key);
    }
    let resp = check(req.send().await.map_err(|e| e.to_string())?).await?;
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = v["data"]
        .as_array()
        .or_else(|| v["models"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|m| m["id"].as_str().or_else(|| m["name"].as_str()).map(String::from))
        .collect();
    ids.sort();
    Ok(ids)
}
