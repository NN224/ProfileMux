use crossterm::event::KeyCode;

use crate::tui::app::App;
use crate::tui::dialogs::{
    expand_tilde, BrowserRunningDialog, ConfirmButton, ConfirmationDialog, Dialog, DialogOutcome,
    DoctorDialog, ErrorDialog, PendingAction, QuitWaitState, TextInputDialog, TextInputKind,
};
use crate::tui::form::{FormField, ProfileForm};

pub fn handle_dialog_key(app: &mut App, code: KeyCode) {
    if code == KeyCode::Esc {
        app.close_dialog();
        return;
    }

    // Take the dialog out while handling so handlers can borrow `app` mutably.
    // The owned dialog is passed to per-dialog handlers and put back if not consumed.
    let Some(dialog) = app.active_dialog.take() else {
        return;
    };

    let retained = match dialog {
        Dialog::TextInput(d) => handle_text_input_key(app, d, code),
        Dialog::Form(f) => handle_form_key(app, *f, code),
        Dialog::Confirmation(c) => handle_confirmation_key(app, *c, code),
        Dialog::BrowserRunning(b) => handle_browser_running_key(app, b, code),
        Dialog::Doctor(d) => handle_doctor_key(app, d, code),
        Dialog::Error(e) => handle_error_key(app, e, code),
    };

    if let Some(d) = retained {
        if app.active_dialog.is_none() && !app.dialog_consumed {
            app.active_dialog = Some(d);
        }
    }
    app.dialog_consumed = false;
}

fn handle_text_input_key(app: &mut App, mut d: TextInputDialog, code: KeyCode) -> Option<Dialog> {
    match code {
        KeyCode::Char(c) => d.handle_char(c),
        KeyCode::Backspace => d.handle_backspace(),
        KeyCode::Left => d.focused_button = 0,
        KeyCode::Right => d.focused_button = 1,
        KeyCode::Tab | KeyCode::BackTab => d.focused_button = (d.focused_button + 1) % 2,
        KeyCode::Enter => match d.activate_selected() {
            DialogOutcome::Activate => {
                submit_text_input(app, d);
                return None;
            }
            DialogOutcome::Close => {
                app.close_dialog();
                return None;
            }
            DialogOutcome::None => {}
        },
        _ => {}
    }
    Some(Dialog::TextInput(d))
}

pub fn submit_text_input(app: &mut App, d: TextInputDialog) {
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
            } else {
                app.close_dialog();
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
        app.close_dialog();
        return;
    }
    let path = expand_tilde(path_str);
    let Some(browser) = app.browsers.get(b_idx) else {
        app.close_dialog();
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

fn activate_form_button(app: &mut App, form: ProfileForm) {
    if form.focused_button == 1 {
        app.close_dialog();
    } else {
        submit_form(app, form);
    }
}

fn handle_form_key(app: &mut App, mut form: ProfileForm, code: KeyCode) -> Option<Dialog> {
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
            FormField::Buttons if c == ' ' => {
                activate_form_button(app, form);
                return None;
            }
            _ => {}
        },
        KeyCode::Backspace => match form.focused_field {
            FormField::Name => form.handle_name_backspace(),
            FormField::Directory => form.handle_directory_backspace(),
            FormField::Avatar => form.handle_avatar_backspace(),
            _ => {}
        },
        KeyCode::Enter => match form.focused_field {
            FormField::Buttons => {
                activate_form_button(app, form);
                return None;
            }
            FormField::OpenAfterCreate => {
                submit_form(app, form);
                return None;
            }
            _ => form.next_field(),
        },
        _ => {}
    }
    Some(Dialog::form(form))
}

pub fn submit_form(app: &mut App, form: ProfileForm) {
    let b_idx = app.selected_browser;
    let Some(browser) = app.browsers.get(b_idx) else {
        app.close_dialog();
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
        app.close_dialog();
        return;
    };
    let Some(source) = app.current_profile().cloned() else {
        app.close_dialog();
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

fn handle_confirmation_key(
    app: &mut App,
    mut c: ConfirmationDialog,
    code: KeyCode,
) -> Option<Dialog> {
    match code {
        KeyCode::Up => c.scroll_up(),
        KeyCode::Down => c.scroll_down(),
        KeyCode::PageUp => c.scroll_page_up(),
        KeyCode::PageDown => c.scroll_page_down(),
        KeyCode::Left => c.focused_button = ConfirmButton::Action,
        KeyCode::Right => c.focused_button = ConfirmButton::Safe,
        KeyCode::Tab | KeyCode::BackTab => {
            c.focused_button = c.focused_button.toggle();
        }
        KeyCode::Enter | KeyCode::Char(' ') => match c.activate_selected() {
            DialogOutcome::Activate => {
                confirm_action(app, c.plan, c.action);
                return None;
            }
            DialogOutcome::Close => {
                app.close_dialog();
                return None;
            }
            DialogOutcome::None => {}
        },
        _ => {}
    }
    Some(Dialog::confirmation(c))
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

fn handle_browser_running_key(
    app: &mut App,
    mut b: BrowserRunningDialog,
    code: KeyCode,
) -> Option<Dialog> {
    match code {
        KeyCode::Left => b.focused_button = ConfirmButton::Action,
        KeyCode::Right => b.focused_button = ConfirmButton::Safe,
        KeyCode::Tab | KeyCode::BackTab => {
            b.focused_button = b.focused_button.toggle();
        }
        KeyCode::Enter | KeyCode::Char(' ') => match b.activate_selected() {
            DialogOutcome::Activate => {
                let b_idx = b.browser_index;
                let b_name = b.browser_name;
                let action = b.pending_action;
                request_quit_browser(app, b_idx, b_name, action);
                return None;
            }
            DialogOutcome::Close => {
                app.close_dialog();
                return None;
            }
            DialogOutcome::None => {}
        },
        _ => {}
    }
    Some(Dialog::BrowserRunning(b))
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

fn handle_doctor_key(app: &mut App, mut d: DoctorDialog, code: KeyCode) -> Option<Dialog> {
    match code {
        KeyCode::Up => d.scroll_up(),
        KeyCode::Down => d.scroll_down(),
        KeyCode::PageUp => d.scroll_page_up(),
        KeyCode::PageDown => d.scroll_page_down(),
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.close_dialog();
            return None;
        }
        _ => {}
    }
    Some(Dialog::Doctor(d))
}

fn handle_error_key(app: &mut App, e: ErrorDialog, code: KeyCode) -> Option<Dialog> {
    match code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.close_dialog();
            None
        }
        _ => Some(Dialog::Error(e)),
    }
}
