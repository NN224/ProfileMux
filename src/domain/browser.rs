use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Stable identity of one browser installation, derived from its kind and the
/// normalized user data root. Never derived from a display name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BrowserInstallId(String);

impl BrowserInstallId {
    pub fn new(kind: BrowserKind, user_data_root: &std::path::Path) -> Self {
        BrowserInstallId(format!("{}:{}", kind.slug(), user_data_root.display()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BrowserInstallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Family a browser belongs to. Drives which adapter handles it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrowserFamily {
    Chromium,
    Firefox,
    Safari,
}

/// Every browser ProfileMux knows how to name. The slug is the stable
/// user-facing identifier used by CLI selectors (`--browser brave-beta`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BrowserKind {
    Brave,
    BraveBeta,
    BraveNightly,
    Chrome,
    ChromeBeta,
    ChromeDev,
    ChromeCanary,
    Chromium,
    Edge,
    EdgeBeta,
    EdgeDev,
    EdgeCanary,
    Vivaldi,
    Opera,
    Firefox,
    FirefoxDeveloper,
    FirefoxNightly,
    Safari,
}

impl BrowserKind {
    pub fn slug(self) -> &'static str {
        match self {
            BrowserKind::Brave => "brave",
            BrowserKind::BraveBeta => "brave-beta",
            BrowserKind::BraveNightly => "brave-nightly",
            BrowserKind::Chrome => "chrome",
            BrowserKind::ChromeBeta => "chrome-beta",
            BrowserKind::ChromeDev => "chrome-dev",
            BrowserKind::ChromeCanary => "chrome-canary",
            BrowserKind::Chromium => "chromium",
            BrowserKind::Edge => "edge",
            BrowserKind::EdgeBeta => "edge-beta",
            BrowserKind::EdgeDev => "edge-dev",
            BrowserKind::EdgeCanary => "edge-canary",
            BrowserKind::Vivaldi => "vivaldi",
            BrowserKind::Opera => "opera",
            BrowserKind::Firefox => "firefox",
            BrowserKind::FirefoxDeveloper => "firefox-developer",
            BrowserKind::FirefoxNightly => "firefox-nightly",
            BrowserKind::Safari => "safari",
        }
    }

    pub fn family(self) -> BrowserFamily {
        match self {
            BrowserKind::Firefox | BrowserKind::FirefoxDeveloper | BrowserKind::FirefoxNightly => {
                BrowserFamily::Firefox
            }
            BrowserKind::Safari => BrowserFamily::Safari,
            _ => BrowserFamily::Chromium,
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        const ALL: [BrowserKind; 18] = [
            BrowserKind::Brave,
            BrowserKind::BraveBeta,
            BrowserKind::BraveNightly,
            BrowserKind::Chrome,
            BrowserKind::ChromeBeta,
            BrowserKind::ChromeDev,
            BrowserKind::ChromeCanary,
            BrowserKind::Chromium,
            BrowserKind::Edge,
            BrowserKind::EdgeBeta,
            BrowserKind::EdgeDev,
            BrowserKind::EdgeCanary,
            BrowserKind::Vivaldi,
            BrowserKind::Opera,
            BrowserKind::Firefox,
            BrowserKind::FirefoxDeveloper,
            BrowserKind::FirefoxNightly,
            BrowserKind::Safari,
        ];
        ALL.into_iter().find(|k| k.slug() == slug)
    }
}

impl fmt::Display for BrowserKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// Release channel of an installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
    Stable,
    Beta,
    Dev,
    Nightly,
    Canary,
}

impl Channel {
    pub fn label(self) -> &'static str {
        match self {
            Channel::Stable => "Stable",
            Channel::Beta => "Beta",
            Channel::Dev => "Dev",
            Channel::Nightly => "Nightly",
            Channel::Canary => "Canary",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// How much ProfileMux can do with an installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupportLevel {
    Full,
    Partial,
    ReadOnly,
    Unsupported,
}

impl SupportLevel {
    pub fn label(self) -> &'static str {
        match self {
            SupportLevel::Full => "Full",
            SupportLevel::Partial => "Partial",
            SupportLevel::ReadOnly => "Read-only",
            SupportLevel::Unsupported => "Unsupported",
        }
    }
}

impl fmt::Display for SupportLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// One discovered browser installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserInstall {
    pub id: BrowserInstallId,
    pub kind: BrowserKind,
    /// Human-facing name, e.g. "Brave Browser Beta".
    pub name: String,
    pub channel: Channel,
    /// Path to the application bundle or executable.
    pub app_path: PathBuf,
    pub bundle_id: Option<String>,
    pub version: Option<String>,
    /// Root directory holding the browser's profiles.
    pub user_data_root: PathBuf,
    /// Root directory holding per-profile caches, when the platform separates them.
    pub cache_root: Option<PathBuf>,
    pub support: SupportLevel,
}

impl BrowserInstall {
    /// Display label used by both CLI and TUI, e.g. "Brave Browser Beta (Beta)".
    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.channel)
    }
}
