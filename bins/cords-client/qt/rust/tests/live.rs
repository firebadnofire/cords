//! Run with the existing server desktop fixture; no frontend-only state mocks.
use anyhow::{Context as _, Result, bail, ensure};
use cords_qt_bridge::{Desktop, ffi::Snapshot, start_desktop};
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn update(desktop: &Desktop, completed: u64) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        for view in desktop.take_updates()? {
            if view.completed >= completed {
                ensure!(
                    view.error.is_empty(),
                    "Native operation failed: {}",
                    view.error
                );
                return Ok(view);
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    bail!("Native live operation timed out")
}

fn automatic_presentation(desktop: &Desktop, owner: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(35);
    let mut last_error = String::new();
    while Instant::now() < deadline {
        for view in desktop.take_updates()? {
            last_error = view.presentation_error;
            if view.directory_loaded
                && view.members.len() == 2
                && view
                    .members
                    .iter()
                    .any(|member| member.id == owner && member.name == "Qt acceptance owner")
                && view.messages.iter().any(|message| {
                    message.account == owner && message.author == "Qt acceptance owner"
                })
                && view.people.iter().any(|person| {
                    person.id == owner
                        && person.image.starts_with("data:image/jxl;base64,")
                        && person.shape == "circle"
                })
            {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    bail!("Automatic verified directory/profile/avatar propagation failed: {last_error}")
}

#[test]
#[ignore = "requires a running disposable desktop_role_acceptance_fixture"]
#[allow(
    clippy::too_many_lines,
    reason = "One uninterrupted independent-vault network and restart acceptance scenario"
)]
fn two_vaults_real_tls_message_profiles_and_catalog() -> Result<()> {
    let root = PathBuf::from(std::env::var("CORDS_QT_ACCEPTANCE_FIXTURE")?);
    let ready: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("ready.json"))?)?;
    let migrations =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../migrations/sqlite");
    let open = || {
        start_desktop(
            ready["state"].as_str().context("fixture state")?.into(),
            migrations.to_string_lossy().into(),
            ready["ca"].as_str().context("fixture CA")?.into(),
        )
    };
    let owner = open()?;
    let picker = update(&owner, 0)?;
    let owner_id = picker
        .accounts
        .iter()
        .find(|a| a.name == "Role acceptance")
        .context("owner account")?
        .id
        .clone();
    let member_id = picker
        .accounts
        .iter()
        .find(|a| a.name == "Member acceptance")
        .context("member account")?
        .id
        .clone();
    // This fixture can be reused: normalize only this synthetic account's
    // notification settings before measuring new delivery and mute behavior.
    {
        let prepared = open()?;
        update(&prepared, 0)?;
        prepared.unlock_account(member_id.clone(), "Test-only Member Vault 5682!".into())?;
        let view = update(&prepared, 1)?;
        let route = view
            .channels
            .iter()
            .find(|channel| channel.name == "Role acceptance")
            .context("fixture channel")?
            .id
            .clone();
        prepared.mute_route(route, false)?;
        update(&prepared, 2)?;
        prepared.mute_server(false)?;
        update(&prepared, 3)?;
        let mut preferences = view.preferences;
        preferences.notifications = true;
        preferences.notification_previews = false;
        preferences.do_not_disturb = false;
        prepared.save_preferences(preferences)?;
        update(&prepared, 4)?;
    }
    owner.unlock_account(owner_id.clone(), "Test-only Role Vault 9321!".into())?;
    let view = update(&owner, 1)?;
    ensure!(
        view.unlocked && view.account == owner_id,
        "wrong unlocked identity"
    );
    let channel = view
        .channels
        .iter()
        .find(|c| c.name == "Role acceptance")
        .context("fixture channel")?
        .id
        .clone();
    owner.select_channel(channel.clone())?;
    let history = update(&owner, 2)?;
    ensure!(
        history
            .messages
            .iter()
            .any(|m| m.body == "Packaged review: owner message"),
        "existing history absent"
    );
    owner.load_roles()?;
    let catalog = update(&owner, 3)?;
    ensure!(
        catalog.roles.len() == 3 && catalog.revision == 0 && !catalog.roles_active,
        "Reading catalog changed defaults or activated enforcement"
    );
    owner.load_directory(String::new())?;
    let directory = update(&owner, 4)?;
    ensure!(
        directory.members.len() == 2,
        "directory did not aggregate two accounts"
    );
    owner.load_profile(owner_id.clone())?;
    let profile = update(&owner, 5)?;
    ensure!(
        profile.profile.account == owner_id,
        "wrong canonical profile"
    );
    owner.save_profile_name("Qt acceptance owner".into(), profile.profile.heads)?;
    let saved = update(&owner, 6)?;
    ensure!(
        saved.profile.name == "Qt acceptance owner",
        "signed profile change not reflected"
    );
    let member = open()?;
    update(&member, 0)?;
    member.unlock_account(member_id.clone(), "Test-only Member Vault 5682!".into())?;
    update(&member, 1)?;
    member.select_channel(channel.clone())?;
    update(&member, 2)?;
    automatic_presentation(&member, &owner_id)?;
    member.notification_environment(false, true);
    member.synchronize()?;
    update(&member, 3)?;
    ensure!(
        member.take_alerts()?.is_empty(),
        "Historical first sync emitted an alert"
    );
    let body = format!("Native Qt bridge acceptance {}", uuid::Uuid::now_v7());
    owner.send_message(channel.clone(), body.clone())?;
    let sent = update(&owner, 7)?;
    ensure!(
        sent.messages
            .iter()
            .any(|m| m.body == body && m.account == owner_id),
        "sender history missing authenticated account"
    );
    member.synchronize()?;
    let received = update(&member, 4)?;
    ensure!(
        received
            .messages
            .iter()
            .any(|m| m.body == body && m.account == owner_id),
        "independent recipient did not receive message"
    );
    let alerts = member.take_alerts()?;
    ensure!(
        alerts.len() == 1 && alerts[0].body == "New message",
        "New delivery did not produce exactly one private notification"
    );
    member.activate_alert(alerts[0].id.clone())?;
    ensure!(
        update(&member, 5)?.route == channel,
        "Notification navigation selected the wrong conversation"
    );
    member.mute_server(true)?;
    ensure!(
        update(&member, 6)?.server_muted,
        "Server mute did not persist"
    );
    owner.send_message(
        channel.clone(),
        format!("Muted server acceptance {}", uuid::Uuid::now_v7()),
    )?;
    update(&owner, 8)?;
    member.synchronize()?;
    update(&member, 7)?;
    ensure!(
        member.take_alerts()?.is_empty(),
        "Muted server emitted an alert"
    );
    member.mute_server(false)?;
    update(&member, 8)?;
    member.mute_route(channel.clone(), true)?;
    ensure!(
        update(&member, 9)?.route_muted,
        "Channel mute did not persist"
    );
    owner.send_message(
        channel.clone(),
        format!("Muted channel acceptance {}", uuid::Uuid::now_v7()),
    )?;
    update(&owner, 9)?;
    member.synchronize()?;
    update(&member, 10)?;
    ensure!(
        member.take_alerts()?.is_empty(),
        "Muted channel emitted an alert"
    );
    member.lock_account(false)?;
    let locked = update(&member, 11)?;
    ensure!(
        !locked.unlocked && locked.messages.is_empty(),
        "locked UI retains plaintext"
    );
    ensure!(
        locked.notifications.is_empty() && member.take_alerts()?.is_empty(),
        "Lock retained notification content or navigation targets"
    );
    drop(member);
    drop(owner);
    let reopened = open()?;
    update(&reopened, 0)?;
    reopened.unlock_account(owner_id.clone(), "Test-only Role Vault 9321!".into())?;
    update(&reopened, 1)?;
    reopened.select_channel(channel)?;
    let persisted = update(&reopened, 2)?;
    ensure!(
        persisted.messages.iter().any(|m| m.body == body),
        "message absent after restart"
    );
    Ok(())
}
