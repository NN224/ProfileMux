use std::path::{Path, PathBuf};

use crate::error::Result;

/// A compact, rollback-capable sequence of filesystem writes.
///
/// Scope is deliberately limited to the mutations ProfileMux performs: creating
/// directories, writing and backing up small metadata files, copying selected
/// files and directories, renaming, and sending data to the Trash.
pub struct Transaction {
    label: String,
    backup_root: PathBuf,
    undo: Vec<UndoStep>,
    finished: bool,
}

/// One recorded inverse action, applied in reverse order on rollback.
enum UndoStep {
    RemovePath(PathBuf),
    RestoreFile { original: PathBuf, backup: PathBuf },
    RenameBack { from: PathBuf, to: PathBuf },
}

impl Transaction {
    pub fn new(label: &str) -> Result<Self> {
        unimplemented!("implemented by the transaction task")
    }

    pub fn create_dir(&mut self, path: &Path) -> Result<()> {
        unimplemented!()
    }

    pub fn write_file(&mut self, path: &Path, bytes: &[u8]) -> Result<()> {
        unimplemented!()
    }

    pub fn copy_file(&mut self, from: &Path, to: &Path) -> Result<()> {
        unimplemented!()
    }

    pub fn copy_dir(&mut self, from: &Path, to: &Path) -> Result<()> {
        unimplemented!()
    }

    pub fn backup_file(&mut self, path: &Path) -> Result<()> {
        unimplemented!()
    }

    pub fn rename(&mut self, from: &Path, to: &Path) -> Result<()> {
        unimplemented!()
    }

    pub fn rollback(self) -> Result<()> {
        unimplemented!()
    }

    pub fn commit(self) -> Result<()> {
        unimplemented!()
    }
}
