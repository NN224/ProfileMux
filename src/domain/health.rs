use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Ok,
    Warning,
    Broken,
}

impl Severity {
    /// Glyph used by the TUI and CLI: healthy, warning, broken.
    pub fn glyph(self) -> &'static str {
        match self {
            Severity::Ok => "\u{2713}",
            Severity::Warning => "\u{26a0}",
            Severity::Broken => "\u{2717}",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Ok => "Healthy",
            Severity::Warning => "Warning",
            Severity::Broken => "Broken",
        }
    }
}

/// One read-only finding produced by the doctor. Findings never carry profile
/// contents, only structural facts and the paths involved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthFinding {
    pub severity: Severity,
    /// Stable machine-readable code, e.g. "profile-directory-missing".
    pub code: String,
    pub message: String,
    pub paths: Vec<PathBuf>,
}

impl HealthFinding {
    pub fn new(severity: Severity, code: &str, message: impl Into<String>) -> Self {
        HealthFinding {
            severity,
            code: code.to_string(),
            message: message.into(),
            paths: Vec::new(),
        }
    }

    /// Returns a copy with paths attached.
    pub fn with_paths(&self, paths: Vec<PathBuf>) -> Self {
        HealthFinding {
            paths,
            ..self.clone()
        }
    }
}
