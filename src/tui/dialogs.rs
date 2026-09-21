use std::path::PathBuf;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::domain::{
    BrowserProfile, CloneProfileSpec, CreateProfileSpec, DeleteMode, HealthFinding, OperationPlan,
    Severity,
};
use crate::tui::app::App;
use crate::tui::form::{FormField, ProfileForm};

/// Expands a leading tilde in a filesystem path using the HOME environment variable.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    } else if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(path)
}

/// Identifies the purpose of a simple text input modal.
#[derive(Debug, Clone)]
pub enum TextInputKind {
    Rename {
        browser_index: usize,
        profile: BrowserProfile,
    },
    Avatar {
        browser_index: usize,
        profile: BrowserProfile,
    },
}

/// Generic text input modal state.
#[derive(Debug, Clone)]
pub struct TextInputDialog {
    pub title: String,
    pub prompt: String,
    pub value: String,
    pub cursor: usize,
    pub kind: TextInputKind,
    pub focused_button: usize, // 0: Save/OK, 1: Cancel
}

impl TextInputDialog {
    pub fn new_rename(browser_index: usize, profile: BrowserProfile) -> Self {
        let val = profile.display_name.clone();
        let len = val.len();
        Self {
            title: "Rename Profile".to_string(),
            prompt: "New display name:".to_string(),
            value: val,
            cursor: len,
            kind: TextInputKind::Rename {
                browser_index,
                profile,
            },
            focused_button: 0,
        }
    }

    pub fn new_avatar(browser_index: usize, profile: BrowserProfile) -> Self {
        Self {
            title: "Set Profile Avatar".to_string(),
            prompt: "Avatar image path (~ supported):".to_string(),
            value: String::new(),
            cursor: 0,
            kind: TextInputKind::Avatar {
                browser_index,
                profile,
            },
            focused_button: 0,
        }
    }

    pub fn handle_char(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn handle_backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.value.remove(self.cursor);
        }
    }
}

/// The two buttons present on confirmation and prompt dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmButton {
    Safe,   // Cancel - safe option, focused by default
    Action, // Confirm / Proceed
}

impl ConfirmButton {
    pub fn toggle(self) -> Self {
        match self {
            ConfirmButton::Safe => ConfirmButton::Action,
            ConfirmButton::Action => ConfirmButton::Safe,
        }
    }
}

/// Profile storage metrics formatted specifically for the deletion confirmation modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteInfo {
    pub profile_name: String,
    pub directory: String,
    pub profile_data_size: String,
    pub cache_size: String,
    pub total_size: String,
}

impl DeleteInfo {
    pub fn from_profile(profile: &BrowserProfile) -> Self {
        let (total, core, cache) = match profile.size {
            Some(s) => (
                crate::fs::size::format_bytes(s.total),
                crate::fs::size::format_bytes(s.core),
                crate::fs::size::format_bytes(s.cache + s.code_cache + s.gpu_cache),
            ),
            None => (
                "not measured".to_string(),
                "not measured".to_string(),
                "not measured".to_string(),
            ),
        };
        Self {
            profile_name: profile.display_name.clone(),
            directory: profile.directory.clone(),
            profile_data_size: core,
            cache_size: cache,
            total_size: total,
        }
    }
}

/// Pending mutation actions ready to be executed upon confirmation.
#[derive(Debug, Clone)]
pub enum PendingAction {
    CreateProfile {
        browser_index: usize,
        spec: CreateProfileSpec,
    },
    CloneProfile {
        browser_index: usize,
        source_profile: BrowserProfile,
        spec: CloneProfileSpec,
    },
    RenameProfile {
        browser_index: usize,
        profile: BrowserProfile,
        new_name: String,
    },
    DeleteProfile {
        browser_index: usize,
        profile: BrowserProfile,
        mode: DeleteMode,
    },
    SetAvatar {
        browser_index: usize,
        profile: BrowserProfile,
        path: PathBuf,
    },
    CleanCache {
        browser_index: usize,
        profile: BrowserProfile,
    },
}

/// Plan execution confirmation modal state.
#[derive(Debug, Clone)]
pub struct ConfirmationDialog {
    pub title: String,
    pub plan: OperationPlan,
    pub delete_info: Option<DeleteInfo>,
    pub action_button_label: String,
    pub cancel_button_label: String,
    pub focused_button: ConfirmButton,
    pub action: PendingAction,
}

impl ConfirmationDialog {
    pub fn new(
        title: impl Into<String>,
        plan: OperationPlan,
        delete_info: Option<DeleteInfo>,
        action_button_label: impl Into<String>,
        action: PendingAction,
    ) -> Self {
        Self {
            title: title.into(),
            plan,
            delete_info,
            action_button_label: action_button_label.into(),
            cancel_button_label: "Cancel".to_string(),
            focused_button: ConfirmButton::Safe, // Safe Cancel focused by default
            action,
        }
    }
}

/// Modal prompting the user to quit a running browser before mutation.
#[derive(Debug, Clone)]
pub struct BrowserRunningDialog {
    pub browser_index: usize,
    pub browser_name: String,
    pub focused_button: ConfirmButton, // Cancel focused by default
    pub pending_action: PendingAction,
}

impl BrowserRunningDialog {
    pub fn new(
        browser_index: usize,
        browser_name: impl Into<String>,
        pending_action: PendingAction,
    ) -> Self {
        Self {
            browser_index,
            browser_name: browser_name.into(),
            focused_button: ConfirmButton::Safe,
            pending_action,
        }
    }
}

/// Modal listing browser diagnostic and health findings.
#[derive(Debug, Clone)]
pub struct DoctorDialog {
    pub browser_name: String,
    pub findings: Vec<HealthFinding>,
    pub scroll: usize,
}

/// Dismissible error dialog.
#[derive(Debug, Clone)]
pub struct ErrorDialog {
    pub title: String,
    pub message: String,
}

/// Tracks a pending operation that is waiting for a browser to quit cleanly.
pub struct QuitWaitState {
    pub browser_index: usize,
    pub browser_name: String,
    pub start_time: std::time::Instant,
    pub pending_action: PendingAction,
}

/// The active modal dialog currently being displayed.
pub enum Dialog {
    TextInput(TextInputDialog),
    // Boxed: the form is by far the largest variant.
    Form(Box<ProfileForm>),
    // Boxed: carries a full OperationPlan and a pending action.
    Confirmation(Box<ConfirmationDialog>),
    BrowserRunning(BrowserRunningDialog),
    Doctor(DoctorDialog),
    Error(ErrorDialog),
}

impl Dialog {
    /// Wraps the boxed form variant so call sites stay readable.
    pub fn form(form: ProfileForm) -> Dialog {
        Dialog::Form(Box::new(form))
    }

    /// Wraps the boxed confirmation variant so call sites stay readable.
    pub fn confirmation(dialog: ConfirmationDialog) -> Dialog {
        Dialog::Confirmation(Box::new(dialog))
    }
}

/// Builds a styled block wrapper for dialog overlays.
fn dialog_block(title: &str) -> Block<'static> {
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
fn modal_rect(width: u16, height: u16, area: Rect) -> Rect {
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

fn button_span(label: &str, is_focused: bool) -> Span<'static> {
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
    let area = modal_rect(60, 9, frame.area());
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
    let area = modal_rect(72, 19, frame.area());
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

fn build_delete_info_lines(info: &DeleteInfo) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("Profile:   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(info.profile_name.clone()),
        ]),
        Line::from(vec![
            Span::styled("Directory: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(info.directory.clone()),
        ]),
        Line::from(vec![
            Span::styled("Data Size: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(info.profile_data_size.clone()),
        ]),
        Line::from(vec![
            Span::styled("Cache:     ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(info.cache_size.clone()),
        ]),
        Line::from(vec![
            Span::styled("Total:     ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(info.total_size.clone()),
        ]),
        Line::raw(""),
    ]
}

fn render_confirmation(frame: &mut Frame, dialog: &ConfirmationDialog) {
    let area = modal_rect(72, 18, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(dialog_block(&dialog.title), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .margin(1)
        .split(area);

    let mut lines = Vec::new();
    if let Some(info) = &dialog.delete_info {
        lines.extend(build_delete_info_lines(info));
    }

    for l in dialog.plan.lines() {
        lines.push(Line::raw(l));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[0]);

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
    let area = modal_rect(64, 10, frame.area());
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
    frame.render_widget(Paragraph::new(msg).wrap(Wrap { trim: false }), chunks[0]);

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

fn render_doctor(frame: &mut Frame, dialog: &DoctorDialog) {
    let area = modal_rect(72, 18, frame.area());
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

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[0]);

    let b_close = button_span("Close", true);
    frame.render_widget(
        Paragraph::new(Line::from(vec![b_close])).alignment(Alignment::Right),
        chunks[1],
    );
}

fn render_error(frame: &mut Frame, dialog: &ErrorDialog) {
    let area = modal_rect(60, 10, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(dialog_block(&format!("Error — {}", dialog.title)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .margin(1)
        .split(area);

    let lines = vec![Line::styled(
        &dialog.message,
        Style::default().fg(Color::Red),
    )];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[0]);

    let b_ok = button_span("OK", true);
    frame.render_widget(
        Paragraph::new(Line::from(vec![b_ok])).alignment(Alignment::Right),
        chunks[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BrowserInstallId, BrowserKind, OperationKind, ProfileId};
    use std::path::Path;

    fn install_id() -> BrowserInstallId {
        BrowserInstallId::new(BrowserKind::Chrome, Path::new("/tmp/chrome"))
    }

    fn make_test_profile() -> BrowserProfile {
        BrowserProfile {
            id: ProfileId::new(&install_id(), "Default"),
            install_id: install_id(),
            display_name: "Default".to_string(),
            directory: "Default".to_string(),
            path: PathBuf::from("/tmp/Default"),
            cache_path: None,
            avatar: None,
            last_active: None,
            registered: true,
            directory_exists: true,
            size: None,
        }
    }

    #[test]
    fn test_confirmation_defaults_to_safe_button() {
        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Chrome", "Default");
        let action = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: make_test_profile(),
            mode: DeleteMode::Trash,
        };
        let dialog = ConfirmationDialog::new("Confirm Delete", plan, None, "Delete", action);
        assert_eq!(dialog.focused_button, ConfirmButton::Safe);
    }

    #[test]
    fn test_browser_running_defaults_to_safe_button() {
        let action = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: make_test_profile(),
            mode: DeleteMode::Trash,
        };
        let dialog = BrowserRunningDialog::new(0, "Google Chrome", action);
        assert_eq!(dialog.focused_button, ConfirmButton::Safe);
    }
}
