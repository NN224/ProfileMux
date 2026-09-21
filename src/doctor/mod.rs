use crate::domain::{BrowserInstall, HealthFinding, ProfileStoreSnapshot};

/// Placeholder: pure, read-only analysis of a profile store snapshot.
pub fn analyze(_install: &BrowserInstall, _snapshot: &ProfileStoreSnapshot) -> Vec<HealthFinding> {
    Vec::new()
}
