use pebble_core::{PebbleError, Result};
use serde::{Deserialize, Serialize};

/// Default object prefix for BYO S3-compatible vaults.
pub const DEFAULT_OBJECT_PREFIX: &str = "pebble";
pub const VAULT_OBJECT_NAME: &str = "vault.json";
pub const VAULT_META_OBJECT_NAME: &str = "vault.json.meta";

/// Lightweight revision record stored next to the encrypted vault blob.
/// Contains no account secrets — the vault payload itself is always ciphertext.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultMeta {
    pub revision: u64,
    pub checksum: String,
    pub device_id: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultConflict {
    pub local: VaultMeta,
    pub cloud: VaultMeta,
}

pub fn normalize_object_prefix(prefix: &str) -> Result<String> {
    let trimmed = prefix.trim().trim_matches('/');
    let value = if trimmed.is_empty() {
        DEFAULT_OBJECT_PREFIX
    } else {
        trimmed
    };
    if value.is_empty()
        || value.contains("..")
        || value.contains('\\')
        || value.contains('\0')
        || value.starts_with('/')
    {
        return Err(PebbleError::Validation(
            "Object prefix must be a relative path without '..'".to_string(),
        ));
    }
    if value.len() > 128 {
        return Err(PebbleError::Validation(
            "Object prefix is too long (max 128 characters)".to_string(),
        ));
    }
    Ok(value.to_string())
}

pub fn vault_object_key(prefix: &str) -> Result<String> {
    Ok(format!(
        "{}/{}",
        normalize_object_prefix(prefix)?,
        VAULT_OBJECT_NAME
    ))
}

pub fn vault_meta_object_key(prefix: &str) -> Result<String> {
    Ok(format!(
        "{}/{}",
        normalize_object_prefix(prefix)?,
        VAULT_META_OBJECT_NAME
    ))
}

pub fn sha256_hex(data: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, data);
    hex::encode(digest.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_prefix_and_object_keys() {
        assert_eq!(normalize_object_prefix("").unwrap(), "pebble");
        assert_eq!(normalize_object_prefix(" /pebble/ ").unwrap(), "pebble");
        assert_eq!(vault_object_key("").unwrap(), "pebble/vault.json");
        assert_eq!(
            vault_meta_object_key("pebble").unwrap(),
            "pebble/vault.json.meta"
        );
    }

    #[test]
    fn rejects_unsafe_prefixes() {
        assert!(normalize_object_prefix("../etc").is_err());
        assert!(normalize_object_prefix("foo\\bar").is_err());
    }

    #[test]
    fn vault_meta_round_trips() {
        let meta = VaultMeta {
            revision: 3,
            checksum: "abc".to_string(),
            device_id: "dev-1".to_string(),
            updated_at: 1_700_000_000,
        };
        let json = serde_json::to_vec(&meta).unwrap();
        let parsed: VaultMeta = serde_json::from_slice(&json).unwrap();
        assert_eq!(parsed, meta);
    }

    #[test]
    fn checksum_is_stable_sha256() {
        let expected = hex::encode(ring::digest::digest(&ring::digest::SHA256, b"pebble").as_ref());
        assert_eq!(sha256_hex(b"pebble"), expected);
        assert_eq!(expected.len(), 64);
        assert_ne!(sha256_hex(b"pebble"), sha256_hex(b"pebble-2"));
    }
}
