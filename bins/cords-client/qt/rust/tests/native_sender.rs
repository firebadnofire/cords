//! Manual native-platform acceptance against synthetic localhost accounts only.
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
                    "Native sender fixture: {}",
                    view.error
                );
                return Ok(view);
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    bail!("Native sender fixture timed out")
}

#[test]
#[ignore = "manual native OS notification acceptance: requires explicit prepare/send mode and disposable fixture"]
fn native_notification_fixture() -> Result<()> {
    let root = PathBuf::from(std::env::var("CORDS_QT_ACCEPTANCE_FIXTURE")?);
    let mode = std::env::var("CORDS_QT_NOTIFICATION_MODE")?;
    ensure!(
        mode == "prepare" || mode == "send",
        "Use explicit prepare or send mode"
    );
    let ready: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("ready.json"))?)?;
    let migrations =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../migrations/sqlite");
    let desktop = start_desktop(
        ready["state"].as_str().context("fixture state")?.into(),
        migrations.to_string_lossy().into(),
        ready["ca"].as_str().context("fixture CA")?.into(),
    )?;
    let picker = update(&desktop, 0)?;
    let preparing = mode == "prepare";
    let label = if preparing {
        "Member acceptance"
    } else {
        "Role acceptance"
    };
    let account = picker
        .accounts
        .iter()
        .find(|entry| entry.name == label)
        .context("fixture account")?
        .id
        .clone();
    // Public test-only credentials defined by desktop_role_acceptance_fixture.
    let password = if preparing {
        "Test-only Member Vault 5682!"
    } else {
        "Test-only Role Vault 9321!"
    };
    desktop.unlock_account(account, password.into())?;
    let view = update(&desktop, 1)?;
    let route = view
        .channels
        .iter()
        .find(|channel| channel.name == "Role acceptance")
        .context("fixture channel")?
        .id
        .clone();
    desktop.select_channel(route.clone())?;
    let view = update(&desktop, 2)?;
    if preparing {
        desktop.mute_route(route, false)?;
        update(&desktop, 3)?;
        desktop.mute_server(false)?;
        update(&desktop, 4)?;
        let mut preferences = view.preferences;
        preferences.notifications = true;
        preferences.notification_previews = false;
        preferences.do_not_disturb = false;
        desktop.save_preferences(preferences)?;
        update(&desktop, 5)?;
        println!("Synthetic member prepared for generic native notification acceptance.");
    } else {
        let body = format!("Native platform delivery {}", uuid::Uuid::now_v7());
        desktop.send_message(route, body.clone())?;
        ensure!(
            update(&desktop, 3)?
                .messages
                .iter()
                .any(|message| message.body == body),
            "Message was not accepted"
        );
        println!("Synthetic new message delivered through TLS for native notification inspection.");
    }
    Ok(())
}
