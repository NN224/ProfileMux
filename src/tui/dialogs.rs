use std::path::PathBuf;

use crate::domain::{
    BrowserProfile, CloneProfileSpec, CreateProfileSpec, DeleteMode, HealthFinding, OperationPlan,
};
use crate::tui::form::{FormField, ProfileForm};

#[path = "dialog_render.rs"]
mod dialog_render;

#[path = "dialog_helpers.rs"]
mod dialog_helpers;

pub use dialog_helpers::*;
pub use dialog_render::{button_span, dialog_block, modal_rect, render_dialog};

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

/// Represents the user's intent when activating a button in a modal dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogOutcome {
    None,
    Close,
    Activate,
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

    pub fn activate_selected(&self) -> DialogOutcome {
        if self.focused_button == 0 {
            DialogOutcome::Activate
        } else {
            DialogOutcome::Close
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeleteInfo {
    pub profile_name: String,
    pub directory: String,
    pub profile_data_size: String,
    pub cache_size: String,
    pub total_size: String,
    pub core_bytes: Option<u64>,
    pub cache_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

impl DeleteInfo {
    pub fn from_profile(profile: &BrowserProfile) -> Self {
        let (total_bytes, core_bytes, cache_bytes) = match profile.size {
            Some(s) => (
                Some(s.total),
                Some(s.core),
                Some(
                    s.cache
                        .saturating_add(s.code_cache)
                        .saturating_add(s.gpu_cache),
                ),
            ),
            None => (None, None, None),
        };
        let total = total_bytes
            .map(crate::fs::size::format_bytes)
            .unwrap_or_else(|| "not measured".to_string());
        let core = core_bytes
            .map(crate::fs::size::format_bytes)
            .unwrap_or_else(|| "not measured".to_string());
        let cache = cache_bytes
            .map(crate::fs::size::format_bytes)
            .unwrap_or_else(|| "not measured".to_string());
        Self {
            profile_name: profile.display_name.clone(),
            directory: profile.directory.clone(),
            profile_data_size: core,
            cache_size: cache,
            total_size: total,
            core_bytes,
            cache_bytes,
            total_bytes,
        }
    }

    pub fn with_sizes(
        profile_name: impl Into<String>,
        directory: impl Into<String>,
        core: Option<u64>,
        cache: Option<u64>,
        total: Option<u64>,
    ) -> Self {
        let profile_data_size = core.map(crate::fs::size::format_bytes).unwrap_or_default();
        let cache_size = cache.map(crate::fs::size::format_bytes).unwrap_or_default();
        let total_size = total.map(crate::fs::size::format_bytes).unwrap_or_default();
        Self {
            profile_name: profile_name.into(),
            directory: directory.into(),
            profile_data_size,
            cache_size,
            total_size,
            core_bytes: core,
            cache_bytes: cache,
            total_bytes: total,
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

/// Default page scroll step for modal dialogs.
pub const DIALOG_PAGE: usize = 10;

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
    pub scroll: usize,
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
            scroll: 0,
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        let max_scroll = build_confirmation_lines(self).len().saturating_sub(1);
        self.scroll = self.scroll.saturating_add(1).min(max_scroll);
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(DIALOG_PAGE);
    }

    pub fn scroll_page_down(&mut self) {
        let max_scroll = build_confirmation_lines(self).len().saturating_sub(1);
        self.scroll = self.scroll.saturating_add(DIALOG_PAGE).min(max_scroll);
    }

    pub fn activate_selected(&self) -> DialogOutcome {
        match self.focused_button {
            ConfirmButton::Action => DialogOutcome::Activate,
            ConfirmButton::Safe => DialogOutcome::Close,
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

    pub fn activate_selected(&self) -> DialogOutcome {
        match self.focused_button {
            ConfirmButton::Action => DialogOutcome::Activate,
            ConfirmButton::Safe => DialogOutcome::Close,
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

impl DoctorDialog {
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(DIALOG_PAGE);
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll = self.scroll.saturating_add(DIALOG_PAGE);
    }

    pub fn activate_selected(&self) -> DialogOutcome {
        DialogOutcome::Close
    }
}

/// Dismissible error dialog.
#[derive(Debug, Clone)]
pub struct ErrorDialog {
    pub title: String,
    pub message: String,
}

impl ErrorDialog {
    pub fn activate_selected(&self) -> DialogOutcome {
        DialogOutcome::Close
    }
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

    pub fn activate_selected(&self) -> DialogOutcome {
        match self {
            Dialog::TextInput(d) => d.activate_selected(),
            Dialog::Form(f) => {
                if f.focused_field == FormField::Buttons {
                    if f.focused_button == 0 {
                        DialogOutcome::Activate
                    } else {
                        DialogOutcome::Close
                    }
                } else if f.focused_field == FormField::OpenAfterCreate {
                    DialogOutcome::Activate
                } else {
                    DialogOutcome::None
                }
            }
            Dialog::Confirmation(c) => c.activate_selected(),
            Dialog::BrowserRunning(b) => b.activate_selected(),
            Dialog::Doctor(d) => d.activate_selected(),
            Dialog::Error(e) => e.activate_selected(),
        }
    }
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
            account_email: None,
            last_active: None,
            registered: true,
            directory_exists: true,
            size: None,
        }
    }

    #[test]
    fn test_computed_dialog_height_grows_with_plan_lines() {
        let h1 = compute_dialog_height(4, 50);
        let h2 = compute_dialog_height(10, 50);
        let h3 = compute_dialog_height(18, 50);
        assert!(h2 > h1);
        assert!(h3 > h2);
    }

    #[test]
    fn test_computed_dialog_height_clamped_to_small_area() {
        let small_height = 10;
        let computed = compute_dialog_height(50, small_height);
        assert!(computed <= small_height);
        assert_eq!(computed, 8);
    }

    #[test]
    fn test_scroll_offset_clamping() {
        let total = 12;
        let visible = 4;
        assert_eq!(clamp_scroll(0, total, visible), 0);
        assert_eq!(clamp_scroll(3, total, visible), 3);
        assert_eq!(clamp_scroll(8, total, visible), 8);
        assert_eq!(clamp_scroll(9, total, visible), 8);
        assert_eq!(clamp_scroll(100, total, visible), 8);

        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Chrome", "Default");
        let action = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: make_test_profile(),
            mode: DeleteMode::Trash,
        };
        let mut dialog = ConfirmationDialog::new("Confirm", plan, None, "Delete", action);
        assert_eq!(dialog.scroll, 0);
        dialog.scroll_up();
        assert_eq!(dialog.scroll, 0);
        dialog.scroll_down();
        assert_eq!(dialog.scroll, 1);
        dialog.scroll_down();
        assert_eq!(dialog.scroll, 2);
        dialog.scroll_up();
        assert_eq!(dialog.scroll, 1);
    }

    #[test]
    fn test_visible_lines_slicing_and_indicator() {
        let lines: Vec<ratatui::text::Line<'static>> = (0..8)
            .map(|i| ratatui::text::Line::raw(format!("Line {i}")))
            .collect();

        // Fits completely
        let vis = visible_lines(&lines, 0, 10, 40);
        assert_eq!(vis.len(), 8);

        // Does not fit, scroll at start -> shows ... more on last row
        let vis = visible_lines(&lines, 0, 4, 40);
        assert_eq!(vis.len(), 4);
        assert_eq!(vis[0].to_string(), "Line 0");
        assert_eq!(vis[2].to_string(), "Line 2");
        assert!(vis[3].to_string().contains("... more"));

        // Scrolled to bottom -> no ... more
        let vis = visible_lines(&lines, 4, 4, 40);
        assert_eq!(vis.len(), 4);
        assert_eq!(vis[0].to_string(), "Line 4");
        assert_eq!(vis[3].to_string(), "Line 7");
        assert!(!vis.iter().any(|l| l.to_string().contains("... more")));
    }

    #[test]
    fn test_delete_confirmation_body_contains_size_lines() {
        let mut plan = OperationPlan::new(OperationKind::DeleteProfile, "Chrome", "Default");
        plan.steps
            .push(crate::domain::PlanStep::new("Remove profile files"));
        plan.reclaimed_bytes = Some(3_800_000);

        let delete_info = DeleteInfo::with_sizes(
            "Default",
            "Default",
            Some(1_200_000),
            Some(2_600_000),
            Some(3_800_000),
        );

        let action = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: make_test_profile(),
            mode: DeleteMode::Trash,
        };
        let dialog = ConfirmationDialog::new(
            "Delete Profile",
            plan,
            Some(delete_info),
            "Move to Trash",
            action,
        );

        let body = build_confirmation_lines(&dialog);
        let body_strs: Vec<String> = body.iter().map(|l| l.to_string()).collect();

        assert!(body_strs.iter().any(|s| s.contains("Profile data:")));
        assert!(body_strs.iter().any(|s| s.contains("Cache:")));
        assert!(body_strs.iter().any(|s| s.contains("Total:")));
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

    #[test]
    fn test_plan_visible_lines_fit_and_short_area() {
        let mut plan = OperationPlan::new(OperationKind::DeleteProfile, "Chrome", "Default");
        for i in 1..=8 {
            plan.steps
                .push(crate::domain::PlanStep::new(format!("Step {i}")));
        }
        let action = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: make_test_profile(),
            mode: DeleteMode::Trash,
        };
        let dialog = ConfirmationDialog::new("Confirm", plan, None, "Delete", action);
        let lines = build_confirmation_lines(&dialog);

        let vis_fit = visible_lines(&lines, 0, 30, 80);
        assert_eq!(vis_fit.len(), lines.len());
        assert!(!vis_fit.iter().any(|l| l.to_string().contains("... more")));

        let vis_short = visible_lines(&lines, 0, 5, 80);
        assert_eq!(vis_short.len(), 5);
        assert!(vis_short.last().unwrap().to_string().contains("... more"));
    }

    #[test]
    fn test_delete_confirmation_body_size_lines_after_scrolling_to_bottom() {
        let mut plan = OperationPlan::new(OperationKind::DeleteProfile, "Chrome", "Default");
        plan.steps
            .push(crate::domain::PlanStep::new("Remove profile files"));
        plan.reclaimed_bytes = Some(3_800_000);

        let delete_info = DeleteInfo::with_sizes(
            "Default",
            "Default",
            Some(1_200_000),
            Some(2_600_000),
            Some(3_800_000),
        );
        let action = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: make_test_profile(),
            mode: DeleteMode::Trash,
        };
        let mut dialog = ConfirmationDialog::new(
            "Delete Profile",
            plan,
            Some(delete_info),
            "Move to Trash",
            action,
        );

        for _ in 0..50 {
            dialog.scroll_down();
        }

        let body = build_confirmation_lines(&dialog);
        let body_strs: Vec<String> = body.iter().map(|l| l.to_string()).collect();
        assert!(body_strs.iter().any(|s| s.contains("Profile data:")));
        assert!(body_strs.iter().any(|s| s.contains("Cache:")));
        assert!(body_strs.iter().any(|s| s.contains("Total:")));
    }

    #[test]
    fn test_scroll_down_clamps_and_scroll_up_stays_zero() {
        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Chrome", "Default");
        let action = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: make_test_profile(),
            mode: DeleteMode::Trash,
        };
        let mut dialog = ConfirmationDialog::new("Confirm", plan, None, "Delete", action);

        assert_eq!(dialog.scroll, 0);
        dialog.scroll_up();
        assert_eq!(dialog.scroll, 0);

        let lines = build_confirmation_lines(&dialog);
        let max_scroll = lines.len().saturating_sub(1);
        for _ in 0..100 {
            dialog.scroll_down();
        }
        assert_eq!(dialog.scroll, max_scroll);
    }

    #[test]
    fn test_dialog_outcome_activation_contract() {
        let prof = make_test_profile();
        let mut txt = TextInputDialog::new_rename(0, prof.clone());
        txt.focused_button = 0;
        assert_eq!(txt.activate_selected(), DialogOutcome::Activate);
        txt.focused_button = 1;
        assert_eq!(txt.activate_selected(), DialogOutcome::Close);

        let plan = OperationPlan::new(OperationKind::DeleteProfile, "Chrome", "Default");
        let act = PendingAction::DeleteProfile {
            browser_index: 0,
            profile: prof.clone(),
            mode: DeleteMode::Trash,
        };
        let mut conf = ConfirmationDialog::new("Confirm", plan, None, "Delete", act.clone());
        conf.focused_button = ConfirmButton::Safe;
        assert_eq!(conf.activate_selected(), DialogOutcome::Close);
        conf.focused_button = ConfirmButton::Action;
        assert_eq!(conf.activate_selected(), DialogOutcome::Activate);

        let mut br = BrowserRunningDialog::new(0, "Chrome", act);
        br.focused_button = ConfirmButton::Safe;
        assert_eq!(br.activate_selected(), DialogOutcome::Close);
        br.focused_button = ConfirmButton::Action;
        assert_eq!(br.activate_selected(), DialogOutcome::Activate);

        let doc = DoctorDialog {
            browser_name: "Chrome".to_string(),
            findings: Vec::new(),
            scroll: 0,
        };
        assert_eq!(doc.activate_selected(), DialogOutcome::Close);

        let err = ErrorDialog {
            title: "Err".to_string(),
            message: "msg".to_string(),
        };
        assert_eq!(err.activate_selected(), DialogOutcome::Close);
    }
}
