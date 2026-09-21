use std::collections::HashSet;

use crate::domain::{
    BrowserCapabilities, BrowserInstall, BrowserProfile, HealthFinding, ProfileId,
};
use crate::tui::keymap::fuzzy_match;
use crate::tui::sizes::{ScanRequest, SizeScanner};

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
            });
        }

        Self {
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
        }
    }

    /// Returns the currently selected browser, if any exist.
    pub fn current_browser(&self) -> Option<&BrowserItem> {
        self.browsers.get(self.selected_browser)
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
        let profile_path = profile.path.clone();
        let cache_path = profile.cache_path.clone();
        let name = profile.display_name.clone();

        if let Some(browser) = self.browsers.get_mut(self.selected_browser) {
            if let Some(p) = browser.profiles.iter_mut().find(|p| p.id == profile_id) {
                p.size = None;
            }
        }
        self.scanning_profiles.insert(profile_id.clone());
        self.scanner.request_scan(ScanRequest {
            profile_id,
            profile_path,
            cache_path,
        });
        self.status_message = Some(format!("Rescanning {name}..."));
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
    }

    pub fn accept_filter(&mut self) {
        self.filter_mode = false;
        self.filter = self.filter_input.clone();
        self.selected_profile = 0;
    }

    pub fn filter_push(&mut self, c: char) {
        self.filter_input.push(c);
        self.filter = self.filter_input.clone();
        self.selected_profile = 0;
    }

    pub fn filter_pop(&mut self) {
        self.filter_input.pop();
        self.filter = self.filter_input.clone();
        self.selected_profile = 0;
    }

    pub fn close_overlays(&mut self) {
        self.show_help = false;
        self.show_browser_overlay = false;
        self.show_profile_overlay = false;
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
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
