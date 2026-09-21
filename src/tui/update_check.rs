//! Background update check for the TUI.
//!
//! Mirrors the size scanner: one detached thread, one `mpsc` message, drained
//! on the event tick. No HTTP call ever happens on the render or event thread,
//! and the first frame draws before any result arrives.

use std::sync::mpsc::{channel, Receiver};
use std::thread;

use crate::update::source::GithubReleaseSource;
use crate::update::{check_update, current_version, UpdateStatus, RELEASE_OWNER, RELEASE_REPO};

/// Result of one background check. A failure is reported as `Unavailable` and
/// never surfaced as an error, so a temporarily offline GitHub stays quiet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckResult {
    Available { current: String, latest: String },
    UpToDate,
    Unavailable,
}

impl UpdateCheckResult {
    pub fn is_available(&self) -> bool {
        matches!(self, UpdateCheckResult::Available { .. })
    }
}

pub struct UpdateChecker {
    rx: Receiver<UpdateCheckResult>,
}

impl UpdateChecker {
    /// Starts one check in the background and returns immediately.
    pub fn spawn() -> Self {
        let (tx, rx) = channel();
        let _ = thread::Builder::new()
            .name("pmux-update-check".to_string())
            .spawn(move || {
                let _ = tx.send(run_check());
            });
        UpdateChecker { rx }
    }

    /// Non-blocking read of the result, if one has arrived.
    pub fn poll(&self) -> Option<UpdateCheckResult> {
        self.rx.try_recv().ok()
    }
}

fn run_check() -> UpdateCheckResult {
    let Ok(current) = current_version() else {
        return UpdateCheckResult::Unavailable;
    };
    let source = GithubReleaseSource::new(RELEASE_OWNER, RELEASE_REPO);
    match check_update(&source, &current) {
        Ok(UpdateStatus::Available { current, latest }) => UpdateCheckResult::Available {
            current: current.to_string(),
            latest: latest.to_string(),
        },
        Ok(UpdateStatus::UpToDate { .. }) => UpdateCheckResult::UpToDate,
        Err(_) => UpdateCheckResult::Unavailable,
    }
}

/// Status-bar text for the current check state. `width` lets a narrow terminal
/// drop the suffix rather than wrap.
pub fn status_text(result: Option<&UpdateCheckResult>, version: &str, width: u16) -> String {
    let base = format!("ProfileMux v{version}");
    match result {
        Some(UpdateCheckResult::Available { latest, .. }) if width >= 80 => {
            format!("{base}   \u{2191} v{latest} available")
        }
        _ => base,
    }
}

/// Why the update action is unavailable, for the status line.
pub fn disabled_reason(result: Option<&UpdateCheckResult>) -> &'static str {
    match result {
        Some(UpdateCheckResult::UpToDate) => "Update: already up to date",
        _ => "Update: status unavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available() -> UpdateCheckResult {
        UpdateCheckResult::Available {
            current: "1.1.0".to_string(),
            latest: "1.2.0".to_string(),
        }
    }

    #[test]
    fn unknown_state_shows_plain_version() {
        assert_eq!(status_text(None, "1.1.0", 120), "ProfileMux v1.1.0");
    }

    #[test]
    fn unavailable_and_up_to_date_show_plain_version() {
        assert_eq!(
            status_text(Some(&UpdateCheckResult::Unavailable), "1.1.0", 120),
            "ProfileMux v1.1.0"
        );
        let up_to_date = status_text(Some(&UpdateCheckResult::UpToDate), "1.1.0", 120);
        assert_eq!(up_to_date, "ProfileMux v1.1.0");
        assert!(!up_to_date.contains('\u{2191}'));
    }

    #[test]
    fn available_state_shows_both_versions() {
        let text = status_text(Some(&available()), "1.1.0", 120);
        assert!(text.contains("1.1.0"));
        assert!(text.contains("1.2.0"));
        assert!(text.contains('\u{2191}'));
    }

    #[test]
    fn narrow_terminal_drops_the_suffix() {
        assert_eq!(
            status_text(Some(&available()), "1.1.0", 70),
            "ProfileMux v1.1.0"
        );
    }

    #[test]
    fn disabled_reasons_are_distinct() {
        assert_eq!(
            disabled_reason(Some(&UpdateCheckResult::UpToDate)),
            "Update: already up to date"
        );
        assert_eq!(disabled_reason(None), "Update: status unavailable");
        assert_eq!(
            disabled_reason(Some(&UpdateCheckResult::Unavailable)),
            "Update: status unavailable"
        );
    }
}
