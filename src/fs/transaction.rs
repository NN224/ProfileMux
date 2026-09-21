use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Error, Result};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A compact, rollback-capable sequence of filesystem writes.
///
/// Scope is deliberately limited to the mutations ProfileMux performs: creating
/// directories, writing and backing up small metadata files, copying selected
/// files and directories, renaming, and sending data to the Trash.
#[derive(Debug)]
pub struct Transaction {
    backup_root: PathBuf,
    undo: Vec<UndoStep>,
    finished: bool,
    backup_counter: u64,
}

/// One recorded inverse action, applied in reverse order on rollback.
#[derive(Debug)]
enum UndoStep {
    RemovePath(PathBuf),
    RestoreFile { original: PathBuf, backup: PathBuf },
    RenameBack { from: PathBuf, to: PathBuf },
}

impl UndoStep {
    fn apply(&self) -> Result<()> {
        match self {
            UndoStep::RemovePath(path) => {
                if path.is_dir() {
                    std::fs::remove_dir_all(path).map_err(|e| Error::io(path, e))?;
                } else if path.exists() || path.symlink_metadata().is_ok() {
                    std::fs::remove_file(path).map_err(|e| Error::io(path, e))?;
                }
                Ok(())
            }
            UndoStep::RestoreFile { original, backup } => {
                std::fs::copy(backup, original).map_err(|e| Error::io(original, e))?;
                Ok(())
            }
            UndoStep::RenameBack { from, to } => {
                std::fs::rename(from, to).map_err(|e| Error::io(from, e))?;
                Ok(())
            }
        }
    }
}

impl Transaction {
    pub fn new(label: &str) -> Result<Self> {
        Self::with_backup_root(label, &std::env::temp_dir())
    }

    pub fn with_backup_root(label: &str, backup_root: &Path) -> Result<Self> {
        std::fs::create_dir_all(backup_root).map_err(|e| Error::io(backup_root, e))?;
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let safe_label: String = label
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        for _ in 0..1000 {
            let count = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir_name = format!("pmux-{safe_label}-{pid}-{nanos}-{count}");
            let backup_dir = backup_root.join(dir_name);
            match std::fs::create_dir(&backup_dir) {
                Ok(()) => {
                    return Ok(Self {
                        backup_root: backup_dir,
                        undo: Vec::new(),
                        finished: false,
                        backup_counter: 0,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Error::io(backup_dir, e)),
            }
        }
        Err(Error::Other(
            "failed to create unique backup directory".to_string(),
        ))
    }

    pub fn create_dir(&mut self, path: &Path) -> Result<()> {
        if path.is_file() {
            return Err(Error::Other(format!(
                "path already exists as a file: {}",
                path.display()
            )));
        }
        let mut to_create = Vec::new();
        let mut curr: Option<&Path> = Some(path);
        while let Some(p) = curr {
            if p.as_os_str().is_empty() || p.exists() {
                break;
            }
            to_create.push(p.to_path_buf());
            curr = p.parent();
        }
        to_create.reverse();
        for dir in to_create {
            std::fs::create_dir(&dir).map_err(|e| Error::io(&dir, e))?;
            self.undo.push(UndoStep::RemovePath(dir));
        }
        Ok(())
    }

    pub fn write_file(&mut self, path: &Path, bytes: &[u8]) -> Result<()> {
        if path.exists() {
            let backup = self
                .backup_root
                .join(format!("backup_{}", self.backup_counter));
            self.backup_counter += 1;
            std::fs::copy(path, &backup).map_err(|e| Error::io(path, e))?;
            self.undo.push(UndoStep::RestoreFile {
                original: path.to_path_buf(),
                backup,
            });
        } else {
            self.undo.push(UndoStep::RemovePath(path.to_path_buf()));
        }
        std::fs::write(path, bytes).map_err(|e| Error::io(path, e))?;
        Ok(())
    }

    pub fn copy_file(&mut self, from: &Path, to: &Path) -> Result<()> {
        if !from.exists() {
            return Err(Error::NotFound(from.to_path_buf()));
        }
        if to.exists() || to.symlink_metadata().is_ok() {
            return Err(Error::Other(format!(
                "destination already exists: {}",
                to.display()
            )));
        }
        std::fs::copy(from, to).map_err(|e| Error::io(from, e))?;
        self.undo.push(UndoStep::RemovePath(to.to_path_buf()));
        Ok(())
    }

    pub fn copy_dir(&mut self, from: &Path, to: &Path) -> Result<()> {
        if !from.exists() {
            return Err(Error::NotFound(from.to_path_buf()));
        }
        if to.exists() || to.symlink_metadata().is_ok() {
            return Err(Error::Other(format!(
                "destination already exists: {}",
                to.display()
            )));
        }
        if let Err(e) = copy_dir_recursive(from, to) {
            let _ = std::fs::remove_dir_all(to);
            return Err(e);
        }
        self.undo.push(UndoStep::RemovePath(to.to_path_buf()));
        Ok(())
    }

    pub fn backup_file(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(Error::NotFound(path.to_path_buf()));
        }
        let backup = self
            .backup_root
            .join(format!("backup_{}", self.backup_counter));
        self.backup_counter += 1;
        std::fs::copy(path, &backup).map_err(|e| Error::io(path, e))?;
        self.undo.push(UndoStep::RestoreFile {
            original: path.to_path_buf(),
            backup,
        });
        Ok(())
    }

    pub fn rename(&mut self, from: &Path, to: &Path) -> Result<()> {
        if !from.exists() {
            return Err(Error::NotFound(from.to_path_buf()));
        }
        if to.exists() || to.symlink_metadata().is_ok() {
            return Err(Error::Other(format!(
                "destination already exists: {}",
                to.display()
            )));
        }
        std::fs::rename(from, to).map_err(|e| Error::io(from, e))?;
        self.undo.push(UndoStep::RenameBack {
            from: to.to_path_buf(),
            to: from.to_path_buf(),
        });
        Ok(())
    }

    pub fn rollback(mut self) -> Result<()> {
        self.finished = true;
        self.apply_rollback()
    }

    pub fn commit(mut self) -> Result<()> {
        self.finished = true;
        if self.backup_root.exists() {
            std::fs::remove_dir_all(&self.backup_root)
                .map_err(|e| Error::io(&self.backup_root, e))?;
        }
        Ok(())
    }

    fn apply_rollback(&mut self) -> Result<()> {
        let mut first_error = None;
        while let Some(step) = self.undo.pop() {
            if let Err(e) = step.apply() {
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
        if self.backup_root.exists() {
            if let Err(e) = std::fs::remove_dir_all(&self.backup_root) {
                if first_error.is_none() {
                    first_error = Some(Error::io(&self.backup_root, e));
                }
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.finished {
            self.finished = true;
            let _ = self.apply_rollback();
        }
    }
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).map_err(|e| Error::io(to, e))?;
    for entry in std::fs::read_dir(from).map_err(|e| Error::io(from, e))? {
        let entry = entry.map_err(|e| Error::io(from, e))?;
        let ft = entry.file_type().map_err(|e| Error::io(entry.path(), e))?;
        if ft.is_symlink() {
            continue;
        }
        let dest = to.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else if ft.is_file() {
            std::fs::copy(entry.path(), &dest).map_err(|e| Error::io(entry.path(), e))?;
        }
    }
    Ok(())
}
