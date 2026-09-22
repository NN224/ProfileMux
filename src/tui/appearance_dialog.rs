use crate::domain::{
    Appearance, AppearanceCapabilities, AppearanceSpec, BrowserProfile, BrowserTheme, WebDarkMode,
};
use crate::policy::LaunchPolicy;
use crate::tui::dialogs::{ConfirmButton, DialogOutcome};

/// The three navigable control groups in the appearance dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceControl {
    BrowserUi,
    WebContent,
    Buttons,
}

/// Dialog for configuring per-profile browser theme and web content dark mode.
#[derive(Debug, Clone)]
pub struct AppearanceDialog {
    pub browser_index: usize,
    pub profile: BrowserProfile,
    pub capabilities: AppearanceCapabilities,
    pub initial_theme: BrowserTheme,
    pub initial_web_dark: WebDarkMode,
    pub selected_theme: BrowserTheme,
    pub selected_web_dark: WebDarkMode,
    pub focused_control: AppearanceControl,
    pub focused_button: ConfirmButton,
    pub focused_theme: BrowserTheme,
    pub focused_web_dark: WebDarkMode,
}

impl AppearanceDialog {
    pub fn new(
        browser_index: usize,
        profile: BrowserProfile,
        capabilities: AppearanceCapabilities,
        current: Appearance,
        policy: LaunchPolicy,
    ) -> Self {
        let selected_theme = match current.theme {
            BrowserTheme::UltraDark if capabilities.ultra_dark => BrowserTheme::UltraDark,
            BrowserTheme::Dark if capabilities.browser_theme => BrowserTheme::Dark,
            BrowserTheme::System if capabilities.browser_theme => BrowserTheme::System,
            _ => BrowserTheme::SETTABLE
                .iter()
                .copied()
                .find(|&t| match t {
                    BrowserTheme::UltraDark => capabilities.ultra_dark,
                    BrowserTheme::Dark | BrowserTheme::System => capabilities.browser_theme,
                    _ => false,
                })
                .unwrap_or(BrowserTheme::System),
        };

        let selected_web_dark = match policy.web_dark {
            WebDarkMode::ForceDark if capabilities.web_dark => WebDarkMode::ForceDark,
            _ => WebDarkMode::Normal,
        };

        Self {
            browser_index,
            profile,
            capabilities,
            initial_theme: selected_theme,
            initial_web_dark: selected_web_dark,
            selected_theme,
            selected_web_dark,
            // Focuses the safe button's group first.
            focused_control: AppearanceControl::Buttons,
            focused_button: ConfirmButton::Safe,
            focused_theme: selected_theme,
            focused_web_dark: selected_web_dark,
        }
    }

    pub fn is_theme_supported(&self, theme: BrowserTheme) -> bool {
        match theme {
            BrowserTheme::System | BrowserTheme::Dark => self.capabilities.browser_theme,
            BrowserTheme::UltraDark => self.capabilities.ultra_dark,
            BrowserTheme::Light | BrowserTheme::Unknown => false,
        }
    }

    pub fn is_web_dark_supported(&self, mode: WebDarkMode) -> bool {
        match mode {
            WebDarkMode::Normal => true,
            WebDarkMode::ForceDark => self.capabilities.web_dark,
        }
    }

    pub fn activate_selected(&self) -> DialogOutcome {
        match self.focused_control {
            AppearanceControl::Buttons => match self.focused_button {
                ConfirmButton::Action => DialogOutcome::Activate,
                ConfirmButton::Safe => DialogOutcome::Close,
            },
            AppearanceControl::BrowserUi | AppearanceControl::WebContent => DialogOutcome::None,
        }
    }

    pub fn select_focused(&mut self) {
        match self.focused_control {
            AppearanceControl::BrowserUi => {
                if self.is_theme_supported(self.focused_theme) {
                    self.selected_theme = self.focused_theme;
                }
            }
            AppearanceControl::WebContent => {
                if self.is_web_dark_supported(self.focused_web_dark) {
                    self.selected_web_dark = self.focused_web_dark;
                }
            }
            AppearanceControl::Buttons => {}
        }
    }

    pub fn next_control(&mut self) {
        self.focused_control = match self.focused_control {
            AppearanceControl::BrowserUi => AppearanceControl::WebContent,
            AppearanceControl::WebContent => AppearanceControl::Buttons,
            AppearanceControl::Buttons => AppearanceControl::BrowserUi,
        };
    }

    pub fn prev_control(&mut self) {
        self.focused_control = match self.focused_control {
            AppearanceControl::BrowserUi => AppearanceControl::Buttons,
            AppearanceControl::WebContent => AppearanceControl::BrowserUi,
            AppearanceControl::Buttons => AppearanceControl::WebContent,
        };
    }

    pub fn move_down(&mut self) {
        match self.focused_control {
            AppearanceControl::BrowserUi => {
                let supported: Vec<BrowserTheme> = BrowserTheme::SETTABLE
                    .into_iter()
                    .filter(|&t| self.is_theme_supported(t))
                    .collect();
                if supported.is_empty() {
                    return;
                }
                let idx = supported
                    .iter()
                    .position(|&t| t == self.focused_theme)
                    .unwrap_or(0);
                self.focused_theme = supported[(idx + 1) % supported.len()];
            }
            AppearanceControl::WebContent => {
                let mut supported = vec![WebDarkMode::Normal];
                if self.capabilities.web_dark {
                    supported.push(WebDarkMode::ForceDark);
                }
                if supported.len() > 1 {
                    let idx = supported
                        .iter()
                        .position(|&m| m == self.focused_web_dark)
                        .unwrap_or(0);
                    self.focused_web_dark = supported[(idx + 1) % supported.len()];
                }
            }
            AppearanceControl::Buttons => {}
        }
    }

    pub fn move_up(&mut self) {
        match self.focused_control {
            AppearanceControl::BrowserUi => {
                let supported: Vec<BrowserTheme> = BrowserTheme::SETTABLE
                    .into_iter()
                    .filter(|&t| self.is_theme_supported(t))
                    .collect();
                if supported.is_empty() {
                    return;
                }
                let idx = supported
                    .iter()
                    .position(|&t| t == self.focused_theme)
                    .unwrap_or(0);
                self.focused_theme = supported[(idx + supported.len() - 1) % supported.len()];
            }
            AppearanceControl::WebContent => {
                let mut supported = vec![WebDarkMode::Normal];
                if self.capabilities.web_dark {
                    supported.push(WebDarkMode::ForceDark);
                }
                if supported.len() > 1 {
                    let idx = supported
                        .iter()
                        .position(|&m| m == self.focused_web_dark)
                        .unwrap_or(0);
                    self.focused_web_dark =
                        supported[(idx + supported.len() - 1) % supported.len()];
                }
            }
            AppearanceControl::Buttons => {}
        }
    }

    fn browser_ui_lines(&self, lines: &mut Vec<String>) {
        lines.push("Browser UI:".to_string());
        for theme in BrowserTheme::SETTABLE {
            let label = theme.label();
            if self.is_theme_supported(theme) {
                let mark = if self.selected_theme == theme {
                    "(*)"
                } else {
                    "( )"
                };
                lines.push(format!("  {mark} {label}"));
            } else {
                let opt = match theme {
                    BrowserTheme::UltraDark => "ultra-dark",
                    _ => "theme",
                };
                let r = self
                    .capabilities
                    .reason_unavailable(opt)
                    .unwrap_or("unavailable");
                lines.push(format!("  ( ) {label}   unavailable - {r}"));
            }
        }
    }

    fn web_content_lines(&self, lines: &mut Vec<String>) {
        lines.push("Web content:".to_string());
        let norm_mark = if self.selected_web_dark == WebDarkMode::Normal {
            "(*)"
        } else {
            "( )"
        };
        lines.push(format!("  {norm_mark} Normal"));

        if self.capabilities.web_dark {
            let f_mark = if self.selected_web_dark == WebDarkMode::ForceDark {
                "(*)"
            } else {
                "( )"
            };
            lines.push(format!("  {f_mark} Force Dark (Experimental)"));
        } else {
            let r = self
                .capabilities
                .reason_unavailable("web-dark")
                .unwrap_or("unavailable");
            lines.push(format!(
                "  ( ) Force Dark (Experimental)   unavailable - {r}"
            ));
        }
    }

    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::with_capacity(10);
        lines.push("Appearance".to_string());
        lines.push(String::new());
        self.browser_ui_lines(&mut lines);
        lines.push(String::new());
        self.web_content_lines(&mut lines);
        lines
    }

    pub fn to_spec(&self) -> AppearanceSpec {
        let theme = if self.selected_theme != self.initial_theme {
            Some(self.selected_theme)
        } else {
            None
        };
        let web_dark = if self.selected_web_dark != self.initial_web_dark {
            Some(self.selected_web_dark)
        } else {
            None
        };
        AppearanceSpec { theme, web_dark }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BrowserInstallId, BrowserKind, ProfileId};
    use std::path::{Path, PathBuf};

    fn make_test_profile() -> BrowserProfile {
        let install_id =
            BrowserInstallId::new(BrowserKind::Chromium, Path::new("/tmp/test-browser"));
        BrowserProfile {
            id: ProfileId::new(&install_id, "Default"),
            install_id,
            display_name: "Default".to_string(),
            directory: "Default".to_string(),
            path: PathBuf::from("/tmp/test-browser/Default"),
            cache_path: None,
            avatar: None,
            account_email: None,
            last_active: None,
            registered: true,
            directory_exists: true,
            size: None,
        }
    }

    #[test]
    fn test_lines_marks_current_selection() {
        let caps = AppearanceCapabilities {
            browser_theme: true,
            ultra_dark: true,
            web_dark: true,
        };
        let app = Appearance {
            theme: BrowserTheme::UltraDark,
            web_dark: WebDarkMode::Normal,
        };
        let policy = LaunchPolicy {
            web_dark: WebDarkMode::Normal,
        };
        let dialog = AppearanceDialog::new(0, make_test_profile(), caps, app, policy);
        let lines = dialog.lines();

        assert!(lines.contains(&"  (*) Ultra Dark".to_string()));
        assert!(lines.contains(&"  ( ) System".to_string()));
        assert!(lines.contains(&"  ( ) Dark".to_string()));
        assert!(lines.contains(&"  (*) Normal".to_string()));
        assert!(lines.contains(&"  ( ) Force Dark (Experimental)".to_string()));
    }

    #[test]
    fn test_unsupported_option_renders_reason_and_skipped_when_cycling() {
        // Chrome-like capabilities: ultra_dark is false
        let caps = AppearanceCapabilities {
            browser_theme: true,
            ultra_dark: false,
            web_dark: true,
        };
        let app = Appearance {
            theme: BrowserTheme::System,
            web_dark: WebDarkMode::Normal,
        };
        let policy = LaunchPolicy {
            web_dark: WebDarkMode::Normal,
        };
        let mut dialog = AppearanceDialog::new(0, make_test_profile(), caps, app, policy);
        let lines = dialog.lines();

        assert!(lines.contains(&"  ( ) Ultra Dark   unavailable - Brave-only".to_string()));

        dialog.focused_control = AppearanceControl::BrowserUi;
        assert_eq!(dialog.focused_theme, BrowserTheme::System);
        dialog.move_down();
        assert_eq!(dialog.focused_theme, BrowserTheme::Dark);
        dialog.move_down();
        // UltraDark is skipped: wraps back to System
        assert_eq!(dialog.focused_theme, BrowserTheme::System);
    }

    #[test]
    fn test_tab_cycles_controls_and_wraps() {
        let caps = AppearanceCapabilities {
            browser_theme: true,
            ultra_dark: true,
            web_dark: true,
        };
        let app = Appearance {
            theme: BrowserTheme::System,
            web_dark: WebDarkMode::Normal,
        };
        let policy = LaunchPolicy::default();
        let mut dialog = AppearanceDialog::new(0, make_test_profile(), caps, app, policy);

        assert_eq!(dialog.focused_control, AppearanceControl::Buttons);
        dialog.next_control();
        assert_eq!(dialog.focused_control, AppearanceControl::BrowserUi);
        dialog.next_control();
        assert_eq!(dialog.focused_control, AppearanceControl::WebContent);
        dialog.next_control();
        assert_eq!(dialog.focused_control, AppearanceControl::Buttons);

        dialog.prev_control();
        assert_eq!(dialog.focused_control, AppearanceControl::WebContent);
        dialog.prev_control();
        assert_eq!(dialog.focused_control, AppearanceControl::BrowserUi);
        dialog.prev_control();
        assert_eq!(dialog.focused_control, AppearanceControl::Buttons);
    }

    #[test]
    fn test_enter_on_focused_radio_option_changes_selection_and_leaves_dialog_open() {
        let caps = AppearanceCapabilities {
            browser_theme: true,
            ultra_dark: true,
            web_dark: true,
        };
        let app = Appearance {
            theme: BrowserTheme::System,
            web_dark: WebDarkMode::Normal,
        };
        let policy = LaunchPolicy::default();
        let mut dialog = AppearanceDialog::new(0, make_test_profile(), caps, app, policy);

        dialog.focused_control = AppearanceControl::BrowserUi;
        assert_eq!(dialog.activate_selected(), DialogOutcome::None);

        dialog.move_down();
        assert_eq!(dialog.focused_theme, BrowserTheme::Dark);
        assert_eq!(dialog.selected_theme, BrowserTheme::System);

        dialog.select_focused();
        assert_eq!(dialog.selected_theme, BrowserTheme::Dark);
        assert_eq!(dialog.activate_selected(), DialogOutcome::None);
    }

    #[test]
    fn test_to_spec_unchanged_dialog_is_empty() {
        let caps = AppearanceCapabilities {
            browser_theme: true,
            ultra_dark: true,
            web_dark: true,
        };
        let app = Appearance {
            theme: BrowserTheme::Dark,
            web_dark: WebDarkMode::Normal,
        };
        let policy = LaunchPolicy::default();
        let dialog = AppearanceDialog::new(0, make_test_profile(), caps, app, policy);

        let spec = dialog.to_spec();
        assert!(spec.is_empty());
        assert_eq!(spec.theme, None);
        assert_eq!(spec.web_dark, None);
    }

    use crate::browsers::BrowserAdapter;
    use crate::domain::{
        BrowserCapabilities, BrowserInstall, Channel, HealthFinding, ProfileStoreSnapshot,
        SupportLevel,
    };
    use crate::error::Result;
    use crate::tui::app::{App, BrowserItem};
    use crate::tui::dialogs::{Dialog, PendingAction};
    use crate::tui::events::events_dialogs::handle_dialog_key;
    use crossterm::event::KeyCode;

    struct DummyAdapter {
        install: BrowserInstall,
        caps: AppearanceCapabilities,
    }
    impl BrowserAdapter for DummyAdapter {
        fn install(&self) -> &BrowserInstall {
            &self.install
        }
        fn capabilities(&self) -> BrowserCapabilities {
            BrowserCapabilities::READ_ONLY
        }
        fn appearance_capabilities(&self) -> AppearanceCapabilities {
            self.caps
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

    fn make_test_app(profile: BrowserProfile, caps: AppearanceCapabilities) -> App {
        let install = BrowserInstall {
            id: profile.install_id.clone(),
            name: "Test Browser".to_string(),
            kind: BrowserKind::Chromium,
            channel: Channel::Stable,
            app_path: PathBuf::from("/tmp/Test.app"),
            user_data_root: PathBuf::from("/tmp/test-browser"),
            cache_root: None,
            bundle_id: None,
            version: None,
            support: SupportLevel::Full,
        };
        let adapter = Box::new(DummyAdapter {
            install: install.clone(),
            caps,
        });
        let browser_item = BrowserItem {
            install,
            capabilities: BrowserCapabilities::READ_ONLY,
            is_running: false,
            profiles: vec![profile],
            doctor_findings: Vec::new(),
            adapter,
        };
        App::with_browsers(vec![browser_item])
    }

    #[test]
    fn test_enter_on_apply_queues_pending_action_with_only_changed_dimensions() {
        let profile = make_test_profile();
        let caps = AppearanceCapabilities {
            browser_theme: true,
            ultra_dark: true,
            web_dark: true,
        };
        let mut app = make_test_app(profile.clone(), caps);

        let mut dialog = AppearanceDialog::new(
            0,
            profile,
            caps,
            Appearance {
                theme: BrowserTheme::System,
                web_dark: WebDarkMode::Normal,
            },
            LaunchPolicy::default(),
        );
        dialog.selected_theme = BrowserTheme::Dark;
        dialog.focused_control = AppearanceControl::Buttons;
        dialog.focused_button = ConfirmButton::Action;

        app.open_dialog(Dialog::Appearance(dialog));
        handle_dialog_key(&mut app, KeyCode::Enter);

        assert!(app.active_dialog.is_none());
        match app.pending_mutation {
            Some(PendingAction::SetAppearance { spec, .. }) => {
                assert_eq!(spec.theme, Some(BrowserTheme::Dark));
                assert_eq!(spec.web_dark, None);
            }
            _ => panic!("expected SetAppearance"),
        }
    }

    #[test]
    fn test_enter_on_cancel_and_esc_close_dialog_and_queue_nothing() {
        let profile = make_test_profile();
        let caps = AppearanceCapabilities {
            browser_theme: true,
            ultra_dark: true,
            web_dark: true,
        };
        let mut app = make_test_app(profile.clone(), caps);

        let dialog = AppearanceDialog::new(
            0,
            profile,
            caps,
            Appearance {
                theme: BrowserTheme::System,
                web_dark: WebDarkMode::Normal,
            },
            LaunchPolicy::default(),
        );

        app.open_dialog(Dialog::Appearance(dialog.clone()));
        handle_dialog_key(&mut app, KeyCode::Enter);
        assert!(app.active_dialog.is_none());
        assert!(app.pending_mutation.is_none());

        app.open_dialog(Dialog::Appearance(dialog));
        handle_dialog_key(&mut app, KeyCode::Esc);
        assert!(app.active_dialog.is_none());
        assert!(app.pending_mutation.is_none());
    }
}
