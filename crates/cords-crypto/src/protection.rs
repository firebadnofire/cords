//! Local encryption never uses MLS epoch or message secrets.
use chacha20poly1305::{
    KeyInit as _, XChaCha20Poly1305, XNonce,
    aead::{Aead as _, Payload},
};
use rand_core::{OsRng, RngCore as _};
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
#[error("local protected data could not be authenticated or unlocked")]
pub struct ProtectionError;

#[must_use]
pub fn random_key() -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0; 32]);
    OsRng.fill_bytes(key.as_mut());
    key
}

/// # Errors
/// Returns an error if key derivation or authenticated encryption/decryption fails.
pub fn derive_wrapping_key(
    passphrase: &[u8],
    salt: &[u8],
) -> Result<Zeroizing<[u8; 32]>, ProtectionError> {
    let mut key = Zeroizing::new([0; 32]);
    let params = argon2::Params::new(65_536, 3, 1, Some(32)).map_err(|_| ProtectionError)?;
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
        .hash_password_into(passphrase, salt, key.as_mut())
        .map_err(|_| ProtectionError)?;
    Ok(key)
}

/// # Errors
/// Returns an error if key derivation or authenticated encryption/decryption fails.
pub fn seal(key: &[u8; 32], context: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, ProtectionError> {
    let mut nonce = [0; 24];
    OsRng.fill_bytes(&mut nonce);
    let mut output = vec![1];
    output.extend(nonce);
    output.extend(
        XChaCha20Poly1305::new(key.into())
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: context,
                },
            )
            .map_err(|_| ProtectionError)?,
    );
    Ok(output)
}

/// # Errors
/// Returns an error if key derivation or authenticated encryption/decryption fails.
pub fn open(
    key: &[u8; 32],
    context: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, ProtectionError> {
    if ciphertext.len() < 41 || ciphertext[0] != 1 {
        return Err(ProtectionError);
    }
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            XNonce::from_slice(&ciphertext[1..25]),
            Payload {
                msg: &ciphertext[25..],
                aad: context,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| ProtectionError)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_cache_context_and_tamper() -> Result<(), ProtectionError> {
        let key = random_key();
        let a = seal(&key, b"installation/cache/message-a/v1", b"private history")?;
        assert_eq!(
            &**open(&key, b"installation/cache/message-a/v1", &a)?,
            b"private history"
        );
        assert!(open(&key, b"installation/cache/message-b/v1", &a).is_err());
        assert!(open(&random_key(), b"installation/cache/message-a/v1", &a).is_err());
        let mut damaged = a;
        damaged[25] ^= 1;
        assert!(open(&key, b"installation/cache/message-a/v1", &damaged).is_err());
        Ok(())
    }
}
