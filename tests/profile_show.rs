use std::path::PathBuf;

use profilemux::cli::commands::{
    account_email_display, build_profile_detail_lines, format_profile_details,
};
use profilemux::domain::{
    AvatarInfo, BrowserInstallId, BrowserKind, BrowserProfile, ProfileId, StorageBreakdown,
};

fn create_handbuilt_profile(account_email: Option<String>) -> BrowserProfile {
    let base_path = PathBuf::from("/test/browser");
    let install_id = BrowserInstallId::new(BrowserKind::Chromium, &base_path);
    let profile_id = ProfileId::new(&install_id, "Default");
    BrowserProfile {
        id: profile_id,
        install_id,
        display_name: "Test Profile".to_string(),
        directory: "Default".to_string(),
        path: base_path.join("Default"),
        cache_path: Some(base_path.join("Cache")),
        avatar: Some(AvatarInfo {
            icon: Some("chrome://theme/IDR_PROFILE_AVATAR_26".to_string()),
            picture_file: None,
            uses_picture: false,
            is_default: true,
        }),
        account_email,
        last_active: Some(1700000000),
        registered: true,
        directory_exists: true,
        size: Some(StorageBreakdown {
            core: 1024,
            cache: 2048,
            code_cache: 512,
            gpu_cache: 256,
            total: 3840,
        }),
    }
}

#[test]
fn test_account_email_some_renders_address() {
    let email = "person@example.com";
    let profile = create_handbuilt_profile(Some(email.to_string()));
    let lines = format_profile_details(&profile, "Chromium (Stable)", false);

    assert_eq!(account_email_display(&profile), email);
    let email_line = lines
        .iter()
        .find(|l| l.contains("Account Email:"))
        .expect("Account Email: line should be present");
    assert!(email_line.contains(email));
    assert_eq!(email_line, &format!("Account Email:     {email}"));
}

#[test]
fn test_account_email_none_renders_not_signed_in() {
    let profile = create_handbuilt_profile(None);
    let lines = format_profile_details(&profile, "Chromium (Stable)", false);

    assert_eq!(account_email_display(&profile), "Not signed in");
    let email_line = lines
        .iter()
        .find(|l| l.contains("Account Email:"))
        .expect("Account Email: line should be present");
    assert!(email_line.contains("Not signed in"));
    assert_eq!(email_line, "Account Email:     Not signed in");
}

#[test]
fn test_account_email_line_ordering_between_avatar_and_last_active() {
    let profile = create_handbuilt_profile(Some("person@example.com".to_string()));
    let lines = format_profile_details(&profile, "Chromium (Stable)", false);

    let avatar_idx = lines
        .iter()
        .position(|l| l.starts_with("Avatar:"))
        .expect("Avatar: line should be present");
    let email_idx = lines
        .iter()
        .position(|l| l.starts_with("Account Email:"))
        .expect("Account Email: line should be present");
    let last_active_idx = lines
        .iter()
        .position(|l| l.starts_with("Last Active:"))
        .expect("Last Active: line should be present");

    assert_eq!(avatar_idx + 1, email_idx);
    assert_eq!(email_idx + 1, last_active_idx);
}

#[test]
fn test_every_label_line_shares_same_value_column() {
    let profile = create_handbuilt_profile(Some("person@example.com".to_string()));
    let lines = format_profile_details(&profile, "Chromium (Stable)", false);

    let value_columns: Vec<usize> = lines
        .iter()
        .filter(|line| *line != "Sizes:")
        .map(|line| {
            let colon_idx = line.find(':').expect("label line must contain colon");
            let after_colon = &line[colon_idx + 1..];
            let space_count = after_colon.len() - after_colon.trim_start().len();
            colon_idx + 1 + space_count
        })
        .collect();

    assert!(!value_columns.is_empty());
    let expected_col = value_columns[0];
    assert_eq!(expected_col, 19, "value column should start at index 19");
    for col in value_columns {
        assert_eq!(col, expected_col, "column alignment regressed");
    }
}

#[test]
fn test_detail_output_excludes_sensitive_strings() {
    let profile = create_handbuilt_profile(Some("person@example.com".to_string()));
    let lines = format_profile_details(&profile, "Chromium (Stable)", false);

    let forbidden_strings = ["gaia_id", "Login Data", "Cookies", "password"];
    for line in &lines {
        for forbidden in &forbidden_strings {
            assert!(
                !line.contains(forbidden),
                "detail output contains forbidden string '{forbidden}': {line}"
            );
        }
    }
}

#[test]
fn test_build_profile_detail_lines_helper() {
    let profile = create_handbuilt_profile(None);
    let lines = build_profile_detail_lines(&profile);
    assert!(lines
        .iter()
        .any(|l| l == "Account Email:     Not signed in"));
}
