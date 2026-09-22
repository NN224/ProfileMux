//! Per-profile appearance state.
//!
//! Two independent concepts that must never be conflated:
//!
//! * [`BrowserTheme`] — the browser's own UI colour scheme, persisted in the
//!   profile's `Preferences` (`browser.theme.color_scheme2`, plus Brave's
//!   `brave.darker_mode` for the darker variant).
//! * [`WebDarkMode`] — Chromium's automatic darkening of web content, which is
//!   a launch-time feature (`WebContentsForceDark`) and is not a profile
//!   preference.

use serde::{Deserialize, Serialize};

/// The browser's UI colour scheme for one profile.
///
/// `System`, `Dark` and `UltraDark` are the settable options. `Light` is only
/// ever reported when a profile already holds that value, and `Unknown` when
/// the preference cannot be determined — ProfileMux never infers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserTheme {
    System,
    Light,
    Dark,
    /// Brave's darker UI variant: dark colour scheme plus `brave.darker_mode`.
    UltraDark,
    Unknown,
}

impl BrowserTheme {
    pub fn label(self) -> &'static str {
        match self {
            BrowserTheme::System => "System",
            BrowserTheme::Light => "Light",
            BrowserTheme::Dark => "Dark",
            BrowserTheme::UltraDark => "Ultra Dark",
            BrowserTheme::Unknown => "Unknown",
        }
    }

    /// CLI spelling accepted by `--theme`.
    pub fn from_cli(value: &str) -> Option<Self> {
        match value {
            "system" => Some(BrowserTheme::System),
            "dark" => Some(BrowserTheme::Dark),
            "ultra-dark" => Some(BrowserTheme::UltraDark),
            _ => None,
        }
    }

    /// The options a user may set, in display order.
    pub const SETTABLE: [BrowserTheme; 3] = [
        BrowserTheme::System,
        BrowserTheme::Dark,
        BrowserTheme::UltraDark,
    ];
}

/// Automatic darkening of web content. Launch-time only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebDarkMode {
    Normal,
    /// Experimental: Chromium darkens pages that provide no dark theme of
    /// their own. It is not a site's native dark mode and can alter rendering.
    ForceDark,
}

impl WebDarkMode {
    pub fn label(self) -> &'static str {
        match self {
            WebDarkMode::Normal => "Off",
            WebDarkMode::ForceDark => "Force Dark (Experimental)",
        }
    }

    pub fn from_cli(value: &str) -> Option<Self> {
        match value {
            "off" => Some(WebDarkMode::Normal),
            "force" => Some(WebDarkMode::ForceDark),
            _ => None,
        }
    }
}

/// Appearance state of one profile as ProfileMux observes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appearance {
    pub theme: BrowserTheme,
    pub web_dark: WebDarkMode,
}

/// What an adapter can change about a profile's appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppearanceCapabilities {
    /// `System` and `Dark` through `browser.theme.color_scheme2`.
    pub browser_theme: bool,
    /// Brave's `brave.darker_mode`.
    pub ultra_dark: bool,
    /// Launch-time `WebContentsForceDark`.
    pub web_dark: bool,
}

impl AppearanceCapabilities {
    pub const NONE: AppearanceCapabilities = AppearanceCapabilities {
        browser_theme: false,
        ultra_dark: false,
        web_dark: false,
    };

    /// Reason an option is unavailable, for the dialog and the CLI.
    pub fn reason_unavailable(&self, option: &str) -> Option<&'static str> {
        match option {
            "ultra-dark" if !self.ultra_dark => Some("Brave-only"),
            "theme" if !self.browser_theme => Some("not supported by this browser"),
            "web-dark" if !self.web_dark => Some("not supported by this browser"),
            _ => None,
        }
    }
}

/// Request to change appearance. `None` leaves that dimension untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppearanceSpec {
    pub theme: Option<BrowserTheme>,
    pub web_dark: Option<WebDarkMode>,
}

impl AppearanceSpec {
    pub fn is_empty(&self) -> bool {
        self.theme.is_none() && self.web_dark.is_none()
    }
}
