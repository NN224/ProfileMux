use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::browsers::BrowserAdapter;
use crate::domain::{
    AvatarInfo, BrowserCapabilities, BrowserInstall, BrowserProfile, HealthFinding, ProfileId,
    ProfileStoreSnapshot,
};
use crate::error::{Error, Result};

use super::local_state::{self, ProfileEntry};

/// Adapter for Chromium-family browsers (Brave, Chrome, Chromium, Edge, ...).
pub struct ChromiumAdapter {
    install: BrowserInstall,
}

impl ChromiumAdapter {
    pub fn new(install: BrowserInstall) -> Self {
        ChromiumAdapter { install }
    }

    fn build_registered_profiles(
        &self,
        entries: &BTreeMap<String, ProfileEntry>,
    ) -> Vec<BrowserProfile> {
        entries
            .iter()
            .map(|(dir, entry)| self.build_profile(dir, entry))
            .collect()
    }

    fn build_profile(&self, directory: &str, entry: &ProfileEntry) -> BrowserProfile {
        let path = self.install.user_data_root.join(directory);
        let directory_exists = path.is_dir();

        let display_name = entry
            .name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(directory)
            .to_string();

        let cache_path = self.install.cache_root.as_ref().and_then(|root| {
            let p = root.join(directory);
            if p.is_dir() {
                Some(p)
            } else {
                None
            }
        });

        let avatar = build_avatar_info(entry);
        let last_active = entry
            .active_time
            .filter(|t| t.is_finite())
            .map(|t| t as i64);

        BrowserProfile {
            id: ProfileId::new(&self.install.id, directory),
            install_id: self.install.id.clone(),
            display_name,
            directory: directory.to_string(),
            path,
            cache_path,
            avatar,
            last_active,
            registered: true,
            directory_exists,
            size: None,
        }
    }

    fn find_unregistered_dirs(&self, entries: &BTreeMap<String, ProfileEntry>) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let read_dir = match std::fs::read_dir(&self.install.user_data_root) {
            Ok(rd) => rd,
            Err(_) => return dirs,
        };

        const EXCLUDED: &[&str] = &[
            "System Profile",
            "Guest Profile",
            "Crashpad",
            "component_crx_cache",
            "extensions_crx_cache",
        ];

        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if EXCLUDED.contains(&name) || entries.contains_key(name) {
                continue;
            }
            if path.join("Preferences").is_file() {
                dirs.push(path);
            }
        }

        dirs.sort();
        dirs
    }

    fn find_orphan_cache_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let cache_root = match &self.install.cache_root {
            Some(cr) => cr,
            None => return dirs,
        };
        let read_dir = match std::fs::read_dir(cache_root) {
            Ok(rd) => rd,
            Err(_) => return dirs,
        };

        const EXCLUDED: &[&str] = &["System Profile", "Guest Profile"];

        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if EXCLUDED.contains(&name) {
                continue;
            }
            let user_data_profile_dir = self.install.user_data_root.join(name);
            if !user_data_profile_dir.is_dir() {
                dirs.push(path);
            }
        }

        dirs.sort();
        dirs
    }
}

impl BrowserAdapter for ChromiumAdapter {
    fn install(&self) -> &BrowserInstall {
        &self.install
    }

    fn capabilities(&self) -> BrowserCapabilities {
        BrowserCapabilities::READ_ONLY
    }

    fn snapshot(&self) -> Result<ProfileStoreSnapshot> {
        let local_state_path = self.install.user_data_root.join("Local State");
        let (entries, parse_error) = match local_state::load(&local_state_path) {
            Ok(ls) => (ls.entries, None),
            Err(Error::Malformed { message, .. }) => (BTreeMap::new(), Some(message)),
            Err(Error::NotFound(_)) => (BTreeMap::new(), None),
            Err(e) => return Err(e),
        };

        let registered = self.build_registered_profiles(&entries);
        let unregistered_dirs = self.find_unregistered_dirs(&entries);
        let orphan_cache_dirs = self.find_orphan_cache_dirs();

        Ok(ProfileStoreSnapshot {
            registered,
            unregistered_dirs,
            orphan_cache_dirs,
            parse_error,
        })
    }

    fn doctor(&self) -> Result<Vec<HealthFinding>> {
        let snapshot = self.snapshot()?;
        Ok(crate::doctor::analyze(&self.install, &snapshot))
    }

    fn is_running(&self) -> bool {
        let lock_path = self.install.user_data_root.join("SingletonLock");
        std::fs::symlink_metadata(&lock_path).is_ok()
    }
}

fn build_avatar_info(entry: &ProfileEntry) -> Option<AvatarInfo> {
    let has_avatar = entry.avatar_icon.is_some()
        || entry.gaia_picture_file_name.is_some()
        || entry.use_gaia_picture.is_some()
        || entry.is_using_default_avatar.is_some();

    if has_avatar {
        Some(AvatarInfo {
            icon: entry.avatar_icon.clone(),
            picture_file: entry.gaia_picture_file_name.clone(),
            uses_picture: entry.use_gaia_picture.unwrap_or(false),
            is_default: entry.is_using_default_avatar.unwrap_or(false),
        })
    } else {
        None
    }
}
