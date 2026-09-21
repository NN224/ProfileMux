use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{Error, Result};

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

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn make_temp_paths(dir: &Path, target: &Path) -> (PathBuf, PathBuf) {
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("pmux");
    let pid = std::process::id();
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let temp = dir.join(format!(".{file_name}.tmp.{pid}_{now}_{count}"));
    let backup = dir.join(format!(".{file_name}.old.{pid}_{now}_{count}"));
    (temp, backup)
}

fn check_dir_writable(dir: &Path, target: &Path) -> Result<()> {
    if let Ok(meta) = std::fs::metadata(dir) {
        if meta.permissions().readonly() {
            return Err(Error::Update(format!(
                "directory `{}` is not writable for executable `{}`",
                dir.display(),
                target.display()
            )));
        }
    }
    Ok(())
}

fn write_temp_binary(
    temp: &Path,
    new_bytes: &[u8],
    mode: u32,
    dir: &Path,
    target: &Path,
) -> Result<()> {
    if let Err(e) = std::fs::write(temp, new_bytes) {
        if temp.exists() {
            let _ = std::fs::remove_file(temp);
        }
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(Error::Update(format!(
                "directory `{}` is not writable for executable `{}`",
                dir.display(),
                target.display()
            )));
        }
        return Err(Error::Update(format!(
            "failed to write temporary file `{}`: {e}",
            temp.display()
        )));
    }

    if let Err(e) = std::fs::set_permissions(temp, std::fs::Permissions::from_mode(mode)) {
        let _ = std::fs::remove_file(temp);
        return Err(Error::Update(format!(
            "failed to set permissions on `{}`: {e}",
            temp.display()
        )));
    }
    Ok(())
}

fn backup_existing(target: &Path, backup: &Path, dir: &Path) -> Result<()> {
    if let Err(e) = std::fs::rename(target, backup) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(Error::Update(format!(
                "directory `{}` is not writable for executable `{}`",
                dir.display(),
                target.display()
            )));
        }
        return Err(Error::Update(format!(
            "failed to move `{}` aside to `{}`: {e}",
            target.display(),
            backup.display()
        )));
    }
    Ok(())
}

fn rollback(target: &Path, backup: &Path, temp: &Path, has_target: bool) {
    if has_target {
        if backup.exists() {
            let _ = std::fs::rename(backup, target);
        }
    } else if target.exists() {
        let _ = std::fs::remove_file(target);
    }
    if temp.exists() {
        let _ = std::fs::remove_file(temp);
    }
}

/// Classifies an executable path without touching it.
pub fn classify_install(exe: &Path) -> InstallKind {
    let parent = match exe.parent() {
        Some(p) => p,
        None => return InstallKind::Standalone,
    };

    let mut seen_target = false;
    for comp in parent.components() {
        let s = comp.as_os_str();
        if s == "target" {
            seen_target = true;
        } else if seen_target && (s == "debug" || s == "release") {
            return InstallKind::Development;
        }
    }

    let cargo_bin = Path::new(".cargo").join("bin");
    if parent.ends_with(&cargo_bin) {
        return InstallKind::CargoBin;
    }

    InstallKind::Standalone
}

/// Atomically replaces `target` with `new_bytes`, preserving the old binary
/// until the replacement is in place so a failure can roll back.
pub fn install_binary(new_bytes: &[u8], target: &Path) -> Result<PathBuf> {
    let dir = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    check_dir_writable(dir, target)?;

    let (temp_path, backup_path) = make_temp_paths(dir, target);
    let mode = target
        .metadata()
        .map(|m| m.permissions().mode())
        .unwrap_or(0o755);
    write_temp_binary(&temp_path, new_bytes, mode, dir, target)?;

    let has_target = target.exists();
    if has_target {
        if let Err(err) = backup_existing(target, &backup_path, dir) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(err);
        }
    }

    if let Err(e) = std::fs::rename(&temp_path, target) {
        rollback(target, &backup_path, &temp_path, has_target);
        return Err(Error::Update(format!(
            "failed to replace binary at `{}`: {e}",
            target.display()
        )));
    }

    let is_valid = std::fs::metadata(target)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false);
    if !is_valid {
        rollback(target, &backup_path, &temp_path, has_target);
        return Err(Error::Update(format!(
            "installed binary `{}` is missing or empty; restored backup",
            target.display()
        )));
    }

    if has_target && backup_path.exists() {
        let _ = std::fs::remove_file(&backup_path);
    }
    Ok(target.to_path_buf())
}
