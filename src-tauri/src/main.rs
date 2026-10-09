#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents;
mod app;
mod approvals;
mod config;
mod desktop;
mod devproc;
mod git;
mod github;
mod hooks;
mod mcp;
mod projects;
mod providers;
mod runner;
mod util;
mod window;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `lody.exe hook [port] [gate]` : relais léger appelé par les hooks Claude Code.
    if args.get(1).map(|a| a == "hook").unwrap_or(false) {
        hooks::client_main(&args[2..]);
        return;
    }
    // `lody.exe mcp [port]` : serveur MCP « bureau » pour les runs Claude Code.
    if args.get(1).map(|a| a == "mcp").unwrap_or(false) {
        desktop::serve(&args[2..]);
        return;
    }
    app::run();
}
