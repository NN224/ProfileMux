use std::collections::HashMap;

use profilemux::update::asset::{asset_name_for_arch, expected_asset_names, select_asset};
use profilemux::update::source::{FixtureReleaseSource, ReleaseAsset, ReleaseInfo};
use profilemux::update::verify::{sha256_hex, verify_sha256};
use profilemux::update::{check_update, compare, parse_version, UpdateStatus};
use semver::Version;

#[test]
fn update_status_equal_version_is_up_to_date() {
    let current = Version::parse("1.1.0").unwrap();
    let release = ReleaseInfo {
        tag: "v1.1.0".to_string(),
        version: current.clone(),
        assets: vec![],
    };
    let source = FixtureReleaseSource {
        release,
        payloads: HashMap::new(),
        fail_with: None,
    };
    let status = check_update(&source, &current).unwrap();
    assert_eq!(status, UpdateStatus::UpToDate { current });
}

#[test]
fn update_status_newer_version_is_available() {
    let current = Version::parse("1.1.0").unwrap();
    let latest = Version::parse("1.2.0").unwrap();
    let release = ReleaseInfo {
        tag: "v1.2.0".to_string(),
        version: latest.clone(),
        assets: vec![],
    };
    let source = FixtureReleaseSource {
        release,
        payloads: HashMap::new(),
        fail_with: None,
    };
    let status = check_update(&source, &current).unwrap();
    assert_eq!(status, UpdateStatus::Available { current, latest });
}

#[test]
fn update_status_older_version_is_up_to_date() {
    let current = Version::parse("1.1.0").unwrap();
    let latest = Version::parse("1.0.0").unwrap();
    let release = ReleaseInfo {
        tag: "v1.0.0".to_string(),
        version: latest,
        assets: vec![],
    };
    let source = FixtureReleaseSource {
        release,
        payloads: HashMap::new(),
        fail_with: None,
    };
    let status = check_update(&source, &current).unwrap();
    assert_eq!(status, UpdateStatus::UpToDate { current });
}

#[test]
fn compare_is_semantic_not_lexicographic() {
    let v_1_9_0 = Version::parse("1.9.0").unwrap();
    let v_1_10_0 = Version::parse("1.10.0").unwrap();
    let status = compare(&v_1_9_0, &v_1_10_0);
    assert_eq!(
        status,
        UpdateStatus::Available {
            current: v_1_9_0.clone(),
            latest: v_1_10_0.clone(),
        }
    );

    let status_rev = compare(&v_1_10_0, &v_1_9_0);
    assert_eq!(status_rev, UpdateStatus::UpToDate { current: v_1_10_0 });
}

#[test]
fn parse_version_with_leading_v() {
    let parsed = parse_version("v1.2.0").unwrap();
    assert_eq!(parsed, Version::new(1, 2, 0));
}

#[test]
fn parse_version_malformed_returns_error() {
    let err = parse_version("release-two").unwrap_err();
    assert!(err.to_string().contains("malformed version"));
}

#[test]
fn check_update_surfaces_source_error() {
    let current = Version::parse("1.1.0").unwrap();
    let source = FixtureReleaseSource {
        release: ReleaseInfo {
            tag: "v1.1.0".to_string(),
            version: current.clone(),
            assets: vec![],
        },
        payloads: HashMap::new(),
        fail_with: Some("connection refused".to_string()),
    };
    let err = check_update(&source, &current).unwrap_err();
    assert!(err.to_string().contains("connection refused"));
}

#[test]
fn asset_selection_supported_architectures() {
    let arm_asset = ReleaseAsset {
        name: "pmux-macos-aarch64".to_string(),
        download_url: "https://example.com/aarch64".to_string(),
        size: 1024,
    };
    let x86_asset = ReleaseAsset {
        name: "pmux-macos-x86_64".to_string(),
        download_url: "https://example.com/x86_64".to_string(),
        size: 2048,
    };
    let assets = vec![arm_asset.clone(), x86_asset.clone()];

    let selected_arm = select_asset(&assets, "aarch64").unwrap();
    assert_eq!(selected_arm, &arm_asset);

    let selected_x86 = select_asset(&assets, "x86_64").unwrap();
    assert_eq!(selected_x86, &x86_asset);
}

#[test]
fn asset_selection_unsupported_arch_errors() {
    let assets = vec![ReleaseAsset {
        name: "pmux-macos-aarch64".to_string(),
        download_url: "https://example.com/aarch64".to_string(),
        size: 1024,
    }];
    let err = select_asset(&assets, "powerpc").unwrap_err();
    assert!(err.to_string().contains("powerpc"));

    let err_arch = asset_name_for_arch("powerpc").unwrap_err();
    assert!(err_arch.to_string().contains("powerpc"));
}

#[test]
fn asset_selection_missing_and_duplicate_errors() {
    let other_asset = ReleaseAsset {
        name: "other-tool".to_string(),
        download_url: "https://example.com/other".to_string(),
        size: 100,
    };
    let err_missing = select_asset(&[other_asset], "aarch64").unwrap_err();
    assert!(err_missing.to_string().contains("pmux-macos-aarch64"));

    let arm_1 = ReleaseAsset {
        name: "pmux-macos-aarch64".to_string(),
        download_url: "https://example.com/arm1".to_string(),
        size: 1000,
    };
    let arm_2 = ReleaseAsset {
        name: "pmux-macos-aarch64".to_string(),
        download_url: "https://example.com/arm2".to_string(),
        size: 1000,
    };
    let err_dup = select_asset(&[arm_1, arm_2], "aarch64").unwrap_err();
    assert!(err_dup.to_string().contains("multiple"));
}

#[test]
fn expected_asset_names_contains_both() {
    let names = expected_asset_names();
    assert_eq!(names, vec!["pmux-macos-aarch64", "pmux-macos-x86_64"]);
}

#[test]
fn verify_sha256_scenarios() {
    let data = b"hello profilemux";
    let digest = sha256_hex(data);

    assert!(verify_sha256(data, &digest).is_ok());
    assert!(verify_sha256(data, &digest.to_uppercase()).is_ok());

    let shasum_line = format!("{digest}  pmux-macos-aarch64");
    assert!(verify_sha256(data, &shasum_line).is_ok());

    let wrong = "0000000000000000000000000000000000000000000000000000000000000000";
    let err_wrong = verify_sha256(data, wrong).unwrap_err();
    let err_msg = err_wrong.to_string();
    assert!(err_msg.contains(wrong));
    assert!(err_msg.contains(&digest));

    let truncated = "abcdef";
    assert!(verify_sha256(data, truncated).is_err());

    let non_hex = "g".repeat(64);
    assert!(verify_sha256(data, &non_hex).is_err());

    assert!(verify_sha256(data, "").is_err());
}
