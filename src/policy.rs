//! Per-profile launch policy persistence.
//!
//! Stores launch-time configuration owned by ProfileMux, such as forced web
//! dark mode. This state cannot be persisted in Chromium's profile preferences
//! because the underlying feature flags reside in browser-wide `Local State`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::{ProfileId, WebDarkMode};
use crate::error::{Error, Result};

/// Launch-time policy for a single browser profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchPolicy {
    pub web_dark: WebDarkMode,
}

impl Default for LaunchPolicy {
    fn default() -> Self {
        Self {
            web_dark: WebDarkMode::Normal,
        }
    }
}

/// Minimal on-disk store for profile launch policies.
///
/// Persists only policy settings keyed by the stable `ProfileId` string.
/// Store nothing but the policy: no profile names, no paths, no emails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyStore {
    path: PathBuf,
    entries: BTreeMap<String, LaunchPolicy>,
}

impl PolicyStore {
    pub fn default_path() -> Result<PathBuf> {
        default_path()
    }

    pub fn load(path: &Path) -> Result<Self> {
        load(path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &BTreeMap<String, LaunchPolicy> {
        &self.entries
    }

    pub fn get(&self, id: &ProfileId) -> LaunchPolicy {
        match self.entries.get(id.as_str()) {
            Some(policy) => *policy,
            None => LaunchPolicy::default(),
        }
    }

    pub fn set(&self, id: &ProfileId, policy: LaunchPolicy) -> Self {
        let mut entries = self.entries.clone();
        entries.insert(id.as_str().to_string(), policy);
        Self {
            path: self.path.clone(),
            entries,
        }
    }

    pub fn save(&self) -> Result<()> {
        let parent = match self.path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };
        if parent != Path::new(".") {
            fs::create_dir_all(parent).map_err(|err| Error::io(parent, err))?;
        }

        // Store nothing but the policy: no profile names, no paths, no emails.
        let json = serde_json::to_string_pretty(&self.entries)
            .map_err(|err| Error::Other(format!("failed to serialize policy store: {err}")))?;

        let file_name = self
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("launch-policy.json");
        let pid = std::process::id();
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_nanos(),
            Err(_) => 0,
        };
        let temp_path = parent.join(format!(".{file_name}.tmp.{pid}_{nanos}"));

        if let Err(err) = fs::write(&temp_path, json.as_bytes()) {
            let _ = fs::remove_file(&temp_path);
            return Err(Error::io(&temp_path, err));
        }

        if let Err(err) = fs::rename(&temp_path, &self.path) {
            let _ = fs::remove_file(&temp_path);
            return Err(Error::io(&self.path, err));
        }

        Ok(())
    }
}

/// Resolves the default launch policy path in the user config directory.
pub fn default_path() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|config| config.join("profilemux").join("launch-policy.json"))
        .ok_or_else(|| Error::Other("could not determine config directory".to_string()))
}

/// Loads launch policies from disk.
///
/// Missing or malformed files return an empty store without panicking or failing.
pub fn load(path: &Path) -> Result<PolicyStore> {
    let entries = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(err) => return Err(Error::io(path, err)),
    };
    Ok(PolicyStore {
        path: path.to_path_buf(),
        entries,
    })
}
