//! Explicit local-vault startup. Passwords never enter argv, environment or IPC.
use anyhow::{Context as _, Result, bail, ensure};
use std::{ffi::OsString, io::BufRead as _};
use zeroize::Zeroizing;

pub(crate) const HELP: &str = "Cords desktop client
  Cords.exe                              Open the account picker
  Cords.exe --list-accounts               Print picker-safe local account IDs as JSON
  Cords.exe --account <ID> --password-stdin
                                         Unlock an existing account and open the UI

--password-stdin reads the first UTF-8 line (up to 4096 bytes) from redirected
standard input. LF/CRLF is removed; spaces are preserved. Never put a password in
arguments, shell history or a script. No password is saved or forwarded to the UI.
Existing CORDS_CLIENT_STATE/MIGRATIONS/CA path overrides remain supported.";

#[derive(PartialEq, Eq)]
pub(crate) enum Launch {
    Picker,
    Help,
    List,
    Unlock { account_id: String },
}

pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Launch> {
    let mut arguments = arguments.into_iter();
    let Some(first) = arguments.next() else {
        return Ok(Launch::Picker);
    };
    let first = first.to_str().context("Startup arguments must be UTF-8")?;
    if matches!(first, "--help" | "-h" | "--list-accounts") {
        ensure!(
            arguments.next().is_none(),
            "Unexpected extra startup arguments"
        );
        return Ok(if first == "--list-accounts" {
            Launch::List
        } else {
            Launch::Help
        });
    }
    // Accept either flag order, but require both and never echo unknown values.
    let mut account = None;
    let mut password_stdin = false;
    let mut current = Some(OsString::from(first));
    while let Some(argument) = current.take().or_else(|| arguments.next()) {
        match argument.to_str() {
            Some("--account") if account.is_none() => {
                let value = arguments
                    .next()
                    .context("--account requires an account ID")?;
                let value = value.to_str().context("Account ID must be UTF-8")?;
                ensure!(
                    value.len() == 43
                        && value
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'),
                    "Use the exact account ID from --list-accounts"
                );
                account = Some(value.to_owned());
            }
            Some("--password-stdin") if !password_stdin => password_stdin = true,
            _ => bail!(
                "Unknown or duplicate startup option; use --help. Password arguments are unsupported"
            ),
        }
    }
    ensure!(password_stdin, "--account requires --password-stdin");
    Ok(Launch::Unlock {
        account_id: account.context("--password-stdin requires --account")?,
    })
}

pub(crate) fn read_password(input: impl std::io::BufRead) -> Result<Zeroizing<String>> {
    let mut bytes = Zeroizing::new(Vec::new());
    input
        .take(4098)
        .read_until(b'\n', &mut bytes)
        .context("Could not read password from standard input")?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
    }
    ensure!(
        !bytes.is_empty() && bytes.len() <= 4096,
        "Password stdin line must contain 1–4096 UTF-8 bytes"
    );
    let password = std::str::from_utf8(&bytes).context("Password stdin line must be UTF-8")?;
    ensure!(
        !password.contains('\0'),
        "Password stdin line contains a NUL byte"
    );
    Ok(Zeroizing::new(password.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Result<Launch> {
        parse(values.iter().map(OsString::from))
    }
    #[test]
    fn startup_options_are_explicit_and_do_not_accept_password_arguments() -> Result<()> {
        assert!(matches!(args(&[])?, Launch::Picker));
        assert!(matches!(args(&["--list-accounts"])?, Launch::List));
        let id = "A".repeat(43);
        for values in [
            ["--account", &id, "--password-stdin"],
            ["--password-stdin", "--account", &id],
        ] {
            assert!(matches!(args(&values)?, Launch::Unlock { account_id } if account_id == id));
        }
        for values in [
            vec!["--account", &id],
            vec!["--password-stdin"],
            vec!["--account", "nickname", "--password-stdin"],
            vec!["--help", "extra"],
            vec!["--password", "private-value"],
            vec!["--account", &id, "--password-stdin", "--password-stdin"],
        ] {
            let error = args(&values)
                .err()
                .context("Invalid invocation accepted")?
                .to_string();
            assert!(!error.contains("private-value"));
        }
        Ok(())
    }
    #[test]
    fn password_input_is_bounded_preserves_spaces_and_strips_only_line_ending() -> Result<()> {
        assert_eq!(
            read_password(&b"  example secret  \r\nignored"[..])?.as_str(),
            "  example secret  "
        );
        assert_eq!(
            read_password(&b"secret without newline"[..])?.as_str(),
            "secret without newline"
        );
        assert!(read_password(&b"\n"[..]).is_err());
        assert!(read_password(&[0xff][..]).is_err());
        assert!(read_password(&b"bad\0secret\n"[..]).is_err());
        assert!(read_password(vec![b'x'; 4097].as_slice()).is_err());
        assert_eq!(read_password(vec![b'x'; 4096].as_slice())?.len(), 4096);
        Ok(())
    }
}
