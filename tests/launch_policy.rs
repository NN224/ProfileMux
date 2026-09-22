use std::fs;
use std::path::Path;

use profilemux::browsers::chromium::launch::launch_args;
use profilemux::domain::{BrowserInstallId, BrowserKind, ProfileId, WebDarkMode};
use profilemux::policy::{self, LaunchPolicy, PolicyStore};
use tempfile::tempdir;

#[test]
fn test_launch_args_normal() {
    let user_data = Path::new("/tmp/test_user_data");
    let profile_dir = "Default";
    let policy = LaunchPolicy {
        web_dark: WebDarkMode::Normal,
    };

    let args = launch_args(user_data, profile_dir, &policy);
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], format!("--user-data-dir={}", user_data.display()));
    assert_eq!(args[1], format!("--profile-directory={profile_dir}"));
    assert!(!args.iter().any(|arg| arg.contains("--enable-features")));
}

#[test]
fn test_launch_args_force_dark() {
    let user_data = Path::new("/tmp/test_user_data");
    let profile_dir = "Default";
    let policy = LaunchPolicy {
        web_dark: WebDarkMode::ForceDark,
    };

    let args = launch_args(user_data, profile_dir, &policy);
    assert_eq!(args.len(), 3);
    assert_eq!(args[0], format!("--user-data-dir={}", user_data.display()));
    assert_eq!(args[1], format!("--profile-directory={profile_dir}"));
    assert_eq!(args[2], "--enable-features=WebContentsForceDark");
    assert_eq!(
        args.iter()
            .filter(|arg| *arg == "--enable-features=WebContentsForceDark")
            .count(),
        1
    );
}

#[test]
fn test_launch_args_space_in_profile_directory() {
    let user_data = Path::new("/tmp/test_user_data");
    let profile_dir = "Profile 1";

    let normal_policy = LaunchPolicy {
        web_dark: WebDarkMode::Normal,
    };
    let normal_args = launch_args(user_data, profile_dir, &normal_policy);
    assert_eq!(normal_args.len(), 2);
    assert_eq!(normal_args[1], "--profile-directory=Profile 1");

    let force_policy = LaunchPolicy {
        web_dark: WebDarkMode::ForceDark,
    };
    let force_args = launch_args(user_data, profile_dir, &force_policy);
    assert_eq!(force_args.len(), 3);
    assert_eq!(force_args[1], "--profile-directory=Profile 1");
    assert_eq!(force_args[2], "--enable-features=WebContentsForceDark");
}

#[test]
fn test_store_missing_path() {
    let tmp = tempdir().expect("tempdir");
    let missing_path = tmp.path().join("does_not_exist").join("launch-policy.json");
    let store = PolicyStore::load(&missing_path).expect("load missing path");

    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    let install_id = BrowserInstallId::new(BrowserKind::Brave, Path::new("/tmp/test_brave"));
    let id = ProfileId::new(&install_id, "Default");
    let policy = store.get(&id);
    assert_eq!(policy.web_dark, WebDarkMode::Normal);
}

#[test]
fn test_set_save_load_roundtrip_and_immutability() {
    let tmp = tempdir().expect("tempdir");
    let policy_path = tmp.path().join("policies").join("launch-policy.json");
    let original_store = PolicyStore::load(&policy_path).expect("load initial store");

    let install_id = BrowserInstallId::new(BrowserKind::Brave, Path::new("/tmp/test_brave"));
    let target_id = ProfileId::new(&install_id, "Default");
    let other_id = ProfileId::new(&install_id, "Profile 1");

    let updated_store = original_store.set(
        &target_id,
        LaunchPolicy {
            web_dark: WebDarkMode::ForceDark,
        },
    );

    // Assert the original store still reports Normal (immutable set)
    assert_eq!(original_store.get(&target_id).web_dark, WebDarkMode::Normal);
    assert_eq!(
        updated_store.get(&target_id).web_dark,
        WebDarkMode::ForceDark
    );

    updated_store.save().expect("save store");

    let reloaded_store = PolicyStore::load(&policy_path).expect("reload store");
    assert_eq!(
        reloaded_store.get(&target_id).web_dark,
        WebDarkMode::ForceDark
    );
    assert_eq!(reloaded_store.get(&other_id).web_dark, WebDarkMode::Normal);
}

#[test]
fn test_malformed_json_loads_as_empty_store() {
    let tmp = tempdir().expect("tempdir");
    let corrupt_path = tmp.path().join("corrupt.json");
    fs::write(&corrupt_path, b"{ not valid json !!!").expect("write corrupt json");

    let store = PolicyStore::load(&corrupt_path).expect("load malformed json");
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    let install_id = BrowserInstallId::new(BrowserKind::Brave, Path::new("/tmp/test_brave"));
    let id = ProfileId::new(&install_id, "Default");
    assert_eq!(store.get(&id).web_dark, WebDarkMode::Normal);
}

#[test]
fn test_saved_json_contains_no_profile_name_and_no_path_beyond_id() {
    let tmp = tempdir().expect("tempdir");
    let policy_path = tmp.path().join("launch-policy.json");
    let store = PolicyStore::load(&policy_path).expect("load");

    let install_id = BrowserInstallId::new(BrowserKind::Brave, Path::new("/tmp/test_brave"));
    let id = ProfileId::new(&install_id, "Profile 1");
    let display_name = "Secret Personal Profile";
    let email = "user@example.com";

    let updated = store.set(
        &id,
        LaunchPolicy {
            web_dark: WebDarkMode::ForceDark,
        },
    );
    updated.save().expect("save");

    let json_text = fs::read_to_string(&policy_path).expect("read policy file");

    assert!(
        !json_text.contains(display_name),
        "saved JSON must not contain profile display name"
    );
    assert!(
        !json_text.contains(email),
        "saved JSON must not contain email addresses"
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&json_text).expect("saved file must be valid JSON");
    let map = parsed.as_object().expect("root must be a JSON object");

    assert_eq!(map.len(), 1);
    assert!(map.contains_key(id.as_str()));

    let entry = map.get(id.as_str()).expect("entry for id");
    let entry_obj = entry.as_object().expect("entry must be an object");
    assert_eq!(entry_obj.len(), 1);
    assert_eq!(
        entry_obj.get("web_dark").and_then(|v| v.as_str()),
        Some("ForceDark")
    );
}

#[test]
fn test_default_path() {
    let path = policy::default_path().expect("default path");
    assert_eq!(
        path.file_name().and_then(|n| n.to_str()),
        Some("launch-policy.json")
    );
    assert_eq!(
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str()),
        Some("profilemux")
    );
}
