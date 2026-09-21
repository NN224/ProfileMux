# Safety Model & Shipped Behaviour

ProfileMux treats browser profiles as critical user assets containing irreplaceable data. This document details the shipped safety model, transaction engine, filesystem guards, process state validation, and privacy invariants implemented in ProfileMux 1.0.0.

## Transactional Architecture and Rollback

Every structural write to disk executes inside a `Transaction` (`src/fs/transaction.rs`). A transaction records every mutating step on an in-memory undo stack and provides deterministic rollback if an operation fails or is interrupted.

### Inverse Action Log
When an operation begins, ProfileMux establishes a temporary backup folder in the operating system temp directory (`/tmp/pmux-<label>-<pid>-<nanos>-<count>`). As filesystem modifications occur, the transaction pushes corresponding `UndoStep` records:

- **Path creation** (`create_dir`, `copy_file`, `copy_dir`): Pushes `UndoStep::RemovePath(path)`. Rollback removes the newly created file or directory tree.
- **File modification** (`write_file`, `backup_file`): Before writing, copies the existing file to the backup directory and pushes `UndoStep::RestoreFile { original, backup }`. Rollback restores the pre-mutation content.
- **Renaming** (`rename`): Pushes `UndoStep::RenameBack { from, to }`. Rollback moves the renamed item back to its original location.

### `Drop` Safety Net
A transaction must be explicitly completed by calling `tx.commit()`. If execution halts prematurely—due to an error returned via the `?` operator, an unhandled error condition, or a panic—the `Drop` implementation automatically executes the rollback:

```rust
impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.finished {
            self.finished = true;
            let _ = self.apply_rollback();
        }
    }
}
```

On rollback, undo actions execute in reverse order of original execution, and the temporary backup directory is purged.

## Metadata Integrity (`Local State`)

Chromium profile registration and configuration are stored in `<user data root>/Local State`.

- **Pre-mutation Backup**: Before `Local State` is modified or overwritten, `Transaction::backup_file` copies the original file into the transaction backup area.
- **Preservation of Unknown Keys**: `Local State` is parsed into a `serde_json::Value` document. ProfileMux modifies only the keys relevant to the target profile (`profile.info_cache.<dir>` and `profile.profiles_order`). All other vendor keys, feature flags, and unknown fields are preserved untouched.
- **Synchronized Registration**: Profile creation, cloning, renaming, and deletion update both `info_cache` and `profiles_order` simultaneously within the transaction.

## Process State: Running Browser Detection and Graceful Quit

Modifying a profile while its browser process is running causes file-lock collisions, database write conflicts, and SQLite WAL corruption.

### Per-User-Data-Root Detection
Chromium allows running multiple browser instances if each uses a distinct `--user-data-dir`. Matching by process executable name alone would incorrectly block mutations on an idle root.

ProfileMux checks running state per user data root (`src/browsers/chromium/launch.rs`):
1. It queries active processes via `/bin/ps -Ao args=`.
2. It verifies whether an active process for that browser executable was launched with `--user-data-dir=<path>`.
3. It inspects `<user data root>/SingletonLock`.

### Stale Lock Recovery
When Chromium crashes or is killed by the operating system, it leaves behind a dangling `SingletonLock` symlink pointing to `<hostname>-<pid>`. ProfileMux extracts the PID from the link target and verifies whether that process is currently alive using `/bin/ps -p <pid> -o pid=`. If the PID is dead, the lock is recognized as stale and does not block mutation.

### Graceful Quit via AppleScript
ProfileMux **never force-terminates** browser processes (`SIGKILL` or `SIGTERM` are never issued).
- If an operation requires the browser to be closed and an instance is running, the CLI halts with an actionable error:
  ```text
  Brave Browser is currently running. Quit the browser or pass --close-browser to continue.
  ```
- If `--close-browser` is passed in the CLI, or if the user selects "Quit Browser" in the TUI dialog, ProfileMux sends an AppleScript command to the browser's bundle identifier:
  ```applescript
  tell application id "<bundle_id>" to quit
  ```
- ProfileMux then polls `is_running()` every 200–250 ms for up to 15 seconds until the browser cleanly closes its files.

## Deletion: OS Trash Only, Never Recursive `rm -rf`

Profile deletion can result in catastrophic data loss if an incorrect path is targeted.

- **Rule**: ProfileMux **never** invokes unrecoverable directory deletion (`std::fs::remove_dir_all` or `rm -rf`) on profile data.
- **`TrashBin` Implementation**: Deleted profiles and external cache folders are moved to `~/.Trash` via `SystemTrash` (`src/fs/trash.rs`).
- **Collision Handling**: If a folder with the same name exists in Trash, ProfileMux increments a counter (`Profile 1 2`, `Profile 1 3`) rather than overwriting.
- **Filesystem Boundaries**: If the profile resides on an external or separate volume, a safe copy-and-remove fallback moves the directory across the device boundary (`EXDEV`).
- **Restoration**: Deleted profiles can be recovered directly from the macOS Trash.

## Directory Name Sanitization

To prevent directory traversal and filesystem errors, directory names are sanitized deterministically (`src/domain/sanitize.rs`):

1. **Deterministic Character Mapping**: Non-alphanumeric characters (excluding `_` and `.`) are converted to hyphens. For example, `NIGHTCLUB & LOUNGE` becomes `NIGHTCLUB-LOUNGE`.
2. **Length Limit**: Directory names are capped at 48 characters.
3. **Traversal Prevention**: Names cannot contain slashes (`/` or `\`), null bytes, leading dots (`.`), or `..`.
4. **Reserved Names**: ProfileMux rejects reserved Chromium directory names:
   - `.`
   - `..`
   - `System Profile`
   - `Guest Profile`
   - `Crashpad`
   - `Local State`
   - `component_crx_cache`
   - `extensions_crx_cache`
5. **Root Containment**: Target directories are verified to ensure `path.parent() == Some(user_data_root)`. Any path escaping the user data root is rejected.
6. **Collision Avoidance**: If a sanitized name collides with an existing profile directory, ProfileMux appends incremental numeric suffixes (`-2`, `-3`).

The human-visible display name preserves full Unicode characters, spaces, and emojis and is never modified by directory sanitization.

## Template Clones & Privacy Exclusion Boundaries

When cloning a profile or creating a profile from a template, ProfileMux strictly isolates private user data:

- **Always Excluded**:
  - Session databases (`Cookies`)
  - Credential databases (`Login Data`, `Login Data For Account`)
  - Browsing histories (`History`, `History-journal`)
  - Open tabs and windows (`Sessions`)
  - Web database stores (`Web Data`)
  - Network state cache (`Network Action Predictor`, `Network Persistent State`)
  - Google Account authentication tokens (`account_info`, `gaia_cookie`)
  - HTML5 storage (`Local Storage`, `IndexedDB`, `Service Worker`)
- **Preference Sanitization**:
  - When `copy_preferences` is enabled, sensitive keys are stripped from `Preferences` before writing to the new profile (`account_info`, `gaia_cookie`, `signin`, `sync`, `google.services`, `password_manager`, `autofill`, etc.).
- **Extension Signing Warning**:
  - Chromium signs extension entries in `Secure Preferences` with a per-profile MAC. ProfileMux does not forge or calculate these MACs. If extension copying is requested (`copy` or `copy-settings`), the extension files are copied, but Chromium may drop them on next launch. This behavior is documented as experimental.

## Cache Cleanup Boundaries

The `pmux cache clean` command and TUI cache clean action remove only verified temporary cache directories:

- `Cache`
- `Code Cache`
- `GPUCache`
- `ShaderCache`
- `GrShaderCache`
- `DawnCache`
- `DawnGraphiteCache`
- `DawnWebGPUCache`
- `component_crx_cache`
- `Service Worker/CacheStorage`
- External macOS cache: `~/Library/Caches/<browser-vendor>/<channel>/<profile-dir>`

### Untouched Locations
Cache cleanup **never** removes or alters:
- `Cookies`
- `Login Data`
- `History`
- `Bookmarks`
- `Preferences`
- `Secure Preferences`
- `Extensions`
- `Local Storage`
- `Sessions`
- `IndexedDB`
- `Web Data`
- Custom profile pictures (`Google Profile Picture.png`)

## Dry-Run Verification

The structural operations `create`, `clone`, `rename`, `delete` and `cache clean` support `--dry-run`. `profile avatar` has no `--dry-run` flag; it is still gated by the browser-running preflight and still writes inside a transaction.
- The adapter computes the entire `OperationPlan`, including affected paths, steps, exclusions, and estimated reclaimed bytes.
- The plan is rendered to stdout without modifying disk or terminating processes.

## Out of Scope

The following items are intentionally not implemented and out of scope for 1.0.0:
- **Permanent deletion**: All deletions move to `~/.Trash`. Unrecoverable deletion is not provided.
- **Non-Chromium browsers**: There is no adapter for Firefox or Safari; neither is implemented.
- **Windows and Linux platforms**: ProfileMux 1.0.0 is macOS-specific.
- **Configuration files & plugin architecture**: No external configuration file is read or written.
- **Standalone audit log**: Audit logging to a dedicated file (`~/.config/profilemux/audit.log`) is not implemented in 1.0.0.
