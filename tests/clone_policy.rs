use std::fs;
use std::path::PathBuf;

use image::ImageFormat;
use profilemux::browsers::chromium::avatar::{self, AVATAR_FILE_NAME, MAX_DIMENSION};
use profilemux::browsers::chromium::clone::{
    copied_labels, excluded_labels, load_and_sanitize, plan_copy_items, sanitize_preferences,
    CopyItem,
};
use profilemux::domain::{ClonePolicy, ExtensionPolicy};
use profilemux::Error;
use serde_json::json;
use tempfile::tempdir;

#[test]
fn test_default_clone_policy_and_labels() {
    let policy = ClonePolicy::default();
    assert!(policy.copy_preferences);
    assert!(!policy.copy_bookmarks);
    assert_eq!(policy.extensions, ExtensionPolicy::None);

    let copied = copied_labels(&policy);
    assert_eq!(copied, vec!["Preferences"]);

    let excluded = excluded_labels(&policy);
    for expected in &["Extensions", "Cookies", "Login Data", "History", "Sessions"] {
        assert!(
            excluded.iter().any(|label| label == expected),
            "excluded_labels should mention {expected}, got: {excluded:?}"
        );
    }

    let tmp = tempdir().expect("tempdir");
    fs::write(tmp.path().join("Preferences"), b"{}").expect("write Preferences");
    fs::write(tmp.path().join("Bookmarks"), b"{}").expect("write Bookmarks");
    fs::create_dir(tmp.path().join("Extensions")).expect("create Extensions");

    let items = plan_copy_items(tmp.path(), &policy);
    assert_eq!(
        items,
        vec![CopyItem {
            relative: PathBuf::from("Preferences"),
            is_dir: false,
        }]
    );
}

#[test]
fn test_copy_extensions_present_and_absent() {
    let policy = ClonePolicy {
        copy_preferences: true,
        copy_bookmarks: false,
        extensions: ExtensionPolicy::CopyExtensions,
    };

    let tmp = tempdir().expect("tempdir");
    fs::write(tmp.path().join("Preferences"), b"{}").expect("write Preferences");

    let items_absent = plan_copy_items(tmp.path(), &policy);
    assert_eq!(
        items_absent,
        vec![CopyItem {
            relative: PathBuf::from("Preferences"),
            is_dir: false,
        }]
    );

    fs::create_dir(tmp.path().join("Extensions")).expect("create Extensions");
    let items_present = plan_copy_items(tmp.path(), &policy);
    assert_eq!(
        items_present,
        vec![
            CopyItem {
                relative: PathBuf::from("Preferences"),
                is_dir: false,
            },
            CopyItem {
                relative: PathBuf::from("Extensions"),
                is_dir: true,
            },
        ]
    );
}

#[test]
fn test_copy_extensions_and_settings() {
    let policy = ClonePolicy {
        copy_preferences: true,
        copy_bookmarks: true,
        extensions: ExtensionPolicy::CopyExtensionsAndSettings,
    };

    let tmp = tempdir().expect("tempdir");
    fs::write(tmp.path().join("Preferences"), b"{}").expect("write Preferences");
    fs::write(tmp.path().join("Bookmarks"), b"{}").expect("write Bookmarks");
    fs::create_dir(tmp.path().join("Extensions")).expect("create Extensions");
    fs::create_dir(tmp.path().join("Local Extension Settings")).expect("create Local Settings");
    fs::create_dir(tmp.path().join("Sync Extension Settings")).expect("create Sync Settings");

    let items = plan_copy_items(tmp.path(), &policy);
    assert_eq!(
        items,
        vec![
            CopyItem {
                relative: PathBuf::from("Preferences"),
                is_dir: false,
            },
            CopyItem {
                relative: PathBuf::from("Bookmarks"),
                is_dir: false,
            },
            CopyItem {
                relative: PathBuf::from("Extensions"),
                is_dir: true,
            },
            CopyItem {
                relative: PathBuf::from("Local Extension Settings"),
                is_dir: true,
            },
            CopyItem {
                relative: PathBuf::from("Sync Extension Settings"),
                is_dir: true,
            },
        ]
    );
}

#[test]
fn test_plan_copy_items_never_copies_sensitive_data() {
    let tmp = tempdir().expect("tempdir");
    fs::write(tmp.path().join("Preferences"), b"{}").expect("write Preferences");
    fs::write(tmp.path().join("Bookmarks"), b"{}").expect("write Bookmarks");
    fs::write(tmp.path().join("Cookies"), b"secret cookies").expect("write Cookies");
    fs::write(tmp.path().join("Login Data"), b"passwords").expect("write Login Data");
    fs::write(tmp.path().join("History"), b"history").expect("write History");
    fs::write(tmp.path().join("Web Data"), b"autofill web data").expect("write Web Data");
    fs::write(tmp.path().join("Network"), b"network state").expect("write Network");
    fs::create_dir(tmp.path().join("Sessions")).expect("create Sessions");
    fs::create_dir(tmp.path().join("Local Storage")).expect("create Local Storage");

    let policies = [
        ClonePolicy::default(),
        ClonePolicy {
            copy_preferences: true,
            copy_bookmarks: true,
            extensions: ExtensionPolicy::CopyExtensions,
        },
        ClonePolicy {
            copy_preferences: true,
            copy_bookmarks: true,
            extensions: ExtensionPolicy::CopyExtensionsAndSettings,
        },
    ];

    let forbidden = [
        "Cookies",
        "Login Data",
        "History",
        "Sessions",
        "Web Data",
        "Network",
        "Local Storage",
    ];

    for policy in &policies {
        let items = plan_copy_items(tmp.path(), policy);
        for item in &items {
            let rel_str = item.relative.to_str().expect("valid str");
            for bad in &forbidden {
                assert_ne!(
                    rel_str, *bad,
                    "Core privacy violation: {bad} must never be copied by plan_copy_items"
                );
            }
        }
    }
}

#[test]
fn test_sanitize_preferences_strips_sensitive_keys_and_preserves_appearance() {
    let input = json!({
        "account_info": [{ "gaia_id": "12345" }],
        "gaia_cookie": "cookie_value",
        "signin": { "allowed": true },
        "sync": { "has_setup_completed": true },
        "password_manager": { "leak_detection": true },
        "autofill": { "profile_enabled": true },
        "credentials_enable_service": false,
        "sessions": { "restore_on_startup": 1 },
        "safebrowsing": { "enabled": true },
        "protection": { "active": true },
        "media_router": { "cast": true },
        "dns_prefetching": { "enabled": true },
        "google": {
            "services": { "sync": true },
            "unrelated_feature": "keep_this"
        },
        "extensions": {
            "settings": { "ext_id_1": {} },
            "install_signature": "signature_data",
            "theme": "custom_extension_theme"
        },
        "ntp": {
            "custom_background_dict": { "url": "https://example.com/bg.png" },
            "num_personal_tiles": 5
        },
        "profile": {
            "content_settings": {
                "exceptions": { "cookies": {} },
                "pref_version": 1
            },
            "password_manager_enabled": false,
            "avatar_index": 26,
            "name": "Custom Profile"
        },
        "browser": {
            "last_known_google_url": "https://www.google.com",
            "show_home_button": true
        },
        "appearance": {
            "theme": "dark",
            "zoom_level": 1.2
        }
    });

    let input_clone = input.clone();
    let sanitized = sanitize_preferences(&input);

    assert_eq!(input, input_clone, "input must not be mutated");

    assert!(sanitized.get("account_info").is_none());
    assert!(sanitized.get("gaia_cookie").is_none());
    assert!(sanitized.get("signin").is_none());
    assert!(sanitized.get("sync").is_none());
    assert!(sanitized.get("password_manager").is_none());
    assert!(sanitized.get("autofill").is_none());
    assert!(sanitized.get("credentials_enable_service").is_none());
    assert!(sanitized.get("sessions").is_none());
    assert!(sanitized.get("safebrowsing").is_none());
    assert!(sanitized.get("protection").is_none());
    assert!(sanitized.get("media_router").is_none());
    assert!(sanitized.get("dns_prefetching").is_none());

    assert!(sanitized["google"].get("services").is_none());
    assert_eq!(sanitized["google"]["unrelated_feature"], "keep_this");

    assert!(sanitized["extensions"].get("settings").is_none());
    assert!(sanitized["extensions"].get("install_signature").is_none());
    assert_eq!(sanitized["extensions"]["theme"], "custom_extension_theme");

    assert!(sanitized["ntp"].get("custom_background_dict").is_none());
    assert_eq!(sanitized["ntp"]["num_personal_tiles"], 5);

    assert!(sanitized["profile"]["content_settings"]
        .get("exceptions")
        .is_none());
    assert_eq!(sanitized["profile"]["content_settings"]["pref_version"], 1);
    assert!(sanitized["profile"]
        .get("password_manager_enabled")
        .is_none());
    assert_eq!(sanitized["profile"]["avatar_index"], 26);
    assert_eq!(sanitized["profile"]["name"], "Custom Profile");

    assert!(sanitized["browser"].get("last_known_google_url").is_none());
    assert_eq!(sanitized["browser"]["show_home_button"], true);

    assert_eq!(sanitized["appearance"]["theme"], "dark");
    assert_eq!(sanitized["appearance"]["zoom_level"], 1.2);
}

#[test]
fn test_sanitize_preferences_non_object() {
    assert_eq!(sanitize_preferences(&json!([1, 2, 3])), json!({}));
    assert_eq!(sanitize_preferences(&json!("a string")), json!({}));
    assert_eq!(sanitize_preferences(&json!(123)), json!({}));
    assert_eq!(sanitize_preferences(&json!(true)), json!({}));
    assert_eq!(sanitize_preferences(&serde_json::Value::Null), json!({}));
}

#[test]
fn test_load_and_sanitize_valid_and_malformed() {
    let tmp = tempdir().expect("tempdir");
    let prefs_path = tmp.path().join("Preferences");

    let prefs_content = json!({
        "signin": { "allowed": true },
        "appearance": { "theme": "dark" }
    });
    fs::write(
        &prefs_path,
        serde_json::to_vec(&prefs_content).expect("serialize"),
    )
    .expect("write prefs");

    let bytes = load_and_sanitize(&prefs_path).expect("load_and_sanitize");
    let deserialized: serde_json::Value =
        serde_json::from_slice(&bytes).expect("deserialize sanitized bytes");

    assert!(deserialized.get("signin").is_none());
    assert_eq!(deserialized["appearance"]["theme"], "dark");

    let non_existent = tmp.path().join("DoesNotExist");
    assert!(matches!(
        load_and_sanitize(&non_existent),
        Err(Error::NotFound(_))
    ));

    let malformed_path = tmp.path().join("Malformed");
    fs::write(&malformed_path, b"not valid json {{{").expect("write malformed");
    assert!(matches!(
        load_and_sanitize(&malformed_path),
        Err(Error::Malformed { .. })
    ));

    let non_obj_path = tmp.path().join("NonObject");
    fs::write(&non_obj_path, b"[1, 2, 3]").expect("write non-object");
    assert!(matches!(
        load_and_sanitize(&non_obj_path),
        Err(Error::Malformed { .. })
    ));
}

#[test]
fn test_normalize_to_png_downscale_and_preserves_source() {
    let tmp = tempdir().expect("tempdir");
    let img_path = tmp.path().join("test_512x300.png");

    let img: image::RgbaImage = image::ImageBuffer::from_fn(512, 300, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 100, 255])
    });

    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut encoded, ImageFormat::Png)
        .expect("encode png");
    let original_bytes = encoded.into_inner();
    fs::write(&img_path, &original_bytes).expect("write original png");

    let normalized_bytes =
        avatar::normalize_to_png(&img_path, MAX_DIMENSION).expect("normalize_to_png");

    let current_bytes = fs::read(&img_path).expect("read source");
    assert_eq!(
        current_bytes, original_bytes,
        "source file must not be modified"
    );

    let decoded = image::load_from_memory(&normalized_bytes).expect("decode normalized png");
    assert!(
        decoded.width() <= MAX_DIMENSION,
        "width {} exceeds max {}",
        decoded.width(),
        MAX_DIMENSION
    );
    assert!(
        decoded.height() <= MAX_DIMENSION,
        "height {} exceeds max {}",
        decoded.height(),
        MAX_DIMENSION
    );
    assert_eq!(decoded.width(), 256);
    assert_eq!(decoded.height(), 150);
}

#[test]
fn test_normalize_to_png_does_not_upscale() {
    let tmp = tempdir().expect("tempdir");
    let img_path = tmp.path().join("test_64x64.png");

    let img: image::RgbaImage = image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgba([(x * 4) as u8, (y * 4) as u8, 150, 255])
    });

    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut encoded, ImageFormat::Png)
        .expect("encode png");
    fs::write(&img_path, encoded.into_inner()).expect("write png");

    let normalized_bytes =
        avatar::normalize_to_png(&img_path, MAX_DIMENSION).expect("normalize_to_png");

    let decoded = image::load_from_memory(&normalized_bytes).expect("decode normalized png");
    assert_eq!(decoded.width(), 64, "image should not have been upscaled");
    assert_eq!(decoded.height(), 64, "image should not have been upscaled");
}

#[test]
fn test_normalize_to_png_random_bytes_returns_error() {
    let tmp = tempdir().expect("tempdir");
    let bad_path = tmp.path().join("random.bin");
    fs::write(
        &bad_path,
        b"not an image file at all -- just random noise bytes",
    )
    .expect("write bad");

    let res = avatar::normalize_to_png(&bad_path, MAX_DIMENSION);
    assert!(
        matches!(res, Err(Error::Malformed { .. })),
        "expected Error::Malformed, got: {res:?}"
    );
}

#[test]
fn test_local_state_avatar_fields_and_clear() {
    let fields = avatar::local_state_avatar_fields();
    assert_eq!(
        fields,
        vec![
            (
                "gaia_picture_file_name",
                serde_json::Value::String(AVATAR_FILE_NAME.to_string())
            ),
            ("use_gaia_picture", serde_json::Value::Bool(true)),
            ("is_using_default_avatar", serde_json::Value::Bool(false)),
        ]
    );

    let clear = avatar::local_state_avatar_clear_fields();
    assert_eq!(
        clear,
        vec![
            ("gaia_picture_file_name", serde_json::Value::Null),
            ("use_gaia_picture", serde_json::Value::Bool(false)),
            ("is_using_default_avatar", serde_json::Value::Bool(true)),
        ]
    );
}
