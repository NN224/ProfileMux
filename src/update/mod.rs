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

/// Downloads, verifies and installs the latest release, without printing
/// anything. The TUI calls this because it runs inside an alternate screen
/// where stray output would corrupt the display; the CLI has its own variant
/// that reports progress.
///
/// Returns the version that was installed. Checksum verification is mandatory:
/// a mismatch aborts before the running binary is touched.
pub fn install_latest_with(
    source: &dyn ReleaseSource,
    exe: &std::path::Path,
    arch: &str,
    current: &Version,
) -> Result<Version> {
    if classify_install(exe) == InstallKind::Development {
        return Err(Error::Update(
            "this is a development build; use cargo build or cargo install instead".to_string(),
        ));
    }

    let release = source.latest_release()?;
    let latest = match compare(current, &release.version) {
        UpdateStatus::UpToDate { .. } => {
            return Err(Error::Update("already up to date".to_string()))
        }
        UpdateStatus::Available { latest, .. } => latest,
    };

    let binary_asset = select_asset(&release.assets, arch)?;
    let checksum_name = checksum_asset_name(&binary_asset.name);
    let checksum_asset = release
        .assets
        .iter()
        .find(|a| a.name == checksum_name)
        .ok_or_else(|| Error::Update(format!("release has no checksum asset `{checksum_name}`")))?;

    let binary = source.download(binary_asset)?;
    let checksum = source.download(checksum_asset)?;
    let expected = String::from_utf8_lossy(&checksum).to_string();
    verify_sha256(&binary, &expected)?;

    install_binary(&binary, exe)?;
    Ok(latest)
}

/// Convenience wrapper resolving the running executable, this machine's
/// architecture and the public GitHub release source.
pub fn install_latest() -> Result<Version> {
    let exe = std::env::current_exe()
        .map_err(|e| Error::Update(format!("cannot resolve the running executable: {e}")))?;
    let current = current_version()?;
    let source = GithubReleaseSource::new(RELEASE_OWNER, RELEASE_REPO);
    install_latest_with(&source, &exe, std::env::consts::ARCH, &current)
}
