use std::collections::{HashMap, HashSet};

use crate::browsers::BrowserAdapter;
use crate::domain::{
    BrowserCapabilities, BrowserInstall, BrowserProfile, HealthFinding, ProfileId,
};
use crate::tui::dialogs::{Dialog, ErrorDialog, PendingAction, QuitWaitState};
use crate::tui::keymap::fuzzy_match;
use crate::tui::sizes::{ScanRequest, SizeScanner};
use crate::tui::update_check::{UpdateCheckResult, UpdateChecker};

/// The three interactive panes in ProfileMux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Browsers,
    Profiles,
    Details,
}

impl Focus {
    /// Cycles to the next pane in forward order.
    pub fn next(self) -> Self {
        match self {
            Focus::Browsers => Focus::Profiles,
            Focus::Profiles => Focus::Details,
            Focus::Details => Focus::Browsers,
        }
    }

    /// Cycles to the previous pane in reverse order.
    pub fn prev(self) -> Self {
        match self {
            Focus::Browsers => Focus::Details,
            Focus::Profiles => Focus::Browsers,
            Focus::Details => Focus::Profiles,
        }
    }

    /// Moves focus horizontally leftward.
    pub fn left(self) -> Self {
        match self {
            Focus::Browsers => Focus::Browsers,
            Focus::Profiles => Focus::Browsers,
            Focus::Details => Focus::Profiles,
        }
    }

    /// Moves focus horizontally rightward.
    pub fn right(self) -> Self {
        match self {
            Focus::Browsers => Focus::Profiles,
            Focus::Profiles => Focus::Details,
            Focus::Details => Focus::Details,
        }
    }
}

/// Discovered browser adapter snapshot and profile collection.
pub struct BrowserItem {
    pub install: BrowserInstall,
    pub capabilities: BrowserCapabilities,
    pub is_running: bool,
    pub profiles: Vec<BrowserProfile>,
    pub doctor_findings: Vec<HealthFinding>,
    pub adapter: Box<dyn BrowserAdapter>,
}

/// Central application state for the ProfileMux terminal interface.
pub struct App {
    pub browsers: Vec<BrowserItem>,
    pub selected_browser: usize,
    pub selected_profile: usize,
    pub focus: Focus,
    pub filter: String,
    pub filter_mode: bool,
    pub filter_input: String,
    pub show_help: bool,
    pub show_browser_overlay: bool,
    pub show_profile_overlay: bool,
    pub status_message: Option<String>,
    pub should_quit: bool,
    pub scanner: SizeScanner,
    pub scanning_profiles: HashSet<ProfileId>,
    pub active_dialog: Option<Dialog>,
    pub pending_mutation: Option<PendingAction>,
    pub waiting_for_quit: Option<QuitWaitState>,
    /// Set when a key handler deliberately dismissed the active dialog, so the
    /// dialog router does not put it back.
    pub dialog_consumed: bool,
    /// Background release check. `None` until a result arrives.
    pub update: Option<UpdateCheckResult>,
    update_checker: UpdateChecker,
    pub appearance_cache: HashMap<ProfileId, crate::domain::Appearance>,
}

impl App {
    /// Discovers browser installations and initializes the TUI state.
    pub fn new() -> Self {
        let adapters = crate::browsers::discover_all();
        let mut browsers = Vec::with_capacity(adapters.len());

        for adapter in adapters {
            let install = adapter.install().clone();
            let capabilities = adapter.capabilities();
            let is_running = adapter.is_running();
            let profiles = adapter.list_profiles().unwrap_or_default();
            let doctor_findings = adapter.doctor().unwrap_or_default();

            browsers.push(BrowserItem {
                install,
                capabilities,
                is_running,
                profiles,
                doctor_findings,
                adapter,
            });
        }

        Self::with_browsers(browsers)
    }

    /// Initializes state with an explicitly supplied list of browsers (for testing).
    pub fn with_browsers(browsers: Vec<BrowserItem>) -> Self {
        let mut app = Self {
            browsers,
            selected_browser: 0,
            selected_profile: 0,
            focus: Focus::Browsers,
            filter: String::new(),
            filter_mode: false,
            filter_input: String::new(),
            show_help: false,
            show_browser_overlay: false,
            show_profile_overlay: false,
            status_message: None,
            should_quit: false,
            scanner: SizeScanner::new(),
            scanning_profiles: HashSet::new(),
            active_dialog: None,
            pending_mutation: None,
            waiting_for_quit: None,
            dialog_consumed: false,
            update: None,
            update_checker: UpdateChecker::spawn(),
            appearance_cache: HashMap::new(),
        };
        app.refresh_selected_appearance();
        app
    }

    /// Returns the currently selected browser, if any exist.
    pub fn current_browser(&self) -> Option<&BrowserItem> {
        self.browsers.get(self.selected_browser)
    }

    /// Returns a mutable reference to the currently selected browser.
    pub fn current_browser_mut(&mut self) -> Option<&mut BrowserItem> {
        self.browsers.get_mut(self.selected_browser)
    }

    /// Returns the filtered profile list for the current browser along with original indices.
    pub fn filtered_profiles(&self) -> Vec<(usize, &BrowserProfile)> {
        let Some(browser) = self.current_browser() else {
            return Vec::new();
        };
        browser
            .profiles
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                fuzzy_match(&self.filter, &p.display_name)
                    || fuzzy_match(&self.filter, &p.directory)
            })
            .collect()
    }

    /// Returns the currently selected profile, if one is chosen.
    pub fn current_profile(&self) -> Option<&BrowserProfile> {
        let filtered = self.filtered_profiles();
        filtered.get(self.selected_profile).map(|(_, p)| *p)
    }

    /// Moves selection up in the currently active pane.
    pub fn move_up(&mut self) {
        match self.focus {
            Focus::Browsers => {
                self.selected_browser = self.selected_browser.saturating_sub(1);
                self.selected_profile = 0;
                self.on_enter_profiles();
            }
            Focus::Profiles => {
                self.selected_profile = self.selected_profile.saturating_sub(1);
            }
            Focus::Details => {}
        }
        self.refresh_selected_appearance();
    }

    /// Moves selection down in the currently active pane.
    pub fn move_down(&mut self) {
        match self.focus {
            Focus::Browsers => {
                if self.selected_browser + 1 < self.browsers.len() {
                    self.selected_browser += 1;
                    self.selected_profile = 0;
                    self.on_enter_profiles();
                }
            }
            Focus::Profiles => {
                let count = self.filtered_profiles().len();
                if count > 0 && self.selected_profile + 1 < count {
                    self.selected_profile += 1;
                }
            }
            Focus::Details => {}
        }
        self.refresh_selected_appearance();
    }

    /// Sets the focus and queues background size measurements if entering the profile list.
    pub fn set_focus(&mut self, new_focus: Focus) {
        let prev_focus = self.focus;
        self.focus = new_focus;
        if prev_focus != Focus::Profiles && new_focus == Focus::Profiles {
            self.on_enter_profiles();
        }
    }

    pub fn focus_next(&mut self) {
        let next = self.focus.next();
        self.set_focus(next);
    }

    pub fn focus_prev(&mut self) {
        let prev = self.focus.prev();
        self.set_focus(prev);
    }

    pub fn focus_left(&mut self) {
        let left = self.focus.left();
        self.set_focus(left);
    }

    pub fn focus_right(&mut self) {
        let right = self.focus.right();
        self.set_focus(right);
    }

    /// Submits unscanned profiles of the current browser to the background worker.
    pub fn on_enter_profiles(&mut self) {
        let Some(browser) = self.browsers.get(self.selected_browser) else {
            return;
        };
        for profile in &browser.profiles {
            if profile.size.is_none() && !self.scanning_profiles.contains(&profile.id) {
                self.scanning_profiles.insert(profile.id.clone());
                self.scanner.request_scan(ScanRequest {
                    profile_id: profile.id.clone(),
                    profile_path: profile.path.clone(),
                    cache_path: profile.cache_path.clone(),
                });
            }
        }
    }

    /// Clears cached size and requests a fresh measurement of the active profile.
    pub fn rescan_selected_profile(&mut self) {
        let Some(profile) = self.current_profile() else {
            return;
        };
        let profile_id = profile.id.clone();
        let name = profile.display_name.clone();
        self.rescan_profile_id(self.selected_browser, &profile_id);
        self.status_message = Some(format!("Rescanning {name}..."));
    }

    /// Clears cached size and queues a scan for a specific profile ID.
    pub fn rescan_profile_id(&mut self, browser_idx: usize, profile_id: &ProfileId) {
        let Some(browser) = self.browsers.get_mut(browser_idx) else {
            return;
        };
        let Some(profile) = browser.profiles.iter_mut().find(|p| &p.id == profile_id) else {
            return;
        };
        profile.size = None;
        let request = ScanRequest {
            profile_id: profile.id.clone(),
            profile_path: profile.path.clone(),
            cache_path: profile.cache_path.clone(),
        };
        self.scanning_profiles.insert(profile.id.clone());
        self.scanner.request_scan(request);
    }

    /// Collects completed measurements from the size worker and attaches them.
    pub fn drain_scan_results(&mut self) {
        let results = self.scanner.drain_results();
        for (id, breakdown) in results {
            self.scanning_profiles.remove(&id);
            for browser in &mut self.browsers {
                for profile in &mut browser.profiles {
                    if profile.id == id {
                        *profile = profile.with_size(breakdown);
                    }
                }
            }
        }
    }

    pub fn start_search(&mut self) {
        self.filter_mode = true;
        self.filter_input = self.filter.clone();
    }

    pub fn cancel_filter(&mut self) {
        self.filter_mode = false;
        self.filter.clear();
        self.filter_input.clear();
        self.selected_profile = 0;
        self.refresh_selected_appearance();
    }

    pub fn accept_filter(&mut self) {
        self.filter_mode = false;
        self.filter = self.filter_input.clone();
        self.selected_profile = 0;
        self.refresh_selected_appearance();
    }

    pub fn filter_push(&mut self, c: char) {
        self.filter_input.push(c);
        self.filter = self.filter_input.clone();
        self.selected_profile = 0;
        self.refresh_selected_appearance();
    }

    pub fn filter_pop(&mut self) {
        self.filter_input.pop();
        self.filter = self.filter_input.clone();
        self.selected_profile = 0;
        self.refresh_selected_appearance();
    }

    pub fn close_overlays(&mut self) {
        self.show_help = false;
        self.show_browser_overlay = false;
        self.show_profile_overlay = false;
        self.active_dialog = None;
        self.status_message = None;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn toggle_browser_overlay(&mut self) {
        self.show_browser_overlay = !self.show_browser_overlay;
    }

    pub fn toggle_profile_overlay(&mut self) {
        self.show_profile_overlay = !self.show_profile_overlay;
    }

    pub fn open_dialog(&mut self, dialog: Dialog) {
        self.active_dialog = Some(dialog);
        self.dialog_consumed = false;
    }

    pub fn close_dialog(&mut self) {
        self.active_dialog = None;
        self.dialog_consumed = true;
    }

    /// Queues a mutation to be executed after drawing a "Working..." frame.
    pub fn queue_mutation(&mut self, action: PendingAction) {
        self.active_dialog = None;
        self.dialog_consumed = true;
        self.status_message = Some("Working...".to_string());
        self.pending_mutation = Some(action);
    }

    /// Starts a fresh update check, for the explicit refresh path.
    pub fn recheck_update(&mut self) {
        self.update = None;
        self.update_checker = UpdateChecker::spawn();
    }

    /// Periodic tick handler updating browser quit wait timeouts.
    pub fn tick(&mut self) {
        if self.update.is_none() {
            if let Some(result) = self.update_checker.poll() {
                self.update = Some(result);
            }
        }

        if let Some(wait) = self.waiting_for_quit.take() {
            let is_running = self
                .browsers
                .get(wait.browser_index)
                .map(|b| b.adapter.is_running())
                .unwrap_or(false);

            if !is_running {
                self.status_message = Some(format!("{} has stopped.", wait.browser_name));
                self.queue_mutation(wait.pending_action);
            } else if wait.start_time.elapsed() > std::time::Duration::from_secs(15) {
                self.status_message = Some(format!(
                    "Timed out waiting for {} to quit",
                    wait.browser_name
                ));
            } else {
                self.status_message = Some(format!("Waiting for {} to quit...", wait.browser_name));
                self.waiting_for_quit = Some(wait);
            }
        }
    }

    /// Reloads profiles from the adapter, preserving sizes for unaffected profiles.
    pub fn reload_browser_profiles(&mut self, browser_idx: usize, mutated_id: Option<&ProfileId>) {
        let Some(browser) = self.browsers.get_mut(browser_idx) else {
            return;
        };
        browser.is_running = browser.adapter.is_running();
        browser.capabilities = browser.adapter.capabilities();
        let fresh = browser.adapter.list_profiles().unwrap_or_default();
        let old = std::mem::replace(&mut browser.profiles, fresh);

        for prof in &mut browser.profiles {
            if Some(&prof.id) != mutated_id {
                if let Some(prev) = old.iter().find(|p| p.id == prof.id) {
                    if let Some(sz) = prev.size {
                        *prof = prof.with_size(sz);
                    }
                }
            }
        }
    }

    /// Executes the queued mutation on the main thread after the "Working..." render.
    pub fn execute_pending_mutation(&mut self) {
        let Some(action) = self.pending_mutation.take() else {
            return;
        };
        match action {
            PendingAction::CreateProfile {
                browser_index,
                spec,
            } => self.execute_create(browser_index, spec),
            PendingAction::CloneProfile {
                browser_index,
                source_profile,
                spec,
            } => self.execute_clone(browser_index, source_profile, spec),
            PendingAction::RenameProfile {
                browser_index,
                profile,
                new_name,
            } => self.execute_rename(browser_index, profile, new_name),
            PendingAction::DeleteProfile {
                browser_index,
                profile,
                mode,
            } => self.execute_delete(browser_index, profile, mode),
            PendingAction::SetAvatar {
                browser_index,
                profile,
                path,
            } => self.execute_set_avatar(browser_index, profile, path),
            PendingAction::CleanCache {
                browser_index,
                profile,
            } => self.execute_clean_cache(browser_index, profile),
            PendingAction::InstallUpdate { latest } => self.execute_update(latest),
            PendingAction::SetAppearance {
                browser_index,
                profile,
                spec,
            } => self.execute_set_appearance(browser_index, profile, spec),
        }
    }

    /// Downloads, checksum-verifies and installs the latest release. Runs on the
    /// main thread after a `Working...` frame, like the other mutations.
    fn execute_update(&mut self, latest: String) {
        match crate::update::install_latest() {
            Ok(installed) => {
                self.update = Some(crate::tui::update_check::UpdateCheckResult::UpToDate);
                self.status_message = Some(format!(
                    "Updated to {installed}. Restart ProfileMux to use the new version."
                ));
            }
            Err(err) => {
                self.status_message = None;
                self.open_dialog(Dialog::Error(ErrorDialog {
                    title: format!("Update to {latest} failed"),
                    message: err.to_string(),
                }));
            }
        }
    }

    fn execute_create(&mut self, b_idx: usize, spec: crate::domain::CreateProfileSpec) {
        let Some(browser) = self.browsers.get(b_idx) else {
            return;
        };
        match browser.adapter.create_profile(&spec) {
            Ok(new_p) => {
                self.reload_browser_profiles(b_idx, None);
                if let Some(b) = self.browsers.get(b_idx) {
                    if let Some(pos) = b.profiles.iter().position(|p| p.id == new_p.id) {
                        self.selected_profile = pos;
                    }
                }
                if spec.open_after_create {
                    if let Some(b) = self.browsers.get(b_idx) {
                        let _ = b.adapter.launch_profile(&new_p);
                    }
                }
                self.status_message = Some(format!("Created profile '{}'", new_p.display_name));
            }
            Err(e) => {
                self.active_dialog = Some(Dialog::Error(ErrorDialog {
                    title: "Create Profile Failed".to_string(),
                    message: e.to_string(),
                }));
            }
        }
    }

    fn execute_clone(
        &mut self,
        b_idx: usize,
        source: BrowserProfile,
        spec: crate::domain::CloneProfileSpec,
    ) {
        let Some(browser) = self.browsers.get(b_idx) else {
            return;
        };
        match browser.adapter.clone_profile(&source, &spec) {
            Ok(new_p) => {
                self.reload_browser_profiles(b_idx, None);
                if let Some(b) = self.browsers.get(b_idx) {
                    if let Some(pos) = b.profiles.iter().position(|p| p.id == new_p.id) {
                        self.selected_profile = pos;
                    }
                }
                if spec.open_after_create {
                    if let Some(b) = self.browsers.get(b_idx) {
                        let _ = b.adapter.launch_profile(&new_p);
                    }
                }
                self.status_message = Some(format!("Cloned profile to '{}'", new_p.display_name));
            }
            Err(e) => {
                self.active_dialog = Some(Dialog::Error(ErrorDialog {
                    title: "Clone Profile Failed".to_string(),
                    message: e.to_string(),
                }));
            }
        }
    }

    fn execute_rename(&mut self, b_idx: usize, profile: BrowserProfile, new_name: String) {
        let Some(browser) = self.browsers.get(b_idx) else {
            return;
        };
        match browser.adapter.rename_display_name(&profile, &new_name) {
            Ok(()) => {
                self.reload_browser_profiles(b_idx, None);
                self.status_message = Some(format!("Renamed profile to '{new_name}'"));
            }
            Err(e) => {
                self.active_dialog = Some(Dialog::Error(ErrorDialog {
                    title: "Rename Profile Failed".to_string(),
                    message: e.to_string(),
                }));
            }
        }
    }

    fn execute_delete(
        &mut self,
        b_idx: usize,
        profile: BrowserProfile,
        mode: crate::domain::DeleteMode,
    ) {
        let Some(browser) = self.browsers.get(b_idx) else {
            return;
        };
        match browser.adapter.delete_profile(&profile, mode) {
            Ok(()) => {
                self.reload_browser_profiles(b_idx, Some(&profile.id));
                let count = self.filtered_profiles().len();
                if self.selected_profile >= count && count > 0 {
                    self.selected_profile = count - 1;
                }
                self.status_message =
                    Some(format!("Moved profile '{}' to Trash", profile.display_name));
            }
            Err(e) => {
                self.active_dialog = Some(Dialog::Error(ErrorDialog {
                    title: "Delete Profile Failed".to_string(),
                    message: e.to_string(),
                }));
            }
        }
    }

    fn execute_set_avatar(
        &mut self,
        b_idx: usize,
        profile: BrowserProfile,
        path: std::path::PathBuf,
    ) {
        let Some(browser) = self.browsers.get(b_idx) else {
            return;
        };
        match browser.adapter.set_avatar(&profile, &path) {
            Ok(()) => {
                self.reload_browser_profiles(b_idx, None);
                self.status_message =
                    Some(format!("Updated avatar for '{}'", profile.display_name));
            }
            Err(e) => {
                self.active_dialog = Some(Dialog::Error(ErrorDialog {
                    title: "Set Avatar Failed".to_string(),
                    message: e.to_string(),
                }));
            }
        }
    }

    fn execute_clean_cache(&mut self, b_idx: usize, profile: BrowserProfile) {
        let Some(browser) = self.browsers.get(b_idx) else {
            return;
        };
        match browser.adapter.clean_cache(&profile) {
            Ok(bytes) => {
                self.reload_browser_profiles(b_idx, Some(&profile.id));
                self.rescan_profile_id(b_idx, &profile.id);
                let formatted = crate::fs::size::format_bytes(bytes);
                self.status_message = Some(format!(
                    "Cleaned cache for '{}' (reclaimed {formatted})",
                    profile.display_name
                ));
            }
            Err(e) => {
                self.active_dialog = Some(Dialog::Error(ErrorDialog {
                    title: "Clean Cache Failed".to_string(),
                    message: e.to_string(),
                }));
            }
        }
    }

    pub fn refresh_selected_appearance(&mut self) {
        let Some(profile) = self.current_profile() else {
            return;
        };
        let profile_id = profile.id.clone();
        let Some(browser) = self.current_browser() else {
            return;
        };
        let theme = match browser.adapter.read_appearance(profile) {
            Ok(app) => app.theme,
            Err(_) => crate::domain::BrowserTheme::Unknown,
        };
        let web_dark = crate::policy::default_path()
            .ok()
            .and_then(|p| crate::policy::load(&p).ok())
            .map(|s| s.get(&profile_id).web_dark)
            .unwrap_or(crate::domain::WebDarkMode::Normal);

        self.appearance_cache
            .insert(profile_id, crate::domain::Appearance { theme, web_dark });
    }

    fn apply_appearance_spec(
        &mut self,
        b_idx: usize,
        profile: &BrowserProfile,
        spec: &crate::domain::AppearanceSpec,
    ) -> crate::error::Result<()> {
        let Some(browser) = self.browsers.get(b_idx) else {
            return Ok(());
        };
        if spec.theme.is_some() {
            browser.adapter.set_appearance(profile, spec)?;
        }
        if let Some(web_dark) = spec.web_dark {
            let policy_path = crate::policy::default_path()?;
            let store = crate::policy::load(&policy_path)?;
            let mut policy = store.get(&profile.id);
            policy.web_dark = web_dark;
            store.set(&profile.id, policy).save()?;
        }
        Ok(())
    }

    fn execute_set_appearance(
        &mut self,
        b_idx: usize,
        profile: BrowserProfile,
        spec: crate::domain::AppearanceSpec,
    ) {
        if let Err(e) = self.apply_appearance_spec(b_idx, &profile, &spec) {
            self.status_message = None;
            self.open_dialog(Dialog::Error(ErrorDialog {
                title: "Set Appearance Failed".to_string(),
                message: e.to_string(),
            }));
            return;
        }
        self.refresh_selected_appearance();
        let mut parts = Vec::new();
        if let Some(t) = spec.theme {
            parts.push(format!("theme: {}", t.label()));
        }
        if let Some(w) = spec.web_dark {
            parts.push(format!("web dark: {}", w.label()));
        }
        let details = parts.join(", ");
        self.status_message = Some(format!(
            "Updated appearance ({details}). Force Dark applies from the next launch ProfileMux performs."
        ));
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
