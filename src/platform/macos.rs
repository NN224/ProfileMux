use std::path::{Path, PathBuf};

/// Metadata extracted from a macOS application bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBundle {
    pub path: PathBuf,
    pub bundle_id: String,
    pub version: Option<String>,
    pub executable: Option<String>,
}

/// Reads and extracts bundle metadata from an application's Info.plist.
/// Returns `None` if Info.plist cannot be read, is not a dictionary, or lacks `CFBundleIdentifier`.
pub fn scan_bundle(path: &Path) -> Option<AppBundle> {
    let plist_path = path.join("Contents").join("Info.plist");
    let val = plist::Value::from_file(&plist_path).ok()?;
    let dict = val.as_dictionary()?;

    let bundle_id = dict
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .filter(|s| !s.trim().is_empty())?
        .to_string();

    let version = dict
        .get("CFBundleShortVersionString")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let executable = dict
        .get("CFBundleExecutable")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    Some(AppBundle {
        path: path.to_path_buf(),
        bundle_id,
        version,
        executable,
    })
}

/// Scans directory entries one level deep for `.app` bundles.
fn scan_dir(dir: &Path) -> Vec<AppBundle> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
        })
        .filter_map(|path| scan_bundle(&path))
        .collect()
}

/// Enumerates `*.app` entries directly inside `/Applications`, `$HOME/Applications`,
/// and `/Applications/Utilities` if present.
pub fn scan_bundles() -> Vec<AppBundle> {
    let search_roots = [
        Some(PathBuf::from("/Applications")),
        Some(PathBuf::from("/Applications/Utilities")).filter(|p| p.is_dir()),
        dirs::home_dir()
            .map(|h| h.join("Applications"))
            .filter(|p| p.is_dir()),
    ];

    search_roots
        .into_iter()
        .flatten()
        .flat_map(|dir| scan_dir(&dir))
        .collect()
}

/// Returns the standard Application Support directory (`$HOME/Library/Application Support`).
pub fn app_support_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("Library").join("Application Support"))
}

/// Returns the standard user caches directory (`$HOME/Library/Caches`).
pub fn caches_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("Library").join("Caches"))
}

/// Reveals the given path in Finder by executing `open -R <path>`.
pub fn reveal_in_finder(path: &Path) -> std::io::Result<()> {
    let status = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "open -R failed with status: {status}"
        )))
    }
}
