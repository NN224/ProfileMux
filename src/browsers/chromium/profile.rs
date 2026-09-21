use crate::browsers::BrowserAdapter;
use crate::domain::{BrowserCapabilities, BrowserInstall, HealthFinding, ProfileStoreSnapshot};
use crate::error::Result;

/// Adapter for Chromium-family browsers (Brave, Chrome, Chromium, Edge, ...).
pub struct ChromiumAdapter {
    install: BrowserInstall,
}

impl ChromiumAdapter {
    pub fn new(install: BrowserInstall) -> Self {
        ChromiumAdapter { install }
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
        Ok(ProfileStoreSnapshot::default())
    }

    fn doctor(&self) -> Result<Vec<HealthFinding>> {
        Ok(Vec::new())
    }

    fn is_running(&self) -> bool {
        false
    }
}
