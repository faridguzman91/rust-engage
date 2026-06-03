// @faridguzman91: Tauri application entry point.
// Registers all IPC commands, initialises the SQLite database, and sets up
// the deep-link handler (engage:// scheme) for production OAuth callbacks.
mod commands;
mod crypto;
mod storage;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

// @faridguzman91: AppState holds the single SQLite connection behind a Mutex.
// All Tauri commands that touch the DB acquire this lock.
pub struct AppState {
    pub db: Mutex<Connection>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("engage.db");
            let conn = storage::db::open(&db_path).expect("failed to open database");
            app.manage(AppState { db: Mutex::new(conn) });

            // @faridguzman91: Register the engage:// URI scheme so Windows routes
            // OAuth deep-link callbacks to this app in production builds.
            // In dev mode the server uses FRONTEND_URL=http://localhost:1420 instead.
            #[cfg(desktop)]
            app.deep_link().register("engage")?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::identity::create_identity,
            commands::identity::get_identity,
            commands::contacts::list_contacts,
            commands::contacts::add_contact,
            commands::contacts::remove_contact,
            commands::messages::list_messages,
            commands::messages::send_message,
            commands::crypto::generate_prekey_bundle,
            commands::crypto::encrypt_message,
            commands::crypto::decrypt_message,
            commands::crypto::init_session,
            commands::crypto::init_inbound_session,
            commands::prekeys::get_opk_status,
            commands::prekeys::generate_and_store_opks,
            commands::disappear::get_disappear_timer,
            commands::disappear::set_disappear_timer,
            commands::disappear::set_message_expiry,
            commands::disappear::sweep_expired_messages,
            commands::groups::cache_group,
            commands::groups::list_cached_groups,
            commands::groups::encrypt_group_message,
            commands::groups::decrypt_group_message,
            commands::groups::get_sender_key_distribution,
            commands::groups::store_received_sender_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running engage");
}
