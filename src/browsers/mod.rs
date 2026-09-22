pub mod chromium;

use crate::domain::{
    Appearance, AppearanceCapabilities, AppearanceSpec, BrowserCapabilities, BrowserInstall,
    BrowserProfile, CloneProfileSpec, CreateProfileSpec, DeleteMode, HealthFinding, OperationPlan,
    ProfileStoreSnapshot,
};
use crate::error::{Error, Result};

/// A browser-family adapter.
///
/// Read-only methods must be implemented. Every mutation comes as a pair: a
/// `plan_*` method that describes the work without touching disk, and an
/// executing method that runs it. Both default to `Error::Unsupported`, so an
/// adapter only ever claims what it has actually verified.
pub trait BrowserAdapter {
    fn install(&self) -> &BrowserInstall;

    fn capabilities(&self) -> BrowserCapabilities;

    /// Single read-only pass over the profile store.
    fn snapshot(&self) -> Result<ProfileStoreSnapshot>;

    fn list_profiles(&self) -> Result<Vec<BrowserProfile>> {
        Ok(self.snapshot()?.registered)
    }

    /// Read-only health findings for this installation.
    fn doctor(&self) -> Result<Vec<HealthFinding>>;

    /// True when the browser currently holds its profile store open.
    fn is_running(&self) -> bool;

    /// Asks the running browser to quit cleanly. Never force-kills.
    fn request_quit(&self) -> Result<()> {
        Err(self.unsupported("request_quit"))
    }

    fn launch_profile(&self, _profile: &BrowserProfile) -> Result<()> {
        Err(self.unsupported("launch"))
    }

    fn plan_create(&self, _spec: &CreateProfileSpec) -> Result<OperationPlan> {
        Err(self.unsupported("create"))
    }

    fn create_profile(&self, _spec: &CreateProfileSpec) -> Result<BrowserProfile> {
        Err(self.unsupported("create"))
    }

    fn plan_clone(
        &self,
        _source: &BrowserProfile,
        _spec: &CloneProfileSpec,
    ) -> Result<OperationPlan> {
        Err(self.unsupported("clone"))
    }

    fn clone_profile(
        &self,
        _source: &BrowserProfile,
        _spec: &CloneProfileSpec,
    ) -> Result<BrowserProfile> {
        Err(self.unsupported("clone"))
    }

    fn rename_display_name(&self, _profile: &BrowserProfile, _new_name: &str) -> Result<()> {
        Err(self.unsupported("rename_display_name"))
    }

    fn plan_rename_directory(
        &self,
        _profile: &BrowserProfile,
        _new_directory: &str,
    ) -> Result<OperationPlan> {
        Err(self.unsupported("rename_directory"))
    }

    fn rename_profile_directory(
        &self,
        _profile: &BrowserProfile,
        _new_directory: &str,
    ) -> Result<()> {
        Err(self.unsupported("rename_directory"))
    }

    fn plan_set_avatar(
        &self,
        _profile: &BrowserProfile,
        _image: &std::path::Path,
    ) -> Result<OperationPlan> {
        Err(self.unsupported("custom_avatar"))
    }

    fn set_avatar(&self, _profile: &BrowserProfile, _image: &std::path::Path) -> Result<()> {
        Err(self.unsupported("custom_avatar"))
    }

    fn plan_delete(&self, _profile: &BrowserProfile, _mode: DeleteMode) -> Result<OperationPlan> {
        Err(self.unsupported("delete"))
    }

    fn delete_profile(&self, _profile: &BrowserProfile, _mode: DeleteMode) -> Result<()> {
        Err(self.unsupported("delete"))
    }

    fn plan_clean_cache(&self, _profile: &BrowserProfile) -> Result<OperationPlan> {
        Err(self.unsupported("clean_cache"))
    }

    /// Removes verified cache locations only. Returns bytes reclaimed.
    fn clean_cache(&self, _profile: &BrowserProfile) -> Result<u64> {
        Err(self.unsupported("clean_cache"))
    }

    /// What this adapter can change about a profile's appearance.
    fn appearance_capabilities(&self) -> AppearanceCapabilities {
        AppearanceCapabilities::NONE
    }

    /// Reads the profile's current appearance. Read-only.
    fn read_appearance(&self, _profile: &BrowserProfile) -> Result<Appearance> {
        Err(self.unsupported("appearance"))
    }

    fn plan_set_appearance(
        &self,
        _profile: &BrowserProfile,
        _spec: &AppearanceSpec,
    ) -> Result<OperationPlan> {
        Err(self.unsupported("appearance"))
    }

    fn set_appearance(&self, _profile: &BrowserProfile, _spec: &AppearanceSpec) -> Result<()> {
        Err(self.unsupported("appearance"))
    }

    fn unsupported(&self, operation: &str) -> Error {
        Error::unsupported(operation, self.install().name.as_str())
    }
}

/// Every installation ProfileMux can find on this machine, in stable order.
pub fn discover_all() -> Vec<Box<dyn BrowserAdapter>> {
    let mut adapters: Vec<Box<dyn BrowserAdapter>> = Vec::new();
    for adapter in chromium::discover() {
        adapters.push(Box::new(adapter));
    }
    adapters
}
