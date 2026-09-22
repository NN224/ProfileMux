use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::domain::{BrowserProfile, HealthFinding, Severity, SupportLevel};
use crate::tui::app::{App, Focus};
use crate::tui::update_check::UpdateCheckResult;

/// Main render entry point.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_panes(frame, chunks[0], app);
    render_status_line(frame, chunks[1], app);
    render_action_bar(frame, chunks[2], app);

    if let Some(dialog) = &app.active_dialog {
        crate::tui::dialogs::render_dialog(frame, app, dialog);
    } else if app.show_help {
        render_help_overlay(frame);
    } else if app.show_browser_overlay {
        render_browser_overlay(frame, app);
    } else if app.show_profile_overlay {
        render_profile_overlay(frame, app);
    }
}

fn render_panes(frame: &mut Frame, area: Rect, app: &App) {
    if area.width < 80 {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        render_browsers_pane(frame, h_chunks[0], app);
        render_profiles_pane(frame, h_chunks[1], app);
    } else {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(28),
                Constraint::Percentage(36),
                Constraint::Percentage(36),
            ])
            .split(area);
        render_browsers_pane(frame, h_chunks[0], app);
        render_profiles_pane(frame, h_chunks[1], app);
        render_details_pane(frame, h_chunks[2], app);
    }
}

fn pane_block(title: &str, is_focused: bool) -> Block<'static> {
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let title_color = if is_focused { Color::Cyan } else { Color::Gray };

    let title_style = if is_focused {
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(title_color)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(format!(" {title} "), title_style))
}

fn render_browsers_pane(frame: &mut Frame, area: Rect, app: &App) {
    let block = pane_block("Browsers", app.focus == Focus::Browsers);
    if app.browsers.is_empty() {
        frame.render_widget(Paragraph::new("No browsers discovered").block(block), area);
        return;
    }

    let mut items = Vec::with_capacity(app.browsers.len());
    for (i, b) in app.browsers.iter().enumerate() {
        let count = format!("[{}]", b.profiles.len());
        let supp_color = match b.install.support {
            SupportLevel::Full => Color::Green,
            SupportLevel::Partial => Color::Yellow,
            SupportLevel::ReadOnly => Color::Blue,
            SupportLevel::Unsupported => Color::Red,
        };

        let is_selected = i == app.selected_browser;
        let line = Line::from(vec![
            Span::raw(format!("{:<18} ", b.install.label())),
            Span::styled(format!("{count:>4} "), Style::default().fg(Color::DarkGray)),
            Span::styled(b.install.support.label(), Style::default().fg(supp_color)),
        ]);

        let item = if is_selected {
            if app.focus == Focus::Browsers {
                ListItem::new(line).style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(line).style(Style::default().fg(Color::White).bg(Color::DarkGray))
            }
        } else {
            ListItem::new(line)
        };
        items.push(item);
    }

    frame.render_widget(List::new(items).block(block), area);
}

fn profile_severity(profile: &BrowserProfile, findings: &[HealthFinding]) -> Severity {
    if !profile.directory_exists {
        return Severity::Broken;
    }
    let mut highest = if profile.registered {
        Severity::Ok
    } else {
        Severity::Warning
    };
    for finding in findings {
        if finding
            .paths
            .iter()
            .any(|p| p == &profile.path || p.starts_with(&profile.path))
            && finding.severity > highest
        {
            highest = finding.severity;
        }
    }
    highest
}

fn render_profiles_pane(frame: &mut Frame, area: Rect, app: &App) {
    let title = if app.filter.is_empty() {
        "Profiles".to_string()
    } else {
        format!("Profiles [/ {}]", app.filter)
    };
    let block = pane_block(&title, app.focus == Focus::Profiles);

    let Some(browser) = app.current_browser() else {
        frame.render_widget(Paragraph::new("No browser selected").block(block), area);
        return;
    };

    let filtered = app.filtered_profiles();
    if filtered.is_empty() {
        let msg = if app.filter.is_empty() {
            "No profiles"
        } else {
            "No matching profiles"
        };
        frame.render_widget(Paragraph::new(msg).block(block), area);
        return;
    }

    let mut items = Vec::with_capacity(filtered.len());
    for (i, (_, profile)) in filtered.iter().enumerate() {
        let severity = profile_severity(profile, &browser.doctor_findings);
        let sev_color = match severity {
            Severity::Ok => Color::Green,
            Severity::Warning => Color::Yellow,
            Severity::Broken => Color::Red,
        };

        let line = Line::from(vec![
            Span::styled(
                format!("{} ", severity.glyph()),
                Style::default().fg(sev_color),
            ),
            Span::raw(&profile.display_name),
            Span::styled(
                format!(" ({})", profile.directory),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        let is_selected = i == app.selected_profile;
        let item = if is_selected {
            if app.focus == Focus::Profiles {
                ListItem::new(line).style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(line).style(Style::default().fg(Color::White).bg(Color::DarkGray))
            }
        } else {
            ListItem::new(line)
        };
        items.push(item);
    }

    frame.render_widget(List::new(items).block(block), area);
}

fn format_storage(
    profile: &BrowserProfile,
    scanning: bool,
) -> (String, String, String, String, String) {
    if let Some(sb) = profile.size {
        (
            crate::fs::size::format_bytes(sb.total),
            crate::fs::size::format_bytes(sb.core),
            crate::fs::size::format_bytes(sb.cache),
            crate::fs::size::format_bytes(sb.code_cache),
            crate::fs::size::format_bytes(sb.gpu_cache),
        )
    } else if scanning {
        (
            "scanning...".to_string(),
            "scanning...".to_string(),
            "scanning...".to_string(),
            "scanning...".to_string(),
            "scanning...".to_string(),
        )
    } else {
        (
            "not measured".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
        )
    }
}

fn detail_line(label: &'static str, val: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(val.to_string()),
    ])
}

pub fn account_email_display(profile: &BrowserProfile) -> &str {
    profile.account_email.as_deref().unwrap_or("Not signed in")
}

pub fn account_email_line(profile: &BrowserProfile) -> Line<'static> {
    detail_line("Account Email:    ", account_email_display(profile))
}

pub fn profile_appearance(
    app: &App,
    id: &crate::domain::ProfileId,
) -> (crate::domain::BrowserTheme, crate::domain::WebDarkMode) {
    match app.appearance_cache.get(id) {
        Some(a) => (a.theme, a.web_dark),
        None => (
            crate::domain::BrowserTheme::Unknown,
            crate::domain::WebDarkMode::Normal,
        ),
    }
}

pub fn browser_theme_line(theme: crate::domain::BrowserTheme) -> Line<'static> {
    detail_line("Browser Theme:    ", theme.label())
}

pub fn web_dark_mode_line(mode: crate::domain::WebDarkMode) -> Line<'static> {
    detail_line("Web Dark Mode:    ", mode.label())
}

fn render_details_pane(frame: &mut Frame, area: Rect, app: &App) {
    let block = pane_block("Details", app.focus == Focus::Details);

    let Some(browser) = app.current_browser() else {
        frame.render_widget(Paragraph::new("No browser selected").block(block), area);
        return;
    };

    let Some(profile) = app.current_profile() else {
        frame.render_widget(Paragraph::new("No profile selected").block(block), area);
        return;
    };

    let is_scanning = app.scanning_profiles.contains(&profile.id);
    let (total_s, core_s, cache_s, code_s, gpu_s) = format_storage(profile, is_scanning);
    let (theme, web_dark) = profile_appearance(app, &profile.id);

    let avatar_str = match &profile.avatar {
        Some(av) => {
            let icon = av.icon.as_deref().unwrap_or("none");
            let pic = if av.uses_picture { "yes" } else { "no" };
            let def = if av.is_default { " (default)" } else { "" };
            format!("icon: {icon}, custom picture: {pic}{def}")
        }
        None => "none".to_string(),
    };

    let last_active_str = match profile.last_active {
        Some(ts) => format!("{ts}"),
        None => "never / unknown".to_string(),
    };

    let cache_str = profile
        .cache_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "none".to_string());

    let running_str = if browser.is_running {
        "running"
    } else {
        "stopped"
    };
    let running_color = if browser.is_running {
        Color::Green
    } else {
        Color::DarkGray
    };

    let health_summary = crate::doctor::summary_line(&browser.doctor_findings);

    let lines = vec![
        detail_line("Display Name:     ", &profile.display_name),
        detail_line("Directory:        ", &profile.directory),
        detail_line("Absolute Path:    ", &profile.path.display().to_string()),
        detail_line("Cache Path:       ", &cache_str),
        detail_line("Avatar:           ", &avatar_str),
        account_email_line(profile),
        browser_theme_line(theme),
        web_dark_mode_line(web_dark),
        detail_line("Last Active:      ", &last_active_str),
        detail_line(
            "Registered:       ",
            if profile.registered { "yes" } else { "no" },
        ),
        detail_line(
            "Directory Exists: ",
            if profile.directory_exists {
                "yes"
            } else {
                "no"
            },
        ),
        Line::from(vec![
            Span::styled(
                "Running State:    ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(running_str, Style::default().fg(running_color)),
        ]),
        detail_line("Storage Total:    ", &total_s),
        Line::from(vec![
            Span::styled("  Core:           ", Style::default().fg(Color::DarkGray)),
            Span::raw(core_s),
        ]),
        Line::from(vec![
            Span::styled("  Cache:          ", Style::default().fg(Color::DarkGray)),
            Span::raw(cache_s),
        ]),
        Line::from(vec![
            Span::styled("  Code Cache:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(code_s),
        ]),
        Line::from(vec![
            Span::styled("  GPU Cache:      ", Style::default().fg(Color::DarkGray)),
            Span::raw(gpu_s),
        ]),
        detail_line("Health:           ", &health_summary),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_status_line(frame: &mut Frame, area: Rect, app: &App) {
    if app.filter_mode {
        let line = Line::from(vec![
            Span::styled(
                "/ ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&app.filter_input),
            Span::styled(
                "  [Enter: apply, Esc: cancel]",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    } else if let Some(msg) = &app.status_message {
        let line = Line::from(vec![Span::styled(
            msg,
            Style::default().fg(Color::LightRed),
        )]);
        frame.render_widget(Paragraph::new(line), area);
    } else {
        frame.render_widget(Paragraph::new(update_status_line(app, area.width)), area);
    }
}

/// Version plus, when a newer release exists, a restrained availability marker
/// in the accent colour already used for focus.
fn update_status_line(app: &App, width: u16) -> Line<'static> {
    let version = env!("CARGO_PKG_VERSION");
    let base = format!("ProfileMux v{version}");
    match &app.update {
        Some(UpdateCheckResult::Available { latest, .. }) if width >= 80 => Line::from(vec![
            Span::styled(base, Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("   \u{2191} v{latest} available"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        _ => Line::from(Span::styled(base, Style::default().fg(Color::DarkGray))),
    }
}

fn render_action_bar(frame: &mut Frame, area: Rect, app: &App) {
    let caps = app.current_browser().map(|b| b.capabilities);
    let has_theme = app
        .current_browser()
        .map(|b| {
            let c = b.adapter.appearance_capabilities();
            c.browser_theme || c.ultra_dark || c.web_dark
        })
        .unwrap_or(false);

    let actions = [
        ("N", "New", caps.map(|c| c.create).unwrap_or(false)),
        ("C", "Clone", caps.map(|c| c.clone).unwrap_or(false)),
        (
            "R",
            "Rename",
            caps.map(|c| c.rename_display_name).unwrap_or(false),
        ),
        ("D", "Delete", caps.map(|c| c.delete).unwrap_or(false)),
        ("L", "Launch", caps.map(|c| c.launch).unwrap_or(false)),
        (
            "A",
            "Avatar",
            caps.map(|c| c.custom_avatar).unwrap_or(false),
        ),
        ("T", "Theme", has_theme),
        ("O", "Folder", caps.map(|c| c.open_folder).unwrap_or(false)),
        ("X", "Clean", caps.map(|c| c.clean_cache).unwrap_or(false)),
        ("H", "Doctor", true),
        (
            "U",
            "Update",
            app.update.as_ref().is_some_and(|u| u.is_available()),
        ),
        ("/", "Search", true),
        ("?", "Help", true),
        ("Q", "Quit", true),
    ];

    let mut spans = Vec::with_capacity(actions.len() * 3);
    for (i, (key, label, enabled)) in actions.iter().enumerate() {
        if *enabled {
            spans.push(Span::styled(
                *key,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(format!(" {label}")));
        } else {
            spans.push(Span::styled(
                format!("{key} {label}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if i + 1 < actions.len() {
            spans.push(Span::raw("   "));
        }
    }

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_help_overlay(frame: &mut Frame) {
    let area = centered_rect(70, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Help - Keybindings (Esc to close) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let lines = vec![
        detail_line("Up / k:          ", "Move selection up"),
        detail_line("Down / j:        ", "Move selection down"),
        detail_line("Tab / Shift-Tab: ", "Cycle focus between panes"),
        detail_line("h / l:           ", "Move focus left / right"),
        detail_line("/:               ", "Filter profiles by name or directory"),
        detail_line("Enter / L:       ", "Launch selected profile"),
        detail_line("i:               ", "Profile details overlay (fullscreen)"),
        detail_line("b:               ", "Browser details overlay (fullscreen)"),
        detail_line(
            "Esc:             ",
            "Clear filter / Close dialog or overlay",
        ),
        detail_line(
            "Dialog keys:     ",
            "Inside a dialog: Up/Down scroll plan by line, PageUp/PageDown by page, Enter confirms focused button, Esc cancels",
        ),
        detail_line("F5:              ", "Re-scan storage for selected profile"),
        detail_line("N:               ", "New profile form"),
        detail_line("C:               ", "Clone profile form"),
        detail_line("R:               ", "Rename display name"),
        detail_line("D:               ", "Delete profile (moves to Trash)"),
        detail_line("A:               ", "Set custom profile avatar"),
        detail_line("T:               ", "Appearance dialog"),
        detail_line("O:               ", "Open profile folder in Finder"),
        detail_line("X:               ", "Clean cache"),
        detail_line("H:               ", "Doctor health findings"),
        detail_line("U:               ", "Install an available update"),
        detail_line("?:               ", "Toggle this help overlay"),
        detail_line("q / Ctrl-C:      ", "Quit pmux"),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_browser_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(80, 80, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Browser Details (Esc to close) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let Some(browser) = app.current_browser() else {
        frame.render_widget(Paragraph::new("No browser selected").block(block), area);
        return;
    };

    let cache_str = browser
        .install
        .cache_root
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "none".to_string());
    let bundle_str = browser.install.bundle_id.as_deref().unwrap_or("none");
    let ver_str = browser.install.version.as_deref().unwrap_or("unknown");

    let running_str = if browser.is_running {
        "running"
    } else {
        "stopped"
    };
    let caps = &browser.capabilities;
    let caps_str = format!(
        "launch:{}, open_folder:{}, create:{}, rename_display:{}, rename_dir:{}, clone:{}, delete:{}, clean_cache:{}",
        caps.launch,
        caps.open_folder,
        caps.create,
        caps.rename_display_name,
        caps.rename_directory,
        caps.clone,
        caps.delete,
        caps.clean_cache,
    );

    let health_summary = crate::doctor::summary_line(&browser.doctor_findings);
    let overall_sev = crate::doctor::overall_severity(&browser.doctor_findings);

    let lines = vec![
        detail_line("Name:            ", &browser.install.name),
        detail_line("Kind:            ", browser.install.kind.slug()),
        detail_line("Channel:         ", browser.install.channel.label()),
        detail_line("Version:         ", ver_str),
        detail_line("Bundle ID:       ", bundle_str),
        detail_line(
            "App Path:        ",
            &browser.install.app_path.display().to_string(),
        ),
        detail_line(
            "User Data Root:  ",
            &browser.install.user_data_root.display().to_string(),
        ),
        detail_line("Cache Root:      ", &cache_str),
        detail_line("Support Level:   ", browser.install.support.label()),
        detail_line("Running State:   ", running_str),
        detail_line("Capabilities:    ", &caps_str),
        detail_line("Health Summary:  ", &health_summary),
        detail_line("Severity:        ", overall_sev.label()),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn render_profile_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(80, 80, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " Profile Details (Esc to close) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let Some(browser) = app.current_browser() else {
        frame.render_widget(Paragraph::new("No browser selected").block(block), area);
        return;
    };

    let Some(profile) = app.current_profile() else {
        frame.render_widget(Paragraph::new("No profile selected").block(block), area);
        return;
    };

    let is_scanning = app.scanning_profiles.contains(&profile.id);
    let (total_s, core_s, cache_s, code_s, gpu_s) = format_storage(profile, is_scanning);
    let (theme, web_dark) = profile_appearance(app, &profile.id);

    let avatar_str = match &profile.avatar {
        Some(av) => {
            let icon = av.icon.as_deref().unwrap_or("none");
            let pic = if av.uses_picture { "yes" } else { "no" };
            let def = if av.is_default { " (default)" } else { "" };
            format!("icon: {icon}, custom picture: {pic}{def}")
        }
        None => "none".to_string(),
    };

    let last_active_str = match profile.last_active {
        Some(ts) => format!("{ts}"),
        None => "never / unknown".to_string(),
    };

    let cache_str = profile
        .cache_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "none".to_string());

    let health_summary = crate::doctor::summary_line(&browser.doctor_findings);

    let lines = vec![
        detail_line("Display Name:     ", &profile.display_name),
        detail_line("Directory:        ", &profile.directory),
        detail_line("Absolute Path:    ", &profile.path.display().to_string()),
        detail_line("Cache Path:       ", &cache_str),
        detail_line("Avatar:           ", &avatar_str),
        account_email_line(profile),
        browser_theme_line(theme),
        web_dark_mode_line(web_dark),
        detail_line("Last Active:      ", &last_active_str),
        detail_line(
            "Registered:       ",
            if profile.registered { "yes" } else { "no" },
        ),
        detail_line(
            "Directory Exists: ",
            if profile.directory_exists {
                "yes"
            } else {
                "no"
            },
        ),
        detail_line("Storage Total:    ", &total_s),
        detail_line("  Core:           ", &core_s),
        detail_line("  Cache:          ", &cache_s),
        detail_line("  Code Cache:     ", &code_s),
        detail_line("  GPU Cache:      ", &gpu_s),
        detail_line("Health Summary:   ", &health_summary),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::domain::{BrowserInstallId, BrowserKind, ProfileId};

    fn make_test_profile(account_email: Option<String>) -> BrowserProfile {
        let install_id = BrowserInstallId::new(BrowserKind::Chromium, Path::new("/test/browser"));
        let profile_id = ProfileId::new(&install_id, "Default");
        BrowserProfile {
            id: profile_id,
            install_id,
            display_name: "Test Profile".to_string(),
            directory: "Default".to_string(),
            path: PathBuf::from("/test/browser/Default"),
            cache_path: None,
            avatar: None,
            account_email,
            last_active: None,
            registered: true,
            directory_exists: true,
            size: None,
        }
    }

    #[test]
    fn test_account_email_display_with_email() {
        let profile = make_test_profile(Some("person@example.com".to_string()));
        assert_eq!(account_email_display(&profile), "person@example.com");

        let line = account_email_line(&profile);
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].content.as_ref(), "Account Email:    ");
        assert_eq!(line.spans[1].content.as_ref(), "person@example.com");
    }

    #[test]
    fn test_account_email_display_none() {
        let profile = make_test_profile(None);
        assert_eq!(account_email_display(&profile), "Not signed in");

        let line = account_email_line(&profile);
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].content.as_ref(), "Account Email:    ");
        assert_eq!(line.spans[1].content.as_ref(), "Not signed in");
    }

    #[test]
    fn test_appearance_detail_lines() {
        let theme_line = browser_theme_line(crate::domain::BrowserTheme::UltraDark);
        assert_eq!(theme_line.spans[1].content.as_ref(), "Ultra Dark");
        let web_line = web_dark_mode_line(crate::domain::WebDarkMode::Normal);
        assert_eq!(web_line.spans[1].content.as_ref(), "Off");
    }
}
