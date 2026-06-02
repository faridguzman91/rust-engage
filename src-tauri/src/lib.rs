mod commands;
mod crypto;
mod storage;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

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

            // Register for deep-link events (engage://...)
            // The frontend listens via the JS plugin API
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running engage");
}
