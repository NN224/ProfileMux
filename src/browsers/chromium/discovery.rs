use std::collections::BTreeMap;
use std::path::Path;

use crate::domain::{BrowserInstall, BrowserInstallId, BrowserKind, Channel, SupportLevel};
#[cfg(target_os = "macos")]
use crate::platform::macos;

/// Static definition of a supported Chromium-family browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserDef {
    pub kind: BrowserKind,
    pub channel: Channel,
    pub name: &'static str,
    pub bundle_id: &'static str,
    pub relative_data_dir: &'static str,
    pub relative_cache_dir: &'static str,
}

pub const DEFINITIONS: &[BrowserDef] = &[
    BrowserDef {
        kind: BrowserKind::Brave,
        channel: Channel::Stable,
        name: "Brave Browser",
        bundle_id: "com.brave.Browser",
        relative_data_dir: "BraveSoftware/Brave-Browser",
        relative_cache_dir: "BraveSoftware/Brave-Browser",
    },
    BrowserDef {
        kind: BrowserKind::BraveBeta,
        channel: Channel::Beta,
        name: "Brave Browser Beta",
        bundle_id: "com.brave.Browser.beta",
        relative_data_dir: "BraveSoftware/Brave-Browser-Beta",
        relative_cache_dir: "BraveSoftware/Brave-Browser-Beta",
    },
    BrowserDef {
        kind: BrowserKind::BraveNightly,
        channel: Channel::Nightly,
        name: "Brave Browser Nightly",
        bundle_id: "com.brave.Browser.nightly",
        relative_data_dir: "BraveSoftware/Brave-Browser-Nightly",
        relative_cache_dir: "BraveSoftware/Brave-Browser-Nightly",
    },
    BrowserDef {
        kind: BrowserKind::Chrome,
        channel: Channel::Stable,
        name: "Google Chrome",
        bundle_id: "com.google.Chrome",
        relative_data_dir: "Google/Chrome",
        relative_cache_dir: "Google/Chrome",
    },
    BrowserDef {
        kind: BrowserKind::ChromeBeta,
        channel: Channel::Beta,
        name: "Google Chrome Beta",
        bundle_id: "com.google.Chrome.beta",
        relative_data_dir: "Google/Chrome Beta",
        relative_cache_dir: "Google/Chrome Beta",
    },
    BrowserDef {
        kind: BrowserKind::ChromeDev,
        channel: Channel::Dev,
        name: "Google Chrome Dev",
        bundle_id: "com.google.Chrome.dev",
        relative_data_dir: "Google/Chrome Dev",
        relative_cache_dir: "Google/Chrome Dev",
    },
    BrowserDef {
        kind: BrowserKind::ChromeCanary,
        channel: Channel::Canary,
        name: "Google Chrome Canary",
        bundle_id: "com.google.Chrome.canary",
        relative_data_dir: "Google/Chrome Canary",
        relative_cache_dir: "Google/Chrome Canary",
    },
    BrowserDef {
        kind: BrowserKind::Chromium,
        channel: Channel::Stable,
        name: "Chromium",
        bundle_id: "org.chromium.Chromium",
        relative_data_dir: "Chromium",
        relative_cache_dir: "Chromium",
    },
    BrowserDef {
        kind: BrowserKind::Edge,
        channel: Channel::Stable,
        name: "Microsoft Edge",
        bundle_id: "com.microsoft.edgemac",
        relative_data_dir: "Microsoft Edge",
        relative_cache_dir: "Microsoft Edge",
    },
    BrowserDef {
        kind: BrowserKind::EdgeBeta,
        channel: Channel::Beta,
        name: "Microsoft Edge Beta",
        bundle_id: "com.microsoft.edgemac.Beta",
        relative_data_dir: "Microsoft Edge Beta",
        relative_cache_dir: "Microsoft Edge Beta",
    },
    BrowserDef {
        kind: BrowserKind::EdgeDev,
        channel: Channel::Dev,
        name: "Microsoft Edge Dev",
        bundle_id: "com.microsoft.edgemac.Dev",
        relative_data_dir: "Microsoft Edge Dev",
        relative_cache_dir: "Microsoft Edge Dev",
    },
    BrowserDef {
        kind: BrowserKind::EdgeCanary,
        channel: Channel::Canary,
        name: "Microsoft Edge Canary",
        bundle_id: "com.microsoft.edgemac.Canary",
        relative_data_dir: "Microsoft Edge Canary",
        relative_cache_dir: "Microsoft Edge Canary",
    },
    BrowserDef {
        kind: BrowserKind::Vivaldi,
        channel: Channel::Stable,
        name: "Vivaldi",
        bundle_id: "com.vivaldi.Vivaldi",
        relative_data_dir: "Vivaldi",
        relative_cache_dir: "Vivaldi",
    },
];

/// Returns all static Chromium-family definitions.
pub fn definitions() -> &'static [BrowserDef] {
    DEFINITIONS
}

/// Matches a bundle identifier (case-insensitively) against supported Chromium browsers.
pub fn match_bundle(bundle_id: &str) -> Option<&'static BrowserDef> {
    DEFINITIONS
        .iter()
        .find(|def| def.bundle_id.eq_ignore_ascii_case(bundle_id))
}

fn preference_key(install: &BrowserInstall) -> (bool, &Path) {
    (
        !install.app_path.starts_with("/Applications"),
        &install.app_path,
    )
}

fn prefers_over(a: &BrowserInstall, b: &BrowserInstall) -> bool {
    preference_key(a) < preference_key(b)
}

/// Deduplicates installs by `BrowserInstallId` (preferring `/Applications` over `~/Applications`)
/// and sorts deterministically by `kind.slug()` then `user_data_root`.
pub fn dedup_and_sort_installs(
    candidates: impl IntoIterator<Item = BrowserInstall>,
) -> Vec<BrowserInstall> {
    let deduped = candidates
        .into_iter()
        .fold(BTreeMap::new(), |mut acc, install| {
            let replace = acc
                .get(&install.id)
                .map_or(true, |existing| prefers_over(&install, existing));
            if replace {
                acc.insert(install.id.clone(), install);
            }
            acc
        });

    let mut results: Vec<BrowserInstall> = deduped.into_values().collect();
    results.sort_by(|a, b| {
        a.kind
            .slug()
            .cmp(b.kind.slug())
            .then_with(|| a.user_data_root.cmp(&b.user_data_root))
    });
    results
}

#[cfg(target_os = "macos")]
fn build_install(
    bundle: macos::AppBundle,
    app_support: &Path,
    caches: Option<&Path>,
) -> Option<BrowserInstall> {
    let def = match_bundle(&bundle.bundle_id)?;
    let user_data_root = app_support.join(def.relative_data_dir);
    if !user_data_root.is_dir() {
        return None;
    }
    let cache_root = caches
        .map(|c| c.join(def.relative_cache_dir))
        .filter(|p| p.is_dir());
    let id = BrowserInstallId::new(def.kind, &user_data_root);

    Some(BrowserInstall {
        id,
        kind: def.kind,
        name: def.name.to_string(),
        channel: def.channel,
        app_path: bundle.path,
        bundle_id: Some(bundle.bundle_id),
        version: bundle.version,
        user_data_root,
        cache_root,
        support: SupportLevel::ReadOnly,
    })
}

/// Discovers installed Chromium-family browsers on the current system.
#[cfg(target_os = "macos")]
pub fn discover_installs() -> Vec<BrowserInstall> {
    let Some(app_support) = macos::app_support_dir() else {
        return Vec::new();
    };
    let caches = macos::caches_dir();
    let candidates = macos::scan_bundles()
        .into_iter()
        .filter_map(|bundle| build_install(bundle, &app_support, caches.as_deref()));

    dedup_and_sort_installs(candidates)
}

#[cfg(not(target_os = "macos"))]
pub fn discover_installs() -> Vec<BrowserInstall> {
    Vec::new()
}
