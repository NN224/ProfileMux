mod common;

use common::ChromiumFixture;
use profilemux::browsers::chromium::ChromiumAdapter;
use profilemux::browsers::BrowserAdapter;
use profilemux::fs::{format_bytes, measure_profile};
use serde_json::json;

#[test]
fn test_valid_local_state_with_avatar() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile_entry(
        "Default",
        json!({
            "name": "Personal",
            "gaia_picture_file_name": "CustomAvatar.png",
            "use_gaia_picture": true,
            "active_time": 1700000000.5,
        }),
        true,
    );
    fixture.add_registered_profile_entry(
        "Profile 1",
        json!({
            "name": "Work",
            "avatar_icon": "chrome://theme/IDR_PROFILE_AVATAR_26",
            "is_using_default_avatar": false,
        }),
        true,
    );

    let adapter = ChromiumAdapter::new(fixture.install());
    let snapshot = adapter.snapshot().expect("snapshot should succeed");

    assert_eq!(snapshot.registered.len(), 2);
    assert!(snapshot.parse_error.is_none());

    let p1 = snapshot
        .find_by_directory("Default")
        .expect("Default profile found");
    assert_eq!(p1.display_name, "Personal");
    assert_eq!(p1.last_active, Some(1700000000));
    let avatar1 = p1.avatar.as_ref().expect("avatar exists");
    assert_eq!(avatar1.picture_file.as_deref(), Some("CustomAvatar.png"));
    assert!(avatar1.uses_picture);

    let p2 = snapshot
        .find_by_directory("Profile 1")
        .expect("Profile 1 found");
    assert_eq!(p2.display_name, "Work");
    let avatar2 = p2.avatar.as_ref().expect("avatar exists");
    assert_eq!(
        avatar2.icon.as_deref(),
        Some("chrome://theme/IDR_PROFILE_AVATAR_26")
    );
    assert!(!avatar2.uses_picture);
}

#[test]
fn test_display_name_fallback_to_directory() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Profile 2", None, true);

    let adapter = ChromiumAdapter::new(fixture.install());
    let snapshot = adapter.snapshot().expect("snapshot should succeed");

    let p = snapshot
        .find_by_directory("Profile 2")
        .expect("Profile 2 found");
    assert_eq!(p.display_name, "Profile 2");
}

#[test]
fn test_missing_directory_flag() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Present"), true);
    fixture.add_registered_profile("Profile Missing", Some("Missing"), false);

    let adapter = ChromiumAdapter::new(fixture.install());
    let snapshot = adapter.snapshot().expect("snapshot should succeed");

    let p_present = snapshot
        .find_by_directory("Default")
        .expect("Default found");
    assert!(p_present.directory_exists);

    let p_missing = snapshot
        .find_by_directory("Profile Missing")
        .expect("Profile Missing found");
    assert!(!p_missing.directory_exists);
}

#[test]
fn test_unregistered_directory_detection() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Main"), true);
    fixture.add_unregistered_profile("Profile Unregistered");

    for ignored in &[
        "System Profile",
        "Guest Profile",
        "Crashpad",
        "component_crx_cache",
        "extensions_crx_cache",
    ] {
        let ignored_dir = fixture.user_data_root().join(ignored);
        std::fs::create_dir_all(&ignored_dir).expect("create ignored dir");
        std::fs::write(ignored_dir.join("Preferences"), "{}").expect("write prefs");
    }

    let not_profile = fixture.user_data_root().join("SomeRandomDir");
    std::fs::create_dir_all(&not_profile).expect("create random dir");

    let adapter = ChromiumAdapter::new(fixture.install());
    let snapshot = adapter.snapshot().expect("snapshot should succeed");

    let expected = vec![fixture.user_data_root().join("Profile Unregistered")];
    assert_eq!(snapshot.unregistered_dirs, expected);
}

#[test]
fn test_orphan_cache_directory_detection() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Main"), true);

    let default_cache = fixture.cache_root().join("Default");
    std::fs::create_dir_all(default_cache).expect("create default cache");

    fixture.add_orphan_cache("Profile Orphaned");
    fixture.add_orphan_cache("System Profile");
    fixture.add_orphan_cache("Guest Profile");

    let adapter = ChromiumAdapter::new(fixture.install());
    let snapshot = adapter.snapshot().expect("snapshot should succeed");

    let expected = vec![fixture.cache_root().join("Profile Orphaned")];
    assert_eq!(snapshot.orphan_cache_dirs, expected);
}

#[test]
fn test_malformed_local_state() {
    let mut fixture = ChromiumFixture::new();
    fixture.write_malformed_local_state("{ invalid json broken syntax");

    let adapter = ChromiumAdapter::new(fixture.install());
    let snapshot = adapter.snapshot().expect("should return snapshot, not Err");

    assert!(snapshot.parse_error.is_some());
    assert!(snapshot.registered.is_empty());
}

#[test]
fn test_duplicate_display_names() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Profile 1", Some("Work"), true);
    fixture.add_registered_profile("Profile 2", Some("Work"), true);

    let adapter = ChromiumAdapter::new(fixture.install());
    let snapshot = adapter.snapshot().expect("snapshot should succeed");

    assert_eq!(snapshot.registered.len(), 2);
    let p1 = snapshot
        .find_by_directory("Profile 1")
        .expect("Profile 1 found");
    let p2 = snapshot
        .find_by_directory("Profile 2")
        .expect("Profile 2 found");

    assert_eq!(p1.display_name, "Work");
    assert_eq!(p2.display_name, "Work");
    assert_ne!(p1.id, p2.id);
}

#[test]
fn test_measure_profile_storage_breakdown() {
    let fixture = ChromiumFixture::new();
    let profile_dir = fixture.user_data_root().join("Default");
    let cache_dir = fixture.cache_root().join("Default");

    fixture.write_file(&profile_dir.join("Preferences"), 100);
    fixture.write_file(&profile_dir.join("History"), 200);
    fixture.write_file(&profile_dir.join("Cache/data_0"), 300);
    fixture.write_file(&profile_dir.join("Code Cache/wasm/index"), 400);
    fixture.write_file(&profile_dir.join("GPUCache/data_0"), 500);

    fixture.write_file(&cache_dir.join("Cache/data_1"), 600);
    fixture.write_file(&cache_dir.join("Code Cache/js/index"), 700);
    fixture.write_file(&cache_dir.join("GPUCache/data_1"), 800);

    let breakdown = measure_profile(&profile_dir, Some(&cache_dir));

    assert_eq!(breakdown.core, 300);
    assert_eq!(breakdown.cache, 900);
    assert_eq!(breakdown.code_cache, 1100);
    assert_eq!(breakdown.gpu_cache, 1300);
    assert_eq!(breakdown.total, 3600);
}

#[test]
fn test_format_bytes_documented_cases() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512 * 1024), "512 KB");
    assert_eq!(format_bytes(880 * 1024 * 1024), "880 MB");
    assert_eq!(format_bytes(1_524_713_390), "1.42 GB");
}

#[test]
fn test_is_running_singleton_lock() {
    let fixture = ChromiumFixture::new();
    let adapter = ChromiumAdapter::new(fixture.install());

    assert!(!adapter.is_running());

    let lock_path = fixture.user_data_root().join("SingletonLock");
    #[cfg(unix)]
    std::os::unix::fs::symlink("non_existent_target_process_123", &lock_path)
        .expect("create symlink");
    #[cfg(not(unix))]
    std::fs::write(&lock_path, "lock").expect("create lock file");

    assert!(adapter.is_running());

    std::fs::remove_file(&lock_path).expect("remove lock file");
    assert!(!adapter.is_running());
}
