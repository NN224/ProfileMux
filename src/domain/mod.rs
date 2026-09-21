pub mod browser;
pub mod capability;
pub mod health;
pub mod operation;
pub mod profile;
pub mod sanitize;

pub use browser::{BrowserInstall, BrowserInstallId, BrowserKind, Channel, SupportLevel};
pub use capability::BrowserCapabilities;
pub use health::{HealthFinding, Severity};
pub use operation::{
    ClonePolicy, CloneProfileSpec, CreateProfileSpec, DeleteMode, ExtensionPolicy, OperationKind,
    OperationPlan, PlanStep,
};
pub use profile::{AvatarInfo, BrowserProfile, ProfileId, ProfileStoreSnapshot, StorageBreakdown};
