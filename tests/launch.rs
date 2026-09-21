use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use profilemux::browsers::chromium::launch::{
    executable_path, is_running, launch_command, request_quit, wait_until_stopped,
};
use profilemux::domain::{BrowserInstall, BrowserInstallId, BrowserKind, Channel, SupportLevel};
use profilemux::error::Error;

fn dummy_install(app_path: PathBuf, user_data_root: PathBuf) -> BrowserInstall {
    let id = BrowserInstallId::new(BrowserKind::Chromium, &user_data_root);
    BrowserInstall {
        id,
        kind: BrowserKind::Chromium,
        name: "Test Browser".to_string(),
        channel: Channel::Stable,
        app_path,
        bundle_id: Some("org.chromium.Chromium".to_string()),
        version: Some("120.0.0.0".to_string()),
        user_data_root,
        cache_root: None,
        support: SupportLevel::Full,
    }
}

#[test]
fn test_launch_command_space_in_profile_directory() {
    let exec = Path::new("/Applications/Test Browser.app/Contents/MacOS/Test Browser");
    let user_data = Path::new("/tmp/test_user_data");
    let profile_dir = "Profile 1";

    let cmd = launch_command(exec, user_data, profile_dir);

    assert_eq!(cmd.get_program(), exec.as_os_str());

    let full_command_values: Vec<&OsStr> = std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .collect();
    assert_eq!(full_command_values.len(), 3);
    assert_eq!(full_command_values[0], exec.as_os_str());
    assert_eq!(
        full_command_values[1],
        OsStr::new(&format!("--user-data-dir={}", user_data.display()))
    );
    assert_eq!(
        full_command_values[2],
        OsStr::new("--profile-directory=Profile 1")
    );

    let args: Vec<&OsStr> = cmd.get_args().collect();
    assert_eq!(args.len(), 2);
    assert_eq!(
        args[0],
        OsStr::new(&format!("--user-data-dir={}", user_data.display()))
    );
    assert_eq!(args[1], OsStr::new("--profile-directory=Profile 1"));
}

#[test]
fn test_launch_command_shell_metacharacters() {
    let exec = Path::new("/custom/bin/browser");
    let user_data = Path::new("/tmp/browser data");
    let metachars_profile = "Work & Personal; rm -rf /; $(whoami) `echo test` \"quotes\" 'single'";

    let cmd = launch_command(exec, user_data, metachars_profile);

    let args: Vec<&OsStr> = cmd.get_args().collect();
    assert_eq!(args.len(), 2);

    let expected_arg = format!("--profile-directory={metachars_profile}");
    assert_eq!(args[1], OsStr::new(&expected_arg));
}

#[test]
fn test_executable_path_with_info_plist() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let app_path = temp_dir.path().join("Sample.app");
    let contents = app_path.join("Contents");
    let macos = contents.join("MacOS");
    std::fs::create_dir_all(&macos).expect("create macos dir");

    let plist_path = contents.join("Info.plist");
    let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>SampleBinary</string>
</dict>
</plist>"#;
    std::fs::write(&plist_path, plist_content).expect("write Info.plist");

    let binary_path = macos.join("SampleBinary");
    std::fs::write(&binary_path, b"dummy").expect("write binary");

    let install = dummy_install(app_path, temp_dir.path().join("user_data"));
    let resolved = executable_path(&install).expect("should resolve binary from Info.plist");
    assert_eq!(resolved, binary_path);
}

#[test]
fn test_executable_path_fallback_single_binary() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let app_path = temp_dir.path().join("Fallback.app");
    let contents = app_path.join("Contents");
    let macos = contents.join("MacOS");
    std::fs::create_dir_all(&macos).expect("create macos dir");

    let binary_path = macos.join("OnlyBinary");
    std::fs::write(&binary_path, b"dummy").expect("write binary");

    let install = dummy_install(app_path, temp_dir.path().join("user_data"));
    let resolved = executable_path(&install).expect("should fall back to single binary");
    assert_eq!(resolved, binary_path);
}

#[test]
fn test_executable_path_not_found_neither() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let app_path = temp_dir.path().join("Empty.app");

    let install = dummy_install(app_path, temp_dir.path().join("user_data"));
    let result = executable_path(&install);
    assert!(result.is_err());
    assert!(matches!(result, Err(Error::NotFound(_))));
}

#[test]
fn test_is_running_singleton_lock_detection() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let user_data_root = temp_dir.path().join("user_data");
    std::fs::create_dir_all(&user_data_root).expect("create user_data_root");

    let app_path = temp_dir.path().join("NonExistent.app");
    let install = dummy_install(app_path, user_data_root.clone());

    assert!(!is_running(&install));

    // A lock naming this test process is live, so the browser counts as running.
    let lock_path = user_data_root.join("SingletonLock");
    std::os::unix::fs::symlink(format!("host.local-{}", std::process::id()), &lock_path)
        .expect("create live lock");
    assert!(is_running(&install));

    // A lock left behind by a crashed process must not block forever.
    std::fs::remove_file(&lock_path).expect("remove lock");
    std::os::unix::fs::symlink("host.local-999999", &lock_path).expect("create stale lock");
    assert!(
        !is_running(&install),
        "a stale SingletonLock must not report the browser as running"
    );

    std::fs::remove_file(&lock_path).expect("remove lock");
    assert!(!is_running(&install));
}

#[test]
fn test_request_quit_without_bundle_id() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let mut install = dummy_install(
        temp_dir.path().join("App.app"),
        temp_dir.path().join("data"),
    );
    install.bundle_id = None;

    let result = request_quit(&install);
    assert!(result.is_err());
    assert!(matches!(result, Err(Error::Other(_))));
}

#[test]
fn test_wait_until_stopped_immediate() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let install = dummy_install(
        temp_dir.path().join("App.app"),
        temp_dir.path().join("data"),
    );
    assert!(wait_until_stopped(&install, Duration::from_millis(50)));
}

#[test]
fn test_wait_until_stopped_timeout() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let user_data = temp_dir.path().join("data");
    std::fs::create_dir_all(&user_data).expect("create data dir");
    std::fs::write(user_data.join("SingletonLock"), b"").expect("write lock");

    let install = dummy_install(temp_dir.path().join("App.app"), user_data);
    assert!(!wait_until_stopped(&install, Duration::from_millis(100)));
}

#[test]
#[ignore]
fn live_browser_launch_check() {
    // Live check placeholder for manual invocation; ignored by default
}
