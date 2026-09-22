use crate::domain::DeleteMode;
use crate::tui::app::App;
use crate::tui::dialogs::{
    ConfirmationDialog, DeleteInfo, Dialog, DoctorDialog, ErrorDialog, PendingAction,
    TextInputDialog,
};
use crate::tui::form::ProfileForm;
use crate::tui::keymap::Action;

pub fn execute_action(app: &mut App, action: Action) {
    match action {
        Action::Quit => app.should_quit = true,
        Action::FocusNext => app.focus_next(),
        Action::FocusPrev => app.focus_prev(),
        Action::FocusLeft => app.focus_left(),
        Action::FocusRight => app.focus_right(),
        Action::MoveUp => app.move_up(),
        Action::MoveDown => app.move_down(),
        Action::StartSearch => app.start_search(),
        Action::ToggleHelp => app.toggle_help(),
        Action::ToggleBrowserDetails => app.toggle_browser_overlay(),
        Action::ToggleProfileDetails => app.toggle_profile_overlay(),
        Action::CloseOverlay => {
            if !app.filter.is_empty() {
                app.cancel_filter();
            } else {
                app.close_overlays();
            }
        }
        Action::Refresh => app.rescan_selected_profile(),
        Action::OpenFolder => handle_open_folder(app),
        Action::NewProfile => handle_new_profile(app),
        Action::CloneProfile => handle_clone_profile(app),
        Action::RenameProfile => handle_rename_profile(app),
        Action::DeleteProfile => handle_delete_profile(app),
        Action::LaunchProfile => handle_launch_profile(app),
        Action::SetAvatar => handle_set_avatar(app),
        Action::Doctor => handle_doctor(app),
        Action::Update => handle_update(app),
        Action::CleanCache => handle_clean_cache(app),
        Action::Appearance => handle_appearance(app),
    }
}

pub fn check_capability(app: &mut App, cap_name: &str) -> bool {
    let Some(browser) = app.current_browser() else {
        app.status_message = Some("No browser selected".to_string());
        return false;
    };

    if let Some(reason) = browser.capabilities.reason_disabled(cap_name) {
        app.status_message = Some(reason.to_string());
        false
    } else {
        true
    }
}

fn handle_new_profile(app: &mut App) {
    if !check_capability(app, "create") {
        return;
    }
    let Some(browser) = app.current_browser() else {
        return;
    };
    let form = ProfileForm::new_create(browser.install.user_data_root.clone(), &browser.profiles);
    app.open_dialog(Dialog::form(form));
}

fn handle_clone_profile(app: &mut App) {
    if !check_capability(app, "clone") {
        return;
    }
    let Some(browser) = app.current_browser() else {
        return;
    };
    let Some(profile) = app.current_profile().cloned() else {
        app.status_message = Some("No profile selected to clone".to_string());
        return;
    };
    let form = ProfileForm::new_clone(
        browser.install.user_data_root.clone(),
        &browser.profiles,
        &profile,
    );
    app.open_dialog(Dialog::form(form));
}

fn handle_rename_profile(app: &mut App) {
    if !check_capability(app, "rename_display_name") {
        return;
    }
    let Some(profile) = app.current_profile().cloned() else {
        app.status_message = Some("No profile selected to rename".to_string());
        return;
    };
    let dialog = TextInputDialog::new_rename(app.selected_browser, profile);
    app.open_dialog(Dialog::TextInput(dialog));
}

fn handle_set_avatar(app: &mut App) {
    if !check_capability(app, "custom_avatar") {
        return;
    }
    let Some(profile) = app.current_profile().cloned() else {
        app.status_message = Some("No profile selected to set avatar".to_string());
        return;
    };
    let dialog = TextInputDialog::new_avatar(app.selected_browser, profile);
    app.open_dialog(Dialog::TextInput(dialog));
}

fn handle_delete_profile(app: &mut App) {
    if !check_capability(app, "delete") {
        return;
    }
    let Some(browser) = app.current_browser() else {
        return;
    };
    let Some(profile) = app.current_profile().cloned() else {
        app.status_message = Some("No profile selected to delete".to_string());
        return;
    };
    match browser.adapter.plan_delete(&profile, DeleteMode::Trash) {
        Ok(plan) => {
            let delete_info = DeleteInfo::from_profile(&profile);
            let action = PendingAction::DeleteProfile {
                browser_index: app.selected_browser,
                profile,
                mode: DeleteMode::Trash,
            };
            app.open_dialog(Dialog::confirmation(ConfirmationDialog::new(
                "Delete Profile",
                plan,
                Some(delete_info),
                "Move to Trash",
                action,
            )));
        }
        Err(e) => {
            app.open_dialog(Dialog::Error(ErrorDialog {
                title: "Delete Plan Failed".to_string(),
                message: e.to_string(),
            }));
        }
    }
}

fn handle_launch_profile(app: &mut App) {
    if !check_capability(app, "launch") {
        return;
    }
    let Some(browser) = app.current_browser() else {
        return;
    };
    let Some(profile) = app.current_profile().cloned() else {
        app.status_message = Some("No profile selected to launch".to_string());
        return;
    };
    match browser.adapter.launch_profile(&profile) {
        Ok(()) => {
            app.status_message = Some(format!("Launched profile '{}'", profile.display_name));
        }
        Err(e) => {
            app.open_dialog(Dialog::Error(ErrorDialog {
                title: "Launch Failed".to_string(),
                message: e.to_string(),
            }));
        }
    }
}

fn handle_clean_cache(app: &mut App) {
    if !check_capability(app, "clean_cache") {
        return;
    }
    let Some(browser) = app.current_browser() else {
        return;
    };
    let Some(profile) = app.current_profile().cloned() else {
        app.status_message = Some("No profile selected to clean".to_string());
        return;
    };
    match browser.adapter.plan_clean_cache(&profile) {
        Ok(plan) => {
            let action = PendingAction::CleanCache {
                browser_index: app.selected_browser,
                profile,
            };
            app.open_dialog(Dialog::confirmation(ConfirmationDialog::new(
                "Clean Profile Cache",
                plan,
                None,
                "Clean Cache",
                action,
            )));
        }
        Err(e) => {
            app.open_dialog(Dialog::Error(ErrorDialog {
                title: "Clean Cache Plan Failed".to_string(),
                message: e.to_string(),
            }));
        }
    }
}

fn handle_doctor(app: &mut App) {
    let Some(browser) = app.current_browser() else {
        return;
    };
    let findings = browser
        .adapter
        .doctor()
        .unwrap_or_else(|_| browser.doctor_findings.clone());
    let dialog = DoctorDialog {
        browser_name: browser.install.name.clone(),
        findings,
        scroll: 0,
    };
    app.open_dialog(Dialog::Doctor(dialog));
}

fn handle_open_folder(app: &mut App) {
    if !check_capability(app, "open_folder") {
        return;
    }

    let Some(profile) = app.current_profile() else {
        app.status_message = Some("Open Folder: no profile selected".to_string());
        return;
    };

    #[cfg(target_os = "macos")]
    {
        match std::process::Command::new("open")
            .arg("-R")
            .arg(&profile.path)
            .spawn()
        {
            Ok(_) => {
                app.status_message = Some(format!("Revealed {} in Finder", profile.directory));
            }
            Err(err) => {
                app.status_message = Some(format!("Failed to open folder: {err}"));
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = profile;
        app.status_message = Some("Open Folder: only macOS is supported in this build".to_string());
    }
}

/// Opens the update confirmation when a newer release is known, and otherwise
/// explains in the status line why the action is unavailable.
fn handle_update(app: &mut App) {
    match app.update.clone() {
        Some(crate::tui::update_check::UpdateCheckResult::Available { current, latest }) => {
            app.open_dialog(Dialog::Update(crate::tui::dialogs::UpdateDialog::new(
                current, latest,
            )));
        }
        other => {
            app.status_message =
                Some(crate::tui::update_check::disabled_reason(other.as_ref()).to_string());
        }
    }
}

fn handle_appearance(app: &mut App) {
    let Some(browser) = app.current_browser() else {
        app.status_message = Some("No browser selected".to_string());
        return;
    };
    let caps = browser.adapter.appearance_capabilities();
    if !caps.browser_theme && !caps.ultra_dark && !caps.web_dark {
        let reason = caps
            .reason_unavailable("theme")
            .unwrap_or("not supported by this browser");
        app.status_message = Some(reason.to_string());
        return;
    }
    let Some(profile) = app.current_profile().cloned() else {
        app.status_message = Some("No profile selected".to_string());
        return;
    };
    let current_appearance = match browser.adapter.read_appearance(&profile) {
        Ok(a) => a,
        Err(e) => {
            app.open_dialog(Dialog::Error(ErrorDialog {
                title: "Read Appearance Failed".to_string(),
                message: e.to_string(),
            }));
            return;
        }
    };
    let policy = crate::policy::default_path()
        .ok()
        .and_then(|p| crate::policy::load(&p).ok())
        .map(|s| s.get(&profile.id))
        .unwrap_or_default();

    let dialog = crate::tui::appearance_dialog::AppearanceDialog::new(
        app.selected_browser,
        profile,
        caps,
        current_appearance,
        policy,
    );
    app.open_dialog(Dialog::Appearance(dialog));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browsers::BrowserAdapter;
    use crate::domain::{
        AppearanceCapabilities, BrowserCapabilities, BrowserInstall, BrowserInstallId, BrowserKind,
        BrowserProfile, Channel, HealthFinding, ProfileId, ProfileStoreSnapshot, SupportLevel,
    };
    use crate::error::Result;
    use crate::tui::app::BrowserItem;
    use std::path::{Path, PathBuf};

    struct TestAdapter {
        install: BrowserInstall,
        caps: BrowserCapabilities,
        app_caps: AppearanceCapabilities,
    }

    impl BrowserAdapter for TestAdapter {
        fn install(&self) -> &BrowserInstall {
            &self.install
        }
        fn capabilities(&self) -> BrowserCapabilities {
            self.caps
        }
        fn appearance_capabilities(&self) -> AppearanceCapabilities {
            self.app_caps
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

    fn make_test_app(app_caps: AppearanceCapabilities) -> App {
        let install = BrowserInstall {
            id: BrowserInstallId::new(BrowserKind::Chromium, Path::new("/tmp/test-data")),
            name: "Test Browser".to_string(),
            kind: BrowserKind::Chromium,
            channel: Channel::Stable,
            app_path: PathBuf::from("/Applications/Test.app"),
            user_data_root: PathBuf::from("/tmp/test-data"),
            cache_root: None,
            bundle_id: None,
            version: None,
            support: SupportLevel::Full,
        };
        let profile = BrowserProfile {
            id: ProfileId::new(&install.id, "Default"),
            install_id: install.id.clone(),
            display_name: "Default".to_string(),
            directory: "Default".to_string(),
            path: PathBuf::from("/tmp/test-data/Default"),
            cache_path: None,
            avatar: None,
            account_email: None,
            last_active: None,
            registered: true,
            directory_exists: true,
            size: None,
        };
        let adapter = Box::new(TestAdapter {
            install: install.clone(),
            caps: BrowserCapabilities::READ_ONLY,
            app_caps,
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
    fn test_pressing_t_when_browser_reports_no_appearance_capability_shows_reason() {
        let mut app = make_test_app(AppearanceCapabilities::NONE);
        execute_action(&mut app, Action::Appearance);

        assert_eq!(
            app.status_message.as_deref(),
            Some("not supported by this browser")
        );
        assert!(app.active_dialog.is_none());
        assert!(app.pending_mutation.is_none());
    }
}
