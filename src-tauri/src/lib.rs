mod asset_store;
mod commands;
mod credentials;
mod db;
mod error;
mod generation;
mod images;
mod models;
mod openrouter;
mod state;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::Manager;
use tokio::sync::Semaphore;

use crate::asset_store::AssetStore;
use crate::db::{Database, DatabaseHandle};
use crate::generation::MAX_CONCURRENT_GENERATIONS;
use crate::openrouter::OpenRouterClient;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let assets = AssetStore::initialize(&app_data_dir.join("assets"))?;
            let mut database = Database::open(&app_data_dir.join("eidos.sqlite3"))?;
            database.recover_interrupted_attempts()?;
            let pending_deletions = database.pending_file_deletions()?;
            let completed_deletions = assets.delete_pending_paths(&pending_deletions);
            database.clear_pending_file_deletions(&completed_deletions)?;

            let remaining_deletions = database.pending_file_deletions()?;
            let mut retained_hashes = database.asset_hashes()?;
            retained_hashes.extend(AssetStore::hashes_for_persisted_paths(&remaining_deletions));
            assets.quarantine_unreferenced_objects(&retained_hashes)?;
            assets.delete_unreferenced_thumbnails(&retained_hashes)?;
            let database = DatabaseHandle::start(database)?;
            let openrouter = OpenRouterClient::production()?;

            app.manage(AppState {
                database,
                openrouter,
                references: Mutex::new(HashMap::new()),
                jobs: Mutex::new(HashMap::new()),
                generation_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_GENERATIONS)),
                assets,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::save_api_key,
            commands::remove_api_key,
            commands::select_reference_image,
            commands::discard_reference,
            commands::start_generation,
            commands::cancel_generation,
            commands::save_output,
            commands::list_history,
            commands::restore_history_reference,
            commands::delete_history_attempt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Eidos Studio");
}
