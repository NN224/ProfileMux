// Shared fixture builder; not every test binary uses every helper.
#![allow(dead_code)]
use std::fs;
use std::path::{Path, PathBuf};

use profilemux::domain::{BrowserInstall, BrowserInstallId, BrowserKind, Channel, SupportLevel};
use tempfile::TempDir;

/// Test fixture managing a temporary Chromium user data root and cache directory.
pub struct ChromiumFixture {
    _temp_dir: TempDir,
    user_data_root: PathBuf,
    cache_root: PathBuf,
    info_cache: serde_json::Map<String, serde_json::Value>,
}

impl Default for ChromiumFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl ChromiumFixture {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let user_data_root = temp_dir.path().join("user_data");
        let cache_root = temp_dir.path().join("cache");
        fs::create_dir_all(&user_data_root).expect("failed to create user_data_root");
        fs::create_dir_all(&cache_root).expect("failed to create cache_root");

        ChromiumFixture {
            _temp_dir: temp_dir,
            user_data_root,
            cache_root,
            info_cache: serde_json::Map::new(),
        }
    }

    pub fn add_registered_profile(
        &mut self,
        directory: &str,
        display_name: Option<&str>,
        create_dir: bool,
    ) -> &mut Self {
        let mut entry = serde_json::Map::new();
        if let Some(name) = display_name {
            entry.insert(
                "name".to_string(),
                serde_json::Value::String(name.to_string()),
            );
        }
        self.add_registered_profile_entry(directory, serde_json::Value::Object(entry), create_dir)
    }

    pub fn add_registered_profile_entry(
        &mut self,
        directory: &str,
        entry: serde_json::Value,
        create_dir: bool,
    ) -> &mut Self {
        if create_dir {
            let profile_dir = self.user_data_root.join(directory);
            fs::create_dir_all(&profile_dir).expect("failed to create profile dir");
        }
        self.info_cache.insert(directory.to_string(), entry);
        self.flush_local_state();
        self
    }

    pub fn add_unregistered_profile(&mut self, directory: &str) -> &mut Self {
        let profile_dir = self.user_data_root.join(directory);
        fs::create_dir_all(&profile_dir).expect("failed to create unregistered profile dir");
        fs::write(profile_dir.join("Preferences"), "{}").expect("failed to write Preferences");
        self
    }

    pub fn add_orphan_cache(&mut self, directory: &str) -> &mut Self {
        let orphan_dir = self.cache_root.join(directory);
        fs::create_dir_all(&orphan_dir).expect("failed to create orphan cache dir");
        self
    }

    pub fn write_malformed_local_state(&mut self, content: &str) -> &mut Self {
        let path = self.user_data_root.join("Local State");
        fs::write(path, content).expect("failed to write malformed Local State");
        self
    }

    pub fn write_file(&self, path: &Path, size_bytes: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent dir");
        }
        let data = vec![b'x'; size_bytes];
        fs::write(path, data).expect("failed to write file of known size");
    }

    pub fn user_data_root(&self) -> &Path {
        &self.user_data_root
    }

    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    pub fn install(&self) -> BrowserInstall {
        BrowserInstall {
            id: BrowserInstallId::new(BrowserKind::Chromium, &self.user_data_root),
            kind: BrowserKind::Chromium,
            name: "Chromium Test".to_string(),
            channel: Channel::Stable,
            app_path: PathBuf::from("/Applications/ChromiumTest.app"),
            bundle_id: None,
            version: None,
            user_data_root: self.user_data_root.clone(),
            cache_root: Some(self.cache_root.clone()),
            support: SupportLevel::ReadOnly,
        }
    }

    fn flush_local_state(&self) {
        let mut profile = serde_json::Map::new();
        profile.insert(
            "info_cache".to_string(),
            serde_json::Value::Object(self.info_cache.clone()),
        );
        let mut root = serde_json::Map::new();
        root.insert("profile".to_string(), serde_json::Value::Object(profile));
        let content = serde_json::to_string_pretty(&root).expect("failed to serialize Local State");
        fs::write(self.user_data_root.join("Local State"), content)
            .expect("failed to write Local State");
    }
}
