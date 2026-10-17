//! Single-owner orchestration over the existing Client, independent of Qt headers.
use crate::ffi;
use anyhow::{Context as _, Result, ensure};
use cords_client_core::notifications::{self, Approved, Classifier};
use cords_client_core::{
    accounts::{AccountRegistry, AccountSummary, assess_password},
    client::Client,
};
use cords_protocol::{
    messaging::ConfidentialityMode,
    roles::{RoleCatalog, RoleChangeRequest},
};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{Notify, mpsc};
use zeroize::Zeroizing;

// Intentionally no Debug: commands may own transient passwords/bootstrap codes.
pub(super) enum Command {
    Refresh,
    Create(String, Zeroizing<String>, bool),
    Unlock(String, Zeroizing<String>),
    Migrate(String, Zeroizing<String>, Zeroizing<String>, bool),
    RemoveAccount(String, Zeroizing<String>),
    Password(String, Zeroizing<String>, Zeroizing<String>, bool),
    Trust(String),
    SelectServer(String),
    Authenticate,
    Claim(Zeroizing<String>),
    Recover(Zeroizing<String>),
    Designate(String),
    Accept(String),
    RemoveServer(String, bool),
    ArchiveServer(String),
    Burn,
    RetryBurns,
    SelectChannel(String),
    OlderMessages(String),
    LatestMessages(String),
    CreateChannel(String, bool),
    ReplaceChannel(String, String, bool),
    RetireChannel(String),
    Acknowledge(String),
    Join(String),
    Publish,
    Send(String, String),
    Sync,
    Directory(String),
    Requests,
    Approve(String),
    Reject(String),
    Add(String, String),
    Remove(String, String),
    Revoke,
    Roles,
    AccountRoles(String),
    RoleChange(RoleChangeRequest),
    Audit(u64),
    Profile(String),
    ProfileName(String, Vec<String>),
    Preferences(ffi::Preferences),
    ActivateAlert(String),
    MuteServer(bool),
    MuteRoute(String, bool),
}

#[derive(Default)]
pub(super) struct Environment {
    pub visible: AtomicBool,
    pub allowed: AtomicBool,
}

#[derive(Clone, Default)]
pub(super) struct Scope {
    account: String,
    server: String,
}
pub(super) struct Envelope {
    pub command: Command,
    pub expected: Scope,
}

pub(super) struct LockSignal {
    request: AtomicU8,
    wakeup: Notify,
    policy: AtomicU8,
}
impl Default for LockSignal {
    fn default() -> Self {
        Self {
            request: AtomicU8::new(0),
            wakeup: Notify::new(),
            policy: AtomicU8::new(3),
        }
    }
}
impl LockSignal {
    pub(super) fn request(&self, sign_out: bool) {
        self.request
            .fetch_or(if sign_out { 3 } else { 1 }, Ordering::AcqRel);
        self.wakeup.notify_one();
    }
    async fn wait(&self) {
        while self.request.load(Ordering::Acquire) == 0 {
            self.wakeup.notified().await;
        }
    }
    pub(super) fn pending(&self) -> bool {
        self.request.load(Ordering::Acquire) != 0
    }
    pub(super) fn platform_request(&self, suspended: bool) -> bool {
        let bit = if suspended { 2 } else { 1 };
        if self.policy.load(Ordering::Acquire) & bit != 0 {
            self.request(false);
            true
        } else {
            false
        }
    }
    fn sign_out(&self) -> bool {
        self.request.load(Ordering::Acquire) & 2 != 0
    }
}

pub(super) fn default_preferences() -> ffi::Preferences {
    ffi::Preferences {
        name: "You".into(),
        theme: "dark".into(),
        compact: false,
        reduced_motion: false,
        disable_animations: false,
        show_members: true,
        auto_lock_minutes: 15,
        genericize: false,
        hide_name: false,
        lock_on_suspend: true,
        lock_on_os_lock: true,
        notifications: true,
        notification_previews: false,
        do_not_disturb: false,
    }
}
pub(super) fn empty_profile() -> ffi::Profile {
    ffi::Profile {
        account: String::new(),
        name: String::new(),
        image: String::new(),
        shape: "circle".into(),
        heads: Vec::new(),
        conflicts: Vec::new(),
        pending: String::new(),
    }
}
pub(super) fn empty_snapshot() -> ffi::Snapshot {
    ffi::Snapshot {
        completed: 0,
        unlocked: false,
        legacy: false,
        account: String::new(),
        device: String::new(),
        server: String::new(),
        origin: String::new(),
        ownership: String::new(),
        admission: String::new(),
        route: String::new(),
        error: String::new(),
        notice: String::new(),
        connected: false,
        can_create: false,
        can_approve: false,
        can_roles: false,
        can_assign: false,
        can_audit: false,
        revoked: false,
        burned: false,
        revision: 0,
        roles_active: false,
        server_muted: false,
        route_muted: false,
        history_older: false,
        history_newer: false,
        history_browsing: false,
        directory_after: String::new(),
        presentation_error: String::new(),
        directory_loaded: false,
        audit_after: 0,
        accounts: Vec::new(),
        servers: Vec::new(),
        channels: Vec::new(),
        messages: Vec::new(),
        members: Vec::new(),
        people: Vec::new(),
        devices: Vec::new(),
        requests: Vec::new(),
        roles: Vec::new(),
        audit: Vec::new(),
        profile: empty_profile(),
        preferences: default_preferences(),
        notifications: Vec::new(),
    }
}

struct DirectoryRefresh {
    after: String,
    members: Vec<ffi::MemberRow>,
    devices: Vec<ffi::DeviceRow>,
    target: usize,
}

pub(super) struct Worker {
    registry: AccountRegistry,
    client: Option<Client>,
    view: ffi::Snapshot,
    output: Arc<Mutex<Option<ffi::Snapshot>>>,
    last_activity: Instant,
    policy: Option<AccountSummary>,
    channel_by_server: BTreeMap<String, String>,
    server_order: Vec<String>,
    scope: Arc<Mutex<Scope>>,
    sync_cursor: usize,
    server_health: BTreeMap<String, (bool, String, bool)>,
    lock: Arc<LockSignal>,
    environment: Arc<Environment>,
    alerts: Arc<Mutex<VecDeque<ffi::Alert>>>,
    classifier: Classifier,
    alert_targets: VecDeque<Approved>,
    directory_refresh: Option<DirectoryRefresh>,
    directory_due: Instant,
    metadata_retry: Instant,
    profile_attempts: BTreeMap<String, Instant>,
    people: BTreeMap<String, ffi::PersonRow>,
    history_scope: (String, String, String),
    history_end: Option<String>,
    history_limit: u32,
}
impl Worker {
    #[allow(clippy::too_many_arguments)] // Explicit lifecycle inputs; no hidden global runtime.
    pub(super) async fn run(
        state: PathBuf,
        migrations: PathBuf,
        ca: Option<PathBuf>,
        mut input: mpsc::Receiver<Envelope>,
        output: Arc<Mutex<Option<ffi::Snapshot>>>,
        activity: Arc<AtomicBool>,
        stopping: Arc<AtomicBool>,
        scope: Arc<Mutex<Scope>>,
        lock: Arc<LockSignal>,
        environment: Arc<Environment>,
        alerts: Arc<Mutex<VecDeque<ffi::Alert>>>,
    ) {
        let registry = match AccountRegistry::open(&state, &migrations, ca.as_deref()).await {
            Ok(registry) => registry,
            Err(error) => {
                let mut view = empty_snapshot();
                view.error = format!("Cannot open account registry: {error:#}");
                if let Ok(mut slot) = output.lock() {
                    *slot = Some(view);
                }
                return;
            }
        };
        let mut worker = Self {
            registry,
            client: None,
            view: empty_snapshot(),
            output,
            last_activity: Instant::now(),
            policy: None,
            channel_by_server: BTreeMap::new(),
            server_order: Vec::new(),
            scope,
            sync_cursor: 0,
            server_health: BTreeMap::new(),
            lock: lock.clone(),
            environment,
            alerts,
            classifier: Classifier::default(),
            alert_targets: VecDeque::new(),
            directory_refresh: None,
            directory_due: Instant::now(),
            metadata_retry: Instant::now(),
            profile_attempts: BTreeMap::new(),
            people: BTreeMap::new(),
            history_scope: (String::new(), String::new(), String::new()),
            history_end: None,
            history_limit: 100,
        };
        if let Err(error) = worker.refresh().await {
            worker.view.error = format!("{error:#}");
        }
        worker.publish();
        let mut tick = tokio::time::interval(Duration::from_secs(3));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut previous = Instant::now();
        loop {
            if stopping.load(Ordering::Acquire) {
                break;
            }
            tokio::select! {
                biased;
                () = lock.wait() => {
                    worker.priority_close(&lock, &mut input).await;
                }
                command = input.recv() => {
                    let Some(envelope) = command else { break };
                    if stopping.load(Ordering::Acquire) { break; }
                    worker.execute(envelope, &lock, &mut input).await;
                    previous = Instant::now();
                }
                _ = tick.tick() => {
                    let now = Instant::now();
                    if activity.swap(false, Ordering::AcqRel) { worker.last_activity = now; }
                    let suspended = now.duration_since(previous) > Duration::from_secs(20);
                    if worker.policy.as_ref().is_some_and(|policy|
                        (policy.lock_on_suspend && suspended) || policy.auto_lock_minutes.is_some_and(|minutes|
                            now.duration_since(worker.last_activity) >= Duration::from_secs(u64::from(minutes) * 60))) {
                        worker.close(false).await;
                        worker.view.notice = "Account locked".into();
                    } else {
                        let result = tokio::select! {
                            biased;
                            () = lock.wait() => None,
                            result = worker.synchronize_next() => Some(result),
                        };
                        let Some(result) = result else {
                            worker.priority_close(&lock, &mut input).await;
                            continue;
                        };
                        if let Err(error) = result {
                            worker.close(false).await;
                            worker.view.error = format!("Storage recovery failed; account locked: {error:#}");
                        }
                    }
                    // Network work is not evidence of operating-system suspend.
                    previous = Instant::now();
                    if let Err(error) = worker.refresh().await { worker.view.error = format!("Refresh failed: {error:#}"); }
                    worker.publish();
                }
            }
        }
        worker.close(false).await;
    }

    async fn execute(
        &mut self,
        envelope: Envelope,
        lock: &LockSignal,
        input: &mut mpsc::Receiver<Envelope>,
    ) {
        self.view.error.clear();
        self.view.notice.clear();
        // Registry/filesystem transitions can span more than one
        // transaction. Complete them before dropping the account;
        // the native view is already hidden by the lock request.
        let local_transition = matches!(
            &envelope.command,
            Command::Create(..)
                | Command::Unlock(..)
                | Command::Migrate(..)
                | Command::RemoveAccount(..)
                | Command::Password(..)
                | Command::Preferences(..)
        );
        let result = if local_transition {
            Some(Box::pin(self.scoped_apply(envelope)).await)
        } else {
            tokio::select! {
                biased;
                () = lock.wait() => None,
                result = Box::pin(self.scoped_apply(envelope)) => Some(result),
            }
        };
        let Some(result) = result else {
            self.priority_close(lock, input).await;
            return;
        };
        let result = match result {
            Ok(()) => self.save_navigation().await,
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            self.view.error = format!("{error:#}");
            // Restore the last durable state after a failed mutating
            // operation, exactly as the original desktop adapter did.
            if let Some(client) = self.client.as_mut()
                && let Err(reload) = client.reload().await
            {
                self.close(false).await;
                self.view.error = format!("Storage recovery failed; account locked: {reload:#}");
            }
        }
        if lock.pending() {
            self.priority_close(lock, input).await;
            return;
        }
        if let Err(error) = self.refresh().await {
            self.view.error = format!("Refresh failed: {error:#}");
        }
        self.view.completed = self.view.completed.wrapping_add(1);
        self.publish();
    }

    async fn priority_close(&mut self, lock: &LockSignal, input: &mut mpsc::Receiver<Envelope>) {
        // Dropping the interrupted future rolls back an unfinished SQLite
        // transaction. shutdown() waits for outstanding database work before
        // releasing the vault. Remote acceptance may already have happened;
        // never silently replay the interrupted operation.
        let mut sign_out = lock.sign_out();
        while input.try_recv().is_ok() {}
        let mut recovery_error =
            (!self.view.error.is_empty()).then(|| std::mem::take(&mut self.view.error));
        if sign_out
            && let Some(client) = self.client.as_mut()
            && let Err(error) = client.reload().await
        {
            sign_out = false;
            recovery_error = Some(format!(
                "Account locked, but sign-out could not recover durable state: {error:#}"
            ));
        }
        self.close(sign_out).await;
        if let Some(error) = recovery_error {
            self.view.error = error;
        }
        self.view.notice =
            "Account locked; any interrupted remote operation must be reviewed after unlocking"
                .into();
        if let Err(error) = self.refresh_accounts().await {
            self.view.error = format!("Cannot refresh locked account picker: {error:#}");
        }
        self.view.completed = self.view.completed.wrapping_add(1);
        lock.request.store(0, Ordering::Release);
        self.publish();
    }

    fn publish(&self) {
        let policy = self.policy.as_ref().map_or(3, |policy| {
            u8::from(policy.lock_on_os_lock) | (u8::from(policy.lock_on_suspend) << 1)
        });
        self.lock.policy.store(policy, Ordering::Release);
        match self.scope.lock() {
            Ok(mut scope) => {
                *scope = Scope {
                    account: self.view.account.clone(),
                    server: self.view.server.clone(),
                }
            }
            Err(error) => eprintln!("Cords scope exchange failed: {error}"),
        }
        match self.output.lock() {
            Ok(mut slot) => *slot = Some(self.view.clone()),
            Err(error) => eprintln!("Cords presentation exchange failed: {error}"),
        }
    }

    async fn synchronize_next(&mut self) -> Result<()> {
        let started = Instant::now();
        let Some(client) = self.client.as_mut() else {
            return Ok(());
        };
        if client.is_burned() {
            return Ok(());
        }
        let servers: Vec<_> = client
            .servers()
            .into_iter()
            .filter(|server| !server.archived)
            .collect();
        if servers.is_empty() {
            return Ok(());
        }
        self.sync_cursor %= servers.len();
        let id = &servers[self.sync_cursor].server_id;
        self.sync_cursor = (self.sync_cursor + 1) % servers.len();
        let health = self.server_health.entry(id.clone()).or_default();
        match client.synchronize_server(id).await {
            Ok(result) => {
                health.0 = true;
                health.1.clear();
                health.2 |= id != &self.view.server
                    && result
                        .messages
                        .iter()
                        .any(|message| message.sender_account_id != self.view.account);
                self.classify(id, &result)?;
            }
            Err(error) => {
                health.0 = false;
                health.1 = format!("{error:#}");
                client.reload().await?;
            }
        }
        // Metadata is lower priority than relay synchronization. One page or
        // profile per tick shares the existing 30-second scheduling budget.
        let budget = Duration::from_secs(30).saturating_sub(started.elapsed());
        if budget >= Duration::from_secs(1)
            && Instant::now() >= self.metadata_retry
            && self
                .server_health
                .get(&self.view.server)
                .is_some_and(|health| health.0)
        {
            let result =
                tokio::time::timeout(budget.min(Duration::from_secs(15)), self.presentation())
                    .await;
            let error = match result {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(format!("Member/profile refresh failed: {error:#}")),
                Err(_) => Some(
                    "Member/profile refresh exceeded its deadline; cached presentation retained"
                        .into(),
                ),
            };
            if let Some(error) = error {
                self.client
                    .as_mut()
                    .context("Account locked")?
                    .reload()
                    .await?;
                self.view.presentation_error = error;
                self.directory_refresh = None;
                self.metadata_retry = Instant::now() + Duration::from_secs(30);
            }
        }
        Ok(())
    }

    fn classify(
        &mut self,
        server: &str,
        result: &cords_client_core::client::SyncResult,
    ) -> Result<()> {
        let client = self.client.as_ref().context("Account locked")?;
        let preferences = notification_preferences(&client.ui_preferences())?;
        let context = notifications::Context {
            account: &self.view.account,
            unlocked: !self.lock.pending(),
            visible_route: self.environment.visible.load(Ordering::Acquire).then(|| {
                notifications::Route {
                    server: self.view.server.clone(),
                    channel: self.view.route.clone(),
                }
            }),
            platform_allowed: self.environment.allowed.load(Ordering::Acquire),
        };
        let approved = self
            .classifier
            .classify(server, result, &context, &preferences)?;
        let mut alerts = self
            .alerts
            .lock()
            .map_err(|_| anyhow::anyhow!("Notification exchange failed"))?;
        for alert in approved {
            if alerts.len() == 32 {
                alerts.pop_front();
            }
            alerts.push_back(ffi::Alert {
                id: alert.id.clone(),
                title: alert.title.clone(),
                body: alert.body.clone(),
                label: String::new(),
            });
            if self.alert_targets.len() == 32 {
                self.alert_targets.pop_front();
            }
            self.alert_targets.retain(|old| old.route != alert.route);
            self.alert_targets.push_back(alert);
        }
        Ok(())
    }

    async fn save_mute(&mut self, route: Option<String>, muted: bool) -> Result<()> {
        let client = self.client.as_mut().context("Account locked")?;
        let server = client.status().server_id;
        ensure!(!server.is_empty(), "Select a server first");
        let mut value = client.ui_preferences();
        let mut preferences = notification_preferences(&value)?;
        let (key, updated) = if let Some(channel) = route {
            ensure!(
                client
                    .cached_channels()
                    .iter()
                    .any(|entry| entry.channel_id == channel),
                "Channel is unavailable on the selected server"
            );
            let target = notifications::Route { server, channel };
            if muted {
                preferences.muted_routes.insert(target);
            } else {
                preferences.muted_routes.remove(&target);
            }
            (
                "muted_routes",
                serde_json::to_value(&preferences.muted_routes)?,
            )
        } else {
            if muted {
                preferences.muted_servers.insert(server);
            } else {
                preferences.muted_servers.remove(&server);
            }
            (
                "muted_servers",
                serde_json::to_value(&preferences.muted_servers)?,
            )
        };
        let object = value
            .as_object_mut()
            .context("Invalid preference document")?
            .entry("notifications")
            .or_insert_with(|| serde_json::json!({}));
        ensure!(
            object.is_object(),
            "Invalid notification preference document"
        );
        object[key] = updated;
        client.save_ui_preferences(value).await?;
        self.alerts
            .lock()
            .map_err(|_| anyhow::anyhow!("Notification exchange failed while saving mute"))?
            .clear();
        self.alert_targets.retain(|alert| {
            !preferences.muted_servers.contains(&alert.route.server)
                && !preferences.muted_routes.contains(&alert.route)
        });
        self.view.notice = if muted {
            "Notifications muted"
        } else {
            "Notifications unmuted"
        }
        .into();
        Ok(())
    }

    async fn activate(&mut self, client: Client) -> Result<()> {
        let account = client.status().account_id;
        let policy = self
            .registry
            .list()
            .await?
            .into_iter()
            .find(|entry| entry.account_id == account)
            .context("Account is absent from registry")?;
        let (order, channels) = navigation(&client.ui_preferences())?;
        self.server_order = order;
        self.channel_by_server = channels;
        self.client = Some(client);
        self.policy = Some(policy);
        self.reset_server_views();
        self.last_activity = Instant::now();
        self.view.notice = "Account unlocked".into();
        Ok(())
    }
    async fn save_navigation(&mut self) -> Result<()> {
        let Some(client) = self.client.as_mut() else {
            return Ok(());
        };
        let mut value = client.ui_preferences();
        let (mut order, mut channels) = navigation(&value)?;
        let known: Vec<_> = client
            .servers()
            .into_iter()
            .map(|server| server.server_id)
            .collect();
        order.retain(|id| known.contains(id));
        for id in &known {
            if !order.contains(id) {
                order.push(id.clone());
            }
        }
        channels.retain(|server, _| known.contains(server));
        let active = client.status().server_id;
        if !self.view.route.is_empty()
            && client
                .cached_channels()
                .iter()
                .any(|channel| channel.channel_id == self.view.route)
        {
            channels.insert(active, self.view.route.clone());
        }
        let saved = serde_json::json!({"serverOrder": order, "channels": channels});
        if value.get("qtNavigation") != Some(&saved) {
            value
                .as_object_mut()
                .context("Invalid preference document")?
                .insert("qtNavigation".into(), saved);
            client
                .save_ui_preferences(value)
                .await
                .context("Cannot persist account navigation")?;
        }
        self.server_order = order;
        self.channel_by_server = channels;
        Ok(())
    }
    async fn close(&mut self, sign_out: bool) {
        let mut failure = None;
        if let Some(mut client) = self.client.take() {
            if sign_out {
                failure = client.sign_out().await.err();
            }
            client.shutdown().await;
        }
        let accounts = std::mem::take(&mut self.view.accounts);
        let legacy = self.view.legacy;
        let completed = self.view.completed;
        self.view = empty_snapshot();
        self.view.completed = completed;
        self.view.accounts = accounts;
        self.view.legacy = legacy;
        self.policy = None;
        self.history_scope = (String::new(), String::new(), String::new());
        self.history_end = None;
        self.history_limit = 100;
        self.channel_by_server.clear();
        self.server_order.clear();
        self.server_health.clear();
        self.sync_cursor = 0;
        self.classifier = Classifier::default();
        self.alert_targets.clear();
        self.people.clear();
        self.profile_attempts.clear();
        self.directory_refresh = None;
        match self.alerts.lock() {
            Ok(mut alerts) => alerts.clear(),
            Err(error) => failure = Some(anyhow::anyhow!("Notification cleanup failed: {error}")),
        }
        if let Some(error) = failure {
            self.view.error = format!("Sign-out could not remove session: {error:#}");
        }
    }

    async fn scoped_apply(&mut self, envelope: Envelope) -> Result<()> {
        let account_scoped = !matches!(
            envelope.command,
            Command::Refresh
                | Command::Create(..)
                | Command::Unlock(..)
                | Command::Migrate(..)
                | Command::RemoveAccount(..)
        );
        let server_scoped = account_scoped
            && !matches!(
                envelope.command,
                Command::Trust(..)
                    | Command::SelectServer(..)
                    | Command::Preferences(..)
                    | Command::Password(..)
                    | Command::Burn
                    | Command::RetryBurns
                    | Command::ActivateAlert(..)
            );
        if account_scoped {
            ensure!(
                envelope.expected.account == self.view.account,
                "Account changed before operation; reopen the current view"
            );
        }
        if server_scoped {
            ensure!(
                envelope.expected.server == self.view.server,
                "Server changed before operation; review the current target"
            );
        }
        Box::pin(self.apply(envelope.command)).await
    }
    #[allow(clippy::too_many_lines)] // One exhaustive typed operation-to-core mapping.
    async fn apply(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Refresh => return Ok(()),
            Command::Create(name, password, weak) => {
                ensure!(self.client.is_none(), "Lock the current account first");
                let client = self.registry.create(&name, &password, weak).await?;
                return self.activate(client).await;
            }
            Command::Unlock(account, password) => {
                ensure!(self.client.is_none(), "Lock the current account first");
                let client = self.registry.unlock(&account, &password).await?;
                return self.activate(client).await;
            }
            Command::Migrate(name, old, password, weak) => {
                ensure!(self.client.is_none(), "Lock the current account first");
                let client = self
                    .registry
                    .migrate_legacy(
                        (!old.is_empty()).then_some(old.as_str()),
                        &password,
                        &name,
                        weak,
                    )
                    .await?;
                return self.activate(client).await;
            }
            Command::RemoveAccount(account, password) => {
                if self
                    .client
                    .as_ref()
                    .is_some_and(|c| c.status().account_id == account)
                {
                    self.close(false).await;
                }
                self.registry.remove(&account, &password).await?;
                return Ok(());
            }
            _ => {}
        }
        let client = self
            .client
            .as_mut()
            .context("Unlock a local account first")?;
        match command {
            Command::Password(account, old, password, weak) => {
                ensure!(
                    client.status().account_id == account,
                    "Active account changed; reopen settings"
                );
                let name = client
                    .ui_preferences()
                    .get("displayName")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                ensure!(
                    !assess_password(&password, &name).weak || weak,
                    "Review the weak-password warning before continuing"
                );
                client
                    .change_password(old.as_bytes(), password.as_bytes())
                    .await?;
                self.view.notice = "Local password changed".into();
            }
            Command::Trust(origin) => {
                client.trust(&origin).await?;
                self.reset_server_views();
            }
            Command::SelectServer(server) => {
                self.channel_by_server
                    .insert(client.status().server_id, self.view.route.clone());
                if server.is_empty() {
                    client.deselect_server().await?;
                } else {
                    client.select_server(&server).await?;
                }
                if let Some(health) = self.server_health.get_mut(&server) {
                    health.2 = false;
                }
                self.reset_server_views();
                self.view.route = self
                    .channel_by_server
                    .get(&server)
                    .cloned()
                    .unwrap_or_default();
            }
            Command::Authenticate => {
                client.authenticate().await?;
                self.view.notice = "Authenticated".into();
            }
            Command::Claim(code) => {
                client.claim_ownership(&code).await?;
                self.view.notice = "Ownership claimed".into();
            }
            Command::Recover(code) => {
                client.recover_owner(&code).await?;
                self.view.notice = "Ownership recovered".into();
            }
            Command::Designate(account) => {
                self.view.notice = client.designate_successor(&account).await?;
            }
            Command::Accept(hash) => {
                client.accept_successor(&hash).await?;
                self.view.notice = "Successor accepted".into();
            }
            Command::RemoveServer(server, local) => {
                let result = client.remove_server(&server, local).await?;
                self.view.notice = result.warning.unwrap_or_else(|| {
                    if result.remote_confirmed {
                        "Departure verified".into()
                    } else {
                        "Removed locally".into()
                    }
                });
                self.reset_server_views();
            }
            Command::ArchiveServer(server) => {
                client.archive_server(&server).await?;
                self.reset_server_views();
            }
            Command::Burn => {
                let result = client.burn_identity().await?;
                self.view.notice = format!(
                    "Identity burned; {} deliveries pending",
                    result.pending.len()
                );
            }
            Command::RetryBurns => {
                let result = client.retry_pending_burns().await?;
                self.view.notice = format!("{} deliveries pending", result.pending.len());
            }
            Command::SelectChannel(route) => {
                ensure!(
                    client
                        .cached_channels()
                        .iter()
                        .any(|channel| channel.channel_id == route),
                    "Channel is unavailable on the selected server"
                );
                self.view.route = route;
            }
            Command::OlderMessages(route) => {
                ensure!(
                    route == self.view.route,
                    "Conversation changed; reopen its history"
                );
                ensure!(
                    self.view.history_older,
                    "No older cached messages are available"
                );
                let end_index = if self.view.messages.len() >= 300 {
                    self.view.messages.len() - 101
                } else {
                    self.view
                        .messages
                        .len()
                        .checked_sub(1)
                        .context("No cached history")?
                };
                self.history_end = Some(self.view.messages[end_index].id.clone());
                self.history_limit = (self.history_limit + 100).min(300);
            }
            Command::LatestMessages(route) => {
                ensure!(
                    route == self.view.route,
                    "Conversation changed; reopen its history"
                );
                self.history_end = None;
                self.history_limit = 100;
            }
            Command::CreateChannel(name, public) => {
                self.view.route = client.create_channel_with_mode(&name, mode(public)).await?;
            }
            Command::ReplaceChannel(route, name, public) => {
                self.view.route = client.replace_channel(&route, &name, mode(public)).await?;
            }
            Command::RetireChannel(route) => {
                client.retire_channel(&route).await?;
                self.view.notice = "Channel retired; local history preserved".into();
            }
            Command::Acknowledge(route) => {
                client.acknowledge_public_successor(&route).await?;
            }
            Command::Join(route) => {
                client.join_channel(&route).await?;
                self.view.route = route;
            }
            Command::Publish => {
                client.publish_key_package().await?;
                self.view.notice = "KeyPackage published".into();
            }
            Command::Send(route, body) => {
                ensure!(
                    route == self.view.route,
                    "Channel changed before send; review the current conversation"
                );
                client.send(&route, &body).await?;
                self.history_end = None;
                self.history_limit = 100;
                self.view.notice = "Message sent".into();
            }
            Command::Sync => {
                let result = client.synchronize().await?;
                let server = client.status().server_id;
                self.classify(&server, &result)?;
                self.view.connected = true;
                self.view.notice = "Synchronized".into();
            }
            Command::Directory(after) => {
                self.directory(&after).await?;
            }
            Command::Requests => {
                self.requests().await?;
            }
            Command::Approve(device) => {
                client.approve_membership(&device).await?;
                self.requests().await?;
            }
            Command::Reject(device) => {
                client.reject_membership(&device).await?;
                self.requests().await?;
            }
            Command::Add(route, device) => {
                client.add_member(&route, &device).await?;
            }
            Command::Remove(route, device) => {
                client.remove_member(&route, &device).await?;
            }
            Command::Revoke => {
                client.revoke_device().await?;
                self.view.notice = "Device revoked".into();
            }
            Command::Roles => {
                let catalog = client.role_catalog().await?;
                self.catalog(catalog)?;
            }
            Command::AccountRoles(account) => {
                let result = client.account_roles(&account).await?;
                for member in &mut self.view.members {
                    if member.id == account {
                        member.roles = result.role_ids.iter().cloned().collect();
                    }
                }
                self.catalog(result.catalog)?;
            }
            Command::RoleChange(request) => {
                client.change_role_policy(&request).await?;
                let catalog = client.role_catalog().await?;
                self.catalog(catalog)?;
                self.view.notice = "Role policy saved".into();
            }
            Command::Audit(after) => {
                let page = client.role_audit(after).await?;
                if after == 0 {
                    self.view.audit.clear();
                }
                self.view.audit_after = page.entries.last().map_or(after, |entry| entry.revision);
                self.view
                    .audit
                    .extend(page.entries.into_iter().map(|entry| ffi::AuditRow {
                        revision: entry.revision,
                        actor: entry.actor_device_id,
                        operation: entry.operation,
                        target: entry.target,
                        timestamp: entry.committed_at,
                    }));
            }
            Command::Profile(account) => {
                self.profile(&account).await?;
            }
            Command::ProfileName(name, heads) => {
                let account = client.status().account_id;
                let current = client.account_profile(&account).await?;
                let mut profile =
                    current
                        .state
                        .agreed
                        .unwrap_or(cords_protocol::profiles::AccountProfile {
                            display_name: String::new(),
                            avatar: None,
                        });
                profile.display_name = name;
                client.save_account_profile(profile, None, heads).await?;
                self.profile(&account).await?;
                self.view.notice = "Signed profile saved".into();
            }
            Command::Preferences(preferences) => {
                // Preserve unrecognized and existing frontend preferences rather
                // than replacing the encrypted preferences document.
                let mut value = client.ui_preferences();
                if !value.is_object() {
                    value = serde_json::json!({"version": 1});
                }
                for (key, field) in [
                    ("displayName", serde_json::json!(preferences.name)),
                    ("theme", serde_json::json!(preferences.theme)),
                    ("compact", serde_json::json!(preferences.compact)),
                    (
                        "reducedMotion",
                        serde_json::json!(preferences.reduced_motion),
                    ),
                    (
                        "disableAvatarAnimations",
                        serde_json::json!(preferences.disable_animations),
                    ),
                    ("showMembers", serde_json::json!(preferences.show_members)),
                    ("genericize", serde_json::json!(preferences.genericize)),
                    (
                        "hideNicknameOnLock",
                        serde_json::json!(preferences.hide_name),
                    ),
                    (
                        "lockOnSuspend",
                        serde_json::json!(preferences.lock_on_suspend),
                    ),
                    (
                        "lockOnOsLock",
                        serde_json::json!(preferences.lock_on_os_lock),
                    ),
                    (
                        "autoLockMinutes",
                        if preferences.auto_lock_minutes == 0 {
                            serde_json::Value::Null
                        } else {
                            serde_json::json!(preferences.auto_lock_minutes)
                        },
                    ),
                ] {
                    value[key] = field;
                }
                let notifications = value
                    .as_object_mut()
                    .context("Invalid preference document")?
                    .entry("notifications")
                    .or_insert_with(|| serde_json::json!({}));
                ensure!(
                    notifications.is_object(),
                    "Invalid notification preference document"
                );
                notifications["enabled"] = serde_json::json!(preferences.notifications);
                notifications["previews"] = serde_json::json!(preferences.notification_previews);
                notifications["do_not_disturb"] = serde_json::json!(preferences.do_not_disturb);
                client.save_ui_preferences(value).await?;
                self.mirror_preferences().await?;
                let mut alerts = self.alerts.lock().map_err(|_| {
                    anyhow::anyhow!("Notification exchange failed while saving preferences")
                })?;
                if !preferences.notifications || preferences.do_not_disturb {
                    alerts.clear();
                    self.alert_targets.clear();
                } else if !preferences.notification_previews {
                    for alert in &mut *alerts {
                        alert.body = "New message".into();
                    }
                    for alert in &mut self.alert_targets {
                        alert.body = "New message".into();
                    }
                }
                self.view.notice = "Preferences saved".into();
            }
            Command::MuteServer(muted) => {
                self.save_mute(None, muted).await?;
            }
            Command::MuteRoute(route, muted) => {
                self.save_mute(Some(route), muted).await?;
            }
            Command::ActivateAlert(id) => {
                let target = self
                    .alert_targets
                    .iter()
                    .find(|target| target.id == id)
                    .cloned()
                    .context("Notification expired; select the conversation manually")?;
                ensure!(
                    target.account == client.status().account_id,
                    "Notification belongs to another locked account"
                );
                self.channel_by_server
                    .insert(client.status().server_id, self.view.route.clone());
                client.select_server(&target.route.server).await?;
                ensure!(
                    client
                        .cached_channels()
                        .iter()
                        .any(|channel| channel.channel_id == target.route.channel),
                    "Notification channel is unavailable"
                );
                self.reset_server_views();
                self.alert_targets
                    .retain(|alert| alert.route != target.route);
                self.view.route = target.route.channel;
            }
            _ => anyhow::bail!("Unexpected account lifecycle command"),
        }
        Ok(())
    }

    fn reset_server_views(&mut self) {
        self.history_scope = (String::new(), String::new(), String::new());
        self.history_end = None;
        self.history_limit = 100;
        self.view.route.clear();
        self.view.messages.clear();
        self.view.members.clear();
        self.view.devices.clear();
        self.view.requests.clear();
        self.view.roles.clear();
        self.view.audit.clear();
        self.view.profile = empty_profile();
        self.view.directory_after.clear();
        self.view.presentation_error.clear();
        self.view.directory_loaded = false;
        self.view.people.clear();
        self.people.clear();
        self.profile_attempts.clear();
        self.directory_refresh = None;
        self.directory_due = Instant::now();
        self.metadata_retry = Instant::now();
        self.view.revision = 0;
        self.view.roles_active = false;
        self.view.connected = false;
    }
    async fn mirror_preferences(&mut self) -> Result<()> {
        let client = self.client.as_ref().context("Account locked")?;
        let value = client.ui_preferences();
        let mut policy = self.policy.clone().context("Account policy missing")?;
        if let Some(name) = value["displayName"].as_str() {
            policy.nickname = name.to_owned();
        }
        policy.genericize = value["genericize"].as_bool().unwrap_or(false);
        policy.hide_nickname_on_lock = value["hideNicknameOnLock"].as_bool().unwrap_or(false);
        policy.auto_lock_minutes = if value["autoLockMinutes"].is_null() {
            None
        } else {
            value["autoLockMinutes"]
                .as_u64()
                .map(u32::try_from)
                .transpose()?
        };
        policy.lock_on_suspend = value["lockOnSuspend"].as_bool().unwrap_or(true);
        policy.lock_on_os_lock = value["lockOnOsLock"].as_bool().unwrap_or(true);
        self.registry.update_metadata(&policy).await?;
        self.policy = Some(policy);
        Ok(())
    }
    async fn directory(&mut self, after: &str) -> Result<()> {
        let page = self
            .client
            .as_mut()
            .context("Account locked")?
            .account_directory(after)
            .await?;
        if after.is_empty() {
            self.view.members.clear();
            self.view.devices.clear();
        }
        self.view.directory_after = page.next_after.clone().unwrap_or_default();
        append_directory(&mut self.view.members, &mut self.view.devices, page);
        self.view.directory_loaded = true;
        self.view.presentation_error.clear();
        self.directory_refresh = None;
        self.directory_due = Instant::now() + Duration::from_mins(1);
        Ok(())
    }

    async fn presentation(&mut self) -> Result<()> {
        if self.directory_refresh.is_none() && Instant::now() >= self.directory_due {
            self.directory_refresh = Some(DirectoryRefresh {
                after: String::new(),
                members: Vec::new(),
                devices: Vec::new(),
                target: self.view.members.len().max(16),
            });
        }
        if let Some(refresh) = self.directory_refresh.as_mut() {
            let page = self
                .client
                .as_mut()
                .context("Account locked")?
                .account_directory(&refresh.after)
                .await?;
            refresh.after = page.next_after.clone().unwrap_or_default();
            append_directory(&mut refresh.members, &mut refresh.devices, page);
            if refresh.after.is_empty() || refresh.members.len() >= refresh.target {
                let refresh = self
                    .directory_refresh
                    .take()
                    .context("Directory refresh missing")?;
                self.view.directory_after = refresh.after;
                // Preserve authoritative role labels already fetched for surviving accounts.
                let mut members = refresh.members;
                for member in &mut members {
                    if let Some(old) = self.view.members.iter().find(|old| old.id == member.id) {
                        member.roles.clone_from(&old.roles);
                    }
                }
                self.view.members = members;
                self.view.devices = refresh.devices;
                self.view.directory_loaded = true;
                self.directory_due = Instant::now() + Duration::from_mins(1);
            }
        } else {
            let mut accounts: Vec<_> = self
                .view
                .messages
                .iter()
                .rev()
                .take(100)
                .map(|message| message.account.clone())
                .collect();
            accounts.extend(self.view.members.iter().map(|member| member.id.clone()));
            let mut unique = std::collections::BTreeSet::new();
            accounts.retain(|account| unique.insert(account.clone()));
            accounts.truncate(128);
            self.people.retain(|account, _| accounts.contains(account));
            self.profile_attempts
                .retain(|account, _| accounts.contains(account));
            if let Some(account) = accounts.into_iter().find(|account| {
                self.profile_attempts
                    .get(account)
                    .is_none_or(|attempt| attempt.elapsed() >= Duration::from_mins(2))
            }) {
                self.profile_attempts
                    .insert(account.clone(), Instant::now());
                let profile = self
                    .client
                    .as_mut()
                    .context("Account locked")?
                    .account_profile(&account)
                    .await?;
                self.cache_person(&account, &profile.state, profile.image.as_deref());
                if let Some(pending) = profile.pending_publication {
                    self.view.presentation_error = pending;
                    return Ok(());
                }
            }
        }
        self.view.presentation_error.clear();
        Ok(())
    }

    fn cache_person(
        &mut self,
        account: &str,
        state: &cords_protocol::profiles::ProfileState,
        image: Option<&str>,
    ) {
        let shape = state
            .agreed
            .as_ref()
            .and_then(|profile| profile.avatar.as_ref())
            .map_or("circle", |avatar| avatar.shape.as_str());
        let image = image
            .map(|image| format!("data:image/jxl;base64,{image}"))
            .unwrap_or_default();
        self.people.insert(
            account.into(),
            ffi::PersonRow {
                id: account.into(),
                image,
                shape: shape.into(),
            },
        );
        // One compressed source per account, shared by all message/member surfaces.
        // Evict imagery rather than retaining an unbounded in-memory media cache.
        let mut bytes: usize = self.people.values().map(|person| person.image.len()).sum();
        for person in self.people.values_mut() {
            if bytes <= 16 * 1024 * 1024 {
                break;
            }
            bytes -= person.image.len();
            person.image.clear();
        }
    }

    async fn requests(&mut self) -> Result<()> {
        self.view.requests = self
            .client
            .as_mut()
            .context("Account locked")?
            .membership_requests()
            .await?
            .into_iter()
            .map(|request| ffi::RequestRow {
                id: request.device_id,
                account: request.account_id,
                name: request
                    .user_card
                    .map_or_else(String::new, |card| card.value.nickname),
                state: request.status,
            })
            .collect();
        Ok(())
    }
    fn catalog(&mut self, catalog: RoleCatalog) -> Result<()> {
        self.view.revision = catalog.revision;
        self.view.roles_active = catalog.active;
        self.view.roles = catalog
            .roles
            .into_iter()
            .map(|role| {
                Ok(ffi::RoleRow {
                    id: role.id,
                    name: role.name,
                    color: role.color,
                    description: role.description,
                    priority: role.priority,
                    permissions: role
                        .permissions
                        .into_iter()
                        .map(|permission| {
                            serde_json::to_value(permission)?
                                .as_str()
                                .map(str::to_owned)
                                .context("Invalid permission encoding")
                        })
                        .collect::<Result<_>>()?,
                    mentionable: role.mentionable,
                })
            })
            .collect::<Result<_>>()?;
        self.view
            .roles
            .sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Ok(())
    }
    async fn profile(&mut self, account: &str) -> Result<()> {
        let result = self
            .client
            .as_mut()
            .context("Account locked")?
            .account_profile(account)
            .await?;
        self.cache_person(account, &result.state, result.image.as_deref());
        let (name, shape) = result.state.agreed.as_ref().map_or_else(
            || (String::new(), "circle".into()),
            |profile| {
                (
                    profile.display_name.clone(),
                    profile
                        .avatar
                        .as_ref()
                        .map_or("circle", |avatar| avatar.shape.as_str())
                        .to_owned(),
                )
            },
        );
        for member in &mut self.view.members {
            if member.id == account && !name.is_empty() {
                member.name.clone_from(&name);
            }
        }
        self.view.profile = ffi::Profile {
            account: account.into(),
            name,
            shape,
            image: result
                .image
                .map(|image| format!("data:image/jxl;base64,{image}"))
                .unwrap_or_default(),
            heads: result.state.heads,
            conflicts: result
                .state
                .conflicts
                .into_iter()
                .map(|field| format!("{field:?}"))
                .collect(),
            pending: result.pending_publication.unwrap_or_default(),
        };
        Ok(())
    }
    async fn refresh_accounts(&mut self) -> Result<()> {
        self.view.legacy = self.registry.legacy_vault_exists();
        self.view.accounts = self
            .registry
            .list()
            .await?
            .into_iter()
            .enumerate()
            .map(|(index, account)| ffi::AccountRow {
                id: account.account_id,
                name: if account.hide_nickname_on_lock {
                    format!("Account {}", index + 1)
                } else {
                    account.nickname
                },
                avatar: if account.genericize {
                    String::new()
                } else {
                    account.avatar_data
                },
                shape: if account.genericize {
                    "circle".into()
                } else {
                    account.avatar_shape
                },
                x_milli: account.avatar_x_milli,
                y_milli: account.avatar_y_milli,
                zoom_milli: u32::from(account.avatar_zoom_milli),
            })
            .collect();
        Ok(())
    }
    async fn refresh(&mut self) -> Result<()> {
        self.refresh_accounts().await?;
        let Some(client) = self.client.as_ref() else {
            return Ok(());
        };
        let status = client.status();
        let identity = client.identity_view();
        self.view.unlocked = true;
        self.view.account = status.account_id;
        self.view.device = status.device_id;
        self.view.server = status.server_id;
        self.view.origin = status.origin;
        self.view.ownership = status.ownership_state;
        self.view.admission = status.admission_state;
        self.view.burned = status.burned;
        self.view.revoked = identity["revoked"].as_bool().unwrap_or(true);
        let capabilities = identity
            .pointer("/membership/capabilities")
            .and_then(serde_json::Value::as_array);
        let has = |capability: &str| {
            capabilities.is_some_and(|values| {
                values
                    .iter()
                    .any(|value| value.as_str() == Some(capability))
            })
        };
        self.view.can_create = has("channel.create");
        self.view.can_approve = has("server.member.approve");
        self.view.can_roles = has("server.roles.manage");
        self.view.can_assign = has("server.roles.assign");
        self.view.can_audit = has("server.audit.read");
        self.view.servers = server_rows(client, &self.server_health, &self.server_order);
        if let Some((connected, error, _)) = self.server_health.get(&self.view.server) {
            self.view.connected = *connected;
            if !error.is_empty() && self.view.error.is_empty() {
                self.view.error = format!("Synchronization failed: {error}");
            }
        }
        self.view.channels = client
            .cached_channels()
            .into_iter()
            .map(|channel| ffi::ChannelRow {
                id: channel.channel_id,
                name: channel.name,
                mode: if channel.confidentiality_mode == ConfidentialityMode::Public {
                    "public"
                } else {
                    "encrypted"
                }
                .into(),
                epoch: channel.epoch,
                joined: channel
                    .members
                    .iter()
                    .any(|contact| contact.authorization.value.device_id == self.view.device),
                retired: channel.locally_archived
                    || channel
                        .transition
                        .as_ref()
                        .is_some_and(|transition| transition.value.retired),
                acknowledgement: channel.requires_public_acknowledgement,
                can_manage: channel.creator_device_id == self.view.device || has("channel.manage"),
            })
            .collect();
        if !self
            .view
            .channels
            .iter()
            .any(|channel| channel.id == self.view.route)
        {
            self.view.route = self
                .channel_by_server
                .get(&self.view.server)
                .cloned()
                .filter(|id| self.view.channels.iter().any(|channel| &channel.id == id))
                .or_else(|| self.view.channels.first().map(|channel| channel.id.clone()))
                .unwrap_or_default();
        }
        self.view.preferences = preferences(&client.ui_preferences());
        let notification_preferences = notification_preferences(&client.ui_preferences())?;
        self.view.server_muted = notification_preferences
            .muted_servers
            .contains(&self.view.server);
        self.view.route_muted =
            notification_preferences
                .muted_routes
                .contains(&notifications::Route {
                    server: self.view.server.clone(),
                    channel: self.view.route.clone(),
                });
        self.refresh_notifications()?;
        self.refresh_messages().await
    }

    fn refresh_notifications(&mut self) -> Result<()> {
        let client = self.client.as_ref().context("Account locked")?;
        let servers = client.servers();
        if self.environment.visible.load(Ordering::Acquire) {
            self.alert_targets.retain(|alert| {
                alert.route.server != self.view.server || alert.route.channel != self.view.route
            });
        }
        self.view.notifications = self
            .alert_targets
            .iter()
            .map(|alert| {
                let origin = servers
                    .iter()
                    .find(|server| server.server_id == alert.route.server)
                    .map_or_else(
                        || alert.route.server.clone(),
                        |server| server.origin.clone(),
                    );
                ffi::Alert {
                    id: alert.id.clone(),
                    title: alert.title.clone(),
                    body: alert.body.clone(),
                    label: format!(
                        "{} · {origin}",
                        client
                            .cached_channel_name(&alert.route.server, &alert.route.channel)
                            .unwrap_or_else(|| "Conversation unavailable".into())
                    ),
                }
            })
            .collect();
        Ok(())
    }

    async fn refresh_messages(&mut self) -> Result<()> {
        let client = self.client.as_ref().context("Account locked")?;
        let scope = (
            self.view.account.clone(),
            self.view.server.clone(),
            self.view.route.clone(),
        );
        if self.history_scope != scope {
            self.history_scope = scope;
            self.history_end = None;
            self.history_limit = 100;
        }
        self.view.history_older = false;
        self.view.history_newer = false;
        self.view.history_browsing = self.history_end.is_some();
        self.view.messages = if self.view.route.is_empty() {
            Vec::new()
        } else {
            let window = client
                .history_window(
                    &self.view.route,
                    self.history_end.as_deref(),
                    self.history_limit,
                )
                .await?;
            self.view.history_older = window.has_older;
            self.view.history_newer = window.has_newer;
            window
                .messages
                .into_iter()
                .map(|message| {
                    let account = message.sender_account_id;
                    let author = client
                        .cached_account_profile_state(&account)?
                        .agreed
                        .map(|profile| profile.display_name)
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| account.clone());
                    Ok(ffi::MessageRow {
                        id: message.message_id,
                        account,
                        author,
                        body: message.body,
                        timestamp: message.client_timestamp,
                    })
                })
                .collect::<Result<_>>()?
        };
        for member in &mut self.view.members {
            member.name = client
                .cached_account_profile_state(&member.id)?
                .agreed
                .map(|profile| profile.display_name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| member.id.clone());
        }
        self.view.people = self.people.values().cloned().collect();
        Ok(())
    }
}
fn append_directory(
    members: &mut Vec<ffi::MemberRow>,
    devices: &mut Vec<ffi::DeviceRow>,
    page: cords_protocol::directory::AccountDirectoryPage,
) {
    for account in page.accounts {
        if members.iter().any(|row| row.id == account.account_id) {
            continue;
        }
        // Legacy device labels never resolve a conflicting canonical name.
        members.push(ffi::MemberRow {
            id: account.account_id.clone(),
            name: account.account_id.clone(),
            roles: Vec::new(),
            group: "Members".into(),
            color: String::new(),
        });
        devices.extend(account.devices.into_iter().map(|device| {
            ffi::DeviceRow {
                id: device.authorization.value.device_id,
                account: account.account_id.clone(),
                name: device
                    .user_card
                    .map_or_else(String::new, |card| card.value.nickname),
                generation: device.authorization.value.generation,
            }
        }));
    }
}
fn server_rows(
    client: &Client,
    health: &BTreeMap<String, (bool, String, bool)>,
    order: &[String],
) -> Vec<ffi::ServerRow> {
    let mut servers = client.servers();
    servers.sort_by_key(|server| {
        order
            .iter()
            .position(|id| id == &server.server_id)
            .unwrap_or(usize::MAX)
    });
    servers
        .into_iter()
        .map(|server| {
            let status = health.get(&server.server_id).cloned().unwrap_or_default();
            ffi::ServerRow {
                id: server.server_id,
                origin: server.origin,
                state: server.ownership_state,
                selected: server.active,
                archived: server.archived,
                connected: status.0,
                error: status.1,
                unread: status.2,
            }
        })
        .collect()
}
fn mode(public: bool) -> ConfidentialityMode {
    if public {
        ConfidentialityMode::Public
    } else {
        ConfidentialityMode::Encrypted
    }
}
fn preferences(value: &serde_json::Value) -> ffi::Preferences {
    let mut result = default_preferences();
    if let Some(name) = value["displayName"].as_str() {
        result.name = name.into();
    }
    if let Some(theme @ ("dark" | "light" | "midnight")) = value["theme"].as_str() {
        result.theme = theme.into();
    }
    result.compact = value["compact"].as_bool().unwrap_or(false);
    result.reduced_motion = value["reducedMotion"].as_bool().unwrap_or(false);
    result.disable_animations = value["disableAvatarAnimations"].as_bool().unwrap_or(false);
    result.show_members = value["showMembers"].as_bool().unwrap_or(true);
    result.genericize = value["genericize"].as_bool().unwrap_or(false);
    result.hide_name = value["hideNicknameOnLock"].as_bool().unwrap_or(false);
    result.lock_on_suspend = value["lockOnSuspend"].as_bool().unwrap_or(true);
    result.lock_on_os_lock = value["lockOnOsLock"].as_bool().unwrap_or(true);
    result.notifications = value
        .pointer("/notifications/enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    result.notification_previews = value
        .pointer("/notifications/previews")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    result.do_not_disturb = value
        .pointer("/notifications/do_not_disturb")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    result.auto_lock_minutes = if value
        .get("autoLockMinutes")
        .is_some_and(serde_json::Value::is_null)
    {
        0
    } else {
        value["autoLockMinutes"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(15)
    };
    result
}

fn notification_preferences(value: &serde_json::Value) -> Result<notifications::Preferences> {
    value.get("notifications").map_or_else(
        || Ok(notifications::Preferences::default()),
        |value| {
            serde_json::from_value(value.clone())
                .context("Invalid encrypted notification preferences")
        },
    )
}

// Navigation lives in the existing encrypted per-account preference document.
// It is presentation state, never evidence of membership or channel access.
fn navigation(value: &serde_json::Value) -> Result<(Vec<String>, BTreeMap<String, String>)> {
    let Some(value) = value.get("qtNavigation") else {
        return Ok((Vec::new(), BTreeMap::new()));
    };
    ensure!(
        value.is_object(),
        "Invalid encrypted navigation preferences"
    );
    let order: Vec<String> = serde_json::from_value(
        value
            .get("serverOrder")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
    )
    .context("Invalid encrypted server order")?;
    let channels = serde_json::from_value(
        value
            .get("channels")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
    .context("Invalid encrypted channel navigation")?;
    ensure!(
        order
            .iter()
            .enumerate()
            .all(|(index, id)| !order[..index].contains(id)),
        "Duplicate server in navigation order"
    );
    Ok((order, channels))
}

#[cfg(test)]
mod navigation_tests {
    use super::*;

    #[test]
    fn navigation_rejects_malformed_and_duplicate_ids() -> Result<()> {
        let empty = navigation(&serde_json::json!({}))?;
        assert!(empty.0.is_empty() && empty.1.is_empty());
        assert!(navigation(&serde_json::json!({"qtNavigation": []})).is_err());
        assert!(
            navigation(&serde_json::json!({"qtNavigation": {"serverOrder": ["a", "a"]}})).is_err()
        );
        assert!(navigation(&serde_json::json!({"qtNavigation": {"channels": {"a": 4}}})).is_err());
        Ok(())
    }

    #[tokio::test]
    async fn navigation_persists_independently_in_real_vaults() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let migrations =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../migrations/sqlite");
        let registry = AccountRegistry::open(directory.path(), &migrations, None).await?;
        let first_password = "Mango Lantern independent navigation 83";
        let second_password = "Mango Lantern independent navigation 27";
        let mut first = registry.create("First", first_password, false).await?;
        let first_id = first.status().account_id;
        let mut value = first.ui_preferences();
        value["qtNavigation"] = serde_json::json!({"serverOrder": ["private-navigation-marker-a", "shared"], "channels": {"shared": "first-channel"}});
        first.save_ui_preferences(value.clone()).await?;
        first.shutdown().await;
        let mut second = registry.create("Second", second_password, false).await?;
        let second_id = second.status().account_id;
        let mut second_value = second.ui_preferences();
        assert!(navigation(&second_value)?.0.is_empty());
        second_value["qtNavigation"] = serde_json::json!({"serverOrder": ["shared", "b"], "channels": {"shared": "second-channel"}});
        second.save_ui_preferences(second_value.clone()).await?;
        second.shutdown().await;
        drop(registry);
        let registry = AccountRegistry::open(directory.path(), &migrations, None).await?;
        assert!(registry.unlock(&first_id, second_password).await.is_err());
        let first = registry.unlock(&first_id, first_password).await?;
        assert_eq!(first.ui_preferences(), value);
        // Presentation IDs do not create server memberships or credentials.
        assert!(first.servers().is_empty());
        assert!(first.identity_view()["membership"].is_null());
        first.shutdown().await;
        let second = registry.unlock(&second_id, second_password).await?;
        assert_eq!(second.ui_preferences(), second_value);
        assert!(second.servers().is_empty());
        second.shutdown().await;
        Ok(())
    }
}
