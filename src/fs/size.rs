use std::path::{Path, PathBuf};

use crate::domain::StorageBreakdown;

const MAX_DEPTH: usize = 64;

/// Iteratively calculates total size of files under `path` up to MAX_DEPTH depth.
/// Never follows symlinks and skips any entries that fail to stat.
pub fn dir_size(path: &Path) -> u64 {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return 0,
    };
    if !meta.is_dir() {
        return if !meta.is_symlink() { meta.len() } else { 0 };
    }

    let mut total: u64 = 0;
    let mut stack: Vec<(PathBuf, usize)> = vec![(path.to_path_buf(), 0)];

    while let Some((current, depth)) = stack.pop() {
        if depth >= MAX_DEPTH {
            continue;
        }
        let read_dir = match std::fs::read_dir(&current) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in read_dir.flatten() {
            let entry_path = entry.path();
            let entry_meta = match std::fs::symlink_metadata(&entry_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if entry_meta.is_dir() {
                stack.push((entry_path, depth + 1));
            } else if !entry_meta.is_symlink() {
                total = total.saturating_add(entry_meta.len());
            }
        }
    }
    total
}

/// Measures profile storage breakdown according to Chromium directory layout.
pub fn measure_profile(profile_path: &Path, cache_path: Option<&Path>) -> StorageBreakdown {
    let profile_total = dir_size(profile_path);
    let profile_cache = dir_size(&profile_path.join("Cache"));
    let profile_code_cache = dir_size(&profile_path.join("Code Cache"));
    let profile_gpu_cache = dir_size(&profile_path.join("GPUCache"));

    let in_profile_special = profile_cache
        .saturating_add(profile_code_cache)
        .saturating_add(profile_gpu_cache);
    let core = profile_total.saturating_sub(in_profile_special);

    let (ext_cache, ext_code_cache, ext_gpu_cache) = match cache_path {
        Some(cp) => (
            dir_size(&cp.join("Cache")),
            dir_size(&cp.join("Code Cache")),
            dir_size(&cp.join("GPUCache")),
        ),
        None => (0, 0, 0),
    };

    let cache = profile_cache.saturating_add(ext_cache);
    let code_cache = profile_code_cache.saturating_add(ext_code_cache);
    let gpu_cache = profile_gpu_cache.saturating_add(ext_gpu_cache);
    let total = core
        .saturating_add(cache)
        .saturating_add(code_cache)
        .saturating_add(gpu_cache);

    StorageBreakdown {
        core,
        cache,
        code_cache,
        gpu_cache,
        total,
    }
}

/// Formats byte counts into human-readable strings (e.g., 1.42 GB, 880 MB, 512 KB, 0 B).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        let val = bytes as f64 / GB as f64;
        format!("{val:.2} GB")
    } else if bytes >= MB {
        let val = bytes as f64 / MB as f64;
        let formatted = format!("{val:.1}");
        let display = formatted.strip_suffix(".0").unwrap_or(&formatted);
        format!("{display} MB")
    } else if bytes >= KB {
        let val = bytes / KB;
        format!("{val} KB")
    } else {
        format!("{bytes} B")
    }
}
