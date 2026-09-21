use std::path::{Path, PathBuf};

use crate::domain::{ClonePolicy, ExtensionPolicy};
use crate::error::{Error, Result};

/// A single item to be copied during a profile clone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyItem {
    pub relative: PathBuf,
    pub is_dir: bool,
}

/// Top-level and dotted `Preferences` keys that must be stripped from a cloned profile.
pub const SENSITIVE_PREFERENCE_KEYS: &[&str] = &[
    "account_info",
    "gaia_cookie",
    "signin",
    "sync",
    "google.services",
    "password_manager",
    "autofill",
    "credentials_enable_service",
    "profile.content_settings.exceptions",
    "profile.password_manager_enabled",
    "sessions",
    "safebrowsing",
    "extensions.settings",
    "extensions.install_signature",
    "ntp.custom_background_dict",
    "media_router",
    "dns_prefetching",
    "browser.last_known_google_url",
    "protection",
];

/// Inspects which allowed profile entries exist in `source_dir` according to `policy`,
/// returning them in deterministic order.
///
/// Extension copying is experimental: Chromium registers extensions in `Secure Preferences`
/// with a per-profile MAC (hash), so extension entries copied into a different profile are
/// frequently dropped by the browser on next start. We do not attempt to forge or
/// recompute those MACs.
pub fn plan_copy_items(source_dir: &Path, policy: &ClonePolicy) -> Vec<CopyItem> {
    let mut candidates = Vec::new();
    if policy.copy_preferences {
        candidates.push((PathBuf::from("Preferences"), false));
    }
    if policy.copy_bookmarks {
        candidates.push((PathBuf::from("Bookmarks"), false));
    }
    match policy.extensions {
        ExtensionPolicy::None => {}
        ExtensionPolicy::CopyExtensions => {
            candidates.push((PathBuf::from("Extensions"), true));
        }
        ExtensionPolicy::CopyExtensionsAndSettings => {
            candidates.push((PathBuf::from("Extensions"), true));
            candidates.push((PathBuf::from("Local Extension Settings"), true));
            candidates.push((PathBuf::from("Sync Extension Settings"), true));
        }
    }

    candidates
        .into_iter()
        .filter(|(rel, _)| source_dir.join(rel).exists())
        .map(|(relative, is_dir)| CopyItem { relative, is_dir })
        .collect()
}

/// Human-readable list of profile items copied according to `policy`.
pub fn copied_labels(policy: &ClonePolicy) -> Vec<String> {
    let mut labels = Vec::new();
    if policy.copy_preferences {
        labels.push("Preferences".to_string());
    }
    if policy.copy_bookmarks {
        labels.push("Bookmarks".to_string());
    }
    match policy.extensions {
        ExtensionPolicy::None => {}
        ExtensionPolicy::CopyExtensions => {
            labels.push("Extensions".to_string());
        }
        ExtensionPolicy::CopyExtensionsAndSettings => {
            labels.push("Extensions and settings".to_string());
        }
    }
    labels
}

/// Human-readable list of profile items deliberately excluded from copying.
pub fn excluded_labels(policy: &ClonePolicy) -> Vec<String> {
    let mut labels = Vec::new();
    if !policy.copy_preferences {
        labels.push("Preferences".to_string());
    }
    if !policy.copy_bookmarks {
        labels.push("Bookmarks".to_string());
    }
    match policy.extensions {
        ExtensionPolicy::None => {
            labels.push("Extensions".to_string());
        }
        ExtensionPolicy::CopyExtensions => {
            labels.push("Extension settings".to_string());
        }
        ExtensionPolicy::CopyExtensionsAndSettings => {}
    }
    labels.push("Cookies".to_string());
    labels.push("Login Data".to_string());
    labels.push("History".to_string());
    labels.push("Sessions".to_string());
    labels.push("Web Data".to_string());
    labels.push("Network state".to_string());
    labels.push("Account identity (GAIA)".to_string());
    labels.push("Local Storage".to_string());
    labels.push("Service Worker".to_string());
    labels.push("Top Sites".to_string());
    labels
}

/// Returns a new JSON document with every sensitive key in `SENSITIVE_PREFERENCE_KEYS`
/// removed, walking nested objects for dotted paths. Never mutates `source`.
///
/// If `source` is not a JSON object, an empty object is returned.
pub fn sanitize_preferences(source: &serde_json::Value) -> serde_json::Value {
    let Some(map) = source.as_object() else {
        return serde_json::Value::Object(serde_json::Map::new());
    };
    let mut sanitized = serde_json::Value::Object(map.clone());
    for key in SENSITIVE_PREFERENCE_KEYS {
        remove_path(&mut sanitized, key);
    }
    sanitized
}

fn remove_path(target: &mut serde_json::Value, path: &str) {
    if let Some(obj) = target.as_object_mut() {
        obj.remove(path);
    }
    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() > 1 {
        remove_nested_path(target, &parts);
    }
}

fn remove_nested_path(target: &mut serde_json::Value, parts: &[&str]) {
    if parts.is_empty() {
        return;
    }
    if let Some(obj) = target.as_object_mut() {
        if parts.len() == 1 {
            obj.remove(parts[0]);
        } else if let Some(child) = obj.get_mut(parts[0]) {
            remove_nested_path(child, &parts[1..]);
        }
    }
}

/// Reads the source `Preferences` file, strips sensitive keys, and returns
/// formatted JSON bytes.
pub fn load_and_sanitize(source_prefs_path: &Path) -> Result<Vec<u8>> {
    let content = match std::fs::read_to_string(source_prefs_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::NotFound(source_prefs_path.to_path_buf()));
        }
        Err(e) => return Err(Error::io(source_prefs_path, e)),
    };

    let raw: serde_json::Value = serde_json::from_str(&content).map_err(|e| Error::Malformed {
        path: source_prefs_path.to_path_buf(),
        message: e.to_string(),
    })?;

    if !raw.is_object() {
        return Err(Error::Malformed {
            path: source_prefs_path.to_path_buf(),
            message: "expected JSON object at root of Preferences".to_string(),
        });
    }

    let sanitized = sanitize_preferences(&raw);
    serde_json::to_vec_pretty(&sanitized).map_err(|e| Error::Other(e.to_string()))
}
