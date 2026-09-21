use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::browser::BrowserInstallId;

/// Stable identity of a profile: installation identity plus the on-disk
/// directory name. Display names are mutable labels and never part of the id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn new(install: &BrowserInstallId, directory: &str) -> Self {
        ProfileId(format!("{}/{}", install.as_str(), directory))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Avatar metadata. Only describes which avatar is in use; never image contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvatarInfo {
    /// Chromium's `avatar_icon` value, e.g. "chrome://theme/IDR_PROFILE_AVATAR_26".
    pub icon: Option<String>,
    /// Chromium's `gaia_picture_file_name` when a custom picture is in use.
    pub picture_file: Option<String>,
    /// Chromium's `use_gaia_picture` flag.
    pub uses_picture: bool,
    /// True when the profile is still on a stock avatar.
    pub is_default: bool,
}

/// Per-area disk usage in bytes. All fields are byte counts; `total` is the sum
/// of the measured areas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageBreakdown {
    pub core: u64,
    pub cache: u64,
    pub code_cache: u64,
    pub gpu_cache: u64,
    pub total: u64,
}

/// One browser profile as ProfileMux understands it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub id: ProfileId,
    pub install_id: BrowserInstallId,
    /// Browser-visible label, freely renamable.
    pub display_name: String,
    /// On-disk directory name inside the user data root, e.g. "Default".
    pub directory: String,
    /// Absolute path of the profile directory.
    pub path: PathBuf,
    /// Absolute path of the profile's cache directory when resolvable.
    pub cache_path: Option<PathBuf>,
    pub avatar: Option<AvatarInfo>,
    /// Last active time in seconds since the Unix epoch, when reliably available.
    pub last_active: Option<i64>,
    /// True when the browser's own metadata lists this profile.
    pub registered: bool,
    pub directory_exists: bool,
    /// Populated only when a size scan has run; `None` means "not measured yet".
    pub size: Option<StorageBreakdown>,
}

impl BrowserProfile {
    /// Returns a copy with storage measurements attached. Never mutates in place.
    pub fn with_size(&self, size: StorageBreakdown) -> Self {
        BrowserProfile {
            size: Some(size),
            ..self.clone()
        }
    }
}

/// Everything read from one installation's profile store in a single read-only
/// pass. Doctor consumes this and needs no browser-specific code of its own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileStoreSnapshot {
    /// Profiles listed in the browser's own metadata.
    pub registered: Vec<BrowserProfile>,
    /// Directories that look like profiles but are absent from the metadata.
    pub unregistered_dirs: Vec<PathBuf>,
    /// Cache directories with no corresponding profile directory.
    pub orphan_cache_dirs: Vec<PathBuf>,
    /// Set when the metadata file exists but could not be parsed.
    pub parse_error: Option<String>,
}

impl ProfileStoreSnapshot {
    pub fn find_by_directory(&self, directory: &str) -> Option<&BrowserProfile> {
        self.registered.iter().find(|p| p.directory == directory)
    }

    pub fn find_by_path(&self, path: &Path) -> Option<&BrowserProfile> {
        self.registered.iter().find(|p| p.path == path)
    }
}
