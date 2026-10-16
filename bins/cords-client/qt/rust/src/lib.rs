//! Typed native presentation boundary. The worker, never Qt, owns the unlocked core.
//!
//! There are no `QObject` pointers in Rust and no JSON command dispatcher. Snapshots
//! contain verified presentation data only. Short exchanges do not span I/O awaits.

mod media;
mod runtime;
mod startup;
use anyhow::{Context as _, Result};
pub use media::{NativeImage, open_image};
use runtime::{Command, Envelope, Environment, LockSignal, Scope, Worker};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};
use tokio::sync::mpsc;
use zeroize::Zeroizing;

#[allow(unsafe_code)] // CXX-generated glue only; no handwritten unsafe code.
#[cxx::bridge(namespace = "cords")]
pub mod ffi {
    #[derive(Debug)]
    struct NativeFrame {
        width: u32,
        height: u32,
        bits: u32,
        duration_ticks: u32,
        ticks_numerator: u32,
        ticks_denominator: u32,
        loops: u32,
        last: bool,
        pixels: Vec<u8>,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct AccountRow {
        id: String,
        name: String,
        avatar: String,
        shape: String,
        x_milli: u32,
        y_milli: u32,
        zoom_milli: u32,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ServerRow {
        id: String,
        origin: String,
        state: String,
        selected: bool,
        archived: bool,
        connected: bool,
        error: String,
        unread: bool,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ChannelRow {
        id: String,
        name: String,
        mode: String,
        epoch: u64,
        joined: bool,
        retired: bool,
        acknowledgement: bool,
        can_manage: bool,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MessageRow {
        id: String,
        account: String,
        author: String,
        body: String,
        timestamp: u64,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MemberRow {
        id: String,
        name: String,
        roles: Vec<String>,
        group: String,
        color: String,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct PersonRow {
        id: String,
        image: String,
        shape: String,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct DeviceRow {
        id: String,
        account: String,
        name: String,
        generation: u64,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct RequestRow {
        id: String,
        account: String,
        name: String,
        state: String,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct RoleRow {
        id: String,
        name: String,
        color: String,
        description: String,
        priority: u32,
        permissions: Vec<String>,
        mentionable: bool,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct AuditRow {
        revision: u64,
        actor: String,
        operation: String,
        target: String,
        timestamp: u64,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Profile {
        account: String,
        name: String,
        image: String,
        shape: String,
        heads: Vec<String>,
        conflicts: Vec<String>,
        pending: String,
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Preferences {
        name: String,
        theme: String,
        compact: bool,
        reduced_motion: bool,
        disable_animations: bool,
        show_members: bool,
        auto_lock_minutes: u32,
        genericize: bool,
        hide_name: bool,
        lock_on_suspend: bool,
        lock_on_os_lock: bool,
        notifications: bool,
        notification_previews: bool,
        do_not_disturb: bool,
    }
    #[derive(Clone, Debug)]
    struct Alert {
        id: String,
        title: String,
        body: String,
        label: String,
    }
    #[derive(Clone, Debug)]
    struct Snapshot {
        completed: u64,
        unlocked: bool,
        legacy: bool,
        account: String,
        device: String,
        server: String,
        origin: String,
        ownership: String,
        admission: String,
        route: String,
        error: String,
        notice: String,
        connected: bool,
        can_create: bool,
        can_approve: bool,
        can_roles: bool,
        can_assign: bool,
        can_audit: bool,
        revoked: bool,
        burned: bool,
        revision: u64,
        roles_active: bool,
        server_muted: bool,
        route_muted: bool,
        directory_after: String,
        presentation_error: String,
        directory_loaded: bool,
        audit_after: u64,
        accounts: Vec<AccountRow>,
        servers: Vec<ServerRow>,
        channels: Vec<ChannelRow>,
        messages: Vec<MessageRow>,
        members: Vec<MemberRow>,
        people: Vec<PersonRow>,
        devices: Vec<DeviceRow>,
        requests: Vec<RequestRow>,
        roles: Vec<RoleRow>,
        audit: Vec<AuditRow>,
        profile: Profile,
        preferences: Preferences,
        notifications: Vec<Alert>,
    }
    extern "Rust" {
        type NativeImage;
        fn open_image(source: &str) -> Result<Box<NativeImage>>;
        fn next_frame(self: &mut NativeImage) -> Result<NativeFrame>;
        type Desktop;
        fn start_desktop(state: String, migrations: String, ca: String) -> Result<Box<Desktop>>;
        fn take_updates(self: &Desktop) -> Result<Vec<Snapshot>>;
        fn take_alerts(self: &Desktop) -> Result<Vec<Alert>>;
        fn notification_environment(self: &Desktop, visible: bool, allowed: bool);
        fn activate_alert(self: &Desktop, id: String) -> Result<()>;
        fn mute_server(self: &Desktop, muted: bool) -> Result<()>;
        fn mute_route(self: &Desktop, route: String, muted: bool) -> Result<()>;
        fn refresh(self: &Desktop) -> Result<()>;
        fn create_account(
            self: &Desktop,
            name: String,
            password: String,
            allow_weak: bool,
        ) -> Result<()>;
        fn unlock_account(self: &Desktop, account: String, password: String) -> Result<()>;
        fn unlock_stdin(self: &Desktop, account: String) -> Result<()>;
        fn migrate_account(
            self: &Desktop,
            name: String,
            old_password: String,
            password: String,
            allow_weak: bool,
        ) -> Result<()>;
        fn lock_account(self: &Desktop, sign_out: bool) -> Result<()>;
        fn platform_lock(self: &Desktop, suspended: bool) -> Result<bool>;
        fn remove_account(self: &Desktop, account: String, password: String) -> Result<()>;
        fn change_password(
            self: &Desktop,
            account: String,
            old_password: String,
            password: String,
            allow_weak: bool,
        ) -> Result<()>;
        fn activity(self: &Desktop);
        fn trust_server(self: &Desktop, origin: String) -> Result<()>;
        fn select_server(self: &Desktop, server: String) -> Result<()>;
        fn authenticate(self: &Desktop) -> Result<()>;
        fn claim_ownership(self: &Desktop, code: String) -> Result<()>;
        fn recover_ownership(self: &Desktop, code: String) -> Result<()>;
        fn designate_successor(self: &Desktop, account: String) -> Result<()>;
        fn accept_successor(self: &Desktop, hash: String) -> Result<()>;
        fn remove_server(self: &Desktop, server: String, local_only: bool) -> Result<()>;
        fn archive_server(self: &Desktop, server: String) -> Result<()>;
        fn burn_identity(self: &Desktop) -> Result<()>;
        fn retry_burns(self: &Desktop) -> Result<()>;
        fn select_channel(self: &Desktop, channel: String) -> Result<()>;
        fn create_channel(self: &Desktop, name: String, is_public: bool) -> Result<()>;
        fn replace_channel(
            self: &Desktop,
            channel: String,
            name: String,
            is_public: bool,
        ) -> Result<()>;
        fn retire_channel(self: &Desktop, channel: String) -> Result<()>;
        fn acknowledge_public(self: &Desktop, channel: String) -> Result<()>;
        fn join_channel(self: &Desktop, channel: String) -> Result<()>;
        fn publish_keys(self: &Desktop) -> Result<()>;
        fn send_message(self: &Desktop, channel: String, body: String) -> Result<()>;
        fn synchronize(self: &Desktop) -> Result<()>;
        fn load_directory(self: &Desktop, after: String) -> Result<()>;
        fn load_requests(self: &Desktop) -> Result<()>;
        fn approve_request(self: &Desktop, device: String) -> Result<()>;
        fn reject_request(self: &Desktop, device: String) -> Result<()>;
        fn add_device(self: &Desktop, channel: String, device: String) -> Result<()>;
        fn remove_device(self: &Desktop, channel: String, device: String) -> Result<()>;
        fn revoke_device(self: &Desktop) -> Result<()>;
        fn load_roles(self: &Desktop) -> Result<()>;
        fn load_account_roles(self: &Desktop, account: String) -> Result<()>;
        fn put_role(self: &Desktop, role: RoleRow, revision: u64, request_id: String)
        -> Result<()>;
        fn delete_role(
            self: &Desktop,
            role: String,
            revision: u64,
            request_id: String,
        ) -> Result<()>;
        fn assign_role(
            self: &Desktop,
            account: String,
            role: String,
            remove: bool,
            revision: u64,
            request_id: String,
        ) -> Result<()>;
        fn load_audit(self: &Desktop, after: u64) -> Result<()>;
        fn load_profile(self: &Desktop, account: String) -> Result<()>;
        fn save_profile_name(self: &Desktop, name: String, heads: Vec<String>) -> Result<()>;
        fn save_preferences(self: &Desktop, preferences: Preferences) -> Result<()>;
    }
}

/// Owns bounded worker input and presentation output; never stores a core Client.
pub struct Desktop {
    sender: Option<mpsc::Sender<Envelope>>,
    updates: Arc<Mutex<Option<ffi::Snapshot>>>,
    activity: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    lock: Arc<LockSignal>,
    environment: Arc<Environment>,
    alerts: Arc<Mutex<std::collections::VecDeque<ffi::Alert>>>,
    scope: Arc<Mutex<Scope>>,
    worker: Option<JoinHandle<()>>,
}
impl std::fmt::Debug for Desktop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desktop").finish_non_exhaustive()
    }
}

/// Start one account runtime. Paths are supplied by the native platform adapter.
/// # Errors
/// Returns an error if the worker cannot be started.
pub fn start_desktop(state: String, migrations: String, ca: String) -> Result<Box<Desktop>> {
    let (sender, receiver) = mpsc::channel(32);
    let updates = Arc::new(Mutex::new(None));
    let activity = Arc::new(AtomicBool::new(false));
    let stopping = Arc::new(AtomicBool::new(false));
    let scope = Arc::new(Mutex::new(Scope::default()));
    let output = updates.clone();
    let input_activity = activity.clone();
    let worker_stopping = stopping.clone();
    let worker_scope = scope.clone();
    let lock = Arc::new(LockSignal::default());
    let worker_lock = lock.clone();
    let environment = Arc::new(Environment::default());
    let worker_environment = environment.clone();
    let alerts = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    let worker_alerts = alerts.clone();
    let worker = std::thread::Builder::new()
        .name("cords-account".into())
        .spawn(move || {
            let result = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build();
            match result {
                Ok(runtime) => runtime.block_on(Worker::run(
                    PathBuf::from(state),
                    PathBuf::from(migrations),
                    (!ca.is_empty()).then(|| PathBuf::from(ca)),
                    receiver,
                    output,
                    input_activity,
                    worker_stopping,
                    worker_scope,
                    worker_lock,
                    worker_environment,
                    worker_alerts,
                )),
                Err(error) => {
                    if let Ok(mut slot) = output.lock() {
                        let mut snapshot = runtime::empty_snapshot();
                        snapshot.error = format!("Cannot start account runtime: {error}");
                        *slot = Some(snapshot);
                    }
                }
            }
        })?;
    Ok(Box::new(Desktop {
        sender: Some(sender),
        updates,
        activity,
        stopping,
        lock,
        environment,
        alerts,
        scope,
        worker: Some(worker),
    }))
}

impl Desktop {
    fn submit(&self, command: Command) -> Result<()> {
        anyhow::ensure!(
            !self.lock.pending(),
            "Account is locking; wait for the locked picker"
        );
        let expected = self
            .scope
            .lock()
            .map_err(|_| anyhow::anyhow!("Account scope exchange failed"))?
            .clone();
        self.sender
            .as_ref()
            .context("Desktop is shutting down")?
            .try_send(Envelope { command, expected })
            .map_err(|error| {
                anyhow::anyhow!(
                    "Account worker is busy or closed: {}",
                    match error {
                        mpsc::error::TrySendError::Full(_) =>
                            "request queue is full; retry after the current operation",
                        mpsc::error::TrySendError::Closed(_) => "worker stopped; restart Cords",
                    }
                )
            })
    }
    /// Drain at most one immutable snapshot without blocking on application I/O.
    /// # Errors
    /// Reports an invalid worker exchange instead of silently losing updates.
    pub fn take_updates(&self) -> Result<Vec<ffi::Snapshot>> {
        Ok(self
            .updates
            .lock()
            .map_err(|_| anyhow::anyhow!("Presentation exchange failed"))?
            .take()
            .into_iter()
            .collect())
    }
    pub fn activity(&self) {
        self.activity.store(true, Ordering::Release);
    }
    /// Drain bounded, already approved notifications for the native presenter.
    /// # Errors
    /// Reports a failed exchange rather than presenting unverified fallback data.
    pub fn take_alerts(&self) -> Result<Vec<ffi::Alert>> {
        let mut alerts = self
            .alerts
            .lock()
            .map_err(|_| anyhow::anyhow!("Notification exchange failed"))?;
        if self.lock.pending() {
            alerts.clear();
        }
        Ok(alerts.drain(..).collect())
    }
    pub fn notification_environment(&self, visible: bool, allowed: bool) {
        self.environment.visible.store(visible, Ordering::Release);
        self.environment.allowed.store(allowed, Ordering::Release);
    }
    /// # Errors
    /// Returns stale notification, account scope, or authoritative selection errors.
    pub fn activate_alert(&self, id: String) -> Result<()> {
        self.submit(Command::ActivateAlert(id))
    }
    /// # Errors
    /// Returns invalid scope or encrypted preference storage errors.
    pub fn mute_server(&self, muted: bool) -> Result<()> {
        self.submit(Command::MuteServer(muted))
    }
    /// # Errors
    /// Returns invalid route or encrypted preference storage errors.
    pub fn mute_route(&self, route: String, muted: bool) -> Result<()> {
        self.submit(Command::MuteRoute(route, muted))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn refresh(&self) -> Result<()> {
        self.submit(Command::Refresh)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn create_account(&self, name: String, password: String, allow_weak: bool) -> Result<()> {
        self.submit(Command::Create(name, Zeroizing::new(password), allow_weak))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn unlock_account(&self, account: String, password: String) -> Result<()> {
        self.submit(Command::Unlock(account, Zeroizing::new(password)))
    }
    /// Read bounded redirected credentials directly in Rust during local startup.
    /// # Errors
    /// Returns invalid account, input, or worker submission errors.
    pub fn unlock_stdin(&self, account: String) -> Result<()> {
        anyhow::ensure!(
            account.len() == 43
                && account
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'),
            "Use the exact account ID from --list-accounts"
        );
        self.submit(Command::Unlock(account, startup::password_stdin()?))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn migrate_account(
        &self,
        name: String,
        old_password: String,
        password: String,
        allow_weak: bool,
    ) -> Result<()> {
        self.submit(Command::Migrate(
            name,
            Zeroizing::new(old_password),
            Zeroizing::new(password),
            allow_weak,
        ))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn lock_account(&self, sign_out: bool) -> Result<()> {
        anyhow::ensure!(
            self.sender
                .as_ref()
                .is_some_and(|sender| !sender.is_closed()),
            "Account worker stopped; restart Cords"
        );
        self.lock.request(sign_out);
        Ok(())
    }
    /// Apply the verified local account policy to an operating-system event.
    /// # Errors
    /// Reports a stopped account runtime instead of claiming the vault locked.
    pub fn platform_lock(&self, suspended: bool) -> Result<bool> {
        anyhow::ensure!(
            self.sender
                .as_ref()
                .is_some_and(|sender| !sender.is_closed()),
            "Account worker stopped; restart Cords"
        );
        Ok(self.lock.platform_request(suspended))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn remove_account(&self, account: String, password: String) -> Result<()> {
        self.submit(Command::RemoveAccount(account, Zeroizing::new(password)))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn change_password(
        &self,
        account: String,
        old_password: String,
        password: String,
        allow_weak: bool,
    ) -> Result<()> {
        self.submit(Command::Password(
            account,
            Zeroizing::new(old_password),
            Zeroizing::new(password),
            allow_weak,
        ))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn trust_server(&self, origin: String) -> Result<()> {
        self.submit(Command::Trust(origin))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn select_server(&self, server: String) -> Result<()> {
        self.submit(Command::SelectServer(server))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn authenticate(&self) -> Result<()> {
        self.submit(Command::Authenticate)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn claim_ownership(&self, code: String) -> Result<()> {
        self.submit(Command::Claim(Zeroizing::new(code)))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn recover_ownership(&self, code: String) -> Result<()> {
        self.submit(Command::Recover(Zeroizing::new(code)))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn designate_successor(&self, account: String) -> Result<()> {
        self.submit(Command::Designate(account))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn accept_successor(&self, hash: String) -> Result<()> {
        self.submit(Command::Accept(hash))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn remove_server(&self, server: String, local_only: bool) -> Result<()> {
        self.submit(Command::RemoveServer(server, local_only))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn archive_server(&self, server: String) -> Result<()> {
        self.submit(Command::ArchiveServer(server))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn burn_identity(&self) -> Result<()> {
        self.submit(Command::Burn)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn retry_burns(&self) -> Result<()> {
        self.submit(Command::RetryBurns)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn select_channel(&self, channel: String) -> Result<()> {
        self.submit(Command::SelectChannel(channel))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn create_channel(&self, name: String, is_public: bool) -> Result<()> {
        self.submit(Command::CreateChannel(name, is_public))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn replace_channel(&self, channel: String, name: String, is_public: bool) -> Result<()> {
        self.submit(Command::ReplaceChannel(channel, name, is_public))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn retire_channel(&self, channel: String) -> Result<()> {
        self.submit(Command::RetireChannel(channel))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn acknowledge_public(&self, channel: String) -> Result<()> {
        self.submit(Command::Acknowledge(channel))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn join_channel(&self, channel: String) -> Result<()> {
        self.submit(Command::Join(channel))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn publish_keys(&self) -> Result<()> {
        self.submit(Command::Publish)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn send_message(&self, channel: String, body: String) -> Result<()> {
        self.submit(Command::Send(channel, body))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn synchronize(&self) -> Result<()> {
        self.submit(Command::Sync)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn load_directory(&self, after: String) -> Result<()> {
        self.submit(Command::Directory(after))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn load_requests(&self) -> Result<()> {
        self.submit(Command::Requests)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn approve_request(&self, device: String) -> Result<()> {
        self.submit(Command::Approve(device))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn reject_request(&self, device: String) -> Result<()> {
        self.submit(Command::Reject(device))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn add_device(&self, channel: String, device: String) -> Result<()> {
        self.submit(Command::Add(channel, device))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn remove_device(&self, channel: String, device: String) -> Result<()> {
        self.submit(Command::Remove(channel, device))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn revoke_device(&self) -> Result<()> {
        self.submit(Command::Revoke)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn load_roles(&self) -> Result<()> {
        self.submit(Command::Roles)
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn load_account_roles(&self, account: String) -> Result<()> {
        self.submit(Command::AccountRoles(account))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn put_role(&self, role: ffi::RoleRow, revision: u64, request_id: String) -> Result<()> {
        let permissions = role
            .permissions
            .iter()
            .map(|name| serde_json::from_value(serde_json::Value::String(name.clone())))
            .collect::<std::result::Result<_, _>>()?;
        self.submit(Command::RoleChange(
            cords_protocol::roles::RoleChangeRequest {
                expected_revision: revision.try_into()?,
                request_id,
                change: cords_protocol::roles::RoleChange::PutRole {
                    role: cords_protocol::roles::Role {
                        id: role.id,
                        name: role.name,
                        color: role.color,
                        description: role.description,
                        priority: role.priority,
                        permissions,
                        mentionable: role.mentionable,
                    },
                },
            },
        ))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn delete_role(&self, role: String, revision: u64, request_id: String) -> Result<()> {
        self.submit(Command::RoleChange(
            cords_protocol::roles::RoleChangeRequest {
                expected_revision: revision.try_into()?,
                request_id,
                change: cords_protocol::roles::RoleChange::DeleteRole { role_id: role },
            },
        ))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn assign_role(
        &self,
        account: String,
        role: String,
        remove: bool,
        revision: u64,
        request_id: String,
    ) -> Result<()> {
        use cords_protocol::roles::RoleChange;
        self.submit(Command::RoleChange(
            cords_protocol::roles::RoleChangeRequest {
                expected_revision: revision.try_into()?,
                request_id,
                change: if remove {
                    RoleChange::RevokeRole {
                        account_id: account,
                        role_id: role,
                    }
                } else {
                    RoleChange::AssignRole {
                        account_id: account,
                        role_id: role,
                    }
                },
            },
        ))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn load_audit(&self, after: u64) -> Result<()> {
        self.submit(Command::Audit(after))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn load_profile(&self, account: String) -> Result<()> {
        self.submit(Command::Profile(account))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn save_profile_name(&self, name: String, heads: Vec<String>) -> Result<()> {
        self.submit(Command::ProfileName(name, heads))
    }
    /// # Errors
    /// Returns a worker submission or input validation error.
    pub fn save_preferences(&self, preferences: ffi::Preferences) -> Result<()> {
        self.submit(Command::Preferences(preferences))
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        // Closing input wakes the worker. Do not abort a vault write or detach it.
        self.stopping.store(true, Ordering::Release);
        self.lock.request(false);
        self.sender.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            eprintln!("Cords account worker failed during shutdown");
        }
    }
}
