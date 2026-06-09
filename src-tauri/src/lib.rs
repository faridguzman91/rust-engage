// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman: Tauri application entry point — desktop, Android, and iOS.
//
// Platform notes:
//   Desktop (Windows/macOS/Linux)
//     — engage:// custom scheme registered at runtime (except macOS, where it
//       is declared statically in Info.plist via tauri.conf.json).
//   Android
//     — App Links (https://engage.app/auth, https://engage.app/invite) handle
//       OAuth callbacks and invite deep links.  The custom scheme is NOT used
//       on Android because App Links verify domain ownership, are harder to
//       hijack, and work without any runtime registration.
//     — The CDYlib is loaded by the Android runtime as libengagelib.so via JNI.
//     — SQLite database lives in app_data_dir() which maps to
//       /data/data/com.engage.app/files/ on Android.
//   iOS
//     — engage:// custom scheme declared in Info.plist (same as macOS).
//     — Database lives in app_data_dir() which maps to the app's Documents dir.
mod commands;
mod crypto;
mod storage;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

// @faridguzman: AppState holds the single SQLite connection behind a Mutex.
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

            // @faridguzman: Register the engage:// URI scheme at runtime on Windows/Linux.
            // macOS: declared statically in Info.plist — runtime registration is a no-op.
            // Android: uses App Links (https scheme) configured in tauri.conf.json —
            //   runtime registration of custom schemes is not supported on Android.
            // iOS: declared in Info.plist — same as macOS.
            #[cfg(all(desktop, not(target_os = "macos")))]
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
            commands::messages::update_message_status,
            commands::messages::queue_pending_message,
            commands::messages::list_pending_messages,
            commands::messages::remove_pending_message,
            commands::messages::increment_pending_retry,
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
