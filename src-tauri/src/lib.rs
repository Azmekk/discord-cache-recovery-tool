//! Discord Cache Recovery: Tauri backend.
//!
//! Offline and read-only: no network requests, Discord's files are only ever opened for
//! reading. See `docs/ARCHITECTURE.md` for formats and the IPC contract.

use std::sync::Arc;

pub mod cache;
pub mod classify;
pub mod export;
pub mod http;
pub mod io;
pub mod media;
pub mod model;
pub mod scan;
pub mod sniff;
pub mod sources;

#[tauri::command]
async fn list_sources() -> Result<Vec<sources::Source>, String> {
    tauri::async_runtime::spawn_blocking(sources::list_sources)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(model::AppState::default()))
        .register_asynchronous_uri_scheme_protocol("media", media::protocol)
        .invoke_handler(tauri::generate_handler![
            list_sources,
            scan::scan,
            export::export_entries
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
