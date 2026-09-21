pub mod browser;
pub mod capability;
pub mod health;
pub mod profile;

pub use browser::{BrowserInstall, BrowserInstallId, BrowserKind, Channel, SupportLevel};
pub use capability::BrowserCapabilities;
pub use health::{HealthFinding, Severity};
pub use profile::{AvatarInfo, BrowserProfile, ProfileId, ProfileStoreSnapshot, StorageBreakdown};
