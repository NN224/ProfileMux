use crate::domain::StorageBreakdown;
use std::path::Path;

/// Placeholder: stat-only recursive measurement of a profile directory.
pub fn measure_profile(_profile_path: &Path, _cache_path: Option<&Path>) -> StorageBreakdown {
    StorageBreakdown::default()
}
