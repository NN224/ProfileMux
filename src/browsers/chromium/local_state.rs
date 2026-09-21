use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Typed representation of an entry in `profile.info_cache`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProfileEntry {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_icon: Option<String>,
    #[serde(default)]
    pub gaia_picture_file_name: Option<String>,
    #[serde(default)]
    pub use_gaia_picture: Option<bool>,
    #[serde(default)]
    pub is_using_default_avatar: Option<bool>,
    #[serde(default)]
    pub is_using_default_name: Option<bool>,
    #[serde(default)]
    pub active_time: Option<f64>,
    #[serde(default)]
    pub user_name: Option<String>,
}

/// Parsed Chromium `Local State` metadata file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalState {
    pub raw: serde_json::Value,
    pub entries: BTreeMap<String, ProfileEntry>,
    pub last_used: Option<String>,
    pub profiles_order: Vec<String>,
}

/// Loads and parses the `Local State` file at the given path.
pub fn load(path: &Path) -> Result<LocalState> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::NotFound(path.to_path_buf()));
        }
        Err(e) => return Err(Error::io(path, e)),
    };

    let raw: serde_json::Value = serde_json::from_str(&content).map_err(|e| Error::Malformed {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;

    if !raw.is_object() {
        return Err(Error::Malformed {
            path: path.to_path_buf(),
            message: "expected JSON object at root of Local State".to_string(),
        });
    }

    let profile_obj = raw.get("profile").and_then(|p| p.as_object());

    let entries = profile_obj
        .and_then(|p| p.get("info_cache"))
        .and_then(|ic| ic.as_object())
        .map(extract_entries)
        .unwrap_or_default();

    let last_used = profile_obj
        .and_then(|p| p.get("last_used"))
        .and_then(|lu| lu.as_str())
        .map(|s| s.to_string());

    let profiles_order = profile_obj
        .and_then(|p| p.get("profiles_order"))
        .and_then(|po| po.as_array())
        .map(|v| extract_profiles_order(v.as_slice()))
        .unwrap_or_default();

    Ok(LocalState {
        raw,
        entries,
        last_used,
        profiles_order,
    })
}

fn extract_entries(
    info_cache: &serde_json::Map<String, serde_json::Value>,
) -> BTreeMap<String, ProfileEntry> {
    let mut map = BTreeMap::new();
    for (k, v) in info_cache {
        if let Ok(entry) = serde_json::from_value::<ProfileEntry>(v.clone()) {
            map.insert(k.clone(), entry);
        }
    }
    map
}

fn extract_profiles_order(order_arr: &[serde_json::Value]) -> Vec<String> {
    order_arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect()
}
