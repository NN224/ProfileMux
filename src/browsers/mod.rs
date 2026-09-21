pub mod chromium;

use crate::domain::{
    BrowserCapabilities, BrowserInstall, BrowserProfile, HealthFinding, ProfileStoreSnapshot,
};
use crate::error::{Error, Result};

/// A browser-family adapter. Read-only methods must be implemented; mutation
/// methods default to `Error::Unsupported` so an adapter only claims what it
/// has actually verified.
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

    fn launch_profile(&self, _profile: &BrowserProfile) -> Result<()> {
        Err(Error::unsupported("launch", self.install().name.as_str()))
    }

    fn rename_display_name(&self, _profile: &BrowserProfile, _new_name: &str) -> Result<()> {
        Err(Error::unsupported(
            "rename_display_name",
            self.install().name.as_str(),
        ))
    }

    fn rename_profile_directory(
        &self,
        _profile: &BrowserProfile,
        _new_directory: &str,
    ) -> Result<()> {
        Err(Error::unsupported(
            "rename_profile_directory",
            self.install().name.as_str(),
        ))
    }

    fn delete_profile(&self, _profile: &BrowserProfile) -> Result<()> {
        Err(Error::unsupported("delete", self.install().name.as_str()))
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
