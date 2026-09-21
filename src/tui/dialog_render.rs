use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::*;
use crate::domain::Severity;
use crate::tui::app::App;
use crate::tui::form::{FormField, ProfileForm};

/// Builds a styled block wrapper for dialog overlays.
pub fn dialog_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
}

/// Computes a centered rectangle within the given parent area.
pub fn modal_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width.saturating_sub(4));
    let h = height.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Renders the active modal dialog centered over the terminal screen.
pub fn render_dialog(frame: &mut Frame, _app: &App, dialog: &Dialog) {
    match dialog {
        Dialog::TextInput(d) => render_text_input(frame, d),
        Dialog::Form(f) => render_profile_form(frame, f),
        Dialog::Confirmation(c) => render_confirmation(frame, c),
        Dialog::BrowserRunning(b) => render_browser_running(frame, b),
        Dialog::Doctor(doc) => render_doctor(frame, doc),
        Dialog::Error(err) => render_error(frame, err),
    }
}

pub fn button_span(label: &str, is_focused: bool) -> Span<'static> {
    if is_focused {
        Span::styled(
            format!(" [{label}] "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!(" [{label}] "), Style::default().fg(Color::DarkGray))
    }
}

fn render_text_input(frame: &mut Frame, dialog: &TextInputDialog) {
    let max_len = dialog.prompt.len().max(dialog.value.len() + 2).max(40);
    let width = compute_dialog_width(max_len, frame.area().width).max(60);
    let height = compute_dialog_height(4, frame.area().height).max(9);
    let area = modal_rect(width, height, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(area);

    let block = dialog_block(&dialog.title);
    frame.render_widget(block, area);

    frame.render_widget(
        Paragraph::new(dialog.prompt.as_str()).style(Style::default().add_modifier(Modifier::BOLD)),
        chunks[0],
    );

    let input_style = Style::default().fg(Color::Cyan);
    let val_display = format!("{} ", dialog.value);
    frame.render_widget(Paragraph::new(val_display).style(input_style), chunks[2]);

    let btn_save = button_span("Save", dialog.focused_button == 0);
    let btn_cancel = button_span("Cancel", dialog.focused_button == 1);
    let btn_line = Line::from(vec![btn_save, Span::raw("  "), btn_cancel]);
    frame.render_widget(
        Paragraph::new(btn_line).alignment(Alignment::Right),
        chunks[4],
    );
}

fn render_profile_form(frame: &mut Frame, form: &ProfileForm) {
    let title = if form.is_clone {
        "Clone Profile"
    } else {
        "New Profile"
    };
    let path_len = form
        .resulting_path()
        .display()
        .to_string()
        .len()
        .saturating_add(16);
    let max_len = path_len.max(form.name.len() + 18).max(50);
    let width = compute_dialog_width(max_len, frame.area().width).max(72);
    let height = compute_dialog_height(14, frame.area().height).max(19);
    let area = modal_rect(width, height, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(dialog_block(title), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // 0: Name
            Constraint::Length(3), // 1: Directory + path
            Constraint::Length(2), // 2: Template
            Constraint::Length(2), // 3: Extensions
            Constraint::Length(2), // 4: Avatar
            Constraint::Length(2), // 5: Open after create
            Constraint::Min(1),    // 6: Buttons
        ])
        .margin(1)
        .split(area);

    render_form_fields(frame, form, &chunks);

    let action_label = if form.is_clone {
        "Clone Profile"
    } else {
        "Create Profile"
    };
    let is_btn_field = form.focused_field == FormField::Buttons;
    let b_create = button_span(action_label, is_btn_field && form.focused_button == 0);
    let b_cancel = button_span("Cancel", is_btn_field && form.focused_button == 1);
    let buttons = Line::from(vec![b_create, Span::raw("  "), b_cancel]);
    frame.render_widget(
        Paragraph::new(buttons).alignment(Alignment::Right),
        chunks[6],
    );
}

fn render_form_fields(frame: &mut Frame, form: &ProfileForm, chunks: &[Rect]) {
    render_form_top_fields(frame, form, chunks);
    render_form_bottom_fields(frame, form, chunks);
}

fn render_form_top_fields(frame: &mut Frame, form: &ProfileForm, chunks: &[Rect]) {
    let f = form.focused_field;
    render_field_line(
        frame,
        chunks[0],
        "Name:           ",
        &form.name,
        f == FormField::Name,
    );

    let dir_display = if form.directory.is_empty() {
        "(auto-suggested from name)"
    } else {
        &form.directory
    };
    render_field_line(
        frame,
        chunks[1],
        "Directory:      ",
        dir_display,
        f == FormField::Directory,
    );
    let path_line = Line::from(vec![
        Span::styled("  → Path:       ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            form.resulting_path().display().to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    let path_rect = Rect::new(chunks[1].x, chunks[1].y + 1, chunks[1].width, 1);
    frame.render_widget(Paragraph::new(path_line), path_rect);

    let template_name = form
        .template_options
        .get(form.template_index)
        .map(|t| t.display_name.as_str())
        .unwrap_or("None");
    let t_display = format!("< {template_name} >");
    render_field_line(
        frame,
        chunks[2],
        "Template:       ",
        &t_display,
        f == FormField::Template,
    );
}

fn render_form_bottom_fields(frame: &mut Frame, form: &ProfileForm, chunks: &[Rect]) {
    let f = form.focused_field;
    let ext_display = format!("< {} >", form.extension_policy.label());
    render_field_line(
        frame,
        chunks[3],
        "Extensions:     ",
        &ext_display,
        f == FormField::Extensions,
    );

    let av_display = if form.avatar_path.is_empty() {
        "(optional image path)"
    } else {
        &form.avatar_path
    };
    render_field_line(
        frame,
        chunks[4],
        "Avatar:         ",
        av_display,
        f == FormField::Avatar,
    );

    let open_str = if form.open_after_create {
        "[ Yes ]"
    } else {
        "[ No ]"
    };
    render_field_line(
        frame,
        chunks[5],
        "Open on create: ",
        open_str,
        f == FormField::OpenAfterCreate,
    );
}

fn render_field_line(
    frame: &mut Frame,
    area: Rect,
    label: &'static str,
    val: &str,
    is_focused: bool,
) {
    let (lbl_style, val_style) = if is_focused {
        (
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            Style::default().add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Reset),
        )
    };
    let line = Line::from(vec![
        Span::styled(label, lbl_style),
        Span::styled(val.to_string(), val_style),
    ]);
    frame.render_widget(
        Paragraph::new(line),
        Rect::new(area.x, area.y, area.width, 1),
    );
}

fn render_confirmation(frame: &mut Frame, dialog: &ConfirmationDialog) {
    let lines = build_confirmation_lines(dialog);
    let max_len = lines.iter().map(|l| l.width()).max().unwrap_or(72);
    let width = compute_dialog_width(max_len, frame.area().width);
    let rows = wrapped_line_count(&lines, width.saturating_sub(2) as usize);
    let height = compute_dialog_height(rows, frame.area().height);
    let area = modal_rect(width, height, frame.area());

    frame.render_widget(Clear, area);
    frame.render_widget(dialog_block(&dialog.title), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .margin(1)
        .split(area);

    let visible_h = chunks[0].height as usize;
    let visible = visible_lines(&lines, dialog.scroll, visible_h, chunks[0].width as usize);
    frame.render_widget(
        Paragraph::new(visible).wrap(Wrap { trim: false }),
        chunks[0],
    );

    let b_act = button_span(
        &dialog.action_button_label,
        dialog.focused_button == ConfirmButton::Action,
    );
    let b_can = button_span(
        &dialog.cancel_button_label,
        dialog.focused_button == ConfirmButton::Safe,
    );
    let buttons = Line::from(vec![b_act, Span::raw("  "), b_can]);
    frame.render_widget(
        Paragraph::new(buttons).alignment(Alignment::Right),
        chunks[1],
    );
}

fn render_browser_running(frame: &mut Frame, dialog: &BrowserRunningDialog) {
    let name_len = dialog.browser_name.len().saturating_add(30);
    let width = compute_dialog_width(name_len, frame.area().width).max(64);
    let height = compute_dialog_height(4, frame.area().height).max(10);
    let area = modal_rect(width, height, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(dialog_block("Browser Running"), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .margin(1)
        .split(area);

    let msg = vec![
        Line::raw(format!("{} is currently running.", dialog.browser_name)),
        Line::raw("This operation requires the browser to be fully closed."),
        Line::raw("Would you like to ask the browser to quit cleanly?"),
    ];
    let visible_h = chunks[0].height as usize;
    let visible = visible_lines(&msg, 0, visible_h, chunks[0].width as usize);
    frame.render_widget(
        Paragraph::new(visible).wrap(Wrap { trim: false }),
        chunks[0],
    );

    let b_quit = button_span(
        "Quit Browser",
        dialog.focused_button == ConfirmButton::Action,
    );
    let b_cancel = button_span("Cancel", dialog.focused_button == ConfirmButton::Safe);
    let buttons = Line::from(vec![b_quit, Span::raw("  "), b_cancel]);
    frame.render_widget(
        Paragraph::new(buttons).alignment(Alignment::Right),
        chunks[1],
    );
}

fn build_doctor_lines(dialog: &DoctorDialog) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if dialog.findings.is_empty() {
        lines.push(Line::raw("No health findings or issues detected."));
    } else {
        for f in &dialog.findings {
            let color = match f.severity {
                Severity::Ok => Color::Green,
                Severity::Warning => Color::Yellow,
                Severity::Broken => Color::Red,
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", f.severity.glyph()),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    f.code.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::raw(format!("  {}", f.message)));
            for path in &f.paths {
                lines.push(Line::styled(
                    format!("  {}", path.display()),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::raw(""));
        }
    }
    lines
}

fn render_doctor(frame: &mut Frame, dialog: &DoctorDialog) {
    let lines = build_doctor_lines(dialog);
    let max_len = lines.iter().map(|l| l.width()).max().unwrap_or(72);
    let width = compute_dialog_width(max_len, frame.area().width);
    let rows = wrapped_line_count(&lines, width.saturating_sub(2) as usize);
    let height = compute_dialog_height(rows, frame.area().height);
    let area = modal_rect(width, height, frame.area());

    frame.render_widget(Clear, area);
    frame.render_widget(
        dialog_block(&format!("Health Findings — {}", dialog.browser_name)),
        area,
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .margin(1)
        .split(area);

    let visible_h = chunks[0].height as usize;
    let visible = visible_lines(&lines, dialog.scroll, visible_h, chunks[0].width as usize);
    frame.render_widget(
        Paragraph::new(visible).wrap(Wrap { trim: false }),
        chunks[0],
    );

    let b_close = button_span("Close", true);
    frame.render_widget(
        Paragraph::new(Line::from(vec![b_close])).alignment(Alignment::Right),
        chunks[1],
    );
}

fn render_error(frame: &mut Frame, dialog: &ErrorDialog) {
    let msg_lines: Vec<Line<'static>> = dialog
        .message
        .lines()
        .map(|l| Line::styled(l.to_string(), Style::default().fg(Color::Red)))
        .collect();
    let max_len = msg_lines.iter().map(|l| l.width()).max().unwrap_or(40);
    let width = compute_dialog_width(max_len, frame.area().width).max(60);
    let height = compute_dialog_height(msg_lines.len().max(1), frame.area().height).max(8);
    let area = modal_rect(width, height, frame.area());

    frame.render_widget(Clear, area);
    frame.render_widget(dialog_block(&format!("Error — {}", dialog.title)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .margin(1)
        .split(area);

    let visible_h = chunks[0].height as usize;
    let visible = visible_lines(&msg_lines, 0, visible_h, chunks[0].width as usize);
    frame.render_widget(
        Paragraph::new(visible).wrap(Wrap { trim: false }),
        chunks[0],
    );

    let b_ok = button_span("OK", true);
    frame.render_widget(
        Paragraph::new(Line::from(vec![b_ok])).alignment(Alignment::Right),
        chunks[1],
    );
}
