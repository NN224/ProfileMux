use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::browsers::BrowserAdapter;
use crate::domain::{
    AvatarInfo, BrowserCapabilities, BrowserInstall, BrowserKind, BrowserProfile, CloneProfileSpec,
    CreateProfileSpec, DeleteMode, HealthFinding, OperationPlan, ProfileId, ProfileStoreSnapshot,
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
        let account_email = entry
            .user_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
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
            account_email,
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
        let custom_avatar = matches!(
            self.install.kind,
            BrowserKind::Brave | BrowserKind::BraveBeta | BrowserKind::BraveNightly
        );
        BrowserCapabilities {
            launch: true,
            open_folder: true,
            create: true,
            rename_display_name: true,
            rename_directory: true,
            clone: true,
            delete: true,
            clean_cache: true,
            custom_avatar,
            experimental_custom_avatar: custom_avatar,
            experimental_rename_directory: true,
        }
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
        super::launch::is_running(&self.install)
    }

    fn request_quit(&self) -> Result<()> {
        super::launch::request_quit(&self.install)
    }

    fn launch_profile(&self, profile: &BrowserProfile) -> Result<()> {
        super::launch::launch(&self.install, &profile.directory)
    }

    fn plan_create(&self, spec: &CreateProfileSpec) -> Result<OperationPlan> {
        super::mutation::plan_create(&self.install, spec)
    }

    fn create_profile(&self, spec: &CreateProfileSpec) -> Result<BrowserProfile> {
        super::mutation::create_profile(&self.install, spec)
    }

    fn plan_clone(
        &self,
        source: &BrowserProfile,
        spec: &CloneProfileSpec,
    ) -> Result<OperationPlan> {
        super::mutation::plan_clone(&self.install, source, spec)
    }

    fn clone_profile(
        &self,
        source: &BrowserProfile,
        spec: &CloneProfileSpec,
    ) -> Result<BrowserProfile> {
        super::mutation::clone_profile(&self.install, source, spec)
    }

    fn rename_display_name(&self, profile: &BrowserProfile, new_name: &str) -> Result<()> {
        super::mutation::rename_display_name(&self.install, profile, new_name)
    }

    fn plan_rename_directory(
        &self,
        profile: &BrowserProfile,
        new_directory: &str,
    ) -> Result<OperationPlan> {
        super::mutation::plan_rename_directory(&self.install, profile, new_directory)
    }

    fn rename_profile_directory(
        &self,
        profile: &BrowserProfile,
        new_directory: &str,
    ) -> Result<()> {
        super::mutation::rename_profile_directory(&self.install, profile, new_directory)
    }

    fn plan_set_avatar(&self, profile: &BrowserProfile, image: &Path) -> Result<OperationPlan> {
        super::mutation::plan_set_avatar(&self.install, profile, image)
    }

    fn set_avatar(&self, profile: &BrowserProfile, image: &Path) -> Result<()> {
        super::mutation::set_avatar(&self.install, profile, image)
    }

    fn plan_delete(&self, profile: &BrowserProfile, mode: DeleteMode) -> Result<OperationPlan> {
        super::mutation::plan_delete(&self.install, profile, mode)
    }

    fn delete_profile(&self, profile: &BrowserProfile, mode: DeleteMode) -> Result<()> {
        super::mutation::delete_profile(&self.install, profile, mode)
    }

    fn plan_clean_cache(&self, profile: &BrowserProfile) -> Result<OperationPlan> {
        super::mutation::plan_clean_cache(&self.install, profile)
    }

    fn clean_cache(&self, profile: &BrowserProfile) -> Result<u64> {
        super::mutation::clean_cache(&self.install, profile)
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
