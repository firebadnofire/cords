//! Narrow desktop commands over the same client used by the acceptance CLI.
use anyhow::{Context as _, Result};
use cords_client_core::{
    accounts::{AccountRegistry, AccountSummary, PasswordAssessment, assess_password},
    client::{Client, Notification, Status},
};
use cords_protocol::messaging::{Channel, Contact, Message};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::Manager as _;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

#[derive(Debug, Default)]
struct Runtime {
    client: Option<Client>,
    registry: Option<AccountRegistry>,
    active_account_id: Option<String>,
    sync_task: Option<tokio::task::JoinHandle<()>>,
    watchdog_task: Option<tokio::task::JoinHandle<()>>,
    generation: u64,
    last_activity: Option<Instant>,
    auto_lock_minutes: Option<u32>,
    lock_on_suspend: bool,
    connected: bool,
    error: Option<String>,
}
#[derive(Debug, Default)]
pub(crate) struct Desktop(Arc<Mutex<Runtime>>);

#[derive(Debug, Serialize)]
pub(crate) struct AccountList {
    accounts: Vec<AccountSummary>,
    legacy_vault: bool,
}

fn paths(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf, Option<PathBuf>), String> {
    let directory = std::env::var_os("CORDS_CLIENT_STATE").map_or_else(
        || app.path().app_data_dir().map_err(|e| e.to_string()),
        |path| Ok(PathBuf::from(path)),
    )?;
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
    Ok((
        directory,
        migrations,
        std::env::var_os("CORDS_CLIENT_CA").map(PathBuf::from),
    ))
}

async fn registry(
    app: &tauri::AppHandle,
    runtime: &mut Runtime,
) -> Result<AccountRegistry, String> {
    if let Some(registry) = runtime.registry.as_ref() {
        return Ok(registry.clone());
    }
    let (directory, migrations, ca) = paths(app)?;
    let opened = AccountRegistry::open(&directory, &migrations, ca.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    runtime.registry = Some(opened.clone());
    Ok(opened)
}

async fn activate(
    state: &Arc<Mutex<Runtime>>,
    account_id: String,
    client: Client,
    policy: &AccountSummary,
) -> Status {
    let status = client.status();
    let mut runtime = state.lock().await;
    if let Some(task) = runtime.sync_task.take() {
        task.abort();
    }
    if let Some(task) = runtime.watchdog_task.take() {
        task.abort();
    }
    runtime.generation = runtime.generation.wrapping_add(1);
    let generation = runtime.generation;
    runtime.client = Some(client);
    runtime.active_account_id = Some(account_id);
    runtime.connected = false;
    runtime.error = None;
    runtime.last_activity = Some(Instant::now());
    runtime.auto_lock_minutes = policy.auto_lock_minutes;
    runtime.lock_on_suspend = policy.lock_on_suspend;
    runtime.sync_task = Some(tokio::spawn(synchronize(state.clone(), generation)));
    runtime.watchdog_task = Some(tokio::spawn(watchdog(state.clone(), generation)));
    status
}

async fn deactivate(runtime: &mut Runtime, sign_out: bool) -> Result<Option<String>, String> {
    if let Some(task) = runtime.sync_task.take() {
        task.abort();
    }
    if let Some(task) = runtime.watchdog_task.take() {
        task.abort();
    }
    runtime.generation = runtime.generation.wrapping_add(1);
    runtime.connected = false;
    runtime.last_activity = None;
    let selected = runtime.active_account_id.take();
    if let Some(mut client) = runtime.client.take() {
        if sign_out {
            client.sign_out().await.map_err(|e| e.to_string())?;
        }
        client.shutdown().await;
    }
    Ok(selected)
}

fn picker_safe(mut accounts: Vec<AccountSummary>) -> Vec<AccountSummary> {
    for (index, account) in accounts.iter_mut().enumerate() {
        if account.genericize {
            account.avatar_data.clear();
            account.avatar_shape = "circle".into();
            account.avatar_x_milli = 50_000;
            account.avatar_y_milli = 50_000;
            account.avatar_zoom_milli = 1_000;
        }
        if account.hide_nickname_on_lock {
            account.nickname = format!("Account {}", index + 1);
        }
    }
    accounts
}

/// Mirror lock-screen-safe presentation metadata after a successful unlock or settings save.
/// The picker never opens a vault merely to obtain this presentation data.
async fn mirror_picker_metadata(
    registry: &AccountRegistry,
    account_id: &str,
    preferences: &Value,
) -> Result<AccountSummary, String> {
    let mut account = registry
        .list()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|item| item.account_id == account_id)
        .ok_or("local account not found")?;
    if let Some(name) = preferences.get("displayName").and_then(Value::as_str) {
        account.nickname = name.to_string();
    }
    account.avatar_data = preferences
        .pointer("/avatar/data")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    account.avatar_shape = preferences
        .pointer("/avatar/shape")
        .and_then(Value::as_str)
        .unwrap_or("circle")
        .to_string();
    let crop_number = |field: &str, default: f64, min: f64, max: f64, scale: f64| {
        preferences
            .pointer(&format!("/avatar/{field}"))
            .and_then(Value::as_f64)
            .filter(|number| number.is_finite() && (*number >= min && *number <= max))
            .map_or(default, |number| (number * scale).round())
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        account.avatar_x_milli = crop_number("x", 50_000.0, 0.0, 100.0, 1_000.0) as u32;
        account.avatar_y_milli = crop_number("y", 50_000.0, 0.0, 100.0, 1_000.0) as u32;
        account.avatar_zoom_milli = crop_number("zoom", 1_000.0, 1.0, 4.0, 1_000.0) as u16;
    }
    account.genericize = preferences
        .get("genericize")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    account.hide_nickname_on_lock = preferences
        .get("hideNicknameOnLock")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    account.auto_lock_minutes = preferences
        .get("autoLockMinutes")
        .and_then(Value::as_u64)
        .and_then(|number| u32::try_from(number).ok());
    if preferences
        .get("autoLockMinutes")
        .is_some_and(Value::is_null)
    {
        account.auto_lock_minutes = None;
    }
    account.lock_on_os_lock = preferences
        .get("lockOnOsLock")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    account.lock_on_suspend = preferences
        .get("lockOnSuspend")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    registry
        .update_metadata(&account)
        .await
        .map_err(|error| error.to_string())?;
    Ok(account)
}

#[tauri::command]
pub(crate) async fn list_accounts(
    app: tauri::AppHandle,
    state: tauri::State<'_, Desktop>,
) -> Result<AccountList, String> {
    let mut runtime = state.0.lock().await;
    let registry = registry(&app, &mut runtime).await?;
    Ok(AccountList {
        accounts: picker_safe(registry.list().await.map_err(|e| e.to_string())?),
        legacy_vault: registry.legacy_vault_exists(),
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri deserializes command fields as owned values.
pub(crate) fn check_password(password: String, nickname: String) -> PasswordAssessment {
    let password = Zeroizing::new(password);
    assess_password(&password, &nickname)
}

#[tauri::command]
pub(crate) async fn create_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Desktop>,
    nickname: String,
    password: String,
    allow_weak: bool,
) -> Result<Status, String> {
    let password = Zeroizing::new(password);
    let (registry, state_arc) = {
        let mut runtime = state.0.lock().await;
        if runtime.client.is_some() {
            return Err("Lock or switch the current account first".into());
        }
        (registry(&app, &mut runtime).await?, state.0.clone())
    };
    let client = registry
        .create(&nickname, &password, allow_weak)
        .await
        .map_err(|e| e.to_string())?;
    let account_id = client.status().account_id;
    let policy = mirror_picker_metadata(&registry, &account_id, &client.ui_preferences()).await?;
    Ok(activate(&state_arc, account_id, client, &policy).await)
}

#[tauri::command]
pub(crate) async fn unlock_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Desktop>,
    account_id: String,
    password: String,
) -> Result<Status, String> {
    launch_account(&app, &state, account_id, Zeroizing::new(password)).await
}

pub(crate) async fn launch_account(
    app: &tauri::AppHandle,
    state: &Desktop,
    account_id: String,
    password: Zeroizing<String>,
) -> Result<Status, String> {
    let (registry, state_arc) = {
        let mut runtime = state.0.lock().await;
        if runtime.client.is_some() {
            return Err("An account is already unlocked".into());
        }
        (registry(app, &mut runtime).await?, state.0.clone())
    };
    let client = registry
        .unlock(&account_id, &password)
        .await
        .map_err(|e| e.to_string())?;
    let policy = mirror_picker_metadata(&registry, &account_id, &client.ui_preferences()).await?;
    Ok(activate(&state_arc, account_id, client, &policy).await)
}

#[tauri::command]
pub(crate) async fn active_account(
    state: tauri::State<'_, Desktop>,
) -> Result<Option<Status>, String> {
    Ok(state.0.lock().await.client.as_ref().map(Client::status))
}

pub(crate) async fn launch_accounts(
    app: &tauri::AppHandle,
    state: &Desktop,
) -> Result<Value, String> {
    let mut runtime = state.0.lock().await;
    let registry = registry(app, &mut runtime).await?;
    let accounts = picker_safe(registry.list().await.map_err(|error| error.to_string())?);
    Ok(json!(
        accounts
            .iter()
            .map(|account| json!({"account_id": account.account_id, "label": account.nickname}))
            .collect::<Vec<_>>()
    ))
}

#[tauri::command]
pub(crate) async fn migrate_legacy_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Desktop>,
    current_password: Option<String>,
    new_password: String,
    nickname: String,
    allow_weak: bool,
) -> Result<Status, String> {
    let current_password = current_password.map(Zeroizing::new);
    let new_password = Zeroizing::new(new_password);
    let (registry, state_arc) = {
        let mut runtime = state.0.lock().await;
        if runtime.client.is_some() {
            return Err("Lock or switch the current account first".into());
        }
        (registry(&app, &mut runtime).await?, state.0.clone())
    };
    let client = registry
        .migrate_legacy(
            current_password.as_deref().map(String::as_str),
            &new_password,
            &nickname,
            allow_weak,
        )
        .await
        .map_err(|e| e.to_string())?;
    let account_id = client.status().account_id;
    let policy = mirror_picker_metadata(&registry, &account_id, &client.ui_preferences()).await?;
    Ok(activate(&state_arc, account_id, client, &policy).await)
}

#[tauri::command]
pub(crate) async fn session_control(
    state: tauri::State<'_, Desktop>,
    action: String,
) -> Result<Option<String>, String> {
    let mut runtime = state.0.lock().await;
    match action.as_str() {
        "lock" | "switch" => deactivate(&mut runtime, false).await,
        "sign_out" => deactivate(&mut runtime, true).await,
        _ => Err("unsupported session action".into()),
    }
}

#[tauri::command]
pub(crate) async fn remove_local_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, Desktop>,
    account_id: String,
    password: String,
) -> Result<(), String> {
    let password = Zeroizing::new(password);
    let registry = {
        let mut runtime = state.0.lock().await;
        if runtime.active_account_id.as_deref() == Some(&account_id) {
            deactivate(&mut runtime, false).await?;
        }
        registry(&app, &mut runtime).await?
    };
    registry
        .remove(&account_id, &password)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn record_activity(state: tauri::State<'_, Desktop>) -> Result<(), String> {
    let mut runtime = state.0.lock().await;
    if runtime.client.is_some() {
        runtime.last_activity = Some(Instant::now());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn change_local_password(
    state: tauri::State<'_, Desktop>,
    account_id: String,
    current_password: String,
    new_password: String,
    allow_weak: bool,
) -> Result<(), String> {
    let current_password = Zeroizing::new(current_password);
    let new_password = Zeroizing::new(new_password);
    let mut runtime = state.0.lock().await;
    let client = runtime.client.as_mut().ok_or("unlock an account first")?;
    if client.status().account_id != account_id {
        return Err(
            "The active account changed; reopen its settings before changing the password".into(),
        );
    }
    let preferences = client.ui_preferences();
    let nickname = preferences
        .get("displayName")
        .and_then(Value::as_str)
        .unwrap_or("");
    if assess_password(&new_password, nickname).weak && !allow_weak {
        return Err("Review the weak-password warning before continuing".into());
    }
    client
        .change_password(current_password.as_bytes(), new_password.as_bytes())
        .await
        .map_err(|error| format!("{error:#}"))?;
    runtime.last_activity = Some(Instant::now());
    Ok(())
}

#[derive(Deserialize)] // Recovery/bootstrap codes must not appear in derived Debug output.
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Action {
    Trust {
        origin: String,
    },
    Servers,
    SelectServer {
        server_id: String,
    },
    DeselectServer,
    RemoveServer {
        server_id: String,
        local_only: bool,
    },
    ArchiveServer {
        server_id: String,
    },
    BurnIdentity,
    RetryBurns,
    DesignateSuccessor {
        account_id: String,
    },
    AcceptSuccessor {
        designation_hash: String,
    },
    RecoverOwner {
        code: String,
    },
    ClaimOwnership {
        code: String,
    },
    Authenticate,
    Publish,
    Channels,
    Create {
        name: String,
        #[serde(default)]
        confidentiality_mode: cords_protocol::messaging::ConfidentialityMode,
    },
    ReplaceChannel {
        route: String,
        name: String,
        confidentiality_mode: cords_protocol::messaging::ConfidentialityMode,
    },
    RetireChannel {
        route: String,
    },
    AcknowledgePublic {
        route: String,
    },
    ChannelArchives,
    ArchiveHistory {
        server_id: String,
        route: String,
    },
    Add {
        route: String,
        device: String,
    },
    Join {
        route: String,
    },
    Send {
        route: String,
        body: String,
    },
    Synchronize,
    Members,
    AccountDirectory {
        after: String,
    },
    MembershipRequests,
    RoleCatalog,
    RoleAudit {
        after: u64,
    },
    AccountRoles {
        account: String,
    },
    ChangeRolePolicy {
        request: cords_protocol::roles::RoleChangeRequest,
    },
    ApproveMembership {
        device: String,
    },
    RejectMembership {
        device: String,
    },
    Remove {
        route: String,
        device: String,
    },
    Revoke,
    AccountProfile {
        account: String,
    },
    AccountProfileImage {
        account: String,
        sha256: String,
    },
    SaveAccountProfile {
        profile: cords_protocol::profiles::AccountProfile,
        image: Option<String>,
        expected_heads: Vec<String>,
    },
    Preferences {
        value: Value,
    },
}

#[derive(Debug, Serialize)]
pub(crate) struct View {
    status: Status,
    connected: bool,
    error: Option<String>,
    messages: Vec<Message>,
    channels: Vec<Channel>,
    contacts: Vec<Contact>,
    identity: Value,
    preferences: Value,
}

async fn apply(client: &mut Client, action: Action) -> Result<Value> {
    Ok(match action {
        Action::Trust { origin } => serde_json::to_value(client.trust(&origin).await?)?,
        Action::Servers => serde_json::to_value(client.servers())?,
        Action::SelectServer { server_id } => {
            serde_json::to_value(client.select_server(&server_id).await?)?
        }
        Action::DeselectServer => serde_json::to_value(client.deselect_server().await?)?,
        Action::RemoveServer {
            server_id,
            local_only,
        } => serde_json::to_value(client.remove_server(&server_id, local_only).await?)?,
        Action::ArchiveServer { server_id } => {
            serde_json::to_value(client.archive_server(&server_id).await?)?
        }
        Action::BurnIdentity => serde_json::to_value(client.burn_identity().await?)?,
        Action::RetryBurns => serde_json::to_value(client.retry_pending_burns().await?)?,
        Action::DesignateSuccessor { account_id } => {
            json!(client.designate_successor(&account_id).await?)
        }
        Action::AcceptSuccessor { designation_hash } => {
            client.accept_successor(&designation_hash).await?;
            json!(true)
        }
        Action::RecoverOwner { code } => {
            let code = Zeroizing::new(code);
            serde_json::to_value(client.recover_owner(&code).await?)?
        }
        Action::ClaimOwnership { code } => {
            let code = Zeroizing::new(code);
            serde_json::to_value(client.claim_ownership(&code).await?)?
        }
        Action::Authenticate => serde_json::to_value(client.authenticate().await?)?,
        action => apply_conversation(client, action).await?,
    })
}

async fn apply_conversation(client: &mut Client, action: Action) -> Result<Value> {
    Ok(match action {
        Action::Publish => client.publish_key_package().await?,
        Action::Channels => serde_json::to_value(client.channels().await?)?,
        Action::Create {
            name,
            confidentiality_mode,
        } => json!(
            client
                .create_channel_with_mode(&name, confidentiality_mode)
                .await?
        ),
        Action::ReplaceChannel {
            route,
            name,
            confidentiality_mode,
        } => json!(
            client
                .replace_channel(&route, &name, confidentiality_mode)
                .await?
        ),
        Action::RetireChannel { route } => {
            client.retire_channel(&route).await?;
            json!(true)
        }
        Action::AcknowledgePublic { route } => {
            client.acknowledge_public_successor(&route).await?;
            json!(true)
        }
        Action::ChannelArchives => serde_json::to_value(client.channel_archives())?,
        Action::ArchiveHistory { server_id, route } => {
            serde_json::to_value(client.archived_channel_history(&server_id, &route).await?)?
        }
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
        Action::AccountProfile { account } => {
            serde_json::to_value(client.account_profile(&account).await?)?
        }
        Action::AccountProfileImage { account, sha256 } => {
            json!(client.account_profile_image(&account, &sha256).await?)
        }
        Action::SaveAccountProfile {
            profile,
            image,
            expected_heads,
        } => serde_json::to_value(
            client
                .save_account_profile(profile, image, expected_heads)
                .await?,
        )?,
        Action::AccountDirectory { after } => {
            serde_json::to_value(client.account_directory(&after).await?)?
        }
        Action::MembershipRequests => serde_json::to_value(client.membership_requests().await?)?,
        Action::RoleCatalog => serde_json::to_value(client.role_catalog().await?)?,
        Action::RoleAudit { after } => serde_json::to_value(client.role_audit(after).await?)?,
        Action::AccountRoles { account } => {
            serde_json::to_value(client.account_roles(&account).await?)?
        }
        Action::ChangeRolePolicy { request } => {
            serde_json::to_value(client.change_role_policy(&request).await?)?
        }
        Action::ApproveMembership { device } => {
            serde_json::to_value(client.approve_membership(&device).await?)?
        }
        Action::RejectMembership { device } => {
            client.reject_membership(&device).await?;
            json!(true)
        }
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
        _ => anyhow::bail!("Server lifecycle action sent to conversation dispatcher"),
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
        .ok_or("Unlock a local account first")?;
    let preferences = match &action {
        Action::Preferences { value } => Some(value.clone()),
        _ => None,
    };
    match apply(client, action).await {
        Ok(value) => {
            runtime.error = None;
            if let (Some(preferences), Some(account_id), Some(registry)) = (
                preferences,
                runtime.active_account_id.clone(),
                runtime.registry.clone(),
            ) {
                let account = mirror_picker_metadata(&registry, &account_id, &preferences).await?;
                runtime.auto_lock_minutes = account.auto_lock_minutes;
                runtime.lock_on_suspend = account.lock_on_suspend;
            }
            Ok(value)
        }
        Err(error) => {
            let message = format!("{error:#}");
            if let Err(reload) = client.reload().await {
                runtime.client = None;
                runtime.error = Some(format!("Storage recovery failed: {reload}"));
            } else {
                runtime.error = if message.contains("CORDS_APPROVAL_PENDING")
                    || message.contains("CORDS_JOIN_REJECTED")
                {
                    None
                } else {
                    Some(message.clone())
                };
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
        .ok_or("Unlock a local account first")?;
    let messages = match route.filter(|r| !r.is_empty()) {
        Some(route) => client.history(&route).await.map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    Ok(View {
        channels: client.cached_channels(),
        contacts: client.cached_members(),
        status: client.status(),
        connected: runtime.connected,
        error: runtime.error.clone(),
        messages,
        identity: client.identity_view(),
        preferences: client.ui_preferences(),
    })
}

async fn synchronize(state: Arc<Mutex<Runtime>>, generation: u64) {
    let mut receiver = None;
    let mut socket: Option<tokio::task::JoinHandle<()>> = None;
    let mut socket_server = String::new();
    let mut timer = tokio::time::interval(Duration::from_secs(3));
    loop {
        let disconnected = tokio::select! {
            () = async {timer.tick().await;} => false,
            notice = async {match &mut receiver {Some(rx) => tokio::sync::mpsc::Receiver::<Notification>::recv(rx).await, None => std::future::pending().await}} => !matches!(notice, Some(Notification::Advanced)),
        };
        let mut runtime = state.lock().await;
        if runtime.generation != generation {
            break;
        }
        if disconnected {
            receiver = None;
            if let Some(task) = socket.take() {
                task.abort();
            }
        }
        let Some(client) = runtime.client.as_mut() else {
            break;
        };
        if socket_server != client.status().server_id || client.is_burned() {
            if let Some(task) = socket.take() {
                task.abort();
            }
            receiver = None;
            socket_server = client.status().server_id;
        }
        if client.is_burned() {
            let retry = client.retry_pending_burns().await;
            runtime.connected = false;
            runtime.error = retry
                .err()
                .map(|e| format!("Identity-burn delivery retry failed: {e}"));
            continue;
        }
        if client.status().server_id.is_empty() {
            continue;
        }
        match client.refresh_ownership_state().await {
            Ok(state) if state == "OWNER_LOCKDOWN" => {
                if let Some(task) = socket.take() {
                    task.abort();
                }
                receiver = None;
                runtime.connected = false;
                runtime.error = None;
                continue;
            }
            Ok(_) => {}
            Err(error) => {
                runtime.connected = false;
                runtime.error = Some(format!("Signed server-status check failed: {error}"));
                continue;
            }
        }
        if matches!(
            client.status().admission_state.as_str(),
            "pending" | "rejected" | "removed"
        ) {
            runtime.connected = false;
            runtime.error = None;
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
                runtime.error = Some(format!("{error:#}"));
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

async fn watchdog(state: Arc<Mutex<Runtime>>, generation: u64) {
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    let mut prior = Instant::now();
    loop {
        tick.tick().await;
        let now = Instant::now();
        let mut runtime = state.lock().await;
        if runtime.generation != generation || runtime.client.is_none() {
            break;
        }
        let suspended = now.duration_since(prior) > Duration::from_secs(20);
        prior = now;
        let activity_elapsed = runtime
            .last_activity
            .map_or(Duration::ZERO, |last| now.duration_since(last));
        if lock_required(
            activity_elapsed,
            runtime.auto_lock_minutes,
            suspended,
            runtime.lock_on_suspend,
        ) {
            let _ = deactivate(&mut runtime, false).await;
            break;
        }
    }
}

fn lock_required(
    activity_elapsed: Duration,
    auto_lock_minutes: Option<u32>,
    suspend_gap: bool,
    lock_on_suspend: bool,
) -> bool {
    auto_lock_minutes
        .is_some_and(|minutes| activity_elapsed >= Duration::from_secs(u64::from(minutes) * 60))
        || (suspend_gap && lock_on_suspend)
}

#[cfg(test)]
mod tests {
    use super::{AccountSummary, lock_required, picker_safe};
    use std::time::Duration;

    #[test]
    fn picker_preserves_visible_shape_and_redacts_genericized_shape() {
        let visible = AccountSummary {
            account_id: "visible".into(),
            nickname: "Visible".into(),
            avatar_data: "data:image/png;base64,aGVsbG8=".into(),
            avatar_shape: "square".into(),
            avatar_x_milli: 25_000,
            avatar_y_milli: 75_000,
            avatar_zoom_milli: 1_500,
            genericize: false,
            hide_nickname_on_lock: false,
            auto_lock_minutes: Some(15),
            lock_on_os_lock: true,
            lock_on_suspend: true,
        };
        let mut hidden = visible.clone();
        hidden.genericize = true;
        hidden.hide_nickname_on_lock = true;
        let result = picker_safe(vec![visible.clone(), hidden]);
        assert_eq!(result[0], visible);
        assert!(result[1].avatar_data.is_empty());
        assert_eq!(result[1].avatar_shape, "circle");
        assert_eq!(result[1].avatar_x_milli, 50_000);
        assert_eq!(result[1].avatar_y_milli, 50_000);
        assert_eq!(result[1].avatar_zoom_milli, 1_000);
        assert_eq!(result[1].nickname, "Account 2");
    }

    #[test]
    fn inactivity_and_suspend_lock_without_network_activity_input() {
        assert!(!lock_required(
            Duration::from_secs(899),
            Some(15),
            false,
            true
        ));
        assert!(lock_required(
            Duration::from_mins(15),
            Some(15),
            false,
            true
        ));
        assert!(!lock_required(Duration::from_hours(24), None, false, true));
        assert!(lock_required(Duration::ZERO, None, true, true));
        assert!(!lock_required(Duration::ZERO, None, true, false));
    }
}
