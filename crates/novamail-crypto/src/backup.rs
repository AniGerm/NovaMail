//! Passphrase-encrypted configuration backups (Phase 1 org sync).
//!
//! Format (JSON envelope, camelCase):
//! ```text
//! {
//!   "format": "novamail-backup-v1",
//!   "kdf": "argon2id",
//!   "saltBase64": "...",
//!   "nonceBase64": "...",
//!   "ciphertextBase64": "..."   // AES-256-GCM over UTF-8 JSON payload
//! }
//! ```

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::{CryptoError, CryptoResult};

pub const BACKUP_FORMAT: &str = "novamail-backup-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedBackupFile {
    pub format: String,
    pub kdf: String,
    pub salt_base64: String,
    pub nonce_base64: String,
    pub ciphertext_base64: String,
}

pub fn encrypt_backup_payload(passphrase: &str, plaintext_json: &[u8]) -> CryptoResult<EncryptedBackupFile> {
    if passphrase.trim().len() < 8 {
        return Err(CryptoError::Backup(
            "passphrase must be at least 8 characters".into(),
        ));
    }

    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| CryptoError::Backup(e.to_string()))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext_json)
        .map_err(|_| CryptoError::Backup("encryption failed".into()))?;

    Ok(EncryptedBackupFile {
        format: BACKUP_FORMAT.into(),
        kdf: "argon2id".into(),
        salt_base64: B64.encode(salt),
        nonce_base64: B64.encode(nonce_bytes),
        ciphertext_base64: B64.encode(ciphertext),
    })
}

pub fn decrypt_backup_payload(
    passphrase: &str,
    file: &EncryptedBackupFile,
) -> CryptoResult<Vec<u8>> {
    if file.format != BACKUP_FORMAT {
        return Err(CryptoError::Backup(format!(
            "unsupported backup format: {}",
            file.format
        )));
    }
    let salt = B64
        .decode(file.salt_base64.as_bytes())
        .map_err(|_| CryptoError::Backup("invalid salt".into()))?;
    let nonce_bytes = B64
        .decode(file.nonce_base64.as_bytes())
        .map_err(|_| CryptoError::Backup("invalid nonce".into()))?;
    let ciphertext = B64
        .decode(file.ciphertext_base64.as_bytes())
        .map_err(|_| CryptoError::Backup("invalid ciphertext".into()))?;
    if nonce_bytes.len() != 12 {
        return Err(CryptoError::Backup("invalid nonce length".into()));
    }

    let key = derive_key(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| CryptoError::Backup(e.to_string()))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| CryptoError::BadPassphrase)
}

fn derive_key(passphrase: &str, salt: &[u8]) -> CryptoResult<[u8; 32]> {
    // Moderate desktop defaults: ~64 MiB memory, 3 iterations.
    let params = Params::new(65_536, 3, 1, Some(32))
        .map_err(|e| CryptoError::Backup(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| CryptoError::Backup(e.to_string()))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_backup() {
        let payload = br#"{"hello":"world"}"#;
        let file = encrypt_backup_payload("correct horse", payload).unwrap();
        let plain = decrypt_backup_payload("correct horse", &file).unwrap();
        assert_eq!(plain, payload);
        assert!(decrypt_backup_payload("wrong pass", &file).is_err());
    }
}
