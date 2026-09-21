use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::domain::{
    BrowserInstall, BrowserProfile, HealthFinding, ProfileStoreSnapshot, Severity,
};

/// Inspects a browser installation's profile store snapshot and generates read-only health findings.
pub fn analyze(install: &BrowserInstall, snapshot: &ProfileStoreSnapshot) -> Vec<HealthFinding> {
    let mut findings: Vec<HealthFinding> =
        check_local_state(install, snapshot.parse_error.as_deref())
            .into_iter()
            .chain(check_missing_directories(&snapshot.registered))
            .chain(check_unregistered_dirs(&snapshot.unregistered_dirs))
            .chain(check_orphan_caches(&snapshot.orphan_cache_dirs))
            .chain(check_duplicate_display_names(&snapshot.registered))
            .chain(check_duplicate_profile_paths(&snapshot.registered))
            .chain(check_no_profiles(snapshot))
            .chain(check_stale_migration_artifacts(&snapshot.registered))
            .collect();

    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.paths.first().cmp(&b.paths.first()))
            .then_with(|| a.message.cmp(&b.message))
    });

    findings
}

/// Returns the maximum severity among findings, or `Severity::Ok` when findings is empty.
pub fn overall_severity(findings: &[HealthFinding]) -> Severity {
    findings
        .iter()
        .fold(Severity::Ok, |max_sev, f| max_sev.max(f.severity))
}

/// Formats a concise summary line, e.g. "2 warnings, 1 broken" or "Healthy".
pub fn summary_line(findings: &[HealthFinding]) -> String {
    let mut warnings = 0usize;
    let mut broken = 0usize;

    for f in findings {
        match f.severity {
            Severity::Broken => broken += 1,
            Severity::Warning => warnings += 1,
            Severity::Ok => {}
        }
    }

    match (warnings, broken) {
        (0, 0) => "Healthy".to_string(),
        (w, 0) => {
            let noun = if w == 1 { "warning" } else { "warnings" };
            format!("{w} {noun}")
        }
        (0, b) => format!("{b} broken"),
        (w, b) => {
            let noun = if w == 1 { "warning" } else { "warnings" };
            format!("{w} {noun}, {b} broken")
        }
    }
}

fn check_local_state(install: &BrowserInstall, parse_error: Option<&str>) -> Option<HealthFinding> {
    parse_error.map(|err| {
        let path = install.user_data_root.join("Local State");
        let message = if err.starts_with("Local State") {
            err.to_string()
        } else {
            format!("Local State malformed: {err}")
        };
        HealthFinding::new(Severity::Broken, "local-state-malformed", message)
            .with_paths(vec![path])
    })
}

fn check_missing_directories<'a>(
    profiles: &'a [BrowserProfile],
) -> impl Iterator<Item = HealthFinding> + 'a {
    profiles.iter().filter(|p| !p.directory_exists).map(|p| {
        let message = format!(
            "Profile '{}' ({}) directory does not exist",
            p.display_name, p.directory
        );
        HealthFinding::new(Severity::Broken, "profile-directory-missing", message)
            .with_paths(vec![p.path.clone()])
    })
}

fn check_unregistered_dirs<'a>(
    unregistered: &'a [PathBuf],
) -> impl Iterator<Item = HealthFinding> + 'a {
    unregistered.iter().map(|dir| {
        let message = format!("Unregistered profile directory: {}", dir.display());
        HealthFinding::new(Severity::Warning, "profile-unregistered", message)
            .with_paths(vec![dir.clone()])
    })
}

fn check_orphan_caches<'a>(
    orphan_caches: &'a [PathBuf],
) -> impl Iterator<Item = HealthFinding> + 'a {
    orphan_caches.iter().map(|dir| {
        let message = format!("Orphan cache directory: {}", dir.display());
        HealthFinding::new(Severity::Warning, "cache-orphan", message).with_paths(vec![dir.clone()])
    })
}

fn check_duplicate_display_names(profiles: &[BrowserProfile]) -> Vec<HealthFinding> {
    let mut groups: BTreeMap<String, Vec<&BrowserProfile>> = BTreeMap::new();
    for p in profiles {
        let key = p.display_name.trim().to_lowercase();
        groups.entry(key).or_default().push(p);
    }

    groups
        .into_values()
        .filter(|colliding| colliding.len() > 1)
        .map(|colliding| {
            let display_name = colliding.first().map_or("", |p| p.display_name.trim());
            let mut paths: Vec<PathBuf> = colliding.iter().map(|p| p.path.clone()).collect();
            paths.sort();
            HealthFinding::new(
                Severity::Warning,
                "duplicate-display-name",
                format!("Duplicate profile display name '{display_name}'"),
            )
            .with_paths(paths)
        })
        .collect()
}

fn check_duplicate_profile_paths(profiles: &[BrowserProfile]) -> Vec<HealthFinding> {
    let mut counts: BTreeMap<&PathBuf, usize> = BTreeMap::new();
    for p in profiles {
        *counts.entry(&p.path).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(path, _)| {
            let message = format!(
                "Profile path is claimed by multiple profiles: {}",
                path.display()
            );
            HealthFinding::new(Severity::Broken, "duplicate-profile-path", message)
                .with_paths(vec![path.clone()])
        })
        .collect()
}

fn check_no_profiles(snapshot: &ProfileStoreSnapshot) -> Option<HealthFinding> {
    if snapshot.registered.is_empty() && snapshot.parse_error.is_none() {
        Some(HealthFinding::new(
            Severity::Warning,
            "no-profiles",
            "No registered profiles found in profile store",
        ))
    } else {
        None
    }
}

fn check_stale_migration_artifacts<'a>(
    profiles: &'a [BrowserProfile],
) -> impl Iterator<Item = HealthFinding> + 'a {
    profiles
        .iter()
        .filter(|p| p.directory.ends_with(".pmux-tmp") || p.directory.starts_with(".pmux-"))
        .map(|p| {
            let message = format!(
                "Profile '{}' uses stale migration directory '{}'",
                p.display_name, p.directory
            );
            HealthFinding::new(Severity::Warning, "stale-migration-artifact", message)
                .with_paths(vec![p.path.clone()])
        })
}
