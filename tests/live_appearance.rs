//! Live appearance validation against the real installed browsers.
//!
//! `#[ignore]` by default. Each test drives a real browser binary against an
//! isolated temporary user data root via Chromium's own `--user-data-dir`, so
//! the developer's real profiles and running sessions are never touched and the
//! production safety preflight is never bypassed.
//!
//! Run with: `cargo test --test live_appearance -- --ignored --nocapture`

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use profilemux::browsers::chromium::{launch, ChromiumAdapter};
use profilemux::browsers::BrowserAdapter;
use profilemux::domain::{
    AppearanceSpec, BrowserInstall, BrowserInstallId, BrowserKind, BrowserTheme, Channel,
    CreateProfileSpec, SupportLevel, WebDarkMode,
};
use profilemux::policy::LaunchPolicy;
use tempfile::TempDir;

fn isolated(kind: BrowserKind, app: &str, root: &Path, cache: &Path) -> Option<BrowserInstall> {
    let app_path = PathBuf::from(app);
    if !app_path.exists() {
        return None;
    }
    Some(BrowserInstall {
        id: BrowserInstallId::new(kind, root),
        kind,
        name: app.to_string(),
        channel: Channel::Stable,
        app_path,
        bundle_id: None,
        version: None,
        user_data_root: root.to_path_buf(),
        cache_root: Some(cache.to_path_buf()),
        support: SupportLevel::Full,
    })
}

fn spawn(exe: &Path, root: &Path, profile: Option<&str>) -> Option<Child> {
    let mut cmd = Command::new(exe);
    cmd.arg(format!("--user-data-dir={}", root.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check");
    if let Some(p) = profile {
        cmd.arg(format!("--profile-directory={p}"));
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// Terminates only instances started against this isolated root.
fn terminate(mut child: Child, root: &Path) {
    let _ = Command::new("/bin/kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status();
    let _ = child.wait();
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

fn wait_for(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

fn prefs_values(profile_dir: &Path) -> (Option<i64>, Option<bool>) {
    let raw = std::fs::read_to_string(profile_dir.join("Preferences")).unwrap_or_default();
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    (
        doc.pointer("/browser/theme/color_scheme2")
            .and_then(|v| v.as_i64()),
        doc.pointer("/brave/darker_mode").and_then(|v| v.as_bool()),
    )
}

fn cycle(kind: BrowserKind, app: &str) -> Option<Vec<String>> {
    let tmp = TempDir::new().ok()?;
    let root = tmp.path().join("User Data");
    let cache = tmp.path().join("Caches");
    std::fs::create_dir_all(&cache).ok()?;
    let install = isolated(kind, app, &root, &cache)?;
    let exe = launch::executable_path(&install).expect("resolve executable");
    let adapter = ChromiumAdapter::new(install.clone());
    let mut log = Vec::new();

    // Let the real browser initialize the isolated root.
    let child = spawn(&exe, &root, None).expect("spawn browser");
    let ready = wait_for(Duration::from_secs(60), || {
        root.join("Local State").is_file() && root.join("Default").is_dir()
    });
    terminate(child, &root);
    assert!(ready, "browser did not initialize the isolated root");
    assert!(wait_for(Duration::from_secs(20), || !adapter.is_running()));

    let mut spec = CreateProfileSpec::new("PMUX APPEARANCE");
    spec.directory = Some("PMUX-APPEARANCE".to_string());
    let profile = adapter.create_profile(&spec).expect("create profile");
    let caps = adapter.appearance_capabilities();
    log.push(format!(
        "caps: theme={} ultra_dark={} web_dark={}",
        caps.browser_theme, caps.ultra_dark, caps.web_dark
    ));

    // Let the browser initialize the new profile before theming it: on a
    // never-launched profile the browser writes its own first-run defaults and
    // would overwrite the values ProfileMux had just set.
    let child = spawn(&exe, &root, Some(&profile.directory)).expect("first run");
    // The browser rewrites the minimal `Preferences` ProfileMux created with its
    // own first-run defaults; a grown file is the signal that it has done so.
    let inited = wait_for(Duration::from_secs(90), || {
        std::fs::metadata(profile.path.join("Preferences"))
            .map(|m| m.len() > 512)
            .unwrap_or(false)
    });
    terminate(child, &root);
    assert!(wait_for(Duration::from_secs(20), || !adapter.is_running()));
    assert!(inited, "browser did not initialize the new profile");
    log.push("profile initialized by the browser".to_string());

    // Dark.
    adapter
        .set_appearance(
            &profile,
            &AppearanceSpec {
                theme: Some(BrowserTheme::Dark),
                web_dark: None,
            },
        )
        .expect("set Dark");
    let (scheme, darker) = prefs_values(&profile.path);
    assert_eq!(scheme, Some(2));
    assert_eq!(darker, if caps.ultra_dark { Some(false) } else { None });
    assert_eq!(
        adapter.read_appearance(&profile).expect("read").theme,
        BrowserTheme::Dark
    );

    // Launch on the profile and confirm the browser keeps the value.
    let child = spawn(&exe, &root, Some(&profile.directory)).expect("spawn on profile");
    let started = wait_for(Duration::from_secs(60), || {
        profile.path.join("Network").is_dir() || profile.path.join("Sessions").is_dir()
    });
    terminate(child, &root);
    assert!(wait_for(Duration::from_secs(20), || !adapter.is_running()));
    assert!(started, "browser did not open the profile");
    let after_dark = prefs_values(&profile.path);
    assert_eq!(after_dark.0, Some(2), "browser changed the colour scheme");
    log.push(format!("Dark survived a real launch: {after_dark:?}"));

    // Ultra Dark, which must also keep the dark colour scheme.
    if caps.ultra_dark {
        adapter
            .set_appearance(
                &profile,
                &AppearanceSpec {
                    theme: Some(BrowserTheme::UltraDark),
                    web_dark: None,
                },
            )
            .expect("set Ultra Dark");
        assert_eq!(prefs_values(&profile.path), (Some(2), Some(true)));
        let child = spawn(&exe, &root, Some(&profile.directory)).expect("relaunch");
        let up = wait_for(Duration::from_secs(60), || adapter.is_running());
        terminate(child, &root);
        assert!(wait_for(Duration::from_secs(20), || !adapter.is_running()));
        assert!(up, "relaunch was not detected");
        let after = prefs_values(&profile.path);
        assert_eq!(
            after,
            (Some(2), Some(true)),
            "browser did not keep Ultra Dark"
        );
        assert_eq!(
            adapter.read_appearance(&profile).expect("read").theme,
            BrowserTheme::UltraDark
        );
        log.push(format!("Ultra Dark survived a relaunch: {after:?}"));
    } else {
        log.push("Ultra Dark not claimed for this browser, skipped".to_string());
    }

    // Revert to System.
    adapter
        .set_appearance(
            &profile,
            &AppearanceSpec {
                theme: Some(BrowserTheme::System),
                web_dark: None,
            },
        )
        .expect("set System");
    let (scheme, darker) = prefs_values(&profile.path);
    assert_eq!(scheme, Some(0));
    assert_eq!(darker, if caps.ultra_dark { Some(false) } else { None });
    assert_eq!(
        adapter.read_appearance(&profile).expect("read").theme,
        BrowserTheme::System
    );
    log.push("reverted to System cleanly".to_string());

    // Force dark is launch-time: the browser must accept the switch and run.
    let args = launch::launch_args(
        &root,
        &profile.directory,
        &LaunchPolicy {
            web_dark: WebDarkMode::ForceDark,
        },
    );
    assert!(args
        .iter()
        .any(|a| a == "--enable-features=WebContentsForceDark"));
    let mut cmd = Command::new(&exe);
    cmd.args(&args)
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = cmd.spawn().expect("spawn with force dark");
    let up = wait_for(Duration::from_secs(60), || adapter.is_running());
    terminate(child, &root);
    assert!(wait_for(Duration::from_secs(20), || !adapter.is_running()));
    assert!(up, "browser did not start with the force-dark switch");
    let (scheme, darker) = prefs_values(&profile.path);
    assert_eq!(
        scheme,
        Some(0),
        "force dark must not change the colour scheme"
    );
    assert_eq!(
        darker,
        if caps.ultra_dark { Some(false) } else { None },
        "force dark must not write profile preferences"
    );
    log.push("force-dark launch accepted and wrote no preferences".to_string());

    Some(log)
}

fn report(label: &str, kind: BrowserKind, app: &str) {
    match cycle(kind, app) {
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
fn live_appearance_brave_beta() {
    report(
        "Brave Beta",
        BrowserKind::BraveBeta,
        "/Applications/Brave Browser Beta.app",
    );
}

#[test]
#[ignore = "live: drives the real browser against an isolated user data root"]
fn live_appearance_brave_stable() {
    report(
        "Brave Stable",
        BrowserKind::Brave,
        "/Applications/Brave Browser.app",
    );
}

#[test]
#[ignore = "live: drives the real browser against an isolated user data root"]
fn live_appearance_chrome() {
    report(
        "Chrome",
        BrowserKind::Chrome,
        "/Applications/Google Chrome.app",
    );
}
