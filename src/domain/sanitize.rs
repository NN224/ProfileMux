/// Maximum length of a generated profile directory name.
const MAX_LEN: usize = 48;

/// Turns a display name into a deterministic, filesystem-safe directory name.
///
/// `NIGHTCLUB & LOUNGE` becomes `NIGHTCLUB-LOUNGE`. The display name itself is
/// never modified. Returns `None` when nothing usable survives sanitization.
pub fn sanitize_directory_name(display_name: &str) -> Option<String> {
    let mapped: String = display_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();

    let collapsed = mapped
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    let trimmed: String = collapsed
        .trim_matches(|c| c == '.' || c == '-')
        .chars()
        .take(MAX_LEN)
        .collect();
    let trimmed = trimmed.trim_end_matches(['-', '.']).to_string();

    if trimmed.is_empty() || is_reserved(&trimmed) {
        None
    } else {
        Some(trimmed)
    }
}

/// Directory names a profile may never take, because Chromium reserves them.
pub fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        "." | ".."
            | "System Profile"
            | "Guest Profile"
            | "Crashpad"
            | "Local State"
            | "component_crx_cache"
            | "extensions_crx_cache"
    )
}

/// True when `name` is a single path component that cannot escape its parent.
pub fn is_safe_component(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && !is_reserved(name)
}

/// Appends `-2`, `-3`, ... until the name does not collide with `taken`.
pub fn deduplicate(base: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken.iter().any(|t| t == candidate))
        .unwrap_or_else(|| base.to_string())
}
