use std::path::{Path, PathBuf};

use profilemux::cli::selector::resolve_profile;
use profilemux::domain::{
    BrowserInstall, BrowserInstallId, BrowserKind, BrowserProfile, Channel, ProfileId, SupportLevel,
};
use profilemux::error::Error;

fn make_install(kind: BrowserKind, user_data_root: &Path) -> BrowserInstall {
    BrowserInstall {
        id: BrowserInstallId::new(kind, user_data_root),
        kind,
        name: format!("{} Browser", kind.slug()),
        channel: Channel::Stable,
        app_path: PathBuf::from("/Applications/Browser.app"),
        bundle_id: Some("com.example.browser".to_string()),
        version: Some("1.0.0".to_string()),
        user_data_root: user_data_root.to_path_buf(),
        cache_root: None,
        support: SupportLevel::Full,
    }
}

fn make_profile(install: &BrowserInstall, directory: &str, display_name: &str) -> BrowserProfile {
    BrowserProfile {
        id: ProfileId::new(&install.id, directory),
        install_id: install.id.clone(),
        display_name: display_name.to_string(),
        directory: directory.to_string(),
        path: install.user_data_root.join(directory),
        cache_path: None,
        avatar: None,
        account_email: None,
        last_active: None,
        registered: true,
        directory_exists: true,
        size: None,
    }
}

#[test]
fn test_exact_profile_id_match() {
    let root = Path::new("/test/profiles/chrome");
    let install = make_install(BrowserKind::Chrome, root);
    let profile = make_profile(&install, "Default", "Personal");
    let data = vec![(install, vec![profile.clone()])];

    let (matched_install, matched_profile) =
        resolve_profile(&data, profile.id.as_str()).expect("should match exact profile id");

    assert_eq!(matched_install.kind, BrowserKind::Chrome);
    assert_eq!(matched_profile.id, profile.id);
}

#[test]
fn test_slug_directory_match() {
    let root = Path::new("/test/profiles/brave-beta");
    let install = make_install(BrowserKind::BraveBeta, root);
    let profile = make_profile(&install, "Profile 1", "Secondary");
    let data = vec![(install, vec![profile])];

    let selector = "brave-beta/Profile 1";
    let (matched_install, matched_profile) =
        resolve_profile(&data, selector).expect("should match slug/directory");

    assert_eq!(matched_install.kind, BrowserKind::BraveBeta);
    assert_eq!(matched_profile.directory, "Profile 1");
}

#[test]
fn test_case_insensitive_display_name_match() {
    let root = Path::new("/test/profiles/firefox");
    let install = make_install(BrowserKind::Firefox, root);
    let profile = make_profile(&install, "default-release", "Work Profile");
    let data = vec![(install, vec![profile])];

    let (_inst1, prof1) = resolve_profile(&data, "work profile").expect("lowercase should match");
    assert_eq!(prof1.display_name, "Work Profile");

    let (_inst2, prof2) = resolve_profile(&data, "WORK PROFILE").expect("uppercase should match");
    assert_eq!(prof2.display_name, "Work Profile");
}

#[test]
fn test_same_display_name_two_different_browsers_ambiguous() {
    let root_chrome = Path::new("/test/profiles/chrome");
    let install_chrome = make_install(BrowserKind::Chrome, root_chrome);
    let prof_chrome = make_profile(&install_chrome, "Default", "Work");

    let root_brave = Path::new("/test/profiles/brave");
    let install_brave = make_install(BrowserKind::Brave, root_brave);
    let prof_brave = make_profile(&install_brave, "Default", "Work");

    let data = vec![
        (install_chrome, vec![prof_chrome]),
        (install_brave, vec![prof_brave]),
    ];

    let err = resolve_profile(&data, "Work").expect_err("should be ambiguous");
    match err {
        Error::AmbiguousSelector {
            selector,
            candidates,
        } => {
            assert_eq!(selector, "Work");
            assert_eq!(candidates, vec!["brave/Default", "chrome/Default"]);
        }
        other => panic!("expected AmbiguousSelector, got: {other:?}"),
    }
}

#[test]
fn test_same_display_name_twice_within_one_browser_ambiguous() {
    let root = Path::new("/test/profiles/chrome");
    let install = make_install(BrowserKind::Chrome, root);
    let prof1 = make_profile(&install, "Profile 1", "Work");
    let prof2 = make_profile(&install, "Profile 2", "Work");
    let data = vec![(install, vec![prof1, prof2])];

    let err = resolve_profile(&data, "Work").expect_err("should be ambiguous");
    match err {
        Error::AmbiguousSelector {
            selector,
            candidates,
        } => {
            assert_eq!(selector, "Work");
            assert_eq!(candidates, vec!["chrome/Profile 1", "chrome/Profile 2"]);
        }
        other => panic!("expected AmbiguousSelector, got: {other:?}"),
    }
}

#[test]
fn test_unknown_selector_no_such_profile() {
    let root = Path::new("/test/profiles/chrome");
    let install = make_install(BrowserKind::Chrome, root);
    let prof = make_profile(&install, "Default", "Personal");
    let data = vec![(install, vec![prof])];

    let err = resolve_profile(&data, "nonexistent").expect_err("should fail");
    match err {
        Error::NoSuchProfile(s) => assert_eq!(s, "nonexistent"),
        other => panic!("expected NoSuchProfile, got: {other:?}"),
    }
}

#[test]
fn test_slug_directory_valid_slug_unknown_dir_no_such_profile() {
    let root = Path::new("/test/profiles/chrome");
    let install = make_install(BrowserKind::Chrome, root);
    let prof = make_profile(&install, "Default", "chrome/NonExistent");
    let data = vec![(install, vec![prof])];

    let err = resolve_profile(&data, "chrome/NonExistent")
        .expect_err("must not fall back to name matching");
    match err {
        Error::NoSuchProfile(s) => assert_eq!(s, "chrome/NonExistent"),
        other => panic!("expected NoSuchProfile, got: {other:?}"),
    }
}
