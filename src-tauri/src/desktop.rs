//! Serveur MCP « bureau » de Lody (`lody.exe mcp <port>`), branché sur les runs
//! Claude Code : ouvrir des applis / liens, contrôler la musique, lire les fenêtres,
//! le presse-papiers, faire une capture d'écran, parler via l'island.

use crate::util::{hide_console, str_of};
use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Command, Stdio};
use std::time::Duration;

const VK_VOLUME_MUTE: u8 = 0xAD;
const VK_VOLUME_DOWN: u8 = 0xAE;
const VK_VOLUME_UP: u8 = 0xAF;

#[cfg(windows)]
mod ffi {
    #[link(name = "user32")]
    extern "system" {
        pub fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
        pub fn mouse_event(flags: u32, dx: u32, dy: u32, data: u32, extra: usize);
        pub fn SetCursorPos(x: i32, y: i32) -> i32;
        pub fn GetSystemMetrics(index: i32) -> i32;
        pub fn SetProcessDPIAware() -> i32;
    }
    #[link(name = "shell32")]
    extern "system" {
        pub fn ShellExecuteW(
            hwnd: isize,
            op: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn shell_open(target: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let (op, file) = (wide("open"), wide(target));
        let r = unsafe { ffi::ShellExecuteW(0, op.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), 1) };
        if r > 32 {
            return Ok(());
        }
        return Err(format!("Impossible d'ouvrir « {target} » (code {r})"));
    }
    #[allow(unreachable_code)]
    Err("Windows uniquement".into())
}

fn press(vk: u8, times: u32) {
    #[cfg(windows)]
    for _ in 0..times.max(1) {
        unsafe {
            ffi::keybd_event(vk, 0, 0, 0);
            ffi::keybd_event(vk, 0, 2, 0);
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// Largeur max des captures envoyées au modèle ; les clics sont exprimés dans ce repère.
// Doit rester égal à la largeur utilisée dans le script de screenshot().
const SHOT_WIDTH: f64 = 1600.0;

fn screen_size() -> (i32, i32) {
    #[cfg(windows)]
    unsafe {
        return (ffi::GetSystemMetrics(0), ffi::GetSystemMetrics(1));
    }
    #[allow(unreachable_code)]
    (0, 0)
}

/// Clic aux coordonnées d'une capture (repère de l'image renvoyée par screenshot).
fn click(x: f64, y: f64, button: &str, double: bool) -> Result<String, String> {
    let (w, h) = screen_size();
    if w == 0 {
        return Err("Écran introuvable".into());
    }
    let scale = (w as f64 / SHOT_WIDTH).max(1.0);
    let (px, py) = ((x * scale).round() as i32, (y * scale).round() as i32);
    if px < 0 || py < 0 || px >= w || py >= h {
        return Err(format!("Coordonnées hors écran ({px}, {py})"));
    }
    let button = if button == "right" { "droit" } else { "gauche" };
    let (down, up) = if button == "droit" { (0x0008, 0x0010) } else { (0x0002, 0x0004) };
    #[cfg(windows)]
    unsafe {
        ffi::SetCursorPos(px, py);
        std::thread::sleep(Duration::from_millis(60));
        for _ in 0..if double { 2 } else { 1 } {
            ffi::mouse_event(down, 0, 0, 0, 0);
            ffi::mouse_event(up, 0, 0, 0, 0);
            std::thread::sleep(Duration::from_millis(80));
        }
    }
    Ok(format!("Clic {button} en ({px}, {py}) à l'écran"))
}

/// Échappe une valeur pour une chaîne PowerShell entre apostrophes.
fn ps_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Exécute un script via un fichier .ps1 temporaire (`-Command -` casse les blocs multi-lignes).
fn powershell(script: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join("lody");
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join(format!("ps-{}.ps1", uuid::Uuid::new_v4()));
    // BOM UTF-8 : Windows PowerShell 5.1 lit sinon le script en ANSI.
    // `[Console]::OutputEncoding` garantit l'UTF-8 pour les titres accentués.
    let full = format!("\u{feff}[Console]::OutputEncoding = [Text.Encoding]::UTF8\n$ProgressPreference='SilentlyContinue'\n$ErrorActionPreference='Stop'\n{script}\n");
    std::fs::write(&file, full).map_err(|e| e.to_string())?;
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut c);
    let res = c.output();
    let _ = std::fs::remove_file(&file);
    let o = res.map_err(|e| e.to_string())?;
    let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if o.status.success() {
        Ok(out)
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

const MEDIA_PRELUDE: &str = r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Await($op, $type) { $t = $asTask.MakeGenericMethod($type).Invoke($null, @($op)); $null = $t.Wait(5000); $t.Result }
$null = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager, Windows.Media.Control, ContentType = WindowsRuntime]
$mgr = Await ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager]::RequestAsync()) ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager])
"#;

fn now_playing() -> Result<String, String> {
    let script = format!(
        "{MEDIA_PRELUDE}\n$list = @(foreach ($s in $mgr.GetSessions()) {{\n  $p = Await ($s.TryGetMediaPropertiesAsync()) ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionMediaProperties])\n  [pscustomobject]@{{ app = $s.SourceAppUserModelId; artist = $p.Artist; title = $p.Title; album = $p.AlbumTitle; status = [string]$s.GetPlaybackInfo().PlaybackStatus }}\n}})\nConvertTo-Json -Compress -InputObject $list"
    );
    let out = powershell(&script)?;
    Ok(if out.is_empty() || out == "[]" { "Aucun lecteur média actif.".into() } else { out })
}

fn media(action: &str, app: &str, steps: u32) -> Result<String, String> {
    match action {
        "volume_up" => {
            press(VK_VOLUME_UP, steps.max(1));
            return Ok(format!("Volume +{}", steps.max(1) * 2));
        }
        "volume_down" => {
            press(VK_VOLUME_DOWN, steps.max(1));
            return Ok(format!("Volume -{}", steps.max(1) * 2));
        }
        "mute" => {
            press(VK_VOLUME_MUTE, 1);
            return Ok("Son coupé / rétabli".into());
        }
        _ => {}
    }
    let method = match action {
        "play" => "TryPlayAsync",
        "pause" => "TryPauseAsync",
        "toggle" => "TryTogglePlayPauseAsync",
        "next" => "TrySkipNextAsync",
        "previous" => "TrySkipPreviousAsync",
        "stop" => "TryStopAsync",
        a => return Err(format!("Action inconnue : {a}")),
    };
    let script = format!(
        "{MEDIA_PRELUDE}\n$app = {app}\n$target = $null\nif ($app) {{ $target = $mgr.GetSessions() | Where-Object {{ $_.SourceAppUserModelId -like \"*$app*\" }} | Select-Object -First 1 }}\nif (-not $target) {{ $target = $mgr.GetCurrentSession() }}\nif (-not $target) {{ 'Aucun lecteur'; exit }}\n$ok = Await ($target.{method}()) ([bool])\nStart-Sleep -Milliseconds 700\n$p = Await ($target.TryGetMediaPropertiesAsync()) ([Windows.Media.Control.GlobalSystemMediaTransportControlsSessionMediaProperties])\n\"$($target.SourceAppUserModelId) | ok=$ok | $($p.Artist) - $($p.Title) | $($target.GetPlaybackInfo().PlaybackStatus)\"",
        app = ps_str(app)
    );
    powershell(&script)
}

fn open(target: &str) -> Result<String, String> {
    let t = target.trim();
    if t.is_empty() {
        return Err("Cible vide".into());
    }
    let is_drive_path = t.len() > 2 && t.as_bytes()[1] == b':' && (t.as_bytes()[2] == b'\\' || t.as_bytes()[2] == b'/');
    let is_uri = !is_drive_path
        && t.split_once(':').map(|(s, _)| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))).unwrap_or(false);
    if is_uri || std::path::Path::new(t).exists() {
        shell_open(t)?;
        return Ok(format!("Ouvert : {t}"));
    }
    // Sinon : nom d'application installée (menu Démarrer, y compris applis du Store).
    let script = format!(
        "$a = Get-StartApps | Where-Object {{ $_.Name -like ('*' + {q} + '*') }} | Select-Object -First 5\nConvertTo-Json -Compress -InputObject @($a | ForEach-Object {{ @{{ name = $_.Name; id = $_.AppID }} }})",
        q = ps_str(t)
    );
    let apps: Value = serde_json::from_str(&powershell(&script)?).unwrap_or(json!([]));
    let first = apps.get(0).ok_or_else(|| format!("Aucune application ne correspond à « {t} »"))?;
    shell_open(&format!("shell:AppsFolder\\{}", str_of(first, "id")))?;
    Ok(format!("Application lancée : {}", str_of(first, "name")))
}

fn windows() -> Result<String, String> {
    powershell("Get-Process | Where-Object { $_.MainWindowTitle } | ForEach-Object { \"$($_.ProcessName): $($_.MainWindowTitle)\" }")
}

fn clipboard(set: Option<&str>) -> Result<String, String> {
    match set {
        Some(text) => powershell(&format!("Set-Clipboard -Value {}; 'Copié'", ps_str(text))),
        None => powershell("Get-Clipboard -Raw"),
    }
}

fn send_keys(keys: &str, window: &str) -> Result<String, String> {
    powershell(&format!(
        "$w = New-Object -ComObject WScript.Shell\nif ({win}) {{ if (-not $w.AppActivate({win})) {{ throw 'Fenêtre introuvable' }}; Start-Sleep -Milliseconds 300 }}\n$w.SendKeys({keys}); 'Touches envoyées'",
        win = ps_str(window),
        keys = ps_str(keys)
    ))
}

/// Capture de l'écran principal, réduite, renvoyée en PNG base64.
fn screenshot() -> Result<String, String> {
    powershell(
        "Add-Type -AssemblyName System.Windows.Forms, System.Drawing\n$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds\n$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height\n$g = [System.Drawing.Graphics]::FromImage($bmp)\n$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)\n$scale = [Math]::Min([double]1, [double]1600 / $b.Width)\n$out = New-Object System.Drawing.Bitmap ([int]($b.Width * $scale)), ([int]($b.Height * $scale))\n$g2 = [System.Drawing.Graphics]::FromImage($out)\n$g2.InterpolationMode = 'HighQualityBicubic'\n$g2.DrawImage($bmp, 0, 0, $out.Width, $out.Height)\n$ms = New-Object System.IO.MemoryStream\n$out.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)\n[Convert]::ToBase64String($ms.ToArray())",
    )
}

/// Envoie une requête au serveur local de Lody (bulle dans l'island).
pub fn post_local(port: u16, path: &str, extra_header: &str, body: &str, timeout: Duration) -> Option<String> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(400)).ok()?;
    let _ = s.set_read_timeout(Some(timeout));
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n{extra_header}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).ok()?;
    s.write_all(body.as_bytes()).ok()?;
    let mut resp = Vec::new();
    let _ = s.read_to_end(&mut resp);
    let text = String::from_utf8_lossy(&resp).to_string();
    text.find("\r\n\r\n").map(|i| text[i + 4..].to_string())
}

fn tool_defs() -> Value {
    let s = |props: Value, req: &[&str]| json!({"type": "object", "properties": props, "required": req});
    json!([
        {"name": "open", "description": "Open something on the user's Windows PC: an installed app by name (e.g. 'Spotify', 'Discord', 'Calculatrice'), a URL, a URI (spotify:track:…, spotify:playlist:…, mailto:, ms-settings:…), a file or a folder.",
         "inputSchema": s(json!({"target": {"type": "string"}}), &["target"])},
        {"name": "now_playing", "description": "List active media players (Spotify, browser, etc.) with artist, title and playback status.",
         "inputSchema": s(json!({}), &[])},
        {"name": "media", "description": "Control music/video playback: play, pause, toggle, next, previous, stop (optionally for one app, e.g. app='Spotify'), or system volume_up / volume_down (steps of 2%) / mute.",
         "inputSchema": s(json!({"action": {"type": "string", "enum": ["play", "pause", "toggle", "next", "previous", "stop", "volume_up", "volume_down", "mute"]}, "app": {"type": "string"}, "steps": {"type": "integer"}}), &["action"])},
        {"name": "windows", "description": "List open application windows (process name and window title).",
         "inputSchema": s(json!({}), &[])},
        {"name": "clipboard_get", "description": "Read the clipboard text.", "inputSchema": s(json!({}), &[])},
        {"name": "clipboard_set", "description": "Copy text to the clipboard.", "inputSchema": s(json!({"text": {"type": "string"}}), &["text"])},
        {"name": "screenshot", "description": "Take a screenshot of the main screen to see what is displayed.", "inputSchema": s(json!({}), &[])},
        {"name": "click", "description": "Click at a position of the LAST screenshot (x, y in that image's pixels). Use it to press buttons in apps (e.g. the green Play button in Spotify). Take a screenshot first, then click, then verify.",
         "inputSchema": s(json!({"x": {"type": "number"}, "y": {"type": "number"}, "button": {"type": "string", "enum": ["left", "right"]}, "double": {"type": "boolean"}}), &["x", "y"])},
        {"name": "send_keys", "description": "Send keystrokes (WScript SendKeys syntax, e.g. '{ENTER}', '^s', ' ') to a window whose title contains `window` (or the focused one).",
         "inputSchema": s(json!({"keys": {"type": "string"}, "window": {"type": "string"}}), &["keys"])},
        {"name": "notify", "description": "Show a short message to the user in the Lody island at the top of the screen (e.g. 'C'est parti : Chill Lo-fi 🎶').",
         "inputSchema": s(json!({"message": {"type": "string"}, "title": {"type": "string"}}), &["message"])},
    ])
}

fn call(params: &Value, port: u16) -> Value {
    let a = &params["arguments"];
    let arg = |k: &str| str_of(a, k);
    let res: Result<Value, String> = match str_of(params, "name") {
        "open" => open(arg("target")).map(|t| json!([{"type": "text", "text": t}])),
        "now_playing" => now_playing().map(|t| json!([{"type": "text", "text": t}])),
        "media" => media(arg("action"), arg("app"), a["steps"].as_u64().unwrap_or(1) as u32)
            .map(|t| json!([{"type": "text", "text": t}])),
        "windows" => windows().map(|t| json!([{"type": "text", "text": t}])),
        "clipboard_get" => clipboard(None).map(|t| json!([{"type": "text", "text": t}])),
        "clipboard_set" => clipboard(Some(arg("text"))).map(|t| json!([{"type": "text", "text": t}])),
        "click" => click(
            a["x"].as_f64().unwrap_or(-1.0),
            a["y"].as_f64().unwrap_or(-1.0),
            arg("button"),
            a["double"].as_bool().unwrap_or(false),
        )
        .map(|t| json!([{"type": "text", "text": t}])),
        "send_keys" => send_keys(arg("keys"), arg("window")).map(|t| json!([{"type": "text", "text": t}])),
        "screenshot" => screenshot().map(|b64| {
            let (w, h) = screen_size();
            let k = (SHOT_WIDTH / w.max(1) as f64).min(1.0);
            json!([
                {"type": "image", "data": b64, "mimeType": "image/png"},
                {"type": "text", "text": format!("Capture {}x{} (écran {w}x{h}). Pour cliquer, donne à click les coordonnées dans cette image.", (w as f64 * k) as i32, (h as f64 * k) as i32)}
            ])
        }),
        "notify" => {
            let body = json!({"title": arg("title"), "message": arg("message")}).to_string();
            match post_local(port, "/notify", "", &body, Duration::from_secs(3)) {
                Some(_) => Ok(json!([{"type": "text", "text": "Affiché dans l'island"}])),
                None => Err("Lody n'est pas joignable".into()),
            }
        }
        n => Err(format!("Outil inconnu : {n}")),
    };
    match res {
        Ok(content) => json!({"content": content}),
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

/// Boucle JSON-RPC sur stdin/stdout (transport stdio MCP).
pub fn serve(args: &[String]) {
    let port = args
        .iter()
        .find_map(|a| a.parse::<u16>().ok())
        .unwrap_or(crate::config::DEFAULT_PORT);
    // Coordonnées physiques cohérentes entre capture et clic.
    #[cfg(windows)]
    unsafe {
        ffi::SetProcessDPIAware();
    }
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue }; // notification
        let method = str_of(&msg, "method");
        let reply = match method {
            "initialize" => json!({"jsonrpc": "2.0", "id": id, "result": {
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "lody", "version": env!("CARGO_PKG_VERSION")},
            }}),
            "tools/list" => json!({"jsonrpc": "2.0", "id": id, "result": {"tools": tool_defs()}}),
            "tools/call" => json!({"jsonrpc": "2.0", "id": id, "result": call(&msg["params"], port)}),
            "ping" => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
            m => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("Méthode inconnue : {m}")}}),
        };
        if writeln!(out, "{reply}").is_err() || out.flush().is_err() {
            break;
        }
    }
}

/// Fichier `--mcp-config` passé aux runs Claude Code.
pub fn mcp_config_file(run_id: &str, port: u16) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?.display().to_string();
    let v = json!({"mcpServers": {"lody": {"type": "stdio", "command": exe, "args": ["mcp", port.to_string()]}}});
    let dir = std::env::temp_dir().join("lody");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("mcp-{run_id}.json"));
    std::fs::write(&path, v.to_string()).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// Outils sans risque utilisables sans validation.
pub const SAFE_TOOLS: &str = "mcp__lody__open,mcp__lody__click,mcp__lody__now_playing,mcp__lody__media,mcp__lody__windows,mcp__lody__clipboard_get,mcp__lody__clipboard_set,mcp__lody__notify,mcp__lody__screenshot,mcp__claude_ai_Spotify__search,mcp__claude_ai_Spotify__get_currently_playing";
