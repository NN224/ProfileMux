use std::path::PathBuf;

use crate::domain::sanitize::sanitize_directory_name;
use crate::domain::{
    BrowserProfile, ClonePolicy, CloneProfileSpec, CreateProfileSpec, ExtensionPolicy,
};
use crate::tui::dialogs::expand_tilde;

/// Form field selection order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Name,
    Directory,
    Template,
    Extensions,
    Avatar,
    OpenAfterCreate,
    Buttons,
}

impl FormField {
    pub fn next(self) -> Self {
        match self {
            FormField::Name => FormField::Directory,
            FormField::Directory => FormField::Template,
            FormField::Template => FormField::Extensions,
            FormField::Extensions => FormField::Avatar,
            FormField::Avatar => FormField::OpenAfterCreate,
            FormField::OpenAfterCreate => FormField::Buttons,
            FormField::Buttons => FormField::Name,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            FormField::Name => FormField::Buttons,
            FormField::Directory => FormField::Name,
            FormField::Template => FormField::Directory,
            FormField::Extensions => FormField::Template,
            FormField::Avatar => FormField::Extensions,
            FormField::OpenAfterCreate => FormField::Avatar,
            FormField::Buttons => FormField::OpenAfterCreate,
        }
    }
}

/// Selector option for the profile template list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateOption {
    pub display_name: String,
    pub directory: Option<String>,
}

/// Profile creation and cloning modal form state.
pub struct ProfileForm {
    pub name: String,
    pub directory: String,
    pub directory_manually_edited: bool,
    pub template_index: usize,
    pub template_options: Vec<TemplateOption>,
    pub extension_policy: ExtensionPolicy,
    pub avatar_path: String,
    pub open_after_create: bool,
    pub focused_field: FormField,
    pub user_data_root: PathBuf,
    pub is_clone: bool,
    pub focused_button: usize, // 0: Submit, 1: Cancel
}

/// Cycles forward through extension policies.
pub fn cycle_extension_policy_next(policy: ExtensionPolicy) -> ExtensionPolicy {
    match policy {
        ExtensionPolicy::None => ExtensionPolicy::CopyExtensions,
        ExtensionPolicy::CopyExtensions => ExtensionPolicy::CopyExtensionsAndSettings,
        ExtensionPolicy::CopyExtensionsAndSettings => ExtensionPolicy::None,
    }
}

/// Cycles backward through extension policies.
pub fn cycle_extension_policy_prev(policy: ExtensionPolicy) -> ExtensionPolicy {
    match policy {
        ExtensionPolicy::None => ExtensionPolicy::CopyExtensionsAndSettings,
        ExtensionPolicy::CopyExtensions => ExtensionPolicy::None,
        ExtensionPolicy::CopyExtensionsAndSettings => ExtensionPolicy::CopyExtensions,
    }
}

impl ProfileForm {
    /// Builds a new profile form initialized with defaults.
    pub fn new_create(user_data_root: PathBuf, profiles: &[BrowserProfile]) -> Self {
        let mut template_options = vec![TemplateOption {
            display_name: "None".to_string(),
            directory: None,
        }];
        for p in profiles {
            template_options.push(TemplateOption {
                display_name: p.display_name.clone(),
                directory: Some(p.directory.clone()),
            });
        }

        Self {
            name: String::new(),
            directory: String::new(),
            directory_manually_edited: false,
            template_index: 0,
            template_options,
            extension_policy: ExtensionPolicy::None,
            avatar_path: String::new(),
            open_after_create: false,
            focused_field: FormField::Name,
            user_data_root,
            is_clone: false,
            focused_button: 0,
        }
    }

    /// Builds a profile form prefilled for cloning an existing profile.
    pub fn new_clone(
        user_data_root: PathBuf,
        profiles: &[BrowserProfile],
        source: &BrowserProfile,
    ) -> Self {
        let mut form = Self::new_create(user_data_root, profiles);
        form.is_clone = true;

        if let Some(pos) = form
            .template_options
            .iter()
            .position(|opt| opt.directory.as_deref() == Some(&source.directory))
        {
            form.template_index = pos;
        }

        form
    }

    pub fn next_field(&mut self) {
        self.focused_field = self.focused_field.next();
    }

    pub fn prev_field(&mut self) {
        self.focused_field = self.focused_field.prev();
    }

    pub fn cycle_template_next(&mut self) {
        if !self.template_options.is_empty() {
            self.template_index = (self.template_index + 1) % self.template_options.len();
        }
    }

    pub fn cycle_template_prev(&mut self) {
        if !self.template_options.is_empty() {
            if self.template_index == 0 {
                self.template_index = self.template_options.len().saturating_sub(1);
            } else {
                self.template_index = self.template_index.saturating_sub(1);
            }
        }
    }

    pub fn cycle_extensions_next(&mut self) {
        self.extension_policy = cycle_extension_policy_next(self.extension_policy);
    }

    pub fn cycle_extensions_prev(&mut self) {
        self.extension_policy = cycle_extension_policy_prev(self.extension_policy);
    }

    pub fn handle_name_char(&mut self, c: char) {
        self.name.push(c);
        if !self.directory_manually_edited {
            self.directory = sanitize_directory_name(&self.name).unwrap_or_default();
        }
    }

    pub fn handle_name_backspace(&mut self) {
        self.name.pop();
        if !self.directory_manually_edited {
            self.directory = sanitize_directory_name(&self.name).unwrap_or_default();
        }
    }

    pub fn handle_directory_char(&mut self, c: char) {
        self.directory_manually_edited = true;
        self.directory.push(c);
    }

    pub fn handle_directory_backspace(&mut self) {
        self.directory_manually_edited = true;
        self.directory.pop();
    }

    pub fn handle_avatar_char(&mut self, c: char) {
        self.avatar_path.push(c);
    }

    pub fn handle_avatar_backspace(&mut self) {
        self.avatar_path.pop();
    }

    pub fn toggle_open_after_create(&mut self) {
        self.open_after_create = !self.open_after_create;
    }

    /// Computes destination directory path for live user feedback.
    pub fn resulting_path(&self) -> PathBuf {
        let dir = self.directory.trim();
        if dir.is_empty() {
            self.user_data_root.clone()
        } else {
            self.user_data_root.join(dir)
        }
    }

    /// Transforms form inputs into a profile creation specification.
    pub fn to_create_spec(&self) -> CreateProfileSpec {
        let dir = if self.directory.trim().is_empty() {
            None
        } else {
            Some(self.directory.trim().to_string())
        };
        let template_dir = self
            .template_options
            .get(self.template_index)
            .and_then(|t| t.directory.clone());
        let avatar_source = if self.avatar_path.trim().is_empty() {
            None
        } else {
            Some(expand_tilde(self.avatar_path.trim()))
        };
        CreateProfileSpec {
            display_name: self.name.clone(),
            directory: dir,
            template_directory: template_dir,
            avatar_source,
            clone_policy: ClonePolicy {
                copy_preferences: true,
                copy_bookmarks: false,
                extensions: self.extension_policy,
            },
            open_after_create: self.open_after_create,
        }
    }

    /// Transforms form inputs into a profile clone specification.
    pub fn to_clone_spec(&self) -> CloneProfileSpec {
        let dir = if self.directory.trim().is_empty() {
            None
        } else {
            Some(self.directory.trim().to_string())
        };
        let avatar_source = if self.avatar_path.trim().is_empty() {
            None
        } else {
            Some(expand_tilde(self.avatar_path.trim()))
        };
        CloneProfileSpec {
            display_name: self.name.clone(),
            directory: dir,
            avatar_source,
            clone_policy: ClonePolicy {
                copy_preferences: true,
                copy_bookmarks: false,
                extensions: self.extension_policy,
            },
            open_after_create: self.open_after_create,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_field_follows_name_until_manually_edited() {
        let mut form = ProfileForm::new_create(PathBuf::from("/tmp"), &[]);
        assert!(!form.directory_manually_edited);
        assert_eq!(form.directory, "");

        for c in "Personal".chars() {
            form.handle_name_char(c);
        }
        assert_eq!(form.name, "Personal");
        assert_eq!(form.directory, "Personal");

        // Manually edit the directory field
        form.handle_directory_char('-');
        form.handle_directory_char('X');
        assert!(form.directory_manually_edited);
        assert_eq!(form.directory, "Personal-X");

        // Further name edits must not overwrite manual directory edits
        form.handle_name_char('2');
        assert_eq!(form.name, "Personal2");
        assert_eq!(form.directory, "Personal-X");
    }

    #[test]
    fn test_sanitize_directory_name_integration() {
        let mut form = ProfileForm::new_create(PathBuf::from("/tmp"), &[]);
        for c in "NIGHTCLUB & LOUNGE".chars() {
            form.handle_name_char(c);
        }
        assert_eq!(form.name, "NIGHTCLUB & LOUNGE");
        assert_eq!(form.directory, "NIGHTCLUB-LOUNGE");
    }

    #[test]
    fn test_cycling_extensions_selector_wraps_and_starts_at_none() {
        let mut form = ProfileForm::new_create(PathBuf::from("/tmp"), &[]);
        assert_eq!(form.extension_policy, ExtensionPolicy::None);
        assert_eq!(form.extension_policy.label(), "Do not copy");

        form.cycle_extensions_next();
        assert_eq!(form.extension_policy, ExtensionPolicy::CopyExtensions);

        form.cycle_extensions_next();
        assert_eq!(
            form.extension_policy,
            ExtensionPolicy::CopyExtensionsAndSettings
        );

        // Wrap around forward
        form.cycle_extensions_next();
        assert_eq!(form.extension_policy, ExtensionPolicy::None);

        // Wrap around backward
        form.cycle_extensions_prev();
        assert_eq!(
            form.extension_policy,
            ExtensionPolicy::CopyExtensionsAndSettings
        );
    }
}
