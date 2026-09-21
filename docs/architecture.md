# ProfileMux Architecture

This document describes the architectural layering, identity model, trait boundaries, and diagnostic pipeline of ProfileMux (`pmux`).

## Layering

ProfileMux enforces strict unidirectional layering. High-level presentation layers delegate entirely to the core library, and no UI or CLI component contains browser-specific filesystem logic.

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
│  │  ProfileStoreSnapshot, HealthFinding, etc.)      │  │
│  └────────────────────────┬─────────────────────────┘  │
│                           │                            │
│  ┌────────────────────────▼─────────────────────────┐  │
│  │                 Browser Adapters                 │  │
│  │  (BrowserAdapter trait, Chromium, Firefox, etc.) │  │
│  └────────────────────────┬─────────────────────────┘  │
│                           │                            │
│  ┌────────────────────────▼─────────────────────────┐  │
│  │                   Doctor Engine                  │  │
│  │     (Pure snapshot analysis, health findings)    │  │
│  └──────────────────────────────────────────────────┘  │
└───────────────────────────┬────────────────────────────┘
                            │
            ┌───────────────┴───────────────┐
            ▼                               ▼
┌───────────────────────┐       ┌───────────────────────┐
│     Platform Layer    │       │     Filesystem Layer  │
│ (macOS bundle lookup, │       │  (Disk size scanning, │
│  app path resolution) │       │   path normalization) │
└───────────────────────┘       └───────────────────────┘
```

### Component Responsibilities

1. **Entry Point (`src/main.rs`)**: Thin executable wrapper delegating directly to `profilemux::cli::run()`.
2. **CLI Layer (`src/cli/`)**: Command dispatch built using `clap`. Parses arguments and flags, invokes domain operations, and prints formatted human-readable or structured output.
3. **TUI Layer (`src/tui/`)**: Interactive terminal application built with `ratatui` and `crossterm`. Manages terminal state, renders a three-pane layout, and translates user input events into library calls.
4. **Domain Layer (`src/domain/`)**: Browser-agnostic data models, identifiers, enums, capability structures, and health findings (`BrowserInstallId`, `ProfileId`, `BrowserInstall`, `BrowserProfile`, `BrowserCapabilities`, `ProfileStoreSnapshot`, `HealthFinding`).
5. **Adapters Layer (`src/browsers/`)**: Implementations of the `BrowserAdapter` trait that understand browser-specific on-disk layouts, configuration file formats (such as Chromium `Local State`), and process execution.
6. **Doctor Engine (`src/doctor/`)**: Pure functional health analysis. Consumes an immutable snapshot and returns diagnostic findings without performing I/O.
7. **Platform & Filesystem Layer (`src/platform/`, `src/fs/`)**: Operating system abstractions (application bundle detection, bundle identifier inspection, cache directory resolution, and recursive disk size measurement).

### Separation Rule

The TUI and CLI are presentation shells only. Neither contains browser-specific filesystem logic, path heuristics, or file parsing routines. If a browser stores profile metadata in JSON, INI, or SQLite, that detail is encapsulated inside the respective `BrowserAdapter`.

## The `BrowserAdapter` Trait and Capabilities

Every browser family is integrated by implementing the `BrowserAdapter` trait (`src/browsers/mod.rs`):

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

    // Mutation methods default to returning Error::unsupported(...)
    fn launch_profile(&self, profile: &BrowserProfile) -> Result<()>;
    fn rename_display_name(&self, profile: &BrowserProfile, new_name: &str) -> Result<()>;
    fn rename_profile_directory(&self, profile: &BrowserProfile, new_dir: &str) -> Result<()>;
    fn delete_profile(&self, profile: &BrowserProfile) -> Result<()>;
}
```

### Explicit Capabilities

Capabilities are declared explicitly via `BrowserCapabilities` (`src/domain/capability.rs`):

```rust
pub struct BrowserCapabilities {
    pub launch: bool,
    pub open_folder: bool,
    pub create: bool,
    pub rename_display_name: bool,
    pub rename_directory: bool,
    pub clone: bool,
    pub delete: bool,
    pub clean_cache: bool,
    pub custom_avatar: bool,
    pub experimental_custom_avatar: bool,
    pub experimental_rename_directory: bool,
}
```

### Why Capabilities Are Explicit

1. **No Assumed Features**: Different browser engines handle profile isolation, process models, and directory hierarchies differently. For instance, Safari does not use separate profile directories in the way Chromium does; Firefox uses an `ini` profiles registry with arbitrary directory names; Chromium pairs profile folders with an entry in `Local State`.
2. **Fail Fast and Prevent Invalid Operations**: Rather than discovering at runtime that an operation corrupts state or fails midway through execution, the UI queries `capabilities()` to disable unsupported buttons and options beforehand.
3. **Safe Defaults**: `BrowserCapabilities::NONE` sets all capabilities to `false`. Adapters opt into capabilities as they are built and verified. In v0.1, the Chromium adapter uses `BrowserCapabilities::READ_ONLY`, which enables only `open_folder` alongside read-only inspection.
4. **Default Trait Implementations**: Mutation methods on `BrowserAdapter` default to returning `Error::Unsupported { operation, browser }`. An adapter cannot accidentally expose a mutation method it has not explicitly implemented.

## Identity Model

ProfileMux uses strict, stable identifiers that do not depend on human-editable display labels.

### `BrowserInstallId`

A browser installation's identity is derived from its `BrowserKind` slug and its canonical, normalized user data root path:

```rust
BrowserInstallId(format!("{}:{}", kind.slug(), user_data_root.display()))
```

- Example: `brave:~/Library/Application Support/BraveSoftware/Brave-Browser`
- **Why**: A user can have multiple release channels (e.g. Brave Stable vs. Brave Beta) or portable browser directories. Deriving the ID from the kind and the verified user data path guarantees uniqueness across installations.

### `ProfileId`

A profile's identity is derived from its parent `BrowserInstallId` and its physical on-disk directory name:

```rust
ProfileId(format!("{}/{}", install.as_str(), directory))
```

- Example: `brave:~/Library/Application Support/BraveSoftware/Brave-Browser/Default`
- **Why**: The directory name (such as `Default`, `Profile 1`, `Profile 2`) is the stable anchor on the filesystem where browser data, extensions, and caches are stored.

### Display Names Are Mutable Labels

Display names (e.g. "Work", "Personal", "Development") are mutable labels stored in browser metadata (such as `Local State` under `profile.info_cache.<dir>.name` in Chromium).
- **Rule**: Display names are **never** used as keys, identifiers, or selectors in internal data structures.
- Renaming a profile's display name updates only the metadata label. The underlying `ProfileId`, path, and directory remain unchanged.
- CLI selectors resolve display names or directories against the snapshot at invocation time, but all internal indexing uses `ProfileId`.

## `ProfileStoreSnapshot` and the Pure Doctor Engine

Health checks in ProfileMux are designed around an immutable snapshot pattern.

### Single Read-Only Pass

When inspecting a browser installation, the adapter scans the filesystem and configuration once to produce a `ProfileStoreSnapshot` (`src/domain/profile.rs`):

```rust
pub struct ProfileStoreSnapshot {
    pub registered: Vec<BrowserProfile>,
    pub unregistered_dirs: Vec<PathBuf>,
    pub orphan_cache_dirs: Vec<PathBuf>,
    pub parse_error: Option<String>,
}
```

The snapshot captures:
- All profiles declared in the browser's metadata (`registered`).
- Directories inside the user data root that look like profiles but are missing from metadata (`unregistered_dirs`).
- Directories in the cache root that have no corresponding profile directory (`orphan_cache_dirs`).
- Any syntax or read errors encountered while parsing the metadata file (`parse_error`).

### Doctor as a Pure Function

The `doctor` module (`src/doctor/mod.rs`) takes the `BrowserInstall` and `ProfileStoreSnapshot` as arguments and performs deterministic, pure analysis:

```rust
pub fn analyze(install: &BrowserInstall, snapshot: &ProfileStoreSnapshot) -> Vec<HealthFinding>;
```

- The doctor performs **no filesystem I/O**, network requests, or system calls.
- It produces a list of `HealthFinding` items (`src/domain/health.rs`) containing a `Severity` (`Ok`, `Warning`, `Broken`), a machine-readable error code (e.g. `profile-directory-missing`, `orphan-cache-dir`), a descriptive message, and the affected paths.
- Because it is a pure function, doctor logic can be tested thoroughly with synthetic snapshots without mocking filesystems or creating temporary directories.

## Adding Future Browser Families

ProfileMux's architecture makes adding new browser families modular:

### 1. Adding a Native Browser Family (e.g. Firefox)

To add support for a new browser family such as Firefox:
1. Ensure the `BrowserKind` enum in `src/domain/browser.rs` includes the relevant variants (e.g. `Firefox`, `FirefoxDeveloper`, `FirefoxNightly`), mapped to `BrowserFamily::Firefox`.
2. Create a module under `src/browsers/firefox/` that implements:
   - Installation discovery (locating app bundles and reading `profiles.ini` / `installs.ini`).
   - Profile enumeration from the configuration file.
   - Cache directory mapping (e.g. `~/Library/Caches/Firefox/Profiles/<profile>`).
3. Implement `BrowserAdapter` for `FirefoxAdapter`. Declare supported capabilities via `capabilities()`.
4. Register the discoverer in `src/browsers/mod.rs::discover_all()`.

No changes to the CLI, TUI, or doctor analysis engine are required.

### 2. User-Defined Browser Definitions (Future)

The separation between `BrowserKind`, `BrowserInstall`, and `BrowserAdapter` creates a clear path for user-defined browser definitions:
- A user configuration file (e.g. `~/.config/profilemux/browsers.toml`) could declare a browser name, executable path, user data root, cache root, and adapter family (e.g. `family = "chromium"`).
- At discovery time, ProfileMux would instantiate the existing generic adapter (e.g. `ChromiumAdapter`) with the user-configured paths.
- Because `BrowserInstallId` and `ProfileId` derive from the paths and kind slugs, custom installations seamlessly integrate into the existing indexing, snapshot, and doctor systems.
