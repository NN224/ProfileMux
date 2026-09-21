# ProfileMux Developer Context (`CONTEXT.md`)

This document provides architectural orientation, module layout, and ground truth constraints for developers and AI agents working on ProfileMux.

---

## 1. Executive Summary

**ProfileMux** (`pmux`) is a local-first Chromium browser profile manager for macOS, written in Rust. It provides discovery, inspection, storage analysis, health diagnostics, launching, and transaction-safe profile mutations across Chromium-family installations (e.g. Brave, Chrome, Chromium, Edge) via an interactive Ratatui TUI and a clap-derive CLI over a single shared core library.

---

## 2. Milestone: 1.0.0 Shipped & Live-Validated

ProfileMux has completed and shipped version **1.0.0**, the first usable release. Real mutation is fully implemented and live-validated.

### Implemented and Verified
- **Discovery**: Locates macOS Chromium application bundles, bundle identifiers, and data roots.
- **Inspection**: Enumerates profiles from `Local State`, extracts avatars, account emails (`user_name`), active timestamps, and registration flags.
- **Storage Breakdown**: Measures core directory, HTTP cache, code cache, and GPU cache footprint.
- **Health Doctor**: Analyzes snapshots for missing directories, orphan caches, duplicate profile entries, and metadata syntax errors.
- **Profile Launching**: Spawns the browser executable with `--user-data-dir` and `--profile-directory`.
- **Profile Creation**: Creates on-disk profile folders and registers entries in `Local State`.
- **Template Profile Creation & Cloning**: Duplicates configuration from an existing profile with an explicit `ClonePolicy` that strips credentials, sessions, and histories.
- **Display-Name Renaming**: Updates profile labels in `Local State` while leaving the underlying directory intact.
- **Custom Profile Avatar**: Normalizes images to 256x256 PNG format, installs `Google Profile Picture.png`, and updates `Local State` avatar fields (Brave only).
- **Safe Profile Deletion**: Moves profile and external cache directories to `~/.Trash` via the `TrashBin` trait and deregisters from `Local State`.
- **Cache Cleanup**: Removes verified cache subdirectories (`Cache`, `Code Cache`, `GPUCache`, shader caches, `DawnCache`, `component_crx_cache`, `Service Worker/CacheStorage`, and external cache) while protecting user state.
- **Dry-Run Engine**: Every mutation command supports `--dry-run`, generating an `OperationPlan` that details steps, copies, exclusions, and reclaimed bytes without touching disk.
- **Filesystem Transaction Layer**: Reversible operations run within a `Transaction` that records undo steps and automatically rolls back on failure or early `Drop`.
- **CLI & TUI Interfaces**: Full command-line suite and three-pane Ratatui TUI with modal dialogs for all mutations.

### Experimental Capabilities
- **Custom Profile Avatar**: Supported and live-validated on Brave Browser and Brave Browser Beta only. The Chrome adapter deliberately does not claim this capability.
- **Profile Directory Renaming**: Renames the physical on-disk profile directory and updates `Local State` references. Implemented across Chromium adapters, but marked experimental due to internal path references maintained by certain extensions.
- **Extension Copying**: `none` (default), `copy`, and `copy-settings`. Chromium signs extension registrations in `Secure Preferences` with a per-profile MAC. ProfileMux deliberately does not forge those signatures, and copied extensions may therefore be dropped by the browser on next launch.

### Out of Scope / Not Implemented
- There is no adapter for Firefox or Safari; neither is implemented.
- Windows and Linux platforms
- Permanent unrecoverable deletion (non-Trash)
- External configuration files or plugin architectures
- Standalone audit-log file subsystem

---

## 3. Verified Facts (Development Machine)

- **Brave Browser (Stable)** and **Brave Browser Beta**: Bundle IDs `com.brave.Browser` (v`153.1.95.104`) and `com.brave.Browser.beta` (v`154.1.97.44`). Full, live-validated Chromium profile management including a live-validated custom avatar.
- **Google Chrome (Stable)**: Bundle ID `com.google.Chrome`, version `153.0.8010.52`. Full, live-validated Chromium profile management except the custom avatar, which the Chrome adapter deliberately does not claim and which was therefore never validated. The capability is not claimed for Chrome.
- **Chromium, Microsoft Edge (all channels), Vivaldi, Brave Nightly, and non-stable Google Chrome channels (Beta, Dev, Canary)**: Discovery definitions and path mappings exist in `src/browsers/chromium/discovery.rs` and share the same adapter code, but are not installed on the validation machine, so they are implemented and not locally validated — never verified, never supported.
- **Non-Chromium Browsers**: There is no adapter for Firefox or Safari; neither is implemented.

---

## 4. Codebase Structure

```text
Cargo.toml                      # Dependencies: clap, ratatui, crossterm, serde, plist, image, dirs, thiserror
src/
├── main.rs                     # Binary entry point -> calls cli::run()
├── lib.rs                      # Core library re-exports
├── error.rs                    # Typed Error enum (NotFound, Malformed, Unsupported, etc.)
├── domain/                     # Core domain models, identifiers, and policies
│   ├── mod.rs
│   ├── browser.rs              # BrowserInstallId, BrowserKind, Channel, SupportLevel, BrowserInstall
│   ├── capability.rs           # BrowserCapabilities struct and disabled reason helper
│   ├── health.rs               # Severity, HealthFinding
│   ├── operation.rs            # OperationPlan, PlanStep, ClonePolicy, ExtensionPolicy, specs
│   ├── profile.rs              # ProfileId, BrowserProfile, StorageBreakdown, ProfileStoreSnapshot
│   └── sanitize.rs             # Directory name sanitization and collision deduplication
├── browsers/                   # Browser adapter trait and implementations
│   ├── mod.rs                  # BrowserAdapter trait and discover_all()
│   └── chromium/               # Chromium-family adapter implementation
│       ├── mod.rs              # ChromiumAdapter struct and discover()
│       ├── avatar.rs           # PNG image normalization (256x256) and Local State avatar keys
│       ├── clone.rs            # CopyItem calculation, sensitive key stripping, clone policies
│       ├── discovery.rs        # Static browser definitions, bundle matching, install deduplication
│       ├── launch.rs           # Binary resolution, launch command, running check, osascript quit
│       ├── local_state.rs      # Local State JSON serialization and parsing
│       ├── mutation.rs         # Plan creation, execution, Local State mutations, cache clean
│       └── profile.rs          # BrowserAdapter implementation for ChromiumAdapter
├── doctor/                     # Pure health analysis
│   └── mod.rs                  # analyze(install, snapshot) -> Vec<HealthFinding>
├── fs/                         # Filesystem, transaction, and trash abstractions
│   ├── mod.rs
│   ├── size.rs                 # Recursive directory disk usage measurement
│   ├── transaction.rs          # Rollback-capable Transaction with undo stack and Drop safety net
│   └── trash.rs                # TrashBin trait, SystemTrash (~/.Trash), and test FakeTrash
├── platform/                   # Platform-specific OS hooks
│   ├── mod.rs
│   └── macos.rs                # macOS bundle resolution and Application directory search
├── cli/                        # Command-line interface
│   ├── mod.rs                  # Entry point
│   ├── commands.rs             # Clap arguments, subcommand dispatch, table rendering
│   ├── mutate.rs               # Preflight confirmation, dry-run dispatch, mutation execution
│   ├── output.rs               # Table formatting and JSON printing helpers
│   └── selector.rs             # Profile resolution by ProfileId, slug/dir, or display name
└── tui/                        # Ratatui terminal user interface
    ├── mod.rs                  # Alternate screen lifecycle, terminal guards, event loop
    ├── app.rs                  # TUI state model, pane focus, active overlays, dialog state
    ├── dialogs.rs              # Modal dialogs (New, Clone, Rename, Delete, Avatar, Clean, etc.)
    ├── events.rs               # Keyboard input routing for panes, dialogs, and text fields
    ├── form.rs                 # Form input state, cursor positioning, and editing logic
    ├── keymap.rs               # Key mapping and fuzzy subsequence filter matcher
    ├── render.rs               # Three-pane layout rendering, status line, action bar, overlays
    └── sizes.rs                # Background channel-based storage measurement scanner
```

---

## 5. Architectural and Safety Invariants

1. **Presentation Layer Isolation**:
   - The TUI and CLI must never perform direct browser-specific filesystem logic, path construction, or metadata parsing.
   - All mutations must be requested through `BrowserAdapter` paired methods (`plan_*` and execute).

2. **Testing Invariant: Never Mutate Real Browser Data**:
   - Automated tests must **never** modify the user's real browser directories or `~/.Trash`.
   - Unit tests use `tempfile::TempDir`, `FakeTrash`, and synthetic fixtures (`ChromiumFixture`).
   - Live integration tests drive real browser binaries against isolated temporary roots using the `--user-data-dir` argument.

3. **Transaction-First Execution**:
   - Every mutating filesystem operation must execute within a `Transaction`.
   - Inverses must be recorded before writes occur.
   - If a function returns early (e.g. via `?`), the `Transaction::drop` implementation automatically triggers rollback.

4. **Stable Identifiers vs. Display Labels**:
   - `BrowserInstallId`: `<kind_slug>:<canonical_user_data_root>`
   - `ProfileId`: `<BrowserInstallId>/<directory>`
   - Display names are mutable labels stored in `Local State`; they must never be used as persistent keys or identifiers in internal data structures.

5. **Running Browser Preflight & No Force-Killing**:
   - Whether a browser is running is decided per user data root rather than per binary.
   - A `SingletonLock` left behind by a crash is treated as stale by checking whether the PID it names is still alive via `/bin/ps`.
   - The CLI refuses a structural operation with an actionable error unless `--close-browser` is passed.
   - The TUI offers Quit Browser or Cancel in a modal dialog.
   - Browser quit is always requested through AppleScript (`tell application id "<bundle_id>" to quit`) - ProfileMux never force-terminates a browser (`SIGKILL` and `SIGTERM` are never issued).

6. **OS Trash Only**:
   - Profile deletions move folders to `~/.Trash`. Unrecoverable recursive removal (`rm -rf` / `remove_dir_all`) is strictly forbidden on user profile data.

7. **Privacy Boundary**:
   - ProfileMux may display the browser profile's own account email, which the browser itself records in its profile metadata (`Local State` under `profile.info_cache.<dir>.user_name`).
   - It does not read or display passwords, cookies, authentication or session tokens, the contents of browsing history, or the contents of `Login Data`, and it never opens a profile's credential or history databases.
   - Reports whether sensitive files exist and how large they are, never what is inside them.
   - Refusing to copy account identity into a cloned profile and displaying an existing profile's account email are separate guarantees; new profiles never inherit signed-in identity.
   - ProfileMux is not a password extractor, a cookie viewer, or a session-token exporter; it makes zero network calls and collects zero telemetry.
