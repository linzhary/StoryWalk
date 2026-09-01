mod commands;
mod db;
mod error;
pub mod chat;
pub mod compression;
pub mod deepseek;
pub mod web_search;
pub mod materials;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;

/// Shared cancellation flag for aborting a running chat request.
pub struct CancelState(pub Arc<AtomicBool>);

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    dotenvy::dotenv().ok();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            app.manage(CancelState(Arc::new(AtomicBool::new(false))));

            let app_dir = app.path().app_data_dir().expect("failed to get app data dir");
            std::fs::create_dir_all(&app_dir).ok();
            // Use project-relative path in dev, app data dir in production
            let db_dir = if cfg!(debug_assertions) {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
            } else {
                app_dir
            };
            db::init_db(&db_dir).expect("failed to initialize database");

            // Create main window programmatically for native macOS traffic lights
            let url = if cfg!(debug_assertions) {
                tauri::WebviewUrl::External("http://localhost:1430".parse().unwrap())
            } else {
                tauri::WebviewUrl::App("index.html".into())
            };

            let mut builder = tauri::WebviewWindowBuilder::new(app, "main", url)
                .title("StoryWalk")
                .inner_size(1280.0, 860.0)
                .min_inner_size(900.0, 600.0)
                .resizable(true);

            #[cfg(target_os = "macos")]
            {
                builder = builder
                    .decorations(true)
                    .title_bar_style(tauri::TitleBarStyle::Overlay)
                    .traffic_light_position(tauri::LogicalPosition::new(12.0, 22.0))
                    .hidden_title(true);
            }

            #[cfg(not(target_os = "macos"))]
            {
                builder = builder.decorations(false);
            }

            builder.build().expect("failed to build main window");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::stories::get_stories,
            commands::stories::get_story,
            commands::stories::create_story,
            commands::stories::update_story,
            commands::stories::delete_story,
            commands::stories::get_breadcrumb_story,
            commands::sessions::get_sessions,
            commands::sessions::create_session,
            commands::sessions::update_session,
            commands::sessions::delete_session,
            chat::summarize_session,
            commands::messages::get_messages,
            commands::messages::get_message_count,
            commands::messages::get_messages_paginated,
            commands::messages::save_message,
            commands::messages::rollback_messages,
            commands::messages::delete_message,
            commands::story_cards::get_story_cards,
            commands::story_cards::save_story_card,
            commands::story_cards::update_story_card,
            commands::story_cards::delete_story_card,
            materials::read_story_materials,
            materials::update_story_materials,
            materials::trigger_material_extraction,
            chat::chat,
            chat::stop_chat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
