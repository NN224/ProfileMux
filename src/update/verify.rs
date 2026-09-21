use crate::error::Result;

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    unimplemented!("implemented by the updater task")
}

/// Fails unless `bytes` hashes to `expected`. Tolerates a `<hash>  <filename>`
/// line as produced by `shasum -a 256`.
pub fn verify_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    unimplemented!("implemented by the updater task")
}
