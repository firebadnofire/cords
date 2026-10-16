//! Bounded local startup input; credentials never cross the Qt object boundary.
use anyhow::{Context as _, Result, ensure};
use std::io::{BufRead as _, IsTerminal as _};
use zeroize::Zeroizing;

pub(super) fn password_stdin() -> Result<Zeroizing<String>> {
    let input = std::io::stdin();
    ensure!(
        !input.is_terminal(),
        "--password-stdin requires redirected standard input"
    );
    read_password(input.lock())
}

fn read_password(input: impl std::io::BufRead) -> Result<Zeroizing<String>> {
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
    let value = std::str::from_utf8(&bytes).context("Password stdin line must be UTF-8")?;
    ensure!(
        !value.contains('\0'),
        "Password stdin line contains a NUL byte"
    );
    Ok(Zeroizing::new(value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_bounded_and_preserved_exactly() -> Result<()> {
        assert_eq!(*read_password(&b"  secret  \r\nignored"[..])?, "  secret  ");
        for bad in [
            b"\n".to_vec(),
            vec![0xff],
            b"secret\0\n".to_vec(),
            vec![b'x'; 4097],
        ] {
            assert!(read_password(bad.as_slice()).is_err());
        }
        assert_eq!(read_password(vec![b'x'; 4096].as_slice())?.len(), 4096);
        Ok(())
    }
}
