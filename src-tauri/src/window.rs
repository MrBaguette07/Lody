use crate::app::AppState;
use serde_json::json;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

fn monitor_for(w: &WebviewWindow) -> Option<tauri::Monitor> {
    w.current_monitor()
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten())
}

fn on_any_monitor(w: &WebviewWindow, x: i32, y: i32) -> bool {
    w.available_monitors()
        .unwrap_or_default()
        .iter()
        .any(|m| {
            let (p, s) = (m.position(), m.size());
            x >= p.x - 50 && y >= p.y - 50 && x < p.x + s.width as i32 && y < p.y + s.height as i32
        })
}

pub fn place_mascot(app: &AppHandle) {
    let Some(w) = app.get_webview_window("mascot") else { return };
    let s = app.state::<AppState>().settings.lock().unwrap().clone();
    let saved = match (s.mascot_x, s.mascot_y) {
        (Some(x), Some(y)) if on_any_monitor(&w, x, y) => Some((x, y)),
        _ => None,
    };
    let pos = saved.or_else(|| {
        let m = w.primary_monitor().ok().flatten().or_else(|| monitor_for(&w))?;
        let size = w.outer_size().ok()?;
        let (mp, ms, sf) = (m.position(), m.size(), m.scale_factor());
        let (ww, wh) = (size.width as i32, size.height as i32);
        Some(if s.mascot_position == "bottom" {
            (
                mp.x + ms.width as i32 - ww - (12.0 * sf) as i32,
                mp.y + ms.height as i32 - wh - (52.0 * sf) as i32,
            )
        } else {
            (mp.x + (ms.width as i32 - ww) / 2, mp.y)
        })
    });
    if let Some((x, y)) = pos {
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    let _ = w.show();
}

/// L'island (fenêtre mascotte) contient toute l'interface : on la déplie sur place.
fn focus_island(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("mascot") {
        let _ = w.show();
        let _ = w.set_ignore_cursor_events(false);
        let _ = w.set_focus();
    }
}

pub fn open_island(app: &AppHandle, tab: Option<String>) {
    focus_island(app);
    let _ = app.emit_to("mascot", "island", json!({"open": true, "tab": tab}));
}

pub fn toggle_island(app: &AppHandle) {
    focus_island(app);
    let _ = app.emit_to("mascot", "island", json!({"toggle": true}));
}

/// Envoie la position du curseur (relative à la fenêtre mascotte) pour les yeux de Lody
/// et pour le « click-through » des zones transparentes.
pub fn spawn_cursor_tracker(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = (f64::MAX, f64::MAX);
        loop {
            std::thread::sleep(Duration::from_millis(40));
            let Some(w) = app.get_webview_window("mascot") else { continue };
            if !w.is_visible().unwrap_or(false) {
                std::thread::sleep(Duration::from_millis(400));
                continue;
            }
            let (Ok(c), Ok(p), Ok(sf)) = (w.cursor_position(), w.inner_position(), w.scale_factor()) else {
                continue;
            };
            let x = (c.x - p.x as f64) / sf;
            let y = (c.y - p.y as f64) / sf;
            if (x - last.0).abs() > 0.5 || (y - last.1).abs() > 0.5 {
                last = (x, y);
                let _ = app.emit_to("mascot", "cursor", json!({"x": x, "y": y}));
            }
        }
    });
}

#[tauri::command]
pub fn focus_mascot(app: AppHandle) {
    focus_island(&app);
}

#[tauri::command]
pub fn save_mascot_pos(app: AppHandle, state: tauri::State<'_, AppState>) {
    let Some(w) = app.get_webview_window("mascot") else { return };
    let Ok(p) = w.outer_position() else { return };
    let mut s = state.settings.lock().unwrap();
    s.mascot_x = Some(p.x);
    s.mascot_y = Some(p.y);
    let _ = crate::config::save(&app, &s);
}

#[tauri::command]
pub fn reset_mascot_pos(app: AppHandle, state: tauri::State<'_, AppState>) {
    {
        let mut s = state.settings.lock().unwrap();
        s.mascot_x = None;
        s.mascot_y = None;
        let _ = crate::config::save(&app, &s);
    }
    place_mascot(&app);
}

#[tauri::command]
pub fn set_mascot_visible(app: AppHandle, visible: bool) {
    if let Some(w) = app.get_webview_window("mascot") {
        let _ = if visible { w.show() } else { w.hide() };
    }
}
