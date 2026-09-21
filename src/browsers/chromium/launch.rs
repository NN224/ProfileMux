use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::domain::BrowserInstall;
use crate::error::{Error, Result};

/// Resolves the real binary inside the application bundle.
///
/// Reads `CFBundleExecutable` from `<app_path>/Contents/Info.plist`. If absent,
/// falls back to the single entry inside `<app_path>/Contents/MacOS` when there
/// is exactly one. Returns `Error::NotFound` if resolution fails or the binary
/// does not exist on disk.
pub fn executable_path(install: &BrowserInstall) -> Result<PathBuf> {
    let contents_dir = install.app_path.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let plist_path = contents_dir.join("Info.plist");

    let cf_bundle_executable = plist::Value::from_file(&plist_path)
        .ok()
        .and_then(|val| {
            val.as_dictionary()
                .and_then(|d| d.get("CFBundleExecutable"))
                .and_then(|v| v.as_string())
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty());

    if let Some(exec_name) = cf_bundle_executable {
        let candidate = macos_dir.join(exec_name);
        if candidate.exists() {
            return Ok(candidate);
        }
        return Err(Error::NotFound(candidate));
    }

    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(&macos_dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            let is_hidden = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'));
            if !is_hidden {
                entries.push(path);
            }
        }
    }

    if entries.len() == 1 {
        let candidate = entries.remove(0);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(Error::NotFound(macos_dir))
}

/// Builds, without spawning, the command that launches a Chromium browser on a specific profile.
///
/// Arguments are constructed without shell-quoting so spaces or metacharacters in the profile
/// directory remain intact as a single argument value.
pub fn launch_command(
    executable: &Path,
    user_data_root: &Path,
    profile_directory: &str,
) -> Command {
    let mut cmd = Command::new(executable);
    cmd.arg(format!("--user-data-dir={}", user_data_root.display()));
    cmd.arg(format!("--profile-directory={profile_directory}"));
    cmd
}

/// Resolves the browser executable, builds the launch command, detaches stdio, and spawns it.
///
/// Returns immediately without waiting for the process to exit.
pub fn launch(install: &BrowserInstall, profile_directory: &str) -> Result<()> {
    let executable = executable_path(install)?;
    let mut cmd = launch_command(&executable, &install.user_data_root, profile_directory);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.spawn().map_err(|err| Error::io(executable, err))?;
    Ok(())
}

fn process_matches_executable(exec_path: &Path) -> bool {
    let Some(exec_str) = exec_path.to_str() else {
        return false;
    };
    let Ok(output) = Command::new("/bin/ps").args(["-Ao", "args="]).output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .any(|line| line.trim_start().starts_with(exec_str))
}

/// Checks whether this specific browser installation is currently running.
///
/// Returns true if either:
/// 1. `<user_data_root>/SingletonLock` exists (including dangling symlinks).
/// 2. A running process matches the resolved executable path in `/bin/ps -Ao args=`.
pub fn is_running(install: &BrowserInstall) -> bool {
    let lock_path = install.user_data_root.join("SingletonLock");
    let lock_exists = std::fs::symlink_metadata(&lock_path).is_ok();

    let process_running = match executable_path(install) {
        Ok(exec_path) => process_matches_executable(&exec_path),
        Err(_) => false,
    };

    lock_exists || process_running
}

/// Requests the browser application to quit gracefully via AppleScript (`osascript`).
///
/// Returns `Error::Other` if the installation has no `bundle_id`.
pub fn request_quit(install: &BrowserInstall) -> Result<()> {
    let Some(bundle_id) = &install.bundle_id else {
        return Err(Error::Other(
            "graceful quit requires a bundle identifier".to_string(),
        ));
    };

    let script = format!("tell application id \"{bundle_id}\" to quit");
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|err| Error::io(PathBuf::from("osascript"), err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let msg = if stderr.is_empty() {
            format!(
                "osascript quit command failed with status: {}",
                output.status
            )
        } else {
            format!("osascript quit command failed: {stderr}")
        };
        return Err(Error::Other(msg));
    }

    Ok(())
}

/// Polls `is_running` every 250 ms until the browser stops or timeout expires.
pub fn wait_until_stopped(install: &BrowserInstall, timeout: Duration) -> bool {
    let start = Instant::now();
    let poll_interval = Duration::from_millis(250);

    while is_running(install) {
        if start.elapsed() >= timeout {
            return false;
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        let sleep_duration = poll_interval.min(remaining);
        std::thread::sleep(sleep_duration);
    }

    true
}
