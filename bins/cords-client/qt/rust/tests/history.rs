//! Bounded native history against a fresh disposable TLS/PostgreSQL fixture.
use anyhow::{Context as _, Result, bail, ensure};
use cords_client_core::accounts::AccountRegistry;
use cords_qt_bridge::{Desktop, ffi::Snapshot, start_desktop};
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn snapshot(desktop: &Desktop, completed: u64) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline {
        for view in desktop.take_updates()? {
            if view.completed >= completed {
                return Ok(view);
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    bail!("Native history operation timed out")
}
fn update(desktop: &Desktop, completed: u64) -> Result<Snapshot> {
    let view = snapshot(desktop, completed)?;
    ensure!(
        view.error.is_empty(),
        "Native history operation: {}",
        view.error
    );
    Ok(view)
}

#[test]
#[ignore = "requires a running disposable desktop_role_acceptance_fixture; seeds or reuses 401 real messages"]
#[allow(
    clippy::too_many_lines,
    reason = "One bounded history/network/lock/restart acceptance scenario"
)]
fn bounded_history_pages_keep_the_frozen_window_and_clear_on_lock() -> Result<()> {
    let root = PathBuf::from(std::env::var("CORDS_QT_ACCEPTANCE_FIXTURE")?);
    let ready: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("ready.json"))?)?;
    let state = PathBuf::from(ready["state"].as_str().context("fixture state")?);
    let ca = PathBuf::from(ready["ca"].as_str().context("fixture CA")?);
    let migrations =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../migrations/sqlite");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let (owner_id, member_id, route) = runtime.block_on(async {
        let registry = AccountRegistry::open(&state, &migrations, Some(&ca)).await?;
        let accounts = registry.list().await?;
        let owner = accounts
            .iter()
            .find(|a| a.nickname == "Role acceptance")
            .context("owner fixture")?
            .account_id
            .clone();
        let member = accounts
            .iter()
            .find(|a| a.nickname == "Member acceptance")
            .context("member fixture")?
            .account_id
            .clone();
        let mut client = registry
            .unlock(&owner, "Test-only Role Vault 9321!")
            .await?;
        let route = client
            .cached_channels()
            .into_iter()
            .find(|c| c.name == "Role acceptance")
            .context("fixture route")?
            .channel_id;
        let existing = client.history_window(&route, None, 300).await?;
        if existing.messages.len() == 300
            && existing.has_older
            && existing
                .messages
                .iter()
                .any(|m| m.body == "History fixture 0400")
        {
            // Reuse this synthetic conversation without uploading another 401 rows.
            client.send(&route, "History fixture 0400").await?;
        } else {
            for index in 0..401 {
                client
                    .send(&route, &format!("History fixture {index:04}"))
                    .await?;
            }
        }
        client.shutdown().await;
        Ok::<_, anyhow::Error>((owner, member, route))
    })?;
    let open = || {
        start_desktop(
            state.to_string_lossy().into(),
            migrations.to_string_lossy().into(),
            ca.to_string_lossy().into(),
        )
    };
    let owner = open()?;
    update(&owner, 0)?;
    owner.unlock_account(owner_id.clone(), "Test-only Role Vault 9321!".into())?;
    let latest = update(&owner, 1)?;
    ensure!(
        latest.messages.len() == 100 && latest.history_older && !latest.history_browsing,
        "Latest history is not bounded"
    );
    ensure!(
        latest.messages.last().context("latest row")?.body == "History fixture 0400",
        "Delivery order changed"
    );
    owner.older_messages(route.clone())?;
    let older = update(&owner, 2)?;
    ensure!(
        older.messages.len() == 200 && older.history_browsing,
        "Older page did not expand the bounded window"
    );
    owner.older_messages(route.clone())?;
    ensure!(
        update(&owner, 3)?.messages.len() == 300,
        "History did not reach the window bound"
    );
    owner.older_messages(route.clone())?;
    let frozen = update(&owner, 4)?;
    ensure!(
        frozen.messages.len() == 300 && frozen.history_newer,
        "History did not prune the newer edge"
    );
    let ids: Vec<_> = frozen.messages.iter().map(|m| m.id.clone()).collect();
    runtime.block_on(async {
        let registry = AccountRegistry::open(&state, &migrations, Some(&ca)).await?;
        let mut member = registry
            .unlock(&member_id, "Test-only Member Vault 5682!")
            .await?;
        member
            .send(&route, "History arrived while reading older messages")
            .await?;
        member.shutdown().await;
        Ok::<_, anyhow::Error>(())
    })?;
    owner.synchronize()?;
    let refreshed = update(&owner, 5)?;
    ensure!(
        refreshed
            .messages
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>()
            == ids,
        "Synchronization moved the frozen history window"
    );
    ensure!(
        refreshed.history_newer,
        "Newer delivery was hidden from history navigation"
    );
    owner.older_messages("different-conversation".into())?;
    ensure!(
        snapshot(&owner, 6)?.error.contains("Conversation changed"),
        "Stale history command was accepted"
    );
    owner.latest_messages(route.clone())?;
    let newest = update(&owner, 7)?;
    ensure!(
        newest.messages.len() == 100 && !newest.history_browsing && !newest.history_newer,
        "Latest did not release the history cursor"
    );
    ensure!(
        newest.messages.last().context("newest row")?.body
            == "History arrived while reading older messages",
        "New content is absent"
    );
    owner.older_messages(route.clone())?;
    update(&owner, 8)?;
    owner.send_message(route.clone(), "Own message while browsing history".into())?;
    let sent = update(&owner, 9)?;
    ensure!(
        sent.messages.len() == 100
            && !sent.history_browsing
            && sent.messages.last().context("sent row")?.body
                == "Own message while browsing history",
        "Sending from older history did not return to the latest content"
    );
    owner.older_messages(route)?;
    update(&owner, 10)?;
    owner.lock_account(false)?;
    let locked = update(&owner, 11)?;
    ensure!(
        locked.messages.is_empty()
            && !locked.history_older
            && !locked.history_newer
            && !locked.history_browsing,
        "Lock retained history state"
    );
    drop(owner);
    let reopened = open()?;
    update(&reopened, 0)?;
    reopened.unlock_account(owner_id, "Test-only Role Vault 9321!".into())?;
    let restored = update(&reopened, 1)?;
    ensure!(
        restored.messages.len() == 100 && !restored.history_browsing,
        "Restart retained an old browsing cursor"
    );
    Ok(())
}
