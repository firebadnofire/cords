//! Real core/vault tests; no frontend mocks or Qt SDK required.
use anyhow::{Context as _, Result, bail};
use cords_qt_bridge::{Desktop, ffi::Snapshot, start_desktop};
use std::{
    thread,
    time::{Duration, Instant},
};

fn wait(desktop: &Desktop, predicate: impl Fn(&Snapshot) -> bool) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        for update in desktop.take_updates()? {
            if predicate(&update) {
                return Ok(update);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    bail!("Timed out waiting for native account state")
}
fn open(path: &std::path::Path) -> Result<Box<Desktop>> {
    let migrations =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../migrations/sqlite");
    start_desktop(
        path.to_string_lossy().into(),
        migrations.to_string_lossy().into(),
        String::new(),
    )
}
#[test]
fn appearance_preferences_validate_and_survive_sealed_restart() -> Result<()> {
    let state = tempfile::tempdir()?;
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    let password = "Mango Lantern verification orchard 73!";
    desktop.create_account("Appearance".into(), password.into(), false)?;
    let created = wait(&desktop, |view| view.completed == 1)?;
    assert!(created.error.is_empty(), "{}", created.error);
    assert!(created.unlocked);
    let account = created.account;
    let mut preferences = created.preferences;
    assert_eq!(preferences.text_scale_percent, 100);
    assert_eq!(preferences.server_icon_size, 48);
    preferences.text_scale_percent = 150;
    preferences.server_icon_size = 56;
    desktop.save_preferences(preferences.clone())?;
    let saved = wait(&desktop, |view| view.completed == 2)?;
    assert!(saved.error.is_empty(), "{}", saved.error);
    preferences.text_scale_percent = 0;
    desktop.save_preferences(preferences)?;
    let rejected = wait(&desktop, |view| view.completed == 3)?;
    assert!(rejected.error.contains("Text size"));
    assert_eq!(rejected.preferences.text_scale_percent, 150);
    drop(desktop);
    let reopened = open(state.path())?;
    wait(&reopened, |_| true)?;
    reopened.unlock_account(account, password.into())?;
    let restored = wait(&reopened, |view| view.completed == 1)?;
    assert!(restored.error.is_empty(), "{}", restored.error);
    assert_eq!(restored.preferences.text_scale_percent, 150);
    assert_eq!(restored.preferences.server_icon_size, 56);
    Ok(())
}
#[test]
fn existing_vault_restart_password_failure_and_lock_clear_plaintext() -> Result<()> {
    let state = tempfile::tempdir()?;
    let password = "a long native vault acceptance phrase 42";
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    desktop.create_account("Native acceptance".into(), password.into(), false)?;
    let created = wait(&desktop, |view| view.completed == 1)?;
    assert!(created.error.is_empty(), "{}", created.error);
    assert!(created.unlocked);
    let account = created.account.clone();
    let device = created.device.clone();
    desktop.lock_account(false)?;
    let locked = wait(&desktop, |view| view.completed == 2)?;
    assert!(!locked.unlocked);
    assert!(locked.account.is_empty() && locked.device.is_empty());
    assert!(locked.messages.is_empty() && locked.members.is_empty() && locked.servers.is_empty());
    drop(desktop);
    let desktop = open(state.path())?;
    let picker = wait(&desktop, |_| true)?;
    assert_eq!(
        picker
            .accounts
            .first()
            .context("missing registered account")?
            .id,
        account
    );
    desktop.unlock_account(account.clone(), "incorrect password".into())?;
    let failure = wait(&desktop, |view| view.completed == 1)?;
    assert!(!failure.unlocked && !failure.error.is_empty());
    desktop.unlock_account(account.clone(), password.into())?;
    let reopened = wait(&desktop, |view| view.completed == 2)?;
    assert!(reopened.error.is_empty(), "{}", reopened.error);
    assert_eq!(reopened.account, account);
    assert_eq!(reopened.device, device);
    // The runtime must join and release the original vault lock on destruction.
    drop(desktop);
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    desktop.unlock_account(account, password.into())?;
    assert!(wait(&desktop, |view| view.completed == 1)?.unlocked);
    Ok(())
}
#[test]
fn failed_registry_start_is_reported_and_queue_failure_has_no_secret() -> Result<()> {
    let state = tempfile::tempdir()?;
    let file = state.path().join("not-a-directory");
    std::fs::write(&file, b"preserve")?;
    let desktop = open(&file)?;
    let view = wait(&desktop, |_| true)?;
    assert!(view.error.contains("Cannot open account registry"));
    let error = desktop
        .unlock_account("account".into(), "secret-never-in-error".into())
        .err()
        .context("closed queue accepted work")?;
    assert!(!error.to_string().contains("secret-never-in-error"));
    assert_eq!(std::fs::read(&file)?, b"preserve");
    Ok(())
}

#[test]
fn authenticated_local_removal_preserves_the_other_vault_after_restart() -> Result<()> {
    let state = tempfile::tempdir()?;
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    let first_password = "Mango Lantern local removal first 57";
    let second_password = "Mango Lantern local removal second 82";
    desktop.create_account("Remove fixture".into(), first_password.into(), false)?;
    let first = wait(&desktop, |view| view.completed == 1)?;
    assert!(first.error.is_empty(), "{}", first.error);
    desktop.lock_account(false)?;
    wait(&desktop, |view| view.completed == 2)?;
    desktop.create_account("Preserve fixture".into(), second_password.into(), false)?;
    let second = wait(&desktop, |view| view.completed == 3)?;
    assert!(second.error.is_empty(), "{}", second.error);
    desktop.lock_account(false)?;
    wait(&desktop, |view| view.completed == 4)?;
    desktop.remove_account(first.account.clone(), "incorrect local password".into())?;
    let rejected = wait(&desktop, |view| view.completed == 5)?;
    assert!(!rejected.error.is_empty());
    assert_eq!(rejected.accounts.len(), 2);
    desktop.unlock_account(first.account.clone(), first_password.into())?;
    let verified = wait(&desktop, |view| view.completed == 6)?;
    assert!(verified.unlocked && verified.error.is_empty());
    assert_eq!(verified.device, first.device);
    desktop.lock_account(false)?;
    wait(&desktop, |view| view.completed == 7)?;
    desktop.remove_account(first.account.clone(), first_password.into())?;
    let removed = wait(&desktop, |view| view.completed == 8)?;
    assert!(removed.error.is_empty(), "{}", removed.error);
    assert!(!removed.unlocked && removed.messages.is_empty() && removed.servers.is_empty());
    assert_eq!(removed.accounts.len(), 1);
    assert_eq!(removed.accounts[0].id, second.account);
    drop(desktop);
    let desktop = open(state.path())?;
    let restarted = wait(&desktop, |_| true)?;
    assert_eq!(restarted.accounts.len(), 1);
    assert_eq!(restarted.accounts[0].id, second.account);
    desktop.unlock_account(second.account.clone(), second_password.into())?;
    let preserved = wait(&desktop, |view| view.completed == 1)?;
    assert!(preserved.error.is_empty(), "{}", preserved.error);
    assert_eq!(preserved.account, second.account);
    assert_eq!(preserved.device, second.device);
    Ok(())
}

#[test]
fn lock_interrupts_stalled_tls_and_releases_original_vault() -> Result<()> {
    use std::io::Read as _;
    let state = tempfile::tempdir()?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let (accepted, connection) = std::sync::mpsc::channel();
    let (release, held) = std::sync::mpsc::channel();
    let server = thread::spawn(move || -> std::io::Result<()> {
        let (mut socket, _) = listener.accept()?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut hello = [0; 4096];
        let size = socket.read(&mut hello)?;
        if size == 0 {
            return Err(std::io::Error::other("missing TLS ClientHello"));
        }
        accepted.send(()).map_err(std::io::Error::other)?;
        // Keep the peer connected without replying until locking is verified.
        held.recv_timeout(Duration::from_secs(5))
            .map_err(std::io::Error::other)?;
        drop(socket);
        Ok(())
    });
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    let password = "bounded TLS interruption 57 Mango Lantern";
    desktop.create_account("Priority lock".into(), password.into(), false)?;
    let created = wait(&desktop, |view| view.completed == 1)?;
    assert!(
        created.unlocked,
        "Account creation failed: {}",
        created.error
    );
    let account = created.account;
    desktop.trust_server(format!("https://{address}"))?;
    if let Err(error) = connection.recv_timeout(Duration::from_secs(5)) {
        let states = desktop.take_updates()?;
        bail!(
            "Stalled TLS fixture was not contacted: {error}; worker errors: {:?}",
            states.iter().map(|view| &view.error).collect::<Vec<_>>()
        );
    }
    let start = Instant::now();
    desktop.lock_account(false)?;
    let locked = wait(&desktop, |view| !view.unlocked && view.completed >= 2)?;
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "Lock waited for the network timeout"
    );
    assert!(locked.error.is_empty(), "{}", locked.error);
    assert!(locked.servers.is_empty() && locked.messages.is_empty());
    release.send(())?;
    server
        .join()
        .map_err(|_| anyhow::anyhow!("stalled TLS fixture panicked"))??;
    drop(desktop);
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    desktop.unlock_account(account, password.into())?;
    assert!(wait(&desktop, |view| view.completed == 1)?.unlocked);
    Ok(())
}

#[test]
fn platform_lock_obeys_verified_account_preferences() -> Result<()> {
    let state = tempfile::tempdir()?;
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    desktop.create_account(
        "Event policy".into(),
        "Mango Lantern bounded transport 57".into(),
        false,
    )?;
    let created = wait(&desktop, |view| view.completed == 1)?;
    assert!(created.unlocked, "{}", created.error);
    let mut preferences = created.preferences;
    preferences.lock_on_os_lock = false;
    preferences.lock_on_suspend = true;
    desktop.save_preferences(preferences)?;
    let saved = wait(&desktop, |view| view.completed == 2)?;
    assert!(saved.error.is_empty(), "{}", saved.error);
    assert!(!desktop.platform_lock(false)?, "OS-lock policy was ignored");
    assert!(desktop.platform_lock(true)?, "Suspend policy was ignored");
    let locked = wait(&desktop, |view| view.completed == 3)?;
    assert!(!locked.unlocked && locked.account.is_empty());
    Ok(())
}

#[test]
fn explicit_password_override_survives_restart_and_rewrap() -> Result<()> {
    for password in ["", "x", "password", "aaaaaaaaaaaa", "Native acceptance"] {
        let state = tempfile::tempdir()?;
        let desktop = open(state.path())?;
        wait(&desktop, |_| true)?;
        desktop.create_account("Native acceptance".into(), password.into(), false)?;
        let denied = wait(&desktop, |v| v.completed == 1)?;
        assert!(!denied.unlocked && !denied.error.is_empty());
        desktop.create_account("Native acceptance".into(), password.into(), true)?;
        let created = wait(&desktop, |v| v.completed == 2)?;
        assert!(created.error.is_empty(), "{}", created.error);
        let account = created.account;
        let device = created.device;
        desktop.change_password(account.clone(), "wrong".into(), "q".into(), true)?;
        assert!(!wait(&desktop, |v| v.completed == 3)?.error.is_empty());
        desktop.change_password(account.clone(), password.into(), "q".into(), false)?;
        assert!(!wait(&desktop, |v| v.completed == 4)?.error.is_empty());
        desktop.change_password(account.clone(), password.into(), "q".into(), true)?;
        assert!(wait(&desktop, |v| v.completed == 5)?.error.is_empty());
        drop(desktop);
        let desktop = open(state.path())?;
        wait(&desktop, |_| true)?;
        desktop.unlock_account(account.clone(), "q".into())?;
        let reopened = wait(&desktop, |v| v.completed == 1)?;
        assert!(reopened.error.is_empty(), "{}", reopened.error);
        assert_eq!(reopened.account, account);
        assert_eq!(reopened.device, device);
        desktop.change_password(account.clone(), "q".into(), "".into(), true)?;
        assert!(wait(&desktop, |v| v.completed == 2)?.error.is_empty());
        drop(desktop);
        let desktop = open(state.path())?;
        wait(&desktop, |_| true)?;
        desktop.unlock_account(account, "".into())?;
        assert!(wait(&desktop, |v| v.completed == 1)?.unlocked);
    }
    Ok(())
}

#[test]
fn native_avatar_edit_persists_signed_profile_and_removal() -> Result<()> {
    use base64::Engine as _;
    let state = tempfile::tempdir()?;
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    let password = "Local profile image acceptance phrase 73";
    desktop.create_account("Profile tester".into(), password.into(), false)?;
    let created = wait(&desktop, |v| v.completed == 1)?;
    assert!(created.error.is_empty(), "{}", created.error);
    let account = created.account;
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../crates/cords-avatar-codec/tests/fixtures/animated.gif.jxl");
    let original = cords_avatar_codec::validate(&std::fs::read(&source)?)?;
    desktop.prepare_avatar(source.to_string_lossy().into())?;
    let prepared = wait(&desktop, |v| v.completed == 2)?;
    assert!(prepared.error.is_empty(), "{}", prepared.error);
    assert!(!prepared.avatar_draft.id.is_empty());
    desktop.save_avatar(prepared.avatar_draft.id, 0.5, 0.5, 1.5, "square".into())?;
    let saved = wait(&desktop, |v| v.completed == 3)?;
    assert!(saved.error.is_empty(), "{}", saved.error);
    assert!(saved.avatar_draft.id.is_empty());
    assert_eq!(saved.profile.shape, "square");
    let encoded = saved
        .profile
        .image
        .strip_prefix("data:image/jxl;base64,")
        .context("missing JXL image")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    let output = cords_avatar_codec::validate(&bytes)?;
    assert_eq!(output.frames, original.frames);
    assert_eq!(output.loops, original.loops);
    for (before, after) in original.durations.iter().zip(&output.durations) {
        assert_eq!(
            u64::from(*before)
                * u64::from(original.ticks_per_second_denominator)
                * u64::from(output.ticks_per_second_numerator),
            u64::from(*after)
                * u64::from(output.ticks_per_second_denominator)
                * u64::from(original.ticks_per_second_numerator)
        );
    }
    drop(desktop);
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    desktop.unlock_account(account.clone(), password.into())?;
    wait(&desktop, |v| v.completed == 1)?;
    desktop.load_profile(account.clone())?;
    let reopened = wait(&desktop, |v| v.completed == 2)?;
    assert_eq!(reopened.profile.image, saved.profile.image);
    desktop.remove_avatar(reopened.profile.heads)?;
    let removed = wait(&desktop, |v| v.completed == 3)?;
    assert!(removed.error.is_empty(), "{}", removed.error);
    assert!(removed.profile.image.is_empty());
    assert!(!removed.profile.heads.is_empty());
    drop(desktop);
    let desktop = open(state.path())?;
    wait(&desktop, |_| true)?;
    desktop.unlock_account(account.clone(), password.into())?;
    wait(&desktop, |v| v.completed == 1)?;
    desktop.load_profile(account)?;
    let reopened = wait(&desktop, |v| v.completed == 2)?;
    assert!(reopened.profile.image.is_empty());
    assert_eq!(reopened.profile.heads, removed.profile.heads);
    let png = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/128x128.png");
    desktop.prepare_avatar(png.to_string_lossy().into())?;
    let prepared = wait(&desktop, |v| v.completed == 3)?;
    assert!(prepared.error.is_empty(), "{}", prepared.error);
    desktop.save_avatar(prepared.avatar_draft.id, 0.5, 0.5, 1.0, "circle".into())?;
    let png_saved = wait(&desktop, |v| v.completed == 4)?;
    assert!(png_saved.error.is_empty(), "{}", png_saved.error);
    assert!(
        png_saved
            .profile
            .image
            .starts_with("data:image/jxl;base64,")
    );
    Ok(())
}
