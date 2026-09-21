use crate::domain::{BrowserInstall, BrowserKind, BrowserProfile};
use crate::error::{Error, Result};

/// Resolves a user-provided selector string to a specific profile.
///
/// Accepts three forms:
/// 1. Full `ProfileId` string (exact match)
/// 2. `<browser-slug>/<directory>` (exact match on slug and directory)
/// 3. Bare display name (case-insensitive) or directory name (exact)
pub fn resolve_profile<'a>(
    installs: &'a [(BrowserInstall, Vec<BrowserProfile>)],
    selector: &str,
) -> Result<(&'a BrowserInstall, &'a BrowserProfile)> {
    if let Some(res) = match_profile_id(installs, selector) {
        return res;
    }
    if let Some(res) = match_slug_directory(installs, selector) {
        return res;
    }
    match_name_or_directory(installs, selector)
}

fn match_profile_id<'a>(
    installs: &'a [(BrowserInstall, Vec<BrowserProfile>)],
    selector: &str,
) -> Option<Result<(&'a BrowserInstall, &'a BrowserProfile)>> {
    let matches: Vec<_> = installs
        .iter()
        .flat_map(|(inst, profiles)| profiles.iter().map(move |p| (inst, p)))
        .filter(|(_, p)| p.id.as_str() == selector)
        .collect();

    match matches.len() {
        0 => None,
        1 => Some(Ok(matches[0])),
        _ => {
            let mut candidates: Vec<String> = matches
                .iter()
                .map(|(inst, p)| format!("{}/{}", inst.kind.slug(), p.directory))
                .collect();
            candidates.sort();
            candidates.dedup();
            Some(Err(Error::AmbiguousSelector {
                selector: selector.to_string(),
                candidates,
            }))
        }
    }
}

fn match_slug_directory<'a>(
    installs: &'a [(BrowserInstall, Vec<BrowserProfile>)],
    selector: &str,
) -> Option<Result<(&'a BrowserInstall, &'a BrowserProfile)>> {
    let (slug, dir) = selector.split_once('/')?;
    let is_valid_slug = BrowserKind::from_slug(slug).is_some()
        || installs.iter().any(|(inst, _)| inst.kind.slug() == slug);

    if !is_valid_slug {
        return None;
    }

    let matches: Vec<_> = installs
        .iter()
        .filter(|(inst, _)| inst.kind.slug() == slug)
        .flat_map(|(inst, profiles)| profiles.iter().map(move |p| (inst, p)))
        .filter(|(_, p)| p.directory == dir)
        .collect();

    match matches.len() {
        0 => Some(Err(Error::NoSuchProfile(selector.to_string()))),
        1 => Some(Ok(matches[0])),
        _ => {
            let mut candidates: Vec<String> = matches
                .iter()
                .map(|(inst, p)| format!("{}/{}", inst.kind.slug(), p.directory))
                .collect();
            candidates.sort();
            candidates.dedup();
            Some(Err(Error::AmbiguousSelector {
                selector: selector.to_string(),
                candidates,
            }))
        }
    }
}

fn match_name_or_directory<'a>(
    installs: &'a [(BrowserInstall, Vec<BrowserProfile>)],
    selector: &str,
) -> Result<(&'a BrowserInstall, &'a BrowserProfile)> {
    let matches: Vec<_> = installs
        .iter()
        .flat_map(|(inst, profiles)| profiles.iter().map(move |p| (inst, p)))
        .filter(|(_, p)| p.directory == selector || p.display_name.eq_ignore_ascii_case(selector))
        .collect();

    match matches.len() {
        0 => Err(Error::NoSuchProfile(selector.to_string())),
        1 => Ok(matches[0]),
        _ => {
            let mut candidates: Vec<String> = matches
                .iter()
                .map(|(inst, p)| format!("{}/{}", inst.kind.slug(), p.directory))
                .collect();
            candidates.sort();
            candidates.dedup();
            Err(Error::AmbiguousSelector {
                selector: selector.to_string(),
                candidates,
            })
        }
    }
}

pub fn collect_installed_profiles(
    adapters: &[Box<dyn crate::browsers::BrowserAdapter>],
) -> Result<Vec<(BrowserInstall, Vec<BrowserProfile>)>> {
    let mut result = Vec::with_capacity(adapters.len());
    for adapter in adapters {
        let profiles = adapter.list_profiles()?;
        result.push((adapter.install().clone(), profiles));
    }
    Ok(result)
}

/// Resolves a template profile within a specific browser installation.
///
/// Accepts a directory name or case-insensitive display name.
/// If ambiguous within that browser, returns `Error::AmbiguousSelector` with candidate list.
pub fn resolve_template_in_browser<'a>(
    install: &'a BrowserInstall,
    profiles: &'a [BrowserProfile],
    template: &str,
) -> Result<&'a BrowserProfile> {
    let clean_template = if let Some((slug, dir)) = template.split_once('/') {
        if slug == install.kind.slug() {
            dir
        } else {
            template
        }
    } else {
        template
    };

    let matches: Vec<_> = profiles
        .iter()
        .filter(|p| {
            p.directory == clean_template || p.display_name.eq_ignore_ascii_case(clean_template)
        })
        .collect();

    match matches.len() {
        0 => Err(Error::NoSuchProfile(template.to_string())),
        1 => Ok(matches[0]),
        _ => {
            let mut candidates: Vec<String> = matches
                .iter()
                .map(|p| format!("{}/{}", install.kind.slug(), p.directory))
                .collect();
            candidates.sort();
            candidates.dedup();
            Err(Error::AmbiguousSelector {
                selector: template.to_string(),
                candidates,
            })
        }
    }
}
