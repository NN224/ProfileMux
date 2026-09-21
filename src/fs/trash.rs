use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Where deleted profile data goes. Abstracted so tests never touch the real
/// user Trash.
pub trait TrashBin {
    fn send(&self, path: &Path) -> Result<PathBuf>;
}

/// The macOS user Trash at `~/.Trash`.
pub struct SystemTrash;

impl TrashBin for SystemTrash {
    fn send(&self, path: &Path) -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| Error::Other("no home directory".into()))?;
        send_to_dir(&home.join(".Trash"), path)
    }
}

/// Moves `path` into `trash_dir`, choosing a non-colliding name. Falls back to
/// a copy-then-remove when the move crosses a filesystem boundary.
pub fn send_to_dir(trash_dir: &Path, path: &Path) -> Result<PathBuf> {
    unimplemented!("implemented by the trash task")
}
