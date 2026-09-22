mod common;

use std::collections::BTreeMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use common::ChromiumFixture;
use profilemux::browsers::chromium::ChromiumAdapter;
use profilemux::browsers::BrowserAdapter;
use profilemux::domain::{
    AppearanceSpec, BrowserInstallId, BrowserKind, BrowserProfile, BrowserTheme, OperationKind,
    WebDarkMode,
};
use profilemux::error::Error;
use serde_json::json;

fn brave_adapter(fixture: &ChromiumFixture) -> ChromiumAdapter {
    let mut install = fixture.install();
    install.id = BrowserInstallId::new(BrowserKind::Brave, &install.user_data_root);
    install.kind = BrowserKind::Brave;
    install.name = "Brave Test".to_string();
    ChromiumAdapter::new(install)
}

fn chromium_adapter(fixture: &ChromiumFixture) -> ChromiumAdapter {
    ChromiumAdapter::new(fixture.install())
}

fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut map = BTreeMap::new();
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
fn test_read_appearance_brave_darker_mode_and_color_scheme2() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let pref = json!({
        "browser": { "theme": { "color_scheme2": 2 } },
        "brave": { "darker_mode": true }
    });
    let bytes = serde_json::to_vec_pretty(&pref).unwrap();
    fs::write(profile_dir.join("Preferences"), bytes).unwrap();

    let brave = brave_adapter(&fixture);
    let chromium = chromium_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);

    let brave_app = brave.read_appearance(&profile).unwrap();
    assert_eq!(brave_app.theme, BrowserTheme::UltraDark);
    assert_eq!(brave_app.web_dark, WebDarkMode::Normal);

    let chromium_app = chromium.read_appearance(&profile).unwrap();
    assert_eq!(chromium_app.theme, BrowserTheme::Dark);
    assert_eq!(chromium_app.web_dark, WebDarkMode::Normal);
}

#[test]
fn test_read_appearance_darker_mode_false_or_absent() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);

    let pref_false = json!({
        "browser": { "theme": { "color_scheme2": 2 } },
        "brave": { "darker_mode": false }
    });
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_vec_pretty(&pref_false).unwrap(),
    )
    .unwrap();
    let app = brave.read_appearance(&profile).unwrap();
    assert_eq!(app.theme, BrowserTheme::Dark);

    let pref_absent = json!({
        "browser": { "theme": { "color_scheme2": 2 } }
    });
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_vec_pretty(&pref_absent).unwrap(),
    )
    .unwrap();
    let app2 = brave.read_appearance(&profile).unwrap();
    assert_eq!(app2.theme, BrowserTheme::Dark);
}

#[test]
fn test_read_appearance_color_scheme2_variants() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);

    let pref_absent = json!({});
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_vec_pretty(&pref_absent).unwrap(),
    )
    .unwrap();
    assert_eq!(
        brave.read_appearance(&profile).unwrap().theme,
        BrowserTheme::System
    );

    let pref_zero = json!({
        "browser": { "theme": { "color_scheme2": 0 } }
    });
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_vec_pretty(&pref_zero).unwrap(),
    )
    .unwrap();
    assert_eq!(
        brave.read_appearance(&profile).unwrap().theme,
        BrowserTheme::System
    );

    let pref_one = json!({
        "browser": { "theme": { "color_scheme2": 1 } }
    });
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_vec_pretty(&pref_one).unwrap(),
    )
    .unwrap();
    assert_eq!(
        brave.read_appearance(&profile).unwrap().theme,
        BrowserTheme::Light
    );
}

#[test]
fn test_read_appearance_malformed_preferences() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);

    fs::write(profile_dir.join("Preferences"), b"{ bad json").unwrap();
    let app = brave.read_appearance(&profile).unwrap();
    assert_eq!(app.theme, BrowserTheme::Unknown);
    assert_eq!(app.web_dark, WebDarkMode::Normal);

    fs::remove_file(profile_dir.join("Preferences")).unwrap();
    let app_missing = brave.read_appearance(&profile).unwrap();
    assert_eq!(app_missing.theme, BrowserTheme::Unknown);

    let missing_profile = BrowserProfile {
        directory: "Ghost".to_string(),
        path: fixture.user_data_root().join("Ghost"),
        ..profile
    };
    let err = brave.read_appearance(&missing_profile);
    assert!(matches!(err, Err(Error::NotFound(_))));
}

#[test]
fn test_apply_ultra_dark_preserves_unrelated_keys() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let initial_pref = json!({
        "account_id": "12345",
        "browser": {
            "unrelated": true,
            "theme": { "color_scheme": 1, "other": 99 }
        }
    });
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_vec_pretty(&initial_pref).unwrap(),
    )
    .unwrap();

    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);
    let spec = AppearanceSpec {
        theme: Some(BrowserTheme::UltraDark),
        web_dark: None,
    };
    brave.set_appearance(&profile, &spec).unwrap();

    let content = fs::read_to_string(profile_dir.join("Preferences")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["browser"]["theme"]["color_scheme2"], json!(2));
    assert_eq!(parsed["brave"]["darker_mode"], json!(true));
    assert_eq!(parsed["browser"]["theme"]["color_scheme"], json!(2));
    assert_eq!(parsed["account_id"], initial_pref["account_id"]);
    assert_eq!(
        parsed["browser"]["unrelated"],
        initial_pref["browser"]["unrelated"]
    );
    assert_eq!(
        parsed["browser"]["theme"]["other"],
        initial_pref["browser"]["theme"]["other"]
    );
}

#[test]
fn test_apply_system_writes_keys() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let initial_pref = json!({
        "browser": { "theme": { "color_scheme2": 2 } },
        "brave": { "darker_mode": true }
    });
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_vec_pretty(&initial_pref).unwrap(),
    )
    .unwrap();

    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);
    let spec = AppearanceSpec {
        theme: Some(BrowserTheme::System),
        web_dark: None,
    };
    brave.set_appearance(&profile, &spec).unwrap();

    let content = fs::read_to_string(profile_dir.join("Preferences")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["browser"]["theme"]["color_scheme2"], json!(0));
    assert_eq!(parsed["brave"]["darker_mode"], json!(false));
}

#[test]
fn test_apply_ultra_dark_refused_on_non_brave() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let initial_pref = json!({
        "browser": { "theme": { "color_scheme2": 0 } }
    });
    let initial_bytes = serde_json::to_vec_pretty(&initial_pref).unwrap();
    fs::write(profile_dir.join("Preferences"), &initial_bytes).unwrap();

    let chromium = chromium_adapter(&fixture);
    let profile = chromium.list_profiles().unwrap().remove(0);
    let spec = AppearanceSpec {
        theme: Some(BrowserTheme::UltraDark),
        web_dark: None,
    };

    assert!(chromium.set_appearance(&profile, &spec).is_err());
    let current_bytes = fs::read(profile_dir.join("Preferences")).unwrap();
    assert_eq!(current_bytes, initial_bytes);
}

#[test]
fn test_write_failure_leaves_file_byte_identical() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let pref_path = profile_dir.join("Preferences");
    let initial_pref = json!({
        "browser": { "theme": { "color_scheme2": 0 } }
    });
    let initial_bytes = serde_json::to_vec_pretty(&initial_pref).unwrap();
    fs::write(&pref_path, &initial_bytes).unwrap();

    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&pref_path).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&pref_path, perms).unwrap();
    }

    let spec = AppearanceSpec {
        theme: Some(BrowserTheme::Dark),
        web_dark: None,
    };
    assert!(brave.set_appearance(&profile, &spec).is_err());

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&pref_path).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&pref_path, perms).unwrap();
    }

    let current_bytes = fs::read(&pref_path).unwrap();
    assert_eq!(current_bytes, initial_bytes);
}

#[test]
fn test_plan_changes_nothing_on_disk() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let initial_pref = json!({
        "browser": { "theme": { "color_scheme2": 0 } }
    });
    fs::write(
        profile_dir.join("Preferences"),
        serde_json::to_vec_pretty(&initial_pref).unwrap(),
    )
    .unwrap();

    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);
    let before = snapshot_tree(fixture.user_data_root());

    let spec = AppearanceSpec {
        theme: Some(BrowserTheme::UltraDark),
        web_dark: Some(WebDarkMode::ForceDark),
    };
    let plan = brave.plan_set_appearance(&profile, &spec).unwrap();
    assert_eq!(plan.kind, OperationKind::SetAppearance);
    assert!(plan.requires_browser_closed);
    assert!(plan
        .paths_affected
        .contains(&profile_dir.join("Preferences")));

    let after = snapshot_tree(fixture.user_data_root());
    assert_eq!(before, after);
}

#[test]
fn test_attempt_while_singleton_lock_present_refused() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let profile_dir = fixture.user_data_root().join("Default");
    let initial_pref = json!({
        "browser": { "theme": { "color_scheme2": 0 } }
    });
    let initial_bytes = serde_json::to_vec_pretty(&initial_pref).unwrap();
    fs::write(profile_dir.join("Preferences"), &initial_bytes).unwrap();

    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);

    let lock_path = fixture.user_data_root().join("SingletonLock");
    fs::write(&lock_path, b"").unwrap();

    let spec = AppearanceSpec {
        theme: Some(BrowserTheme::Dark),
        web_dark: None,
    };
    assert!(brave.set_appearance(&profile, &spec).is_err());

    let current_bytes = fs::read(profile_dir.join("Preferences")).unwrap();
    assert_eq!(current_bytes, initial_bytes);
}

#[test]
fn test_plan_steps_and_validation() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Default", Some("Default"), true);
    let brave = brave_adapter(&fixture);
    let profile = brave.list_profiles().unwrap().remove(0);

    assert!(brave
        .plan_set_appearance(&profile, &AppearanceSpec::default())
        .is_err());

    let light_spec = AppearanceSpec {
        theme: Some(BrowserTheme::Light),
        web_dark: None,
    };
    assert!(brave.plan_set_appearance(&profile, &light_spec).is_err());

    let ultra_spec = AppearanceSpec {
        theme: Some(BrowserTheme::UltraDark),
        web_dark: Some(WebDarkMode::ForceDark),
    };
    let plan = brave.plan_set_appearance(&profile, &ultra_spec).unwrap();
    assert!(plan
        .steps
        .iter()
        .any(|s| s.description.contains("color_scheme2 = 2")));
    assert!(plan
        .steps
        .iter()
        .any(|s| s.description.contains("brave.darker_mode = true")));
    assert!(plan
        .steps
        .iter()
        .any(|s| s.description.contains("launch time")));
}

/// Regression test for a defect found during live validation: writing only
/// `browser.theme.color_scheme2` is not enough, because a profile that still
/// follows the system colours has its scheme recomputed and reset by the
/// browser on the next launch.
#[test]
fn explicit_theme_opts_out_of_following_system_colours() {
    let mut fixture = ChromiumFixture::new();
    fixture.add_registered_profile("Themed", Some("Themed"), true);
    let install = fixture.install();
    let adapter = ChromiumAdapter::new(install.clone());
    let profile = adapter
        .snapshot()
        .expect("snapshot")
        .find_by_directory("Themed")
        .expect("profile")
        .clone();

    // Ultra Dark is Brave-only and is covered by its own tests; the point here is
    // the follows_system_colors opt-out, which applies to every Chromium build.
    for (theme, expect_follows) in [(BrowserTheme::Dark, false), (BrowserTheme::System, true)] {
        adapter
            .set_appearance(
                &profile,
                &AppearanceSpec {
                    theme: Some(theme),
                    web_dark: None,
                },
            )
            .expect("set theme");
        let doc: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(profile.path.join("Preferences")).expect("read prefs"),
        )
        .expect("parse prefs");
        assert_eq!(
            doc.pointer("/browser/theme/follows_system_colors")
                .and_then(|v| v.as_bool()),
            Some(expect_follows),
            "{theme:?} must set follows_system_colors to {expect_follows}"
        );
    }
}
