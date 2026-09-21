use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::domain::DeleteMode;
use crate::tui::app::App;
use crate::tui::dialogs::{
    expand_tilde, BrowserRunningDialog, ConfirmButton, ConfirmationDialog, DeleteInfo, Dialog,
    DoctorDialog, ErrorDialog, PendingAction, QuitWaitState, TextInputDialog, TextInputKind,
};
use crate::tui::form::{FormField, ProfileForm};
use crate::tui::keymap::{map_key, Action};

/// Handles incoming key events and delegates to state updates.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.kind != KeyEventKind::Press {
        return;
    }

    if app.active_dialog.is_some() {
        handle_dialog_key(app, key.code);
        return;
    }

    if !app.filter_mode && app.status_message.is_some() && app.waiting_for_quit.is_none() {
        app.status_message = None;
    }

    if app.filter_mode {
        handle_filter_key(app, key.code);
        return;
    }

    if app.show_help || app.show_browser_overlay || app.show_profile_overlay {
        handle_overlay_key(app, key.code);
        return;
    }

    if let Some(action) = map_key(key) {
        execute_action(app, action);
    }
}

pub fn handle_dialog_key(app: &mut App, code: KeyCode) {
    if code == KeyCode::Esc {
        app.close_dialog();
        return;
    }

    // Take the dialog out while handling so the handlers can also borrow `app`;
    // it is put back unless the handler closed or replaced it.
    let Some(mut dialog) = app.active_dialog.take() else {
        return;
    };

    match &mut dialog {
        Dialog::TextInput(d) => handle_text_input_key(app, d, code),
        Dialog::Form(f) => handle_form_key(app, f, code),
        Dialog::Confirmation(c) => handle_confirmation_key(app, c, code),
        Dialog::BrowserRunning(b) => handle_browser_running_key(app, b, code),
        Dialog::Doctor(_) | Dialog::Error(_) => {
            if code == KeyCode::Enter {
                app.close_dialog();
            }
        }
    }

    if app.active_dialog.is_none() && !app.dialog_consumed {
        app.active_dialog = Some(dialog);
    }
    app.dialog_consumed = false;
}

fn handle_text_input_key(app: &mut App, d: &mut TextInputDialog, code: KeyCode) {
    match code {
        KeyCode::Char(c) => d.handle_char(c),
        KeyCode::Backspace => d.handle_backspace(),
        KeyCode::Left => d.focused_button = 0,
        KeyCode::Right => d.focused_button = 1,
        KeyCode::Tab => d.focused_button = (d.focused_button + 1) % 2,
        KeyCode::Enter => submit_text_input(app),
        _ => {}
    }
}

fn submit_text_input(app: &mut App) {
    let Some(Dialog::TextInput(d)) = app.active_dialog.take() else {
        return;
    };
    if d.focused_button == 1 {
        return;
    }
    match d.kind {
        TextInputKind::Rename {
            browser_index,
            profile,
        } => {
            let new_name = d.value.trim().to_string();
            if !new_name.is_empty() {
                app.queue_mutation(PendingAction::RenameProfile {
                    browser_index,
                    profile,
                    new_name,
                });
            }
        }
        TextInputKind::Avatar {
            browser_index,
            profile,
        } => {
            submit_avatar_input(app, browser_index, profile, d.value.trim());
        }
    }
}

fn submit_avatar_input(
    app: &mut App,
    b_idx: usize,
    profile: crate::domain::BrowserProfile,
    path_str: &str,
) {
    if path_str.is_empty() {
        return;
    }
    let path = expand_tilde(path_str);
    let Some(browser) = app.browsers.get(b_idx) else {
        return;
    };
    match browser.adapter.plan_set_avatar(&profile, &path) {
        Ok(plan) => {
            let action = PendingAction::SetAvatar {
                browser_index: b_idx,
                profile,
                path,
            };
            app.open_dialog(Dialog::confirmation(ConfirmationDialog::new(
                "Set Profile Avatar",
                plan,
                None,
                "Set Avatar",
                action,
            )));
        }
        Err(e) => {
            app.open_dialog(Dialog::Error(ErrorDialog {
                title: "Set Avatar Failed".to_string(),
                message: e.to_string(),
            }));
        }
    }
}

fn handle_form_key(app: &mut App, form: &mut ProfileForm, code: KeyCode) {
    match code {
        KeyCode::Tab | KeyCode::Down => form.next_field(),
        KeyCode::BackTab | KeyCode::Up => form.prev_field(),
        KeyCode::Left => match form.focused_field {
            FormField::Template => form.cycle_template_prev(),
            FormField::Extensions => form.cycle_extensions_prev(),
            FormField::OpenAfterCreate => form.toggle_open_after_create(),
            FormField::Buttons => form.focused_button = 0,
            _ => {}
        },
        KeyCode::Right => match form.focused_field {
            FormField::Template => form.cycle_template_next(),
            FormField::Extensions => form.cycle_extensions_next(),
            FormField::OpenAfterCreate => form.toggle_open_after_create(),
            FormField::Buttons => form.focused_button = 1,
            _ => {}
        },
        KeyCode::Char(c) => match form.focused_field {
            FormField::Name => form.handle_name_char(c),
            FormField::Directory => form.handle_directory_char(c),
            FormField::Avatar => form.handle_avatar_char(c),
            FormField::OpenAfterCreate if c == ' ' => form.toggle_open_after_create(),
            _ => {}
        },
        KeyCode::Backspace => match form.focused_field {
            FormField::Name => form.handle_name_backspace(),
            FormField::Directory => form.handle_directory_backspace(),
            FormField::Avatar => form.handle_avatar_backspace(),
            _ => {}
        },
        KeyCode::Enter => {
            if form.focused_field == FormField::Buttons && form.focused_button == 1 {
                app.close_dialog();
            } else if form.focused_field == FormField::Buttons
                || form.focused_field == FormField::OpenAfterCreate
            {
                submit_form(app);
            } else {
                form.next_field();
            }
        }
        _ => {}
    }
}

fn submit_form(app: &mut App) {
    let Some(Dialog::Form(form)) = app.active_dialog.take() else {
        return;
    };
    let b_idx = app.selected_browser;
    let Some(browser) = app.browsers.get(b_idx) else {
        return;
    };

    if form.is_clone {
        submit_clone_form(app, b_idx, &form);
    } else {
        let spec = form.to_create_spec();
        match browser.adapter.plan_create(&spec) {
            Ok(plan) => {
                let action = PendingAction::CreateProfile {
                    browser_index: b_idx,
                    spec,
                };
                app.open_dialog(Dialog::confirmation(ConfirmationDialog::new(
                    "Create Profile",
                    plan,
                    None,
                    "Create Profile",
                    action,
                )));
            }
            Err(e) => {
                app.open_dialog(Dialog::Error(ErrorDialog {
                    title: "Create Profile Failed".to_string(),
                    message: e.to_string(),
                }));
            }
        }
    }
}

fn submit_clone_form(app: &mut App, b_idx: usize, form: &ProfileForm) {
    let Some(browser) = app.browsers.get(b_idx) else {
        return;
    };
    let Some(source) = app.current_profile().cloned() else {
        return;
    };
    let spec = form.to_clone_spec();
    match browser.adapter.plan_clone(&source, &spec) {
        Ok(plan) => {
            let action = PendingAction::CloneProfile {
                browser_index: b_idx,
                source_profile: source,
                spec,
            };
            app.open_dialog(Dialog::confirmation(ConfirmationDialog::new(
                "Clone Profile",
                plan,
                None,
                "Clone Profile",
                action,
            )));
        }
        Err(e) => {
            app.open_dialog(Dialog::Error(ErrorDialog {
                title: "Clone Profile Failed".to_string(),
                message: e.to_string(),
            }));
        }
    }
}

fn handle_confirmation_key(app: &mut App, c: &mut ConfirmationDialog, code: KeyCode) {
    match code {
        KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
            c.focused_button = c.focused_button.toggle();
        }
        KeyCode::Enter => {
            if c.focused_button == ConfirmButton::Safe {
                app.close_dialog();
            } else {
                let plan = c.plan.clone();
                let action = c.action.clone();
                confirm_action(app, plan, action);
            }
        }
        _ => {}
    }
}

fn confirm_action(app: &mut App, plan: crate::domain::OperationPlan, action: PendingAction) {
    let b_idx = match &action {
        PendingAction::CreateProfile { browser_index, .. } => *browser_index,
        PendingAction::CloneProfile { browser_index, .. } => *browser_index,
        PendingAction::RenameProfile { browser_index, .. } => *browser_index,
        PendingAction::DeleteProfile { browser_index, .. } => *browser_index,
        PendingAction::SetAvatar { browser_index, .. } => *browser_index,
        PendingAction::CleanCache { browser_index, .. } => *browser_index,
    };

    let is_running = app
        .browsers
        .get(b_idx)
        .map(|b| b.adapter.is_running())
        .unwrap_or(false);

    if plan.requires_browser_closed && is_running {
        let name = app
            .browsers
            .get(b_idx)
            .map(|b| b.install.name.clone())
            .unwrap_or_default();
        app.open_dialog(Dialog::BrowserRunning(BrowserRunningDialog::new(
            b_idx, name, action,
        )));
    } else {
        app.queue_mutation(action);
    }
}

fn handle_browser_running_key(app: &mut App, b: &mut BrowserRunningDialog, code: KeyCode) {
    match code {
        KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
            b.focused_button = b.focused_button.toggle();
        }
        KeyCode::Enter => {
            if b.focused_button == ConfirmButton::Safe {
                app.close_dialog();
            } else {
                let b_idx = b.browser_index;
                let b_name = b.browser_name.clone();
                let action = b.pending_action.clone();
                request_quit_browser(app, b_idx, b_name, action);
            }
        }
        _ => {}
    }
}

fn request_quit_browser(app: &mut App, b_idx: usize, b_name: String, action: PendingAction) {
    let Some(browser) = app.browsers.get(b_idx) else {
        app.close_dialog();
        return;
    };
    match browser.adapter.request_quit() {
        Ok(()) => {
            app.close_dialog();
            app.status_message = Some(format!("Waiting for {b_name} to quit..."));
            app.waiting_for_quit = Some(QuitWaitState {
                browser_index: b_idx,
                browser_name: b_name,
                start_time: std::time::Instant::now(),
                pending_action: action,
            });
        }
        Err(e) => {
            app.open_dialog(Dialog::Error(ErrorDialog {
                title: "Quit Request Failed".to_string(),
                message: e.to_string(),
            }));
        }
    }
}

fn handle_filter_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.cancel_filter(),
        KeyCode::Enter => app.accept_filter(),
        KeyCode::Backspace => app.filter_pop(),
        KeyCode::Char(c) => app.filter_push(c),
        _ => {}
    }
}

fn handle_overlay_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.close_overlays(),
        KeyCode::Char('?') if app.show_help => app.show_help = false,
        KeyCode::Char('b') if app.show_browser_overlay => app.show_browser_overlay = false,
        KeyCode::Char('i') if app.show_profile_overlay => app.show_profile_overlay = false,
        KeyCode::Enter if app.show_profile_overlay => app.show_profile_overlay = false,
        KeyCode::Char('q') => app.should_quit = true,
        _ => {}
    }
}

fn execute_action(app: &mut App, action: Action) {
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
        Action::CleanCache => handle_clean_cache(app),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browsers::BrowserAdapter;
    use crate::domain::{
        BrowserCapabilities, BrowserInstall, BrowserInstallId, BrowserKind, BrowserProfile,
        Channel, HealthFinding, OperationKind, OperationPlan, ProfileId, ProfileStoreSnapshot,
        SupportLevel,
    };
    use crate::error::Result;
    use crate::tui::app::BrowserItem;
    use std::path::{Path, PathBuf};

    struct TestAdapter {
        install: BrowserInstall,
        caps: BrowserCapabilities,
    }

    impl BrowserAdapter for TestAdapter {
        fn install(&self) -> &BrowserInstall {
            &self.install
        }
        fn capabilities(&self) -> BrowserCapabilities {
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

    fn make_test_app(caps: BrowserCapabilities) -> App {
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
            last_active: None,
            registered: true,
            directory_exists: true,
            size: None,
        };
        let adapter = Box::new(TestAdapter {
            install: install.clone(),
            caps,
        });
        let browser_item = BrowserItem {
            install,
            capabilities: caps,
            is_running: false,
            profiles: vec![profile],
            doctor_findings: Vec::new(),
            adapter,
        };
        App::with_browsers(vec![browser_item])
    }

    #[test]
    fn test_disabled_capability_produces_reason_and_no_action() {
        let caps = BrowserCapabilities {
            create: false,
            ..BrowserCapabilities::READ_ONLY
        };
        let mut app = make_test_app(caps);
        execute_action(&mut app, Action::NewProfile);

        assert_eq!(
            app.status_message.as_deref(),
            Some("not supported by this browser adapter")
        );
        assert!(app.active_dialog.is_none());
        assert!(app.pending_mutation.is_none());
    }

    #[test]
    fn test_escape_from_every_dialog_kind_returns_to_main_view() {
        let caps = BrowserCapabilities::READ_ONLY;
        let mut app = make_test_app(caps);
        let prof = app.current_profile().unwrap().clone();
        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Test", "Default");

        let dialogs = vec![
            Dialog::TextInput(TextInputDialog::new_rename(0, prof.clone())),
            Dialog::form(ProfileForm::new_create(PathBuf::from("/tmp"), &[])),
            Dialog::confirmation(ConfirmationDialog::new(
                "Confirm",
                plan,
                None,
                "Delete",
                PendingAction::DeleteProfile {
                    browser_index: 0,
                    profile: prof.clone(),
                    mode: DeleteMode::Trash,
                },
            )),
            Dialog::BrowserRunning(BrowserRunningDialog::new(
                0,
                "Test",
                PendingAction::DeleteProfile {
                    browser_index: 0,
                    profile: prof,
                    mode: DeleteMode::Trash,
                },
            )),
            Dialog::Doctor(DoctorDialog {
                browser_name: "Test".to_string(),
                findings: Vec::new(),
                scroll: 0,
            }),
            Dialog::Error(ErrorDialog {
                title: "Err".to_string(),
                message: "msg".to_string(),
            }),
        ];

        for d in dialogs {
            app.open_dialog(d);
            assert!(app.active_dialog.is_some());
            handle_dialog_key(&mut app, KeyCode::Esc);
            assert!(app.active_dialog.is_none());
            assert!(app.pending_mutation.is_none());
        }
    }
}
