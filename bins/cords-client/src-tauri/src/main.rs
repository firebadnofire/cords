#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cords_client_core::{InspectedServerViewModel, ServerInspector};
mod conversation;
mod images;

#[tauri::command]
async fn inspect_server(origin: String) -> Result<InspectedServerViewModel, String> {
    ServerInspector::default()
        .inspect(&origin)
        .await
        .map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .manage(conversation::Desktop::default())
        .invoke_handler(tauri::generate_handler![
            inspect_server,
            conversation::open_client,
            conversation::conversation_action,
            conversation::conversation_view,
            images::load_image_url
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("Cords client startup failed: {error}");
            std::process::exit(1);
        });
}
