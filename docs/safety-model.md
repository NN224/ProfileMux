# Safety Model & Mutation Contract

> **IMPORTANT**: ProfileMux v0.1 is strictly **READ-ONLY**. The safety mechanisms, operation lifecycle, rollback strategies, and mutation rules documented here represent the **design contract** for the upcoming mutation engine (planned for v0.2+). None of these mutation operations are shipped or active in v0.1.

ProfileMux treats browser profiles as critical user assets containing non-recoverable data (session states, local databases, extension storage). This document outlines the mandatory safety invariant that every future mutation feature must follow.

---

## The 8-Stage Operation Lifecycle

All profile mutations (creation, renaming, cloning, deletion, cache pruning) must execute through an 8-stage transactional lifecycle:

```text
┌─────────┐     ┌───────────┐     ┌────────────┐     ┌─────────┐
│ 1.      │────▶│ 2.        │────▶│ 3.         │────▶│ 4.      │
│ Inspect │     │ Preflight │     │ Build Plan │     │ Confirm │
└─────────┘     └───────────┘     └────────────┘     └────┬────┘
                                                          │
┌─────────┐     ┌───────────┐     ┌────────────┐     ┌────┴────┐
│ 8.      │◀────│ 7.        │◀────│ 6.         │◀────│ 5.      │
│ Commit  │     │ Validate  │     │ Execute    │     │ Backup  │
└─────────┘     └───────────┘     └────────────┘     └─────────┘
                      │                 │
                      ▼                 ▼
             ┌──────────────────────────────────┐
             │       Failure -> Rollback        │
             └──────────────────────────────────┘
```

1. **Inspect**:
   Take an immutable `ProfileStoreSnapshot` of the browser installation. Verify that the target profile and its parent directories exist in a known, parseable state.
2. **Preflight**:
   Validate environment preconditions:
   - Verify the browser is **not running** (`is_running() == false`).
   - Check filesystem permissions and ensure sufficient disk space is available for operations like cloning.
   - Verify that target paths do not collide with existing files or directories.
3. **Build Plan**:
   Generate an ordered sequence of discrete, reversible filesystem and metadata actions (e.g. `CopyDirectory(src, dst)`, `UpdateMetadataKey(file, key, val)`).
4. **Confirm**:
   Present the plan to the user. In CLI mode, prompt for confirmation unless an explicit `--yes` flag is passed; in TUI mode, display a plan review dialog. Support `--dry-run` to print the plan and exit without executing.
5. **Metadata Backup**:
   Before modifying any file on disk, create a timestamped backup of the browser's registry files (e.g. `Local State.pmux-backup.<timestamp>`).
6. **Execute**:
   Execute the planned actions in sequence. If any step returns an I/O or serialization error, immediately abort forward execution and enter the Rollback sequence.
7. **Validate**:
   Perform post-execution sanity checks: re-read the configuration file, ensure JSON/INI documents parse cleanly, and verify that target paths exist with expected permissions. If validation fails, initiate Rollback.
8. **Commit**:
   Remove temporary scratch files and prune expired metadata backups. Record the completed action in the local audit log.

### Rollback on Failure

If execution fails or post-validation detects corruption:
- Revert all filesystem changes in reverse order of execution (e.g. removing created destination directories, moving back renamed folders).
- Restore the original metadata files from the backup created in Stage 5.
- Leave the user's browser installation in its original, functional state.
- Surface the exact failure reason and the restoration status to the user.

---

## Process State: Never Kill the Browser

Structural mutations (renaming directories, editing `Local State`, copying SQLite databases) while a browser is active risk file lock contention, write collisions, and SQLite database corruption (e.g. invalidating WAL files).

- **Rule**: ProfileMux **never** issues `SIGKILL`, `SIGTERM`, or platform process-termination calls against a running browser.
- **Enforcement**: If `adapter.is_running()` evaluates to `true`, ProfileMux halts the operation during Preflight and prompts the user to quit the browser normally. Mutation proceeds only when the browser process has cleanly exited and unlocked its files.

---

## Deletion: OS Trash Only, Never `rm -rf`

Accidental profile deletion can lead to permanent data loss.

- **Rule**: ProfileMux **never** invokes unrecoverable recursive directory removal (`rm -rf` or `std::fs::remove_dir_all`) when deleting a user profile.
- **Implementation**: Deletion operations will use platform-native trash APIs (e.g. moving the profile folder to `~/.Trash` on macOS).
- **Recovery**: If a profile is deleted by mistake, the user can recover the directory directly from the operating system's Trash.

---

## Directory-Name Sanitization

When creating or renaming profile directories on disk, the directory name must adhere to strict sanitization rules:

1. **Deterministic**: Given the same input name, the sanitization function always produces the identical directory slug.
2. **Filesystem-Safe**: Restrict output characters to ASCII alphanumerics, hyphens, and underscores (`[a-zA-Z0-9_-]`).
3. **No Slashes or Traversal**: Reject or strip `/`, `\`, `.`, and `..` to prevent directory traversal attacks.
4. **Normalized Whitespace**: Trim leading and trailing whitespace; collapse interior spaces and invalid characters into single hyphens.
5. **Collision-Avoiding**: If the generated directory name matches an existing profile folder, append an incremental numeric suffix (e.g. `Profile-Work-1`).
6. **Display Name Untouched**: The human-visible display name (stored in metadata) preserves full Unicode characters, spaces, and emojis. Only the physical on-disk directory name is sanitized.
7. **Preview Before Creation**: The sanitized directory name is always shown in the execution plan before confirmation.
8. **Data Root Containment**: Every target path is checked against `user_data_root.canonicalize()`. Any path that escapes the user data directory is rejected immediately.

---

## Dry-Run Mode

Every mutation command in the CLI will require support for a `--dry-run` flag:
- Evaluates Inspect, Preflight, and Build Plan stages without executing any I/O.
- Outputs the complete list of filesystem operations, source/destination paths, and metadata diffs.
- Reports whether the browser is running or if disk space is insufficient.

---

## Audit Log and Privacy Boundary

### Audit Log Specification

When mutation is introduced, ProfileMux will maintain a local, append-only audit log located at `~/.config/profilemux/audit.log` (or OS equivalent).

The audit log records:
- ISO 8601 timestamp.
- Executed command and flags.
- Browser installation identifier (`BrowserInstallId`).
- Affected profile directories and filesystem paths.
- Success, failure, or rollback status.

### Privacy Boundary

ProfileMux enforces an absolute privacy boundary:
- **Never Logged**: User browsing history, opened URLs, cookies, passwords, authentication tokens, search queries, or form autofill values.
- **Never Read**: ProfileMux does not open or inspect SQLite database contents (`History`, `Cookies`, `Login Data`, `Web Data`).
- **Local Only**: Audit logs remain exclusively on the user's local disk. ProfileMux contains no network clients and sends no telemetry.
