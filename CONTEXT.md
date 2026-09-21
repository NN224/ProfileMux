# ProfileMux Developer Context (`CONTEXT.md`)

This document provides architectural orientation, module layout, and ground truth constraints for developers and AI agents working on ProfileMux.

---

## 1. Executive Summary

**ProfileMux** (`pmux`) is a local-first Chromium browser profile manager for macOS, written in Rust. It provides discovery, inspection, storage analysis, health diagnostics, launching, and transaction-safe profile mutations across Chromium-family installations (e.g. Brave, Chrome, Chromium, Edge) via an interactive Ratatui TUI and a clap-derive CLI over a single shared core library.

---

## 2. Milestone: v1 Shipped & Live-Validated

ProfileMux has completed and shipped the **v1** milestone. Real mutation is fully implemented and live-validated.

### Implemented and Verified
- **Discovery**: Locates macOS Chromium application bundles, bundle identifiers, and data roots.
- **Inspection**: Enumerates profiles from `Local State`, extracts avatars, active timestamps, and registration flags.
- **Storage Breakdown**: Measures core directory, HTTP cache, code cache, and GPU cache footprint.
- **Health Doctor**: Analyzes snapshots for missing directories, orphan caches, duplicate profile entries, and metadata syntax errors.
- **Profile Launching**: Spawns the browser executable with `--user-data-dir` and `--profile-directory`.
- **Profile Creation**: Creates on-disk profile folders and registers entries in `Local State`.
- **Template Profile Creation & Cloning**: Duplicates configuration from an existing profile with an explicit `ClonePolicy` that strips credentials, sessions, and histories.
- **Display-Name Renaming**: Updates profile labels in `Local State` while leaving the underlying directory intact.
- **Custom Profile Avatar**: Normalizes images to 256x256 PNG format, installs `Google Profile Picture.png`, and updates `Local State` avatar fields (Brave only).
- **Safe Profile Deletion**: Moves profile and external cache directories to `~/.Trash` via the `TrashBin` trait and deregisters from `Local State`.
- **Cache Cleanup**: Removes verified cache subdirectories (`Cache`, `Code Cache`, `GPUCache`, etc.) while protecting user state.
- **Dry-Run Engine**: Every mutation command supports `--dry-run`, generating an `OperationPlan` that details steps, copies, exclusions, and reclaimed bytes without touching disk.
- **Filesystem Transaction Layer**: Reversible operations run within a `Transaction` that records undo steps and automatically rolls back on failure or early `Drop`.
- **CLI & TUI Interfaces**: Full command-line suite and three-pane Ratatui TUI with modal dialogs for all mutations.

### Experimental Capabilities
- **Custom Profile Avatar**: Supported on Brave Browser and Brave Browser Beta only. Google Chrome does not claim this capability.
- **Profile Directory Renaming**: Renames the physical on-disk profile directory and updates `Local State` references. Implemented across Chromium adapters, but marked experimental due to internal path references maintained by certain extensions.
- **Extension Copying**: `none` (default), `copy`, and `copy-settings`. Chromium signs extension registrations in `Secure Preferences` with a per-profile MAC that ProfileMux deliberately does not forge. Copied extensions may be dropped by the browser on next launch.

### Out of Scope / Not Implemented
- Firefox and Safari adapters
- Windows and Linux platforms
- Permanent unrecoverable deletion (non-Trash)
- External configuration files or plugin architectures
- Standalone audit-log file subsystem

---

## 3. Verified Facts (Development Machine)

- **Brave Browser (Stable)**: Bundle ID `com.brave.Browser`, version `153.1.95.104`. Live-validated for discovery, inspection, doctor, launch, create, clone, rename, custom avatar, delete to Trash, and running-instance guard.
- **Brave Browser Beta**: Bundle ID `com.brave.Browser.beta`, version `154.1.97.44`. Live-validated across all capabilities including custom avatar. Operates completely independently from Brave Stable.
- **Google Chrome (Stable)**: Bundle ID `com.google.Chrome`, version `153.0.8010.52`. Live-validated across discovery, inspection, doctor, launch, create, clone, rename, delete to Trash, and running-instance guard. Custom avatar is not supported.
- **Chromium, Edge, Vivaldi, Brave Nightly, Chrome Beta/Dev/Canary**: Definitions and path heuristics exist in `src/browsers/chromium/discovery.rs`, but application bundles are not installed on this machine. They are classified as **implemented and untested**—never describe them as verified.
- **Firefox & Safari**: No adapters exist in the codebase; marked unsupported.

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

5. **No Force-Killing**:
   - ProfileMux never sends `SIGKILL` or `SIGTERM` to browser processes.
   - A running browser is requested to quit gracefully via AppleScript (`osascript`) using its bundle identifier.
   - Stale `SingletonLock` files left behind by crashes are detected by checking PID liveness.

6. **OS Trash Only**:
   - Profile deletions move folders to `~/.Trash`. Unrecoverable recursive removal (`rm -rf` / `remove_dir_all`) is strictly forbidden on user profile data.

7. **Privacy Boundary**:
   - ProfileMux is not a credential extractor, cookie viewer, or history exporter.
   - It reports names, paths, sizes, and metadata only.
   - Internal contents of `Cookies`, `Login Data`, `History`, and web storage are never inspected or exposed.
   - Zero network requests, zero telemetry.
