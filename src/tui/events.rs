use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::tui::app::App;
use crate::tui::keymap::{map_key, Action};

/// Handles incoming key events and delegates to state updates.
pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.kind != KeyEventKind::Press {
        return;
    }

    if !app.filter_mode && app.status_message.is_some() {
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
        Action::NewProfile => check_capability(app, "New", "create"),
        Action::CloneProfile => check_capability(app, "Clone", "clone"),
        Action::RenameProfile => check_capability(app, "Rename", "rename_display_name"),
        Action::DeleteProfile => check_capability(app, "Delete", "delete"),
        Action::LaunchProfile => check_capability(app, "Launch", "launch"),
        Action::Template => {
            app.status_message = Some("Template: not implemented in this milestone".to_string());
        }
        Action::Health => {
            if let Some(browser) = app.current_browser() {
                let summary = crate::doctor::summary_line(&browser.doctor_findings);
                app.status_message = Some(format!("Health: {summary}"));
            }
        }
        Action::CleanCache => check_capability(app, "Clean Cache", "clean_cache"),
    }
}

fn check_capability(app: &mut App, label: &str, cap_name: &str) {
    let Some(browser) = app.current_browser() else {
        app.status_message = Some(format!("{label}: no browser selected"));
        return;
    };

    if let Some(reason) = browser.capabilities.reason_disabled(cap_name) {
        app.status_message = Some(format!("{label}: {reason}"));
    } else {
        app.status_message = Some(format!("{label}: not implemented in this milestone"));
    }
}

fn handle_open_folder(app: &mut App) {
    let Some(browser) = app.current_browser() else {
        app.status_message = Some("Open Folder: no browser selected".to_string());
        return;
    };

    if let Some(reason) = browser.capabilities.reason_disabled("open_folder") {
        app.status_message = Some(format!("Open Folder: {reason}"));
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
