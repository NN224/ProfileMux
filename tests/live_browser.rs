//! Live validation against the real browsers installed on this machine.
//!
//! These tests are `#[ignore]` by default. Each one drives the real browser
//! binary against an isolated temporary user data root created with Chromium's
//! own `--user-data-dir` override, so the developer's real profiles and running
//! sessions are never touched and the production safety preflight is never
//! bypassed or weakened.
//!
//! Run with: `cargo test --test live_browser -- --ignored --nocapture`

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use profilemux::browsers::chromium::{launch, mutation, ChromiumAdapter};
use profilemux::browsers::BrowserAdapter;
use profilemux::domain::{
    BrowserInstall, BrowserInstallId, BrowserKind, Channel, ClonePolicy, CreateProfileSpec,
    DeleteMode, ExtensionPolicy, Severity, SupportLevel,
};
use tempfile::TempDir;

/// An install pointing at the real application bundle but an isolated data
/// root. This is the live-validation harness parameter; production discovery is
/// unchanged and still resolves the browser's real root.
fn isolated_install(
    kind: BrowserKind,
    app_path: &str,
    root: &Path,
    cache: &Path,
) -> Option<BrowserInstall> {
    let app = PathBuf::from(app_path);
    if !app.exists() {
        return None;
    }
    Some(BrowserInstall {
        id: BrowserInstallId::new(kind, root),
        kind,
        name: app_path.to_string(),
        channel: Channel::Stable,
        app_path: app,
        bundle_id: None,
        version: None,
        user_data_root: root.to_path_buf(),
        cache_root: Some(cache.to_path_buf()),
        support: SupportLevel::Full,
    })
}

/// Spawns the real browser against the isolated root. Optionally on a profile.
fn spawn_browser(exe: &Path, root: &Path, profile_directory: Option<&str>) -> Option<Child> {
    let mut cmd = Command::new(exe);
    cmd.arg(format!("--user-data-dir={}", root.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-background-networking");
    if let Some(dir) = profile_directory {
        cmd.arg(format!("--profile-directory={dir}"));
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// Terminates only the instances this test spawned, identified by the isolated
/// user data root in their arguments. The user's own browser instances carry a
/// different root and are never signalled.
fn terminate(mut child: Child, root: &Path) {
    let _ = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status();
    let _ = child.wait();
    // Chromium re-execs itself, so the spawned pid is not always the browser.
    // Signal by the unique isolated root instead, never by executable name.
    // The pattern must not start with a dash or pkill parses it as a flag.
    let pattern = root.display().to_string();
    for _ in 0..40 {
        let matched = Command::new("/usr/bin/pkill")
            .args(["-f", &pattern])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !matched {
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn wait_for(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

fn write_test_png(path: &Path) {
    let img = image::RgbaImage::from_pixel(512, 300, image::Rgba([10, 120, 220, 255]));
    let _ = img.save(path);
}

/// The full live cycle for one browser against an isolated user data root.
fn live_cycle(kind: BrowserKind, app_path: &str) -> Option<Vec<String>> {
    let tmp = TempDir::new().ok()?;
    let root = tmp.path().join("User Data");
    let cache = tmp.path().join("Caches");
    std::fs::create_dir_all(&cache).ok()?;
    let install = isolated_install(kind, app_path, &root, &cache)?;
    let exe = launch::executable_path(&install).expect("resolve real executable");
    let adapter = ChromiumAdapter::new(install.clone());
    let mut log = Vec::new();
    log.push(format!("binary {}", exe.display()));

    // 1. Let the real browser initialize the isolated root itself.
    let child = spawn_browser(&exe, &root, None).expect("spawn real browser");
    let initialized = wait_for(Duration::from_secs(60), || {
        root.join("Local State").is_file() && root.join("Default").is_dir()
    });
    terminate(child, &root);
    assert!(initialized, "browser did not initialize the isolated root");
    assert!(
        wait_for(Duration::from_secs(20), || !adapter.is_running()),
        "isolated instance still reported as running"
    );
    log.push("browser initialized isolated root".to_string());

    // 2. Create a Master template profile and give it settings plus private data.
    let mut master_spec = CreateProfileSpec::new("PMUX TEST MASTER");
    master_spec.directory = Some("PMUX-TEST-MASTER".to_string());
    let master = adapter.create_profile(&master_spec).expect("create master");
    std::fs::write(
        master.path.join("Preferences"),
        br#"{"browser":{"custom_chrome_frame":true},"side_panel":{"is_right_aligned":false},"account_info":[{"email":"someone@example.com"}],"extensions":{"settings":{"abc":1}}}"#,
    )
    .expect("write master preferences");
    for private in ["Cookies", "Login Data", "History", "Web Data"] {
        std::fs::write(master.path.join(private), b"private").expect("write private fixture");
    }
    std::fs::create_dir_all(master.path.join("Extensions/abc")).expect("extension fixture");
    std::fs::write(master.path.join("Extensions/abc/manifest.json"), b"{}").expect("manifest");
    log.push("master created and populated".to_string());

    // 3. Create PMUX TEST from the template with the default extension policy.
    let mut spec = CreateProfileSpec::new("PMUX TEST");
    spec.directory = Some("PMUX-TEST".to_string());
    spec.template_directory = Some(master.directory.clone());
    spec.clone_policy = ClonePolicy {
        copy_preferences: true,
        copy_bookmarks: false,
        extensions: ExtensionPolicy::None,
    };
    let created = adapter.create_profile(&spec).expect("create from template");
    let prefs = std::fs::read_to_string(created.path.join("Preferences")).expect("clone prefs");
    assert!(
        prefs.contains("custom_chrome_frame"),
        "UI settings must clone"
    );
    assert!(prefs.contains("side_panel"), "sidebar layout must clone");
    assert!(
        !prefs.contains("account_info"),
        "account identity must not clone"
    );
    assert!(
        !prefs.contains("\"settings\""),
        "extension settings must not clone"
    );
    for private in ["Cookies", "Login Data", "History", "Web Data"] {
        assert!(
            !created.path.join(private).exists(),
            "{private} must not be cloned"
        );
    }
    assert!(
        !created.path.join("Extensions").exists(),
        "extensions must not be cloned under the default policy"
    );
    log.push("template clone carried settings only".to_string());

    // 4. Launch the created profile with the real binary and let it start up.
    let child = spawn_browser(&exe, &root, Some(&created.directory)).expect("spawn on profile");
    let accepted = wait_for(Duration::from_secs(60), || {
        created.path.join("Network").is_dir() || created.path.join("Sessions").is_dir()
    });
    terminate(child, &root);
    assert!(
        wait_for(Duration::from_secs(20), || !adapter.is_running()),
        "launched instance still reported as running"
    );
    assert!(accepted, "browser did not open the created profile");
    let state: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("Local State")).expect("read local state"),
    )
    .expect("parse local state");
    let entry = state
        .pointer(&format!("/profile/info_cache/{}", created.directory))
        .expect("browser kept the profile registered");
    assert_eq!(
        entry.get("name").and_then(|v| v.as_str()),
        Some("PMUX TEST"),
        "browser kept the display name"
    );
    log.push("browser launched the profile and kept its registration".to_string());

    // 5. Rename the display name only.
    adapter
        .rename_display_name(&created, "PMUX TEST RENAMED")
        .expect("rename display name");
    let snapshot = adapter.snapshot().expect("snapshot");
    let renamed = snapshot
        .find_by_directory("PMUX-TEST")
        .expect("profile still at its directory");
    assert_eq!(renamed.display_name, "PMUX TEST RENAMED");
    assert_eq!(renamed.directory, "PMUX-TEST");
    log.push("renamed display name, directory untouched".to_string());

    // 6. Custom avatar, where the adapter claims support.
    if adapter.capabilities().custom_avatar {
        let src = tmp.path().join("avatar.png");
        write_test_png(&src);
        let before = std::fs::read(&src).expect("read source image");
        adapter.set_avatar(renamed, &src).expect("set avatar");
        assert!(renamed.path.join("Google Profile Picture.png").is_file());
        assert_eq!(before, std::fs::read(&src).expect("reread source"));
        let state: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("Local State")).expect("read local state"),
        )
        .expect("parse local state");
        let entry = state
            .pointer(&format!("/profile/info_cache/{}", renamed.directory))
            .expect("entry");
        assert_eq!(
            entry.get("gaia_picture_file_name").and_then(|v| v.as_str()),
            Some("Google Profile Picture.png")
        );
        assert_eq!(
            entry.get("use_gaia_picture").and_then(|v| v.as_bool()),
            Some(true)
        );
        log.push("custom avatar installed and registered".to_string());
    } else {
        log.push("custom avatar not claimed for this browser, skipped".to_string());
    }

    // 7. Doctor over the isolated root.
    let findings = adapter.doctor().expect("doctor");
    assert!(
        findings.iter().all(|f| f.severity != Severity::Broken),
        "doctor reported a broken profile store: {findings:?}"
    );
    log.push(format!(
        "doctor: {} finding(s), none broken",
        findings.len()
    ));

    // 8. Delete through the real Trash path, then clean up what we trashed.
    let profile_path = renamed.path.clone();
    adapter
        .delete_profile(renamed, DeleteMode::Trash)
        .expect("delete to trash");
    assert!(!profile_path.exists(), "profile directory must be gone");
    let after = adapter.snapshot().expect("snapshot after delete");
    assert!(after.find_by_directory("PMUX-TEST").is_none());
    if let Some(home) = dirs::home_dir() {
        for name in ["PMUX-TEST", "Caches"] {
            let trashed = home.join(".Trash").join(name);
            if trashed.exists() {
                let _ = std::fs::remove_dir_all(&trashed);
            }
        }
    }
    log.push("deleted to Trash and deregistered".to_string());

    // 9. The production preflight is intact: a running instance still refuses.
    let guard = spawn_browser(&exe, &root, None).expect("spawn for preflight check");
    let saw_running = wait_for(Duration::from_secs(60), || adapter.is_running());
    let refused = mutation::ensure_stopped(&install).is_err();
    terminate(guard, &root);
    assert!(saw_running, "running instance was not detected");
    assert!(refused, "preflight must refuse while the browser runs");
    log.push("preflight still refuses while the browser runs".to_string());

    Some(log)
}

fn report(label: &str, kind: BrowserKind, app_path: &str) {
    match live_cycle(kind, app_path) {
        Some(log) => {
            println!("\n=== {label} ===");
            for line in log {
                println!("  {line}");
            }
        }
        None => println!("\n=== {label} === not installed, skipped"),
    }
}

#[test]
#[ignore = "live: drives the real browser against an isolated user data root"]
fn live_brave_beta() {
    report(
        "Brave Beta",
        BrowserKind::BraveBeta,
        "/Applications/Brave Browser Beta.app",
    );
}

#[test]
#[ignore = "live: drives the real browser against an isolated user data root"]
fn live_brave_stable() {
    report(
        "Brave Stable",
        BrowserKind::Brave,
        "/Applications/Brave Browser.app",
    );
}

#[test]
#[ignore = "live: drives the real browser against an isolated user data root"]
fn live_chrome() {
    report(
        "Chrome",
        BrowserKind::Chrome,
        "/Applications/Google Chrome.app",
    );
}
