use std::path::PathBuf;

use profilemux::doctor::{analyze, overall_severity, summary_line};
use profilemux::domain::{
    BrowserInstall, BrowserInstallId, BrowserKind, BrowserProfile, Channel, HealthFinding,
    ProfileId, ProfileStoreSnapshot, Severity, SupportLevel,
};

fn make_install() -> BrowserInstall {
    let root = PathBuf::from("/mock/browser/userdata");
    let id = BrowserInstallId::new(BrowserKind::Chromium, &root);
    BrowserInstall {
        id,
        kind: BrowserKind::Chromium,
        name: "Mock Chromium".to_string(),
        channel: Channel::Stable,
        app_path: PathBuf::from("/mock/browser/app"),
        bundle_id: Some("org.chromium.mock".to_string()),
        version: Some("120.0.0.0".to_string()),
        user_data_root: root,
        cache_root: None,
        support: SupportLevel::Full,
    }
}

fn make_profile(
    install: &BrowserInstall,
    directory: &str,
    display_name: &str,
    directory_exists: bool,
) -> BrowserProfile {
    let path = install.user_data_root.join(directory);
    BrowserProfile {
        id: ProfileId::new(&install.id, directory),
        install_id: install.id.clone(),
        display_name: display_name.to_string(),
        directory: directory.to_string(),
        path,
        cache_path: None,
        avatar: None,
        last_active: None,
        registered: true,
        directory_exists,
        size: None,
    }
}

#[test]
fn healthy_snapshot_yields_empty_findings_and_ok_severity() {
    let install = make_install();
    let profile_a = make_profile(&install, "profile_a", "Workspace Alpha", true);
    let profile_b = make_profile(&install, "profile_b", "Workspace Beta", true);
    let snapshot = ProfileStoreSnapshot {
        registered: vec![profile_a, profile_b],
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert!(findings.is_empty());
    assert_eq!(overall_severity(&findings), Severity::Ok);
    assert_eq!(summary_line(&findings), "Healthy");
}

#[test]
fn finding_local_state_malformed() {
    let install = make_install();
    let snapshot = ProfileStoreSnapshot {
        registered: Vec::new(),
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: Some("Syntax error on line 42".to_string()),
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "local-state-malformed");
    assert_eq!(finding.severity, Severity::Broken);
    assert!(finding.message.contains("Syntax error on line 42"));
    assert_eq!(
        finding.paths,
        vec![install.user_data_root.join("Local State")]
    );
    assert_eq!(overall_severity(&findings), Severity::Broken);
}

#[test]
fn finding_profile_directory_missing() {
    let install = make_install();
    let missing_profile = make_profile(&install, "profile_missing", "Workspace Missing", false);
    let expected_path = missing_profile.path.clone();
    let snapshot = ProfileStoreSnapshot {
        registered: vec![missing_profile],
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "profile-directory-missing");
    assert_eq!(finding.severity, Severity::Broken);
    assert!(finding.message.contains("Workspace Missing"));
    assert!(finding.message.contains("profile_missing"));
    assert_eq!(finding.paths, vec![expected_path]);
    assert_eq!(overall_severity(&findings), Severity::Broken);
}

#[test]
fn finding_profile_unregistered() {
    let install = make_install();
    let unreg_dir = install.user_data_root.join("unknown_profile");
    let profile = make_profile(&install, "profile_a", "Workspace Alpha", true);
    let snapshot = ProfileStoreSnapshot {
        registered: vec![profile],
        unregistered_dirs: vec![unreg_dir.clone()],
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "profile-unregistered");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.paths, vec![unreg_dir]);
    assert_eq!(overall_severity(&findings), Severity::Warning);
}

#[test]
fn finding_cache_orphan() {
    let install = make_install();
    let orphan = PathBuf::from("/mock/browser/cache/orphan_entry");
    let profile = make_profile(&install, "profile_a", "Workspace Alpha", true);
    let snapshot = ProfileStoreSnapshot {
        registered: vec![profile],
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: vec![orphan.clone()],
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "cache-orphan");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.paths, vec![orphan]);
    assert_eq!(overall_severity(&findings), Severity::Warning);
}

#[test]
fn finding_duplicate_display_name() {
    let install = make_install();
    let p1 = make_profile(&install, "dir_1", "Testing Profile", true);
    let p2 = make_profile(&install, "dir_2", "Testing Profile", true);
    let mut expected_paths = vec![p1.path.clone(), p2.path.clone()];
    expected_paths.sort();

    let snapshot = ProfileStoreSnapshot {
        registered: vec![p1, p2],
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "duplicate-display-name");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.paths, expected_paths);
}

#[test]
fn duplicate_display_name_case_and_whitespace() {
    let install = make_install();
    let p1 = make_profile(&install, "dir_1", "  Developer Tools  ", true);
    let p2 = make_profile(&install, "dir_2", "developer tools", true);
    let mut expected_paths = vec![p1.path.clone(), p2.path.clone()];
    expected_paths.sort();

    let snapshot = ProfileStoreSnapshot {
        registered: vec![p1, p2],
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "duplicate-display-name");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.paths, expected_paths);
}

#[test]
fn duplicate_display_name_three_occurrences_yields_one_finding_three_paths() {
    let install = make_install();
    let p1 = make_profile(&install, "dir_1", "Workspace Gamma", true);
    let p2 = make_profile(&install, "dir_2", "  workspace gamma  ", true);
    let p3 = make_profile(&install, "dir_3", "WORKSPACE GAMMA", true);
    let mut expected_paths = vec![p1.path.clone(), p2.path.clone(), p3.path.clone()];
    expected_paths.sort();

    let snapshot = ProfileStoreSnapshot {
        registered: vec![p1, p2, p3],
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "duplicate-display-name");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.paths.len(), 3);
    assert_eq!(finding.paths, expected_paths);
}

#[test]
fn finding_duplicate_profile_path() {
    let install = make_install();
    let mut p1 = make_profile(&install, "dir_1", "Profile One", true);
    let mut p2 = make_profile(&install, "dir_2", "Profile Two", true);
    let shared_path = install.user_data_root.join("shared_dir");
    p1.path = shared_path.clone();
    p2.path = shared_path.clone();

    let snapshot = ProfileStoreSnapshot {
        registered: vec![p1, p2],
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "duplicate-profile-path");
    assert_eq!(finding.severity, Severity::Broken);
    assert_eq!(finding.paths, vec![shared_path]);
    assert_eq!(overall_severity(&findings), Severity::Broken);
}

#[test]
fn finding_no_profiles() {
    let install = make_install();
    let snapshot = ProfileStoreSnapshot {
        registered: Vec::new(),
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.code, "no-profiles");
    assert_eq!(finding.severity, Severity::Warning);
    assert!(finding.paths.is_empty());
}

#[test]
fn finding_stale_migration_artifact() {
    let install = make_install();
    let p1 = make_profile(&install, "profile_1.pmux-tmp", "Temp Artifact", true);
    let p2 = make_profile(&install, ".pmux-backup", "Prefix Artifact", true);
    let p1_path = p1.path.clone();
    let p2_path = p2.path.clone();

    let snapshot = ProfileStoreSnapshot {
        registered: vec![p1, p2],
        unregistered_dirs: Vec::new(),
        orphan_cache_dirs: Vec::new(),
        parse_error: None,
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 2);
    for finding in &findings {
        assert_eq!(finding.code, "stale-migration-artifact");
        assert_eq!(finding.severity, Severity::Warning);
    }
    assert_eq!(findings[0].paths, vec![p2_path]);
    assert_eq!(findings[1].paths, vec![p1_path]);
}

#[test]
fn ordering_deterministic_broken_before_warning() {
    let install = make_install();
    let unreg = install.user_data_root.join("unreg");
    let missing_prof = make_profile(&install, "missing_prof", "Missing Display", false);
    let orphan = PathBuf::from("/mock/cache/orphan");

    let snapshot = ProfileStoreSnapshot {
        registered: vec![missing_prof],
        unregistered_dirs: vec![unreg],
        orphan_cache_dirs: vec![orphan],
        parse_error: Some("corrupted file".to_string()),
    };

    let findings = analyze(&install, &snapshot);
    assert_eq!(findings.len(), 4);

    assert_eq!(findings[0].severity, Severity::Broken);
    assert_eq!(findings[1].severity, Severity::Broken);
    assert_eq!(findings[2].severity, Severity::Warning);
    assert_eq!(findings[3].severity, Severity::Warning);

    assert_eq!(findings[0].code, "local-state-malformed");
    assert_eq!(findings[1].code, "profile-directory-missing");
    assert_eq!(findings[2].code, "cache-orphan");
    assert_eq!(findings[3].code, "profile-unregistered");

    assert_eq!(overall_severity(&findings), Severity::Broken);
}

#[test]
fn summary_line_formatting() {
    let ok_finding = HealthFinding::new(Severity::Ok, "test-ok", "all good");
    let warn_1 = HealthFinding::new(Severity::Warning, "warn-1", "warn");
    let warn_2 = HealthFinding::new(Severity::Warning, "warn-2", "warn");
    let broken_1 = HealthFinding::new(Severity::Broken, "broken-1", "bad");
    let broken_2 = HealthFinding::new(Severity::Broken, "broken-2", "bad");

    assert_eq!(summary_line(&[]), "Healthy");
    assert_eq!(summary_line(&[ok_finding]), "Healthy");
    assert_eq!(summary_line(std::slice::from_ref(&warn_1)), "1 warning");
    assert_eq!(
        summary_line(&[warn_1.clone(), warn_2.clone()]),
        "2 warnings"
    );
    assert_eq!(summary_line(std::slice::from_ref(&broken_1)), "1 broken");
    assert_eq!(
        summary_line(&[broken_1.clone(), broken_2.clone()]),
        "2 broken"
    );
    assert_eq!(
        summary_line(&[warn_1.clone(), broken_1.clone()]),
        "1 warning, 1 broken"
    );
    assert_eq!(
        summary_line(&[warn_1, warn_2, broken_1]),
        "2 warnings, 1 broken"
    );
}
