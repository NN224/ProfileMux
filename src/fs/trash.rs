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

/// A fake trash bin for tests that directs files to a specific directory.
pub struct FakeTrash {
    pub dir: PathBuf,
}

impl FakeTrash {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

impl TrashBin for FakeTrash {
    fn send(&self, path: &Path) -> Result<PathBuf> {
        send_to_dir(&self.dir, path)
    }
}

/// Moves `path` into `trash_dir`, choosing a non-colliding name. Falls back to
/// a copy-then-remove when the move crosses a filesystem boundary.
pub fn send_to_dir(trash_dir: &Path, path: &Path) -> Result<PathBuf> {
    match path.symlink_metadata() {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::NotFound(path.to_path_buf()));
        }
        Err(e) => return Err(Error::io(path, e)),
    }

    std::fs::create_dir_all(trash_dir).map_err(|e| Error::io(trash_dir, e))?;

    let file_name = path.file_name().ok_or_else(|| {
        Error::Other(format!("invalid path has no file name: {}", path.display()))
    })?;
    let dest = pick_trash_destination(trash_dir, file_name);

    match std::fs::rename(path, &dest) {
        Ok(()) => Ok(dest),
        Err(err) if is_cross_device(&err) => {
            cross_device_move(path, &dest)?;
            Ok(dest)
        }
        Err(err) => Err(Error::io(path, err)),
    }
}

fn pick_trash_destination(trash_dir: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let base = Path::new(file_name);
    let initial = trash_dir.join(base);
    if initial.symlink_metadata().is_err() {
        return initial;
    }
    let stem = base.file_stem().unwrap_or(file_name).to_string_lossy();
    let ext = base.extension().map(|e| e.to_string_lossy());
    let mut counter = 2;
    loop {
        let name = match &ext {
            Some(ext) => format!("{stem} {counter}.{ext}"),
            None => format!("{stem} {counter}"),
        };
        let candidate = trash_dir.join(name);
        if candidate.symlink_metadata().is_err() {
            return candidate;
        }
        counter += 1;
    }
}

fn is_cross_device(err: &std::io::Error) -> bool {
    // EXDEV. `ErrorKind::CrossesDevices` is only stable since 1.85, and this
    // crate targets 1.80.
    err.raw_os_error() == Some(18)
}

fn cross_device_move(source: &Path, dest: &Path) -> Result<()> {
    let meta = source
        .symlink_metadata()
        .map_err(|e| Error::io(source, e))?;
    if meta.is_dir() {
        if let Err(e) = copy_dir_recursive(source, dest) {
            let _ = std::fs::remove_dir_all(dest);
            return Err(e);
        }
        std::fs::remove_dir_all(source).map_err(|e| Error::io(source, e))?;
    } else {
        if let Err(e) = std::fs::copy(source, dest) {
            let _ = std::fs::remove_file(dest);
            return Err(Error::io(source, e));
        }
        std::fs::remove_file(source).map_err(|e| Error::io(source, e))?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| Error::io(dst, e))?;
    for entry in std::fs::read_dir(src).map_err(|e| Error::io(src, e))? {
        let entry = entry.map_err(|e| Error::io(src, e))?;
        let ft = entry.file_type().map_err(|e| Error::io(entry.path(), e))?;
        if ft.is_symlink() {
            continue;
        }
        let child_dst = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_recursive(&entry.path(), &child_dst)?;
        } else if ft.is_file() {
            std::fs::copy(entry.path(), &child_dst).map_err(|e| Error::io(entry.path(), e))?;
        }
    }
    Ok(())
}
