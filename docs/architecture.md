# ProfileMux Architecture

This document describes the architectural layering, mutation execution engine, transactional safety guarantees, identity model, trait boundaries, and diagnostic pipeline of ProfileMux (`pmux`).

## Layering

ProfileMux enforces strict unidirectional layering. Presentation layers (CLI and TUI) delegate entirely to the core library. No presentation component contains browser-specific filesystem logic, path heuristics, or file parsing routines.

```text
┌────────────────────────────────────────────────────────┐
│                   Entry Point (main)                   │
└───────────────────────────┬────────────────────────────┘
                            │
            ┌───────────────┴───────────────┐
            ▼                               ▼
┌───────────────────────┐       ┌───────────────────────┐
│       CLI Layer       │       │       TUI Layer       │
│      (clap-derive)    │       │       (ratatui)       │
└───────────┬───────────┘       └───────────┬───────────┘
            │                               │
            └───────────────┬───────────────┘
                            ▼
┌────────────────────────────────────────────────────────┐
│                     Core Library                       │
│  ┌──────────────────────────────────────────────────┐  │
│  │                   Domain Model                   │  │
│  │ (BrowserInstall, BrowserProfile, Capabilities,   │  │
│  │  OperationPlan, ClonePolicy, ExtensionPolicy)    │  │
│  └────────────────────────┬─────────────────────────┘  │
│                           │                            │
│  ┌────────────────────────▼─────────────────────────┐  │
│  │                 Browser Adapters                 │  │
│  │  (BrowserAdapter trait, plan_* / execute pairs,  │  │
│  │   ChromiumAdapter: mutation, clone, avatar, etc) │  │
│  └────────────────────────┬─────────────────────────┘  │
│                           │                            │
│  ┌────────────────────────▼─────────────────────────┐  │
│  │                   Doctor Engine                  │  │
│  │     (Pure snapshot analysis, health findings)    │  │
│  └────────────────────────┬─────────────────────────┘  │
│                           │                            │
│  ┌────────────────────────▼─────────────────────────┐  │
│  │                 Transaction Layer                │  │
│  │  (Transaction rollback, TrashBin, backup storage)│  │
│  └──────────────────────────────────────────────────┘  │
└───────────────────────────┬────────────────────────────┘
                            │
            ┌───────────────┴───────────────┐
            ▼                               ▼
┌───────────────────────┐       ┌───────────────────────┐
│     Platform Layer    │       │     Filesystem Layer  │
│ (macOS bundle lookup, │       │  (Disk size scanning, │
│  app launch, osascript│       │   path normalization, │
│  singleton lock check)│       │   sanitization)       │
└───────────────────────┘       └───────────────────────┘
```

### Component Responsibilities

1. **Entry Point (`src/main.rs`)**: Executable entry point delegating directly to `profilemux::cli::run()`.
2. **CLI Layer (`src/cli/`)**: Command parsing and execution using `clap`. Subcommands validate arguments, resolve selectors to concrete profiles, prompt for confirmation when interactive, and invoke library operations.
3. **TUI Layer (`src/tui/`)**: Fullscreen terminal interface built with `ratatui` and `crossterm`. Manages state across three panes (Browsers, Profiles, Details), renders modal dialogs for all mutations, and handles background disk-usage scans.
4. **Domain Layer (`src/domain/`)**: Browser-agnostic data models, identifiers, operation plans, cloning specs, and capabilities (`BrowserInstallId`, `ProfileId`, `BrowserInstall`, `BrowserProfile`, `BrowserCapabilities`, `OperationPlan`, `PlanStep`, `ClonePolicy`, `ExtensionPolicy`, `CreateProfileSpec`, `CloneProfileSpec`, `HealthFinding`).
5. **Adapters Layer (`src/browsers/`)**: Implementations of the `BrowserAdapter` trait handling browser-specific formats (Chromium `Local State`, `Preferences`), process lifecycle, and filesystem operations.
6. **Doctor Engine (`src/doctor/`)**: Pure functional health analyzer. Consumes an immutable `ProfileStoreSnapshot` and returns diagnostic findings without performing I/O.
7. **Filesystem & Transaction Layer (`src/fs/`)**: Rollback-capable `Transaction` engine, `TrashBin` abstraction for moving files to macOS Trash, path normalization, directory sanitization, and storage breakdown measurement.
8. **Platform Layer (`src/platform/`)**: Operating system abstractions for macOS application bundle discovery, executable resolution, process running detection via `SingletonLock` and `/bin/ps`, and AppleScript graceful quit execution.

## The `BrowserAdapter` Trait and Mutation Layer

Every browser family is integrated by implementing the `BrowserAdapter` trait (`src/browsers/mod.rs`).

### `plan_*` and Execute Method Pairing

Mutations are strictly structured as pairs:
- A `plan_*` method that constructs an `OperationPlan` describing every action without touching disk.
- An execute method that performs the actual mutation inside a rollback-capable transaction.

```rust
pub trait BrowserAdapter {
    fn install(&self) -> &BrowserInstall;
    fn capabilities(&self) -> BrowserCapabilities;
    fn snapshot(&self) -> Result<ProfileStoreSnapshot>;
    fn list_profiles(&self) -> Result<Vec<BrowserProfile>> {
        Ok(self.snapshot()?.registered)
    }
    fn doctor(&self) -> Result<Vec<HealthFinding>>;
    fn is_running(&self) -> bool;
    fn request_quit(&self) -> Result<()>;
    fn launch_profile(&self, profile: &BrowserProfile) -> Result<()>;

    // Paired mutation methods
    fn plan_create(&self, spec: &CreateProfileSpec) -> Result<OperationPlan>;
    fn create_profile(&self, spec: &CreateProfileSpec) -> Result<BrowserProfile>;

    fn plan_clone(&self, source: &BrowserProfile, spec: &CloneProfileSpec) -> Result<OperationPlan>;
    fn clone_profile(&self, source: &BrowserProfile, spec: &CloneProfileSpec) -> Result<BrowserProfile>;

    fn rename_display_name(&self, profile: &BrowserProfile, new_name: &str) -> Result<()>;

    fn plan_rename_directory(&self, profile: &BrowserProfile, new_dir: &str) -> Result<OperationPlan>;
    fn rename_profile_directory(&self, profile: &BrowserProfile, new_dir: &str) -> Result<()>;

    fn plan_set_avatar(&self, profile: &BrowserProfile, image: &Path) -> Result<OperationPlan>;
    fn set_avatar(&self, profile: &BrowserProfile, image: &Path) -> Result<()>;

    fn plan_delete(&self, profile: &BrowserProfile, mode: DeleteMode) -> Result<OperationPlan>;
    fn delete_profile(&self, profile: &BrowserProfile, mode: DeleteMode) -> Result<()>;

    fn plan_clean_cache(&self, profile: &BrowserProfile) -> Result<OperationPlan>;
    fn clean_cache(&self, profile: &BrowserProfile) -> Result<u64>;
}
```

Both default to returning `Error::Unsupported`, ensuring an adapter only claims operations it has explicitly implemented and validated.

### `OperationPlan`: Shared Dry-Run and Confirmation Representation

`OperationPlan` (`src/domain/operation.rs`) is the single data structure that backs both CLI `--dry-run` output and TUI modal confirmation dialogs:

```rust
pub struct OperationPlan {
    pub kind: OperationKind,
    pub browser: String,
    pub profile: String,
    pub steps: Vec<PlanStep>,
    pub copies: Vec<String>,
    pub excludes: Vec<String>,
    pub paths_affected: Vec<PathBuf>,
    pub reclaimed_bytes: Option<u64>,
    pub requires_browser_closed: bool,
}
```

- The `lines()` method renders the plan into standardized lines of text.
- In the CLI, passing `--dry-run` displays this text and exits with code 0 without modifying files.
- In the TUI, this exact same plan text is rendered inside confirmation popups prior to execution.

### The `Transaction` Engine with Undo Log and `Drop` Safety Net

Every structural write runs inside a `Transaction` (`src/fs/transaction.rs`).
The transaction manages a temporary backup directory (`/tmp/pmux-<label>-<pid>-<nanos>-<count>`) and records inverse actions on an internal undo stack:

- `UndoStep::RemovePath(PathBuf)`: Inverse of directory or file creation; deletes the path on rollback.
- `UndoStep::RestoreFile { original, backup }`: Inverse of modifying an existing file; restores the pre-mutation content from the backup directory.
- `UndoStep::RenameBack { from, to }`: Inverse of directory or file renaming; moves the item back to its original location.

#### Invariants:
1. **Atomic Commits**: Calling `tx.commit()` marks the transaction finished and removes the backup directory.
2. **Reverse Rollback**: Calling `tx.rollback()` iterates through recorded undo steps in reverse order, restoring original files and removing newly created paths, then removes the backup directory.
3. **`Drop` Safety Net**: If a `Transaction` is dropped while neither committed nor explicitly rolled back (for example, if a function returns early via `?` or a panic occurs), the `Drop` implementation automatically executes the rollback:
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

### The `TrashBin` Abstraction

Profile deletions move data to the operating system Trash rather than performing permanent unrecoverable removals (`rm -rf` / `remove_dir_all`):

```rust
pub trait TrashBin {
    fn send(&self, path: &Path) -> Result<PathBuf>;
}
```

- **`SystemTrash` (`src/fs/trash.rs`)**: Implementation used in production. Resolves `~/.Trash`, handles file name collisions by appending counter increments (`Profile 2`, `Profile 3`), and falls back to a safe copy-then-remove if the user data root crosses a filesystem device boundary (`EXDEV`).
- **`FakeTrash`**: Test fixture implementation directing trash operations to a temporary directory, ensuring unit and integration tests never pollute the developer's real `~/.Trash`.

### `ClonePolicy` and `ExtensionPolicy`

When creating a profile from a template or cloning an existing profile, ProfileMux enforces an explicit policy rather than copying the source directory wholesale:

```rust
pub struct ClonePolicy {
    pub copy_preferences: bool,
    pub copy_bookmarks: bool,
    pub extensions: ExtensionPolicy,
}

pub enum ExtensionPolicy {
    None,
    CopyExtensions,
    CopyExtensionsAndSettings,
}
```

#### Private Data Exclusions
Regardless of flags, ProfileMux **always excludes** private session data from clones:
- `Cookies`, `Login Data`, `History`, `Sessions`, `Web Data`, `Network state`, `Account identity (GAIA)`, `Local Storage`, `Service Worker`, and `Top Sites`.
- When copying `Preferences`, sensitive keys are stripped out before writing the target file (`account_info`, `gaia_cookie`, `signin`, `sync`, `google.services`, `password_manager`, `autofill`, etc.).

#### Extension Copying Mechanics and Constraints
- Default policy is `ExtensionPolicy::None` (no extensions copied).
- `CopyExtensions`: Copies the `Extensions` directory.
- `CopyExtensionsAndSettings`: Copies `Extensions`, `Local Extension Settings`, and `Sync Extension Settings`.
- **Chromium Signature Boundary**: Chromium signs extension registrations in `Secure Preferences` with a per-profile MAC (keyed to profile identity). ProfileMux deliberately does not attempt to forge or recompute these MAC signatures. Consequently, copied extensions may be flagged as modified and dropped by the browser upon next launch. This behavior is documented honestly as experimental.

## Identity Model

ProfileMux uses strict, stable identifiers that never rely on human-editable display names.

### `BrowserInstallId`
Derived from the browser's kind slug and its canonical, normalized user data root path:
```rust
BrowserInstallId(format!("{}:{}", kind.slug(), user_data_root.display()))
```
- Example: `brave:/Users/alice/Library/Application Support/BraveSoftware/Brave-Browser`
- Ensures release channels (Stable, Beta) and custom roots remain completely distinct.

### `ProfileId`
Derived from the parent `BrowserInstallId` and the physical on-disk directory name:
```rust
ProfileId(format!("{}/{}", install.as_str(), directory))
```
- Example: `brave:/Users/alice/Library/Application Support/BraveSoftware/Brave-Browser/Default`
- Grounded in the stable filesystem folder where profile data resides.

### Display Names Are Mutable Labels
- Display names (e.g. "Personal", "Work") are mutable strings stored inside browser metadata (`Local State` under `profile.info_cache.<dir>.name`).
- Display names are **never** used as keys or identifiers in internal data structures.
- Renaming a display name changes only the metadata label; the underlying `ProfileId`, path, and directory remain untouched.
- Selectors in the CLI accept display names for user convenience, but ambiguous matches are immediately rejected with a list of candidates.

## `ProfileStoreSnapshot` and Doctor Engine

Health checks in ProfileMux use an immutable snapshot architecture.

### Single Read-Only Pass
When inspecting an installation, the adapter executes a single read pass over the filesystem and `Local State` to construct a `ProfileStoreSnapshot`:
```rust
pub struct ProfileStoreSnapshot {
    pub registered: Vec<BrowserProfile>,
    pub unregistered_dirs: Vec<PathBuf>,
    pub orphan_cache_dirs: Vec<PathBuf>,
    pub parse_error: Option<String>,
}
```

### Pure Doctor Function
The `doctor` module (`src/doctor/mod.rs`) takes `&BrowserInstall` and `&ProfileStoreSnapshot` and performs pure functional analysis:
```rust
pub fn analyze(install: &BrowserInstall, snapshot: &ProfileStoreSnapshot) -> Vec<HealthFinding>;
```
- Performs **no filesystem I/O** and makes no system calls.
- Inspects snapshot data for missing profile directories, unregistered folders, orphan cache directories, duplicate display names, and metadata corruption.
- Because it is pure, the doctor engine is thoroughly tested with synthetic snapshots without touching disk.
