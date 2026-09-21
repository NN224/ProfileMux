use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::tui::app::App;
use crate::tui::keymap::map_key;

#[path = "events_actions.rs"]
mod events_actions;

#[path = "events_dialogs.rs"]
mod events_dialogs;

pub use events_actions::check_capability;
use events_actions::execute_action;
pub use events_dialogs::{handle_dialog_key, submit_form, submit_text_input};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browsers::BrowserAdapter;
    use crate::domain::{
        BrowserCapabilities, BrowserInstall, BrowserInstallId, BrowserKind, BrowserProfile,
        Channel, DeleteMode, HealthFinding, OperationKind, OperationPlan, ProfileId,
        ProfileStoreSnapshot, SupportLevel,
    };
    use crate::error::Result;
    use crate::tui::app::{BrowserItem, Focus};
    use crate::tui::dialogs::{
        BrowserRunningDialog, ConfirmButton, ConfirmationDialog, Dialog, DoctorDialog, ErrorDialog,
        PendingAction, TextInputDialog,
    };
    use crate::tui::form::ProfileForm;
    use crate::tui::keymap::Action;
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
            account_email: None,
            last_active: None,
            registered: true,
            directory_exists: true,
            size: None,
        };
        let profile2 = BrowserProfile {
            id: ProfileId::new(&install.id, "Second Profile"),
            install_id: install.id.clone(),
            display_name: "Second Profile".to_string(),
            directory: "Second Profile".to_string(),
            path: PathBuf::from("/tmp/test-data/Second Profile"),
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
            caps,
        });
        let browser_item = BrowserItem {
            install,
            capabilities: caps,
            is_running: false,
            profiles: vec![profile, profile2],
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

    #[test]
    fn test_confirmation_dialog_scrolling_and_keys() {
        let mut app = make_test_app(BrowserCapabilities::READ_ONLY);
        let prof = app.current_profile().unwrap().clone();
        let mut plan = OperationPlan::new(OperationKind::DeleteProfile, "Test", "Default");
        for i in 0..20 {
            plan.steps
                .push(crate::domain::PlanStep::new(format!("Step {i}")));
        }
        let act = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: prof,
            mode: DeleteMode::Trash,
        };
        let open_dlg = |app: &mut App| {
            app.open_dialog(Dialog::confirmation(ConfirmationDialog::new(
                "Confirm",
                plan.clone(),
                None,
                "Delete",
                act.clone(),
            )))
        };
        open_dlg(&mut app);
        handle_dialog_key(&mut app, KeyCode::Down);
        assert!(matches!(&app.active_dialog, Some(Dialog::Confirmation(c)) if c.scroll == 1));
        handle_dialog_key(&mut app, KeyCode::Up);
        assert!(matches!(&app.active_dialog, Some(Dialog::Confirmation(c)) if c.scroll == 0));
        handle_dialog_key(&mut app, KeyCode::PageDown);
        assert!(
            matches!(&app.active_dialog, Some(Dialog::Confirmation(c)) if c.scroll == crate::tui::dialogs::DIALOG_PAGE)
        );
        handle_dialog_key(&mut app, KeyCode::Enter);
        assert!(app.active_dialog.is_none() && app.pending_mutation.is_none());
        open_dlg(&mut app);
        handle_dialog_key(&mut app, KeyCode::Tab);
        handle_dialog_key(&mut app, KeyCode::Enter);
        assert!(app.active_dialog.is_none() && app.pending_mutation.is_some());
        app.pending_mutation = None;
        open_dlg(&mut app);
        handle_dialog_key(&mut app, KeyCode::Esc);
        assert!(app.active_dialog.is_none() && app.pending_mutation.is_none());
    }

    #[test]
    fn test_regression_enter_on_confirmation_dialog_with_action_button_focused_queues_mutation() {
        let mut app = make_test_app(BrowserCapabilities::READ_ONLY);
        let prof = app.current_profile().unwrap().clone();
        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Test", "Default");
        let act = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: prof,
            mode: DeleteMode::Trash,
        };
        let mut dialog = ConfirmationDialog::new("Confirm Delete", plan, None, "Delete", act);
        dialog.focused_button = ConfirmButton::Action;
        app.open_dialog(Dialog::confirmation(dialog));

        handle_dialog_key(&mut app, KeyCode::Enter);

        assert!(app.active_dialog.is_none());
        assert!(matches!(
            app.pending_mutation,
            Some(PendingAction::DeleteProfile {
                mode: DeleteMode::Trash,
                ..
            })
        ));
    }

    #[test]
    fn test_enter_affirmative_and_cancel_on_text_input_dialog() {
        let mut app = make_test_app(BrowserCapabilities::READ_ONLY);
        let prof = app.current_profile().unwrap().clone();

        // 1. Enter with Save (0) button focused queues RenameProfile mutation
        let mut dialog = TextInputDialog::new_rename(0, prof.clone());
        dialog.value = "New Display Name".to_string();
        dialog.focused_button = 0;
        app.open_dialog(Dialog::TextInput(dialog));

        handle_dialog_key(&mut app, KeyCode::Enter);
        assert!(app.active_dialog.is_none());
        assert!(matches!(
            &app.pending_mutation,
            Some(PendingAction::RenameProfile { new_name, .. }) if new_name == "New Display Name"
        ));
        app.pending_mutation = None;

        // 2. Enter with Cancel (1) button focused closes dialog and queues nothing
        let mut dialog = TextInputDialog::new_rename(0, prof);
        dialog.value = "Unsaved Name".to_string();
        dialog.focused_button = 1;
        app.open_dialog(Dialog::TextInput(dialog));

        handle_dialog_key(&mut app, KeyCode::Enter);
        assert!(app.active_dialog.is_none());
        assert!(app.pending_mutation.is_none());
    }

    #[test]
    fn test_enter_cancel_button_on_confirmation_dialog_queues_nothing() {
        let mut app = make_test_app(BrowserCapabilities::READ_ONLY);
        let prof = app.current_profile().unwrap().clone();
        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Test", "Default");
        let act = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: prof,
            mode: DeleteMode::Trash,
        };
        let mut dialog = ConfirmationDialog::new("Confirm", plan, None, "Delete", act);
        dialog.focused_button = ConfirmButton::Safe;
        app.open_dialog(Dialog::confirmation(dialog));

        handle_dialog_key(&mut app, KeyCode::Enter);
        assert!(app.active_dialog.is_none());
        assert!(app.pending_mutation.is_none());
    }

    #[test]
    fn test_left_and_right_change_selected_button_visible_to_renderer() {
        let mut app = make_test_app(BrowserCapabilities::READ_ONLY);
        let prof = app.current_profile().unwrap().clone();

        // Confirmation dialog: Left focuses Action, Right focuses Safe
        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Test", "Default");
        let act = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: prof.clone(),
            mode: DeleteMode::Trash,
        };
        let dialog = ConfirmationDialog::new("Confirm", plan, None, "Delete", act);
        app.open_dialog(Dialog::confirmation(dialog));

        handle_dialog_key(&mut app, KeyCode::Left);
        assert!(matches!(
            &app.active_dialog,
            Some(Dialog::Confirmation(c)) if c.focused_button == ConfirmButton::Action
        ));

        handle_dialog_key(&mut app, KeyCode::Right);
        assert!(matches!(
            &app.active_dialog,
            Some(Dialog::Confirmation(c)) if c.focused_button == ConfirmButton::Safe
        ));

        // TextInput dialog: Left focuses 0 (Save), Right focuses 1 (Cancel)
        let dialog = TextInputDialog::new_rename(0, prof);
        app.open_dialog(Dialog::TextInput(dialog));

        handle_dialog_key(&mut app, KeyCode::Right);
        assert!(matches!(
            &app.active_dialog,
            Some(Dialog::TextInput(d)) if d.focused_button == 1
        ));

        handle_dialog_key(&mut app, KeyCode::Left);
        assert!(matches!(
            &app.active_dialog,
            Some(Dialog::TextInput(d)) if d.focused_button == 0
        ));
    }

    #[test]
    fn test_space_activates_button_and_inserts_in_text_field() {
        let mut app = make_test_app(BrowserCapabilities::READ_ONLY);
        let prof = app.current_profile().unwrap().clone();

        // In text input, text field has focus: Space inserts literal space into value
        let dialog = TextInputDialog::new_rename(0, prof.clone());
        app.open_dialog(Dialog::TextInput(dialog));
        handle_dialog_key(&mut app, KeyCode::Char(' '));
        assert!(app.active_dialog.is_some());
        assert!(app.pending_mutation.is_none());
        assert!(matches!(
            &app.active_dialog,
            Some(Dialog::TextInput(d)) if d.value == "Default "
        ));

        // In confirmation dialog, button has focus: Space activates selected button
        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Test", "Default");
        let act = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: prof,
            mode: DeleteMode::Trash,
        };
        let mut dialog = ConfirmationDialog::new("Confirm", plan, None, "Delete", act);
        dialog.focused_button = ConfirmButton::Action;
        app.open_dialog(Dialog::confirmation(dialog));

        handle_dialog_key(&mut app, KeyCode::Char(' '));
        assert!(app.active_dialog.is_none());
        assert!(matches!(
            app.pending_mutation,
            Some(PendingAction::DeleteProfile { .. })
        ));
    }

    #[test]
    fn test_no_fall_through_to_panes_while_dialog_is_open() {
        let mut app = make_test_app(BrowserCapabilities::READ_ONLY);
        app.set_focus(Focus::Profiles);
        assert_eq!(app.selected_profile, 0);

        let prof = app.current_profile().unwrap().clone();
        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Test", "Default");
        let act = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: prof,
            mode: DeleteMode::Trash,
        };
        let dialog = ConfirmationDialog::new("Confirm", plan, None, "Delete", act);
        app.open_dialog(Dialog::confirmation(dialog));

        // Down key in profile pane without dialog would move selection to 1.
        // With dialog open, it must not move selection.
        let key_down = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        handle_key(&mut app, key_down);
        assert_eq!(app.selected_profile, 0);

        let key_enter = KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        handle_key(&mut app, key_enter);
        assert_eq!(app.selected_profile, 0);
    }
}

#[cfg(test)]
mod update_action_tests {
    use super::events_dialogs::handle_dialog_key;
    use crate::tui::app::App;
    use crate::tui::dialogs::{ConfirmButton, Dialog, PendingAction};
    use crate::tui::keymap::Action;
    use crate::tui::update_check::UpdateCheckResult;
    use crossterm::event::KeyCode;

    fn app_with(update: Option<UpdateCheckResult>) -> App {
        let mut app = App::new();
        app.update = update;
        app
    }

    #[test]
    fn u_without_an_update_explains_and_opens_nothing() {
        for state in [
            None,
            Some(UpdateCheckResult::UpToDate),
            Some(UpdateCheckResult::Unavailable),
        ] {
            let mut app = app_with(state);
            super::execute_action(&mut app, Action::Update);
            assert!(app.active_dialog.is_none());
            assert!(app.pending_mutation.is_none());
            let msg = app.status_message.clone().unwrap_or_default();
            assert!(msg.starts_with("Update:"), "unexpected status: {msg}");
        }
    }

    #[test]
    fn u_with_an_update_opens_the_dialog_focused_on_cancel() {
        let mut app = app_with(Some(UpdateCheckResult::Available {
            current: "1.1.0".to_string(),
            latest: "1.2.0".to_string(),
        }));
        super::execute_action(&mut app, Action::Update);
        match &app.active_dialog {
            Some(Dialog::Update(d)) => {
                assert_eq!(d.focused_button, ConfirmButton::Safe);
                assert_eq!(d.current, "1.1.0");
                assert_eq!(d.latest, "1.2.0");
                let body = d.lines().join("\n");
                assert!(body.contains("1.1.0") && body.contains("1.2.0"));
            }
            _ => panic!("expected the update dialog"),
        }
        assert!(app.pending_mutation.is_none());
    }

    #[test]
    fn enter_on_cancel_closes_without_updating() {
        let mut app = app_with(Some(UpdateCheckResult::Available {
            current: "1.1.0".to_string(),
            latest: "1.2.0".to_string(),
        }));
        super::execute_action(&mut app, Action::Update);
        handle_dialog_key(&mut app, KeyCode::Enter);
        assert!(app.active_dialog.is_none());
        assert!(app.pending_mutation.is_none());
    }

    #[test]
    fn enter_on_update_queues_the_install() {
        let mut app = app_with(Some(UpdateCheckResult::Available {
            current: "1.1.0".to_string(),
            latest: "1.2.0".to_string(),
        }));
        super::execute_action(&mut app, Action::Update);
        handle_dialog_key(&mut app, KeyCode::Left);
        handle_dialog_key(&mut app, KeyCode::Enter);
        assert!(app.active_dialog.is_none());
        match &app.pending_mutation {
            Some(PendingAction::InstallUpdate { latest }) => assert_eq!(latest, "1.2.0"),
            _ => panic!("expected a queued install"),
        }
    }

    #[test]
    fn esc_closes_without_updating() {
        let mut app = app_with(Some(UpdateCheckResult::Available {
            current: "1.1.0".to_string(),
            latest: "1.2.0".to_string(),
        }));
        super::execute_action(&mut app, Action::Update);
        handle_dialog_key(&mut app, KeyCode::Esc);
        assert!(app.active_dialog.is_none());
        assert!(app.pending_mutation.is_none());
    }
}
