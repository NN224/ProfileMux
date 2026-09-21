//! Self-update support: discovering the latest published release, selecting the
//! right asset for this machine, verifying its checksum and replacing the
//! running executable.
//!
//! Networking is deliberately blocking (`ureq`) so the updater matches the rest
//! of the codebase, which uses no async runtime. Every piece of I/O sits behind
//! a trait so tests never touch GitHub or the real executable.

pub mod asset;
pub mod install;
pub mod source;
pub mod verify;

pub use asset::{asset_name_for_arch, checksum_asset_name, select_asset};
pub use install::{classify_install, install_binary, InstallKind};
pub use source::{GithubReleaseSource, ReleaseAsset, ReleaseInfo, ReleaseSource};
pub use verify::{sha256_hex, verify_sha256};

use semver::Version;

use crate::error::{Error, Result};

/// Repository the updater reads releases from.
pub const RELEASE_OWNER: &str = "NN224";
pub const RELEASE_REPO: &str = "ProfileMux";

/// The version of the running binary.
pub fn current_version() -> Result<Version> {
    parse_version(env!("CARGO_PKG_VERSION"))
}

/// Parses a version, tolerating a leading `v` as used by git tags.
pub fn parse_version(raw: &str) -> Result<Version> {
    let trimmed = raw.trim().trim_start_matches('v');
    Version::parse(trimmed).map_err(|e| Error::Update(format!("malformed version `{raw}`: {e}")))
}

/// Outcome of an update check. Comparison is semantic, never lexicographic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    UpToDate { current: Version },
    Available { current: Version, latest: Version },
}

impl UpdateStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, UpdateStatus::Available { .. })
    }

    pub fn latest(&self) -> Option<&Version> {
        match self {
            UpdateStatus::Available { latest, .. } => Some(latest),
            UpdateStatus::UpToDate { .. } => None,
        }
    }
}

/// Compares the running version against the latest release.
pub fn compare(current: &Version, latest: &Version) -> UpdateStatus {
    if latest > current {
        UpdateStatus::Available {
            current: current.clone(),
            latest: latest.clone(),
        }
    } else {
        UpdateStatus::UpToDate {
            current: current.clone(),
        }
    }
}

/// Reads the latest release and reports whether an update exists. Performs no
/// download and never touches the executable.
pub fn check_update(source: &dyn ReleaseSource, current: &Version) -> Result<UpdateStatus> {
    let release = source.latest_release()?;
    Ok(compare(current, &release.version))
}
