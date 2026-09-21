use serde::{Deserialize, Serialize};

/// What an adapter can actually do. The UI disables actions whose capability is
/// false instead of failing at execution time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserCapabilities {
    pub launch: bool,
    pub open_folder: bool,
    pub create: bool,
    pub rename_display_name: bool,
    pub rename_directory: bool,
    pub clone: bool,
    pub delete: bool,
    pub clean_cache: bool,
    pub custom_avatar: bool,
    /// Capabilities implemented but not yet proven stable for this browser.
    pub experimental_custom_avatar: bool,
    pub experimental_rename_directory: bool,
}

impl BrowserCapabilities {
    /// Nothing supported. Adapters opt in explicitly from this baseline.
    pub const NONE: BrowserCapabilities = BrowserCapabilities {
        launch: false,
        open_folder: false,
        create: false,
        rename_display_name: false,
        rename_directory: false,
        clone: false,
        delete: false,
        clean_cache: false,
        custom_avatar: false,
        experimental_custom_avatar: false,
        experimental_rename_directory: false,
    };

    /// Read-only inspection plus the harmless "reveal in file manager" action.
    pub const READ_ONLY: BrowserCapabilities = BrowserCapabilities {
        open_folder: true,
        ..BrowserCapabilities::NONE
    };

    /// Human-readable reason an action is unavailable, for the UI status line.
    pub fn reason_disabled(&self, action: &str) -> Option<&'static str> {
        let supported = match action {
            "launch" => self.launch,
            "open_folder" => self.open_folder,
            "create" => self.create,
            "rename_display_name" => self.rename_display_name,
            "rename_directory" => self.rename_directory,
            "clone" => self.clone,
            "delete" => self.delete,
            "clean_cache" => self.clean_cache,
            "custom_avatar" => self.custom_avatar,
            _ => false,
        };
        if supported {
            None
        } else {
            Some("not supported by this browser adapter")
        }
    }
}
