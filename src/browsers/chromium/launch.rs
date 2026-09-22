use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::domain::{BrowserInstall, WebDarkMode};
use crate::error::{Error, Result};
use crate::policy::LaunchPolicy;

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

/// Builds the launch arguments for Chromium.
///
/// Force dark is launch-time only, meaning it does not require the browser to
/// be closed to change. It applies from the next launch ProfileMux performs; a
/// browser window the user opened themselves will not have it.
pub fn launch_args(
    user_data_root: &Path,
    profile_directory: &str,
    policy: &LaunchPolicy,
) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", user_data_root.display()),
        format!("--profile-directory={profile_directory}"),
    ];
    if policy.web_dark == WebDarkMode::ForceDark {
        args.push("--enable-features=WebContentsForceDark".to_string());
    }
    args
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
    cmd.args(launch_args(
        user_data_root,
        profile_directory,
        &LaunchPolicy::default(),
    ));
    cmd
}

/// Resolves the browser executable, builds the launch command with the given policy, detaches stdio, and spawns it.
///
/// Returns immediately without waiting for the process to exit.
pub fn launch_with_policy(
    install: &BrowserInstall,
    profile_directory: &str,
    policy: &LaunchPolicy,
) -> Result<()> {
    let executable = executable_path(install)?;
    let mut cmd = Command::new(&executable);
    cmd.args(launch_args(
        &install.user_data_root,
        profile_directory,
        policy,
    ));
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.spawn().map_err(|err| Error::io(executable, err))?;
    Ok(())
}

/// Resolves the browser executable, builds the launch command, detaches stdio, and spawns it.
///
/// Returns immediately without waiting for the process to exit.
pub fn launch(install: &BrowserInstall, profile_directory: &str) -> Result<()> {
    launch_with_policy(install, profile_directory, &LaunchPolicy::default())
}

/// True when a process for `exec_path` is running against `user_data_root`.
///
/// A Chromium instance started normally carries no `--user-data-dir` argument
/// and uses its default root, so matching on the executable alone would report
/// an unrelated root as busy. Matching the argument keeps the check per-root,
/// which is what every preflight actually needs.
fn process_matches_root(exec_path: &Path, user_data_root: &Path) -> bool {
    let (Some(exec_str), Some(root_str)) = (exec_path.to_str(), user_data_root.to_str()) else {
        return false;
    };
    let Ok(output) = Command::new("/bin/ps").args(["-Ao", "args="]).output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let needle = format!("--user-data-dir={root_str}");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.trim_start().starts_with(exec_str))
        .any(|line| line.contains(&needle))
}

/// True when the browser holds `user_data_root` open right now.
///
/// Chromium writes `SingletonLock` as a symlink to `<hostname>-<pid>` and
/// removes it on a clean exit. After a crash or a kill the link survives, so
/// the pid it names is checked for liveness; a stale lock must not block
/// mutation forever.
pub fn is_running(install: &BrowserInstall) -> bool {
    if singleton_lock_is_live(&install.user_data_root) {
        return true;
    }
    match executable_path(install) {
        Ok(exec_path) => process_matches_root(&exec_path, &install.user_data_root),
        Err(_) => false,
    }
}

/// Whether `SingletonLock` exists and names a process that is still alive.
pub fn singleton_lock_is_live(user_data_root: &Path) -> bool {
    let lock_path = user_data_root.join("SingletonLock");
    if std::fs::symlink_metadata(&lock_path).is_err() {
        return false;
    }
    let Ok(target) = std::fs::read_link(&lock_path) else {
        // Not a symlink: treat its presence as conservative evidence of a
        // running browser, since the pid cannot be checked.
        return true;
    };
    let Some(pid) = target
        .to_str()
        .and_then(|t| t.rsplit('-').next())
        .and_then(|p| p.parse::<u32>().ok())
    else {
        return true;
    };
    pid_is_alive(pid)
}

fn pid_is_alive(pid: u32) -> bool {
    Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "pid="])
        .output()
        .map(|out| out.status.success() && !out.stdout.is_empty())
        .unwrap_or(false)
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
