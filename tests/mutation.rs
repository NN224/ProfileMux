mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::ChromiumFixture;
use profilemux::browsers::chromium::mutation::delete_profile_with;
use profilemux::browsers::chromium::ChromiumAdapter;
use profilemux::browsers::BrowserAdapter;
use profilemux::domain::{
    ClonePolicy, CloneProfileSpec, CreateProfileSpec, DeleteMode, ExtensionPolicy,
};
use profilemux::fs::trash::FakeTrash;
use serde_json::json;
use tempfile::TempDir;

const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn snapshot_dir(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut map = BTreeMap::new();
    if !root.exists() {
        return map;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.is_file() {
                    if let Ok(rel) = p.strip_prefix(root) {
                        if let Ok(bytes) = fs::read(&p) {
                            map.insert(rel.to_path_buf(), bytes);
                        }
                    }
                }
            }
        }
    }
    map
}

#[test]
fn test_create_writes_real_dir_and_preserves_unrelated_local_state() {
    let fixture = ChromiumFixture::new();
    let ls_path = fixture.user_data_root().join("Local State");
    let initial_ls = json!({
        "unrelated_key": {"setting": 42},
        "profile": {
            "info_cache": {}
        }
    });
    fs::write(&ls_path, serde_json::to_string_pretty(&initial_ls).unwrap()).unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let spec = CreateProfileSpec::new("Engineering");
    let profile = adapter
        .create_profile(&spec)
        .expect("create should succeed");

    assert!(profile.path.is_dir());
    assert!(profile.path.join("Preferences").is_file());
    assert_eq!(fs::read(profile.path.join("Preferences")).unwrap(), b"{}");

    let content = fs::read_to_string(&ls_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["unrelated_key"], json!({"setting": 42}));
    assert_eq!(
        parsed["profile"]["info_cache"][&profile.directory]["name"],
        "Engineering"
    );
    assert_eq!(
        parsed["profile"]["info_cache"][&profile.directory]["is_using_default_name"],
        false
    );
}

#[test]
fn test_create_rejects_unsafe_directories() {
    let fixture = ChromiumFixture::new();
    let adapter = ChromiumAdapter::new(fixture.install());

    let bad_names = [
        "bad/dir",
        "bad\\dir",
        "..",
        ".hidden",
        "Local State",
        "Crashpad",
        "",
        "\0",
    ];
    for name in bad_names {
        let mut spec = CreateProfileSpec::new("Valid Display");
        spec.directory = Some(name.to_string());
        let res = adapter.create_profile(&spec);
        assert!(res.is_err(), "expected rejection for `{name}`");
        if !name.is_empty() && name != "." && name != ".." && !name.contains('\0') {
            assert!(!fixture.user_data_root().join(name).exists());
        }
    }
}

#[test]
fn test_create_sanitizes_and_deduplicates() {
    let fixture = ChromiumFixture::new();
    let adapter = ChromiumAdapter::new(fixture.install());

    let spec1 = CreateProfileSpec::new("NIGHTCLUB & LOUNGE");
    let p1 = adapter
        .create_profile(&spec1)
        .expect("create 1 should succeed");
    assert_eq!(p1.directory, "NIGHTCLUB-LOUNGE");

    let spec2 = CreateProfileSpec::new("NIGHTCLUB & LOUNGE");
    let p2 = adapter
        .create_profile(&spec2)
        .expect("create 2 should succeed");
    assert_eq!(p2.directory, "NIGHTCLUB-LOUNGE-2");
}

#[test]
fn test_create_from_template_sanitizes_and_excludes_private_data() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Template", Some("Template"), true);
    let tpl_dir = fixture.user_data_root().join("Template");

    fs::write(
        tpl_dir.join("Preferences"),
        r#"{"browser":{"show_home_button":true}}"#,
    )
    .unwrap();
    fs::write(tpl_dir.join("Cookies"), b"secret_cookies").unwrap();
    fs::write(tpl_dir.join("Login Data"), b"passwords").unwrap();
    fs::write(tpl_dir.join("History"), b"history").unwrap();
    fs::write(tpl_dir.join("Sessions"), b"sessions").unwrap();
    fs::create_dir_all(tpl_dir.join("Extensions")).unwrap();
    fs::write(tpl_dir.join("Extensions").join("manifest.json"), b"{}").unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let mut spec = CreateProfileSpec::new("Cloned");
    spec.template_directory = Some("Template".to_string());
    spec.clone_policy = ClonePolicy::default();

    let p = adapter
        .create_profile(&spec)
        .expect("create from template should succeed");
    assert!(p.path.is_dir());
    assert!(p.path.join("Preferences").is_file());
    assert!(!p.path.join("Cookies").exists());
    assert!(!p.path.join("Login Data").exists());
    assert!(!p.path.join("History").exists());
    assert!(!p.path.join("Sessions").exists());
    assert!(!p.path.join("Extensions").exists());
}

#[test]
fn test_create_from_template_with_copy_extensions() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Template", Some("Template"), true);
    let tpl_dir = fixture.user_data_root().join("Template");
    fs::write(tpl_dir.join("Preferences"), b"{}").unwrap();
    let ext_dir = tpl_dir.join("Extensions").join("my_extension");
    fs::create_dir_all(&ext_dir).unwrap();
    fs::write(ext_dir.join("manifest.json"), b"{}").unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let mut spec = CreateProfileSpec::new("WithExt");
    spec.template_directory = Some("Template".to_string());
    spec.clone_policy = ClonePolicy {
        copy_preferences: true,
        copy_bookmarks: false,
        extensions: ExtensionPolicy::CopyExtensions,
    };

    let p = adapter
        .create_profile(&spec)
        .expect("create with extensions should succeed");
    assert!(p.path.join("Extensions").is_dir());
    assert!(p
        .path
        .join("Extensions")
        .join("my_extension")
        .join("manifest.json")
        .is_file());
}

#[test]
fn test_rename_display_name() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Profile 1", Some("Original Name"), true);
    let p_dir = fixture.user_data_root().join("Profile 1");
    fs::write(p_dir.join("data.txt"), b"important data").unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let snapshot = adapter.snapshot().unwrap();
    let profile = snapshot.find_by_directory("Profile 1").unwrap();

    adapter
        .rename_display_name(profile, "New Display Name")
        .expect("rename should succeed");

    let ls_path = fixture.user_data_root().join("Local State");
    let content = fs::read_to_string(&ls_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        parsed["profile"]["info_cache"]["Profile 1"]["name"],
        "New Display Name"
    );
    assert_eq!(
        parsed["profile"]["info_cache"]["Profile 1"]["is_using_default_name"],
        false
    );

    assert!(p_dir.is_dir());
    assert_eq!(fs::read(p_dir.join("data.txt")).unwrap(), b"important data");

    assert!(adapter.rename_display_name(profile, "").is_err());
    assert!(adapter.rename_display_name(profile, "   ").is_err());
}

#[test]
fn test_set_avatar() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Personal"), true);
    let adapter = ChromiumAdapter::new(fixture.install());
    let profile = adapter
        .snapshot()
        .unwrap()
        .find_by_directory("Default")
        .unwrap()
        .clone();

    let temp_img = fixture.user_data_root().join("source_avatar.png");
    fs::write(&temp_img, TINY_PNG).unwrap();
    let source_bytes = fs::read(&temp_img).unwrap();

    adapter
        .set_avatar(&profile, &temp_img)
        .expect("set avatar should succeed");

    assert_eq!(fs::read(&temp_img).unwrap(), source_bytes);
    let avatar_file = profile.path.join("Google Profile Picture.png");
    assert!(avatar_file.is_file());

    let ls_path = fixture.user_data_root().join("Local State");
    let content = fs::read_to_string(&ls_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let entry = &parsed["profile"]["info_cache"]["Default"];
    assert_eq!(entry["use_gaia_picture"], true);
    assert_eq!(
        entry["gaia_picture_file_name"],
        "Google Profile Picture.png"
    );
    assert_eq!(entry["is_using_default_avatar"], false);
}

#[test]
fn test_delete_sends_to_trash_and_updates_local_state() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("ToDelete", Some("To Delete"), true);
    let p_dir = fixture.user_data_root().join("ToDelete");
    let c_dir = fixture.cache_root().join("ToDelete");
    fs::create_dir_all(&c_dir).unwrap();
    fs::write(p_dir.join("cookie"), b"data").unwrap();
    fs::write(c_dir.join("cache.tmp"), b"cache").unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let profile = adapter
        .snapshot()
        .unwrap()
        .find_by_directory("ToDelete")
        .unwrap()
        .clone();

    let trash_dir = TempDir::new().unwrap();
    let fake_trash = FakeTrash::new(trash_dir.path());
    delete_profile_with(adapter.install(), &profile, &fake_trash).expect("delete should succeed");

    assert!(!p_dir.exists());
    assert!(!c_dir.exists());

    let ls_path = fixture.user_data_root().join("Local State");
    let content = fs::read_to_string(&ls_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["profile"]["info_cache"].get("ToDelete").is_none());
}

#[test]
fn test_clean_cache_removes_cache_and_preserves_user_data() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Profile 1", Some("User"), true);
    let p_dir = fixture.user_data_root().join("Profile 1");
    let ext_cache = fixture.cache_root().join("Profile 1");

    fs::create_dir_all(&ext_cache).unwrap();
    fs::write(ext_cache.join("ext.bin"), b"12345").unwrap();

    let cache_dirs = [
        "Cache",
        "Code Cache",
        "GPUCache",
        "ShaderCache",
        "GrShaderCache",
        "DawnCache",
        "DawnGraphiteCache",
        "DawnWebGPUCache",
        "component_crx_cache",
    ];
    for dir in cache_dirs {
        let sub = p_dir.join(dir);
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("data.bin"), b"12345").unwrap();
    }
    let sw_cache = p_dir.join("Service Worker").join("CacheStorage");
    fs::create_dir_all(&sw_cache).unwrap();
    fs::write(sw_cache.join("sw.bin"), b"12345").unwrap();

    let preserved = [
        "Cookies",
        "Login Data",
        "History",
        "Bookmarks",
        "Preferences",
    ];
    for name in preserved {
        fs::write(p_dir.join(name), b"critical user data").unwrap();
    }
    fs::create_dir_all(p_dir.join("Extensions")).unwrap();
    fs::write(p_dir.join("Extensions").join("ext.json"), b"{}").unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let profile = adapter
        .snapshot()
        .unwrap()
        .find_by_directory("Profile 1")
        .unwrap()
        .clone();
    assert_eq!(adapter.clean_cache(&profile).expect("clean cache"), 55);

    assert!(!ext_cache.exists());
    assert!(cache_dirs.iter().all(|d| !p_dir.join(d).exists()));
    assert!(!sw_cache.exists());
    assert!(preserved.iter().all(|n| p_dir.join(n).is_file()));
    assert!(p_dir.join("Extensions").is_dir());
}

#[test]
fn test_rename_directory() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile_entry(
        "OldDir",
        json!({
            "name": "Custom Profile",
            "avatar_icon": "chrome://theme/IDR_PROFILE_AVATAR_26",
            "active_time": 1700000000.0,
            "custom_flag": true,
        }),
        true,
    );
    let old_p_dir = fixture.user_data_root().join("OldDir");
    let old_c_dir = fixture.cache_root().join("OldDir");
    fs::create_dir_all(&old_c_dir).unwrap();
    fs::write(old_p_dir.join("file.txt"), b"profile file").unwrap();
    fs::write(old_c_dir.join("file.cache"), b"cache file").unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let profile = adapter
        .snapshot()
        .unwrap()
        .find_by_directory("OldDir")
        .unwrap()
        .clone();

    adapter
        .rename_profile_directory(&profile, "NewDir")
        .expect("rename dir should succeed");

    assert!(!old_p_dir.exists());
    assert!(!old_c_dir.exists());

    let new_p_dir = fixture.user_data_root().join("NewDir");
    let new_c_dir = fixture.cache_root().join("NewDir");
    assert!(new_p_dir.is_dir());
    assert!(new_c_dir.is_dir());
    assert_eq!(
        fs::read(new_p_dir.join("file.txt")).unwrap(),
        b"profile file"
    );

    let ls_path = fixture.user_data_root().join("Local State");
    let content = fs::read_to_string(&ls_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["profile"]["info_cache"].get("OldDir").is_none());

    let new_entry = &parsed["profile"]["info_cache"]["NewDir"];
    assert_eq!(new_entry["name"], "Custom Profile");
    assert_eq!(
        new_entry["avatar_icon"],
        "chrome://theme/IDR_PROFILE_AVATAR_26"
    );
    assert_eq!(new_entry["active_time"], 1700000000.0);
    assert_eq!(new_entry["custom_flag"], true);
}

#[test]
fn test_mid_operation_failure_rolls_back() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("TemplateCorrupted", Some("Corrupted"), true);
    let tpl_dir = fixture.user_data_root().join("TemplateCorrupted");
    fs::write(tpl_dir.join("Preferences"), b"INVALID NOT JSON DATA").unwrap();

    let before = snapshot_dir(fixture.user_data_root());

    let adapter = ChromiumAdapter::new(fixture.install());
    let mut spec = CreateProfileSpec::new("NewProfile");
    spec.template_directory = Some("TemplateCorrupted".to_string());

    assert!(adapter.create_profile(&spec).is_err());

    let after = snapshot_dir(fixture.user_data_root());
    assert_eq!(before, after);
}

#[test]
fn test_plan_functions_do_not_modify_disk() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Personal"), true);
    let img_path = fixture.user_data_root().join("avatar.png");
    fs::write(&img_path, TINY_PNG).unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let profile = adapter
        .snapshot()
        .unwrap()
        .find_by_directory("Default")
        .unwrap()
        .clone();

    let before = snapshot_dir(fixture.user_data_root());

    let create_spec = CreateProfileSpec::new("Plan Create");
    let clone_spec = CloneProfileSpec {
        display_name: "Plan Clone".to_string(),
        directory: None,
        avatar_source: None,
        clone_policy: ClonePolicy::default(),
        open_after_create: false,
    };

    adapter.plan_create(&create_spec).expect("plan create");
    adapter
        .plan_clone(&profile, &clone_spec)
        .expect("plan clone");
    adapter
        .plan_rename_directory(&profile, "NewDir")
        .expect("plan rename dir");
    adapter
        .plan_set_avatar(&profile, &img_path)
        .expect("plan set avatar");
    adapter
        .plan_delete(&profile, DeleteMode::Trash)
        .expect("plan delete");
    adapter
        .plan_clean_cache(&profile)
        .expect("plan clean cache");

    let after = snapshot_dir(fixture.user_data_root());
    assert_eq!(before, after);
}

#[test]
fn test_operation_refused_when_running() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Personal"), true);
    let img_path = fixture.user_data_root().join("avatar.png");
    fs::write(&img_path, TINY_PNG).unwrap();

    // Create SingletonLock to simulate running browser
    fs::write(fixture.user_data_root().join("SingletonLock"), b"pid").unwrap();

    let adapter = ChromiumAdapter::new(fixture.install());
    let profile = adapter
        .snapshot()
        .unwrap()
        .find_by_directory("Default")
        .unwrap()
        .clone();

    let before = snapshot_dir(fixture.user_data_root());

    let create_spec = CreateProfileSpec::new("Should Fail");
    assert!(adapter.create_profile(&create_spec).is_err());
    assert!(adapter.rename_display_name(&profile, "New Name").is_err());
    assert!(adapter
        .rename_profile_directory(&profile, "NewDir")
        .is_err());
    assert!(adapter.set_avatar(&profile, &img_path).is_err());
    assert!(adapter.delete_profile(&profile, DeleteMode::Trash).is_err());
    assert!(adapter.clean_cache(&profile).is_err());

    let after = snapshot_dir(fixture.user_data_root());
    assert_eq!(before, after);
}
