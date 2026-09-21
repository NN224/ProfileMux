use std::path::{Path, PathBuf};

use crate::error::Result;

/// How the running binary was installed, which decides whether replacing it is
/// appropriate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallKind {
    /// A build artifact under `target/debug` or `target/release`.
    Development,
    /// Installed by `cargo install`, typically under `~/.cargo/bin`.
    CargoBin,
    /// A standalone binary downloaded from a release.
    Standalone,
}

/// Classifies an executable path without touching it.
pub fn classify_install(exe: &Path) -> InstallKind {
    unimplemented!("implemented by the updater task")
}

/// Atomically replaces `target` with `new_bytes`, preserving the old binary
/// until the replacement is in place so a failure can roll back.
pub fn install_binary(new_bytes: &[u8], target: &Path) -> Result<PathBuf> {
    unimplemented!("implemented by the updater task")
}
