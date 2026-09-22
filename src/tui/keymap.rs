use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Actions supported by the terminal user interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    FocusNext,
    FocusPrev,
    FocusLeft,
    FocusRight,
    MoveUp,
    MoveDown,
    StartSearch,
    ToggleHelp,
    ToggleBrowserDetails,
    ToggleProfileDetails,
    CloseOverlay,
    Refresh,
    OpenFolder,
    NewProfile,
    CloneProfile,
    RenameProfile,
    DeleteProfile,
    LaunchProfile,
    SetAvatar,
    Doctor,
    CleanCache,
    Update,
    Appearance,
}

/// Pure case-insensitive subsequence matcher.
/// Returns true if all characters in `pattern` appear in `candidate` in order.
pub fn fuzzy_match(pattern: &str, candidate: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    let mut pat_chars = pattern.chars().flat_map(|c| c.to_lowercase());
    let mut current_pat = match pat_chars.next() {
        Some(c) => c,
        None => return true,
    };

    for cand_c in candidate.chars().flat_map(|c| c.to_lowercase()) {
        if cand_c == current_pat {
            match pat_chars.next() {
                Some(next_c) => current_pat = next_c,
                None => return true,
            }
        }
    }
    false
}

/// Maps keyboard inputs to UI actions when not in text input mode.
pub fn map_key(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Some(Action::Quit),
        KeyCode::Tab => Some(Action::FocusNext),
        KeyCode::BackTab => Some(Action::FocusPrev),
        KeyCode::Char('h') | KeyCode::Left => Some(Action::FocusLeft),
        KeyCode::Char('l') | KeyCode::Right => Some(Action::FocusRight),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('/') => Some(Action::StartSearch),
        KeyCode::Char('?') => Some(Action::ToggleHelp),
        KeyCode::Char('b') | KeyCode::Char('B') => Some(Action::ToggleBrowserDetails),
        KeyCode::Char('i') | KeyCode::Char('I') => Some(Action::ToggleProfileDetails),
        KeyCode::Enter => Some(Action::LaunchProfile),
        KeyCode::Esc => Some(Action::CloseOverlay),
        KeyCode::F(5) => Some(Action::Refresh),
        KeyCode::Char('r') | KeyCode::Char('R') => Some(Action::RenameProfile),
        KeyCode::Char('u') | KeyCode::Char('U') => Some(Action::Update),
        KeyCode::Char('n') | KeyCode::Char('N') => Some(Action::NewProfile),
        KeyCode::Char('c') | KeyCode::Char('C') => Some(Action::CloneProfile),
        KeyCode::Char('d') | KeyCode::Char('D') => Some(Action::DeleteProfile),
        KeyCode::Char('L') => Some(Action::LaunchProfile),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(Action::SetAvatar),
        KeyCode::Char('o') | KeyCode::Char('O') => Some(Action::OpenFolder),
        KeyCode::Char('H') => Some(Action::Doctor),
        KeyCode::Char('x') | KeyCode::Char('X') => Some(Action::CleanCache),
        KeyCode::Char('t') | KeyCode::Char('T') => Some(Action::Appearance),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::Focus;

    #[test]
    fn test_empty_filter_matches_everything() {
        assert!(fuzzy_match("", ""));
        assert!(fuzzy_match("", "Default"));
        assert!(fuzzy_match("", "Work Profile"));
    }

    #[test]
    fn test_subsequence_matching_is_case_insensitive() {
        assert!(fuzzy_match("abc", "ABC"));
        assert!(fuzzy_match("abc", "a-b-c"));
        assert!(fuzzy_match("def", "Default"));
        assert!(fuzzy_match("DEF", "default"));
        assert!(fuzzy_match("work", "My Work Profile"));
        assert!(fuzzy_match("mwp", "My Work Profile"));
    }

    #[test]
    fn test_non_subsequences_do_not_match() {
        assert!(!fuzzy_match("xyz", "Default"));
        assert!(!fuzzy_match("cba", "abc"));
        assert!(!fuzzy_match("profx", "profile"));
    }

    #[test]
    fn test_focus_cycling_wraps_correctly() {
        assert_eq!(Focus::Browsers.next(), Focus::Profiles);
        assert_eq!(Focus::Profiles.next(), Focus::Details);
        assert_eq!(Focus::Details.next(), Focus::Browsers);

        assert_eq!(Focus::Browsers.prev(), Focus::Details);
        assert_eq!(Focus::Details.prev(), Focus::Profiles);
        assert_eq!(Focus::Profiles.prev(), Focus::Browsers);
    }

    #[test]
    fn test_key_mappings() {
        assert_eq!(
            map_key(KeyEvent::from(KeyCode::Enter)),
            Some(Action::LaunchProfile)
        );
        assert_eq!(
            map_key(KeyEvent::from(KeyCode::Char('i'))),
            Some(Action::ToggleProfileDetails)
        );
        assert_eq!(
            map_key(KeyEvent::from(KeyCode::Char('a'))),
            Some(Action::SetAvatar)
        );
        assert_eq!(
            map_key(KeyEvent::from(KeyCode::Char('H'))),
            Some(Action::Doctor)
        );
        assert_eq!(
            map_key(KeyEvent::from(KeyCode::Char('t'))),
            Some(Action::Appearance)
        );
        assert_eq!(
            map_key(KeyEvent::from(KeyCode::Char('T'))),
            Some(Action::Appearance)
        );
    }
}
