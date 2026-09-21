use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Fails unless `bytes` hashes to `expected`. Tolerates a `<hash>  <filename>`
/// line as produced by `shasum -a 256`.
pub fn verify_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let trimmed = expected.trim();
    let normalized = match trimmed.split_whitespace().next() {
        Some(token) => token,
        None => {
            return Err(Error::Update("checksum cannot be empty".to_string()));
        }
    };

    if normalized.len() != 64 || !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Update(format!(
            "invalid checksum `{normalized}`: expected 64 hex characters"
        )));
    }

    let computed = sha256_hex(bytes);
    if !computed.eq_ignore_ascii_case(normalized) {
        return Err(Error::Update(format!(
            "checksum mismatch: expected {normalized}, computed {computed}"
        )));
    }

    Ok(())
}
