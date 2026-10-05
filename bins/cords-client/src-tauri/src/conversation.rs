//! Narrow desktop commands over the same client used by the acceptance CLI.
use anyhow::{Context as _, Result};
use cords_client_core::client::{Client, Notification, Status};
use cords_protocol::messaging::Message;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tauri::Manager as _;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

#[derive(Debug, Default)]
struct Runtime {
    client: Option<Client>,
    connected: bool,
    error: Option<String>,
}
#[derive(Debug, Default)]
pub(crate) struct Desktop(Arc<Mutex<Runtime>>);

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Action {
    Trust { origin: String },
    OwnershipCode,
    ClaimOwnership { code: String },
    Authenticate,
    Publish,
    Channels,
    Create { name: String },
    Add { route: String, device: String },
    Join { route: String },
    Send { route: String, body: String },
    Synchronize,
    Members,
    Remove { route: String, device: String },
    Revoke,
    Preferences { value: Value },
}

#[derive(Debug, Serialize)]
pub(crate) struct View {
    status: Status,
    connected: bool,
    error: Option<String>,
    messages: Vec<Message>,
    identity: Value,
    preferences: Value,
}

#[tauri::command]
pub(crate) async fn open_client(
    app: tauri::AppHandle,
    state: tauri::State<'_, Desktop>,
    passphrase: Option<String>,
) -> Result<Status, String> {
    let passphrase = passphrase.map(Zeroizing::new);
    let mut runtime = state.0.lock().await;
    if runtime.client.is_some() {
        return Err("This installation is already unlocked".into());
    }
    let directory = std::env::var_os("CORDS_CLIENT_STATE").map_or_else(
        || app.path().app_data_dir().map_err(|e| e.to_string()),
        |path| Ok(PathBuf::from(path)),
    )?;
    // Development overrides are native process configuration, never web content.
    let migrations = if let Some(path) = std::env::var_os("CORDS_CLIENT_MIGRATIONS") {
        PathBuf::from(path)
    } else if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../migrations/sqlite")
    } else {
        app.path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join("migrations/sqlite")
    };
    let ca = std::env::var_os("CORDS_CLIENT_CA").map(PathBuf::from);
    let client = Client::open(
        &directory,
        &migrations,
        passphrase.as_ref().map(|p| p.as_bytes()),
        ca.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;
    let status = client.status();
    runtime.client = Some(client);
    drop(runtime);
    tauri::async_runtime::spawn(synchronize(state.0.clone()));
    Ok(status)
}

async fn apply(client: &mut Client, action: Action) -> Result<Value> {
    Ok(match action {
        Action::Trust { origin } => serde_json::to_value(client.trust(&origin).await?)?,
        Action::OwnershipCode => json!(Client::ownership_claim_code()),
        Action::ClaimOwnership { code } => {
            let code = Zeroizing::new(code);
            serde_json::to_value(client.claim_ownership(&code).await?)?
        }
        Action::Authenticate => serde_json::to_value(client.authenticate().await?)?,
        Action::Publish => client.publish_key_package().await?,
        Action::Channels => serde_json::to_value(client.channels().await?)?,
        Action::Create { name } => json!(client.create_channel(&name).await?),
        Action::Add { route, device } => {
            client.add_member(&route, &device).await?;
            json!(true)
        }
        Action::Join { route } => {
            client.join_channel(&route).await?;
            json!(true)
        }
        Action::Send { route, body } => json!(client.send(&route, &body).await?),
        Action::Synchronize => serde_json::to_value(client.synchronize().await?)?,
        Action::Members => serde_json::to_value(client.members().await?)?,
        Action::Remove { route, device } => {
            client.remove_member(&route, &device).await?;
            json!(true)
        }
        Action::Revoke => {
            client.revoke_device().await?;
            json!(true)
        }
        Action::Preferences { value } => {
            client.save_ui_preferences(value).await?;
            json!(true)
        }
    })
}

#[tauri::command]
pub(crate) async fn conversation_action(
    state: tauri::State<'_, Desktop>,
    action: Action,
) -> Result<Value, String> {
    let mut runtime = state.0.lock().await;
    let client = runtime
        .client
        .as_mut()
        .ok_or("Unlock this installation first")?;
    match apply(client, action).await {
        Ok(value) => {
            runtime.error = None;
            Ok(value)
        }
        Err(error) => {
            let message = error.to_string();
            if let Err(reload) = client.reload().await {
                runtime.client = None;
                runtime.error = Some(format!("Storage recovery failed: {reload}"));
            } else {
                runtime.error = Some(message.clone());
            }
            Err(message)
        }
    }
}

#[tauri::command]
pub(crate) async fn conversation_view(
    state: tauri::State<'_, Desktop>,
    route: Option<String>,
) -> Result<View, String> {
    let runtime = state.0.lock().await;
    let client = runtime
        .client
        .as_ref()
        .ok_or("Unlock this installation first")?;
    let messages = match route.filter(|r| !r.is_empty()) {
        Some(route) => client.history(&route).await.map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    Ok(View {
        status: client.status(),
        connected: runtime.connected,
        error: runtime.error.clone(),
        messages,
        identity: client.identity_view(),
        preferences: client.ui_preferences(),
    })
}

async fn synchronize(state: Arc<Mutex<Runtime>>) {
    let mut receiver = None;
    let mut socket: Option<tokio::task::JoinHandle<()>> = None;
    let mut timer = tokio::time::interval(Duration::from_secs(3));
    loop {
        let disconnected = tokio::select! {
            () = async {timer.tick().await;} => false,
            notice = async {match &mut receiver {Some(rx) => tokio::sync::mpsc::Receiver::<Notification>::recv(rx).await, None => std::future::pending().await}} => !matches!(notice, Some(Notification::Advanced)),
        };
        let mut runtime = state.lock().await;
        if disconnected {
            receiver = None;
            if let Some(task) = socket.take() {
                task.abort();
            }
        }
        let Some(client) = runtime.client.as_mut() else {
            break;
        };
        if client.status().server_id.is_empty() {
            continue;
        }
        let result: Result<()> = async {
            if receiver.is_none() {
                let (task, rx) = client
                    .notifications()
                    .await
                    .context("Realtime connection failed")?;
                socket = Some(task);
                receiver = Some(rx);
            }
            client.synchronize().await?;
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {
                runtime.connected = true;
                runtime.error = None;
            }
            Err(error) => {
                let recovery = client.reload().await;
                runtime.connected = false;
                runtime.error = Some(error.to_string());
                if let Err(error) = recovery {
                    runtime.error = Some(format!("Storage recovery failed: {error}"));
                    runtime.client = None;
                    break;
                }
            }
        }
    }
    if let Some(task) = socket {
        task.abort();
    }
}
