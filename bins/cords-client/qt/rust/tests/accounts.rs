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
