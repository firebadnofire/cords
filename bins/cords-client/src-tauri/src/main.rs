#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cords_client_core::{InspectedServerViewModel, ServerInspector};
use tauri::Manager as _;
mod conversation;
mod images;
mod launch;

#[tauri::command]
async fn inspect_server(origin: String) -> Result<InspectedServerViewModel, String> {
    ServerInspector::default()
        .inspect(&origin)
        .await
        .map_err(|error| error.to_string())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Cords client startup failed: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    use std::io::IsTerminal as _;
    let launch = launch::parse(std::env::args_os().skip(1))?;
    if launch == launch::Launch::Help {
        println!("{}", launch::HELP);
        return Ok(());
    }
    let password = if matches!(launch, launch::Launch::Unlock { .. }) {
        anyhow::ensure!(
            !std::io::stdin().is_terminal(),
            "--password-stdin requires redirected standard input, not an echoed terminal"
        );
        Some(launch::read_password(std::io::stdin().lock())?)
    } else {
        None
    };
    let app = tauri::Builder::default()
        .manage(conversation::Desktop::default())
        .invoke_handler(tauri::generate_handler![
            inspect_server,
            conversation::list_accounts,
            conversation::active_account,
            conversation::check_password,
            conversation::create_account,
            conversation::unlock_account,
            conversation::migrate_legacy_account,
            conversation::session_control,
            conversation::remove_local_account,
            conversation::record_activity,
            conversation::change_local_password,
            conversation::conversation_action,
            conversation::conversation_view,
            images::load_image_url
        ])
        .build(tauri::generate_context!())?;
    match launch {
        launch::Launch::List => {
            let accounts = tauri::async_runtime::block_on(conversation::launch_accounts(
                app.handle(),
                &app.state(),
            ))
            .map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&accounts)?);
            return Ok(());
        }
        launch::Launch::Unlock { account_id } => {
            tauri::async_runtime::block_on(conversation::launch_account(
                app.handle(),
                &app.state(),
                account_id,
                password.ok_or_else(|| anyhow::anyhow!("Password input was not supplied"))?,
            ))
            .map_err(anyhow::Error::msg)?;
        }
        launch::Launch::Picker | launch::Launch::Help => {}
    }
    app.run(|_, _| {});
    Ok(())
}
