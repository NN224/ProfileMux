use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Which structural operation a plan describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationKind {
    CreateProfile,
    CloneProfile,
    RenameDisplayName,
    RenameDirectory,
    SetAvatar,
    DeleteProfile,
    CleanCache,
    SetAppearance,
}

impl OperationKind {
    pub fn title(self) -> &'static str {
        match self {
            OperationKind::CreateProfile => "CREATE PROFILE",
            OperationKind::CloneProfile => "CLONE PROFILE",
            OperationKind::RenameDisplayName => "RENAME DISPLAY NAME",
            OperationKind::RenameDirectory => "RENAME PROFILE DIRECTORY",
            OperationKind::SetAvatar => "SET PROFILE AVATAR",
            OperationKind::DeleteProfile => "DELETE PROFILE",
            OperationKind::CleanCache => "CLEAN CACHE",
            OperationKind::SetAppearance => "SET APPEARANCE",
        }
    }
}

/// How a profile's data is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteMode {
    /// Move the profile and cache directories to the macOS Trash. The default.
    Trash,
}

/// What a template clone copies from the source profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionPolicy {
    /// Copy no extensions at all. The default.
    None,
    /// Copy installed extension payloads without their local settings.
    CopyExtensions,
    /// Copy extension payloads and their local settings. Experimental.
    CopyExtensionsAndSettings,
}

impl ExtensionPolicy {
    pub fn label(self) -> &'static str {
        match self {
            ExtensionPolicy::None => "Do not copy",
            ExtensionPolicy::CopyExtensions => "Copy installed extensions",
            ExtensionPolicy::CopyExtensionsAndSettings => {
                "Copy extensions + settings (experimental)"
            }
        }
    }

    pub fn from_cli(value: &str) -> Option<Self> {
        match value {
            "none" => Some(ExtensionPolicy::None),
            "copy" => Some(ExtensionPolicy::CopyExtensions),
            "copy-settings" => Some(ExtensionPolicy::CopyExtensionsAndSettings),
            _ => None,
        }
    }
}

/// Explicit policy describing what a template clone carries over. Never a
/// blanket copy of the source profile directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonePolicy {
    /// Copy UI/appearance/browser settings out of the source `Preferences`.
    pub copy_preferences: bool,
    pub copy_bookmarks: bool,
    pub extensions: ExtensionPolicy,
}

impl Default for ClonePolicy {
    fn default() -> Self {
        ClonePolicy {
            copy_preferences: true,
            copy_bookmarks: false,
            extensions: ExtensionPolicy::None,
        }
    }
}

/// Request to create a new profile, optionally from a template profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProfileSpec {
    pub display_name: String,
    /// Sanitized directory name. `None` means "let the adapter pick the next
    /// `Profile N` slot".
    pub directory: Option<String>,
    /// Directory name of the profile used as a template, if any.
    pub template_directory: Option<String>,
    pub avatar_source: Option<PathBuf>,
    pub clone_policy: ClonePolicy,
    pub open_after_create: bool,
}

impl CreateProfileSpec {
    pub fn new(display_name: impl Into<String>) -> Self {
        CreateProfileSpec {
            display_name: display_name.into(),
            directory: None,
            template_directory: None,
            avatar_source: None,
            clone_policy: ClonePolicy::default(),
            open_after_create: false,
        }
    }
}

/// Request to clone an existing profile under a new identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneProfileSpec {
    pub display_name: String,
    pub directory: Option<String>,
    pub avatar_source: Option<PathBuf>,
    pub clone_policy: ClonePolicy,
    pub open_after_create: bool,
}

/// One line of a rendered plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub description: String,
    pub detail: Option<String>,
}

impl PlanStep {
    pub fn new(description: impl Into<String>) -> Self {
        PlanStep {
            description: description.into(),
            detail: None,
        }
    }

    pub fn with_detail(description: impl Into<String>, detail: impl Into<String>) -> Self {
        PlanStep {
            description: description.into(),
            detail: Some(detail.into()),
        }
    }
}

/// A fully described, not-yet-executed operation. The same value backs the CLI
/// `--dry-run` output and the TUI confirmation dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationPlan {
    pub kind: OperationKind,
    /// Browser label, e.g. "Brave Browser Beta (Beta)".
    pub browser: String,
    /// Profile the operation acts on or creates.
    pub profile: String,
    pub steps: Vec<PlanStep>,
    /// Data areas the operation copies, for clone/create plans.
    pub copies: Vec<String>,
    /// Data areas deliberately excluded, for clone/create plans.
    pub excludes: Vec<String>,
    pub paths_affected: Vec<PathBuf>,
    /// Bytes freed or removed, when the operation can estimate it.
    pub reclaimed_bytes: Option<u64>,
    /// True when the browser must be fully closed before executing.
    pub requires_browser_closed: bool,
}

impl OperationPlan {
    pub fn new(
        kind: OperationKind,
        browser: impl Into<String>,
        profile: impl Into<String>,
    ) -> Self {
        OperationPlan {
            kind,
            browser: browser.into(),
            profile: profile.into(),
            steps: Vec::new(),
            copies: Vec::new(),
            excludes: Vec::new(),
            paths_affected: Vec::new(),
            reclaimed_bytes: None,
            requires_browser_closed: false,
        }
    }

    /// Plain-text rendering shared by the CLI dry-run output and the TUI dialog.
    pub fn lines(&self) -> Vec<String> {
        let mut out = vec![
            self.kind.title().to_string(),
            String::new(),
            format!("Browser:   {}", self.browser),
            format!("Profile:   {}", self.profile),
        ];
        if !self.steps.is_empty() {
            out.push(String::new());
            out.push("Steps:".to_string());
            for step in &self.steps {
                match &step.detail {
                    Some(detail) => out.push(format!("  {} — {}", step.description, detail)),
                    None => out.push(format!("  {}", step.description)),
                }
            }
        }
        if !self.copies.is_empty() {
            out.push(String::new());
            out.push("Copy:".to_string());
            out.extend(self.copies.iter().map(|c| format!("  {c}")));
        }
        if !self.excludes.is_empty() {
            out.push(String::new());
            out.push("Exclude:".to_string());
            out.extend(self.excludes.iter().map(|c| format!("  {c}")));
        }
        if let Some(bytes) = self.reclaimed_bytes {
            out.push(String::new());
            out.push(format!(
                "Reclaimed: {}",
                crate::fs::size::format_bytes(bytes)
            ));
        }
        if self.requires_browser_closed {
            out.push(String::new());
            out.push("Requires the browser to be fully closed.".to_string());
        }
        out
    }
}

impl fmt::Display for OperationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.lines().join("\n"))
    }
}
