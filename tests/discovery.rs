use std::collections::HashSet;
use std::path::PathBuf;

use profilemux::browsers::chromium::discovery::{
    dedup_and_sort_installs, match_bundle, DEFINITIONS,
};
use profilemux::domain::{BrowserInstall, BrowserInstallId, BrowserKind, Channel, SupportLevel};

#[test]
fn test_match_bundle_brave() {
    let stable = match_bundle("com.brave.Browser").expect("should match brave stable");
    assert_eq!(stable.kind, BrowserKind::Brave);
    assert_eq!(stable.channel, Channel::Stable);
    assert_ne!(stable.kind, BrowserKind::BraveBeta);

    let beta = match_bundle("com.brave.Browser.beta").expect("should match brave beta");
    assert_eq!(beta.kind, BrowserKind::BraveBeta);
    assert_eq!(beta.channel, Channel::Beta);
}

#[test]
fn test_match_bundle_edge_and_edgemark() {
    assert!(match_bundle("io.github.ender-wang.EdgeMark").is_none());

    let edge = match_bundle("com.microsoft.edgemac").expect("should match edge stable");
    assert_eq!(edge.kind, BrowserKind::Edge);
    assert_eq!(edge.channel, Channel::Stable);

    let edge_beta = match_bundle("com.microsoft.edgemac.Beta").expect("should match edge beta");
    assert_eq!(edge_beta.kind, BrowserKind::EdgeBeta);
    assert_eq!(edge_beta.channel, Channel::Beta);
}

#[test]
fn test_match_bundle_unknown() {
    assert!(match_bundle("com.unknown.browser").is_none());
    assert!(match_bundle("org.mozilla.firefox").is_none());
    assert!(match_bundle("").is_none());
}

#[test]
fn test_match_bundle_case_insensitive() {
    let chrome = match_bundle("COM.GOOGLE.CHROME").expect("should match case-insensitively");
    assert_eq!(chrome.kind, BrowserKind::Chrome);
    assert_eq!(chrome.channel, Channel::Stable);

    let edge = match_bundle("com.microsoft.EDGEMAC.beta").expect("should match case-insensitively");
    assert_eq!(edge.kind, BrowserKind::EdgeBeta);
    assert_eq!(edge.channel, Channel::Beta);
}

#[test]
fn test_no_duplicate_bundle_ids_or_kinds() {
    let mut seen_ids = HashSet::new();
    let mut seen_kinds = HashSet::new();

    for def in DEFINITIONS {
        let lower_id = def.bundle_id.to_ascii_lowercase();
        assert!(
            seen_ids.insert(lower_id),
            "Duplicate bundle ID found: {}",
            def.bundle_id
        );
        assert!(
            seen_kinds.insert(def.kind),
            "Duplicate BrowserKind found: {:?}",
            def.kind
        );
    }
}

#[test]
fn test_dedup_prefers_applications_over_home() {
    let data_root = PathBuf::from("/Users/test/Library/Application Support/Google/Chrome");
    let id = BrowserInstallId::new(BrowserKind::Chrome, &data_root);

    let sys_install = BrowserInstall {
        id: id.clone(),
        kind: BrowserKind::Chrome,
        name: "Google Chrome".to_string(),
        channel: Channel::Stable,
        app_path: PathBuf::from("/Applications/Google Chrome.app"),
        bundle_id: Some("com.google.Chrome".to_string()),
        version: Some("120.0".to_string()),
        user_data_root: data_root.clone(),
        cache_root: None,
        support: SupportLevel::ReadOnly,
    };

    let user_install = BrowserInstall {
        id: id.clone(),
        kind: BrowserKind::Chrome,
        name: "Google Chrome".to_string(),
        channel: Channel::Stable,
        app_path: PathBuf::from("/Users/test/Applications/Google Chrome.app"),
        bundle_id: Some("com.google.Chrome".to_string()),
        version: Some("120.0".to_string()),
        user_data_root: data_root.clone(),
        cache_root: None,
        support: SupportLevel::ReadOnly,
    };

    let res1 = dedup_and_sort_installs(vec![user_install.clone(), sys_install.clone()]);
    assert_eq!(res1.len(), 1);
    assert_eq!(
        res1[0].app_path,
        PathBuf::from("/Applications/Google Chrome.app")
    );

    let res2 = dedup_and_sort_installs(vec![sys_install, user_install]);
    assert_eq!(res2.len(), 1);
    assert_eq!(
        res2[0].app_path,
        PathBuf::from("/Applications/Google Chrome.app")
    );
}

#[test]
#[ignore]
fn test_live_discovery() {
    let installs = profilemux::browsers::chromium::discovery::discover_installs();
    for install in &installs {
        assert!(install.app_path.exists());
        assert!(install.user_data_root.exists());
        assert_eq!(install.support, SupportLevel::ReadOnly);
    }
}
