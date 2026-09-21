# ProfileMux Developer Context (`CONTEXT.md`)

This document provides architectural orientation and ground truth constraints for developers and AI agents working on ProfileMux.

---

## 1. Executive Summary

**ProfileMux** (`pmux`) is a local-first, cross-browser profile manager for power users, written in Rust. It provides unified discovery, inspection, storage analysis, and health diagnostics across multiple browser installations (e.g. Brave, Chrome, Chromium, Edge) via an interactive Ratatui TUI and a clap-derive CLI over a single shared core library.

---

## 2. Current Milestone: v0.1 (Strictly Read-Only)

ProfileMux is at **v0.1**. It is strictly a **read-only** tool.

### Implemented
- Browser installation discovery across macOS application bundles and user data roots.
- Profile enumeration from browser metadata.
- Profile details and metadata extraction (avatar info, registration status, last active timestamps).
- Recursive per-profile disk size measurement (core profile directory, HTTP cache, code cache, GPU cache).
- Health doctor diagnostics: checks for unregistered profile directories, orphan caches, and metadata parse errors.
- CLI subcommands: `pmux browsers`, `pmux profiles`, `pmux profile show`, `pmux profile open`, `pmux doctor`.
- Ratatui three-pane TUI dashboard.

### Not Implemented (Must Not Be Overstated)
- Profile creation (`create`)
- Profile cloning (`clone`)
- Display name renaming (`rename_display_name`)
- Directory renaming (`rename_directory`)
- Profile deletion (`delete`)
- Cache cleanup (`clean_cache`)
- Profile launching (`launch`)
- Profile configuration templates
- Custom avatar management
- Transaction execution engine and rollback system
- Dry-run mode for mutations
- Firefox adapter
- Safari adapter

All unimplemented mutation methods return `Error::Unsupported` and are disabled in the UI.

---

## 3. Verified Facts (Development Machine)

- **Brave Browser (Stable)**: Bundle ID `com.brave.Browser`, user data root `~/Library/Application Support/BraveSoftware/Brave-Browser`, cache root `~/Library/Caches/BraveSoftware/Brave-Browser`. Tested and operational.
- **Brave Browser Beta**: Bundle ID `com.brave.Browser.beta`, user data root `~/Library/Application Support/BraveSoftware/Brave-Browser-Beta`, cache root `~/Library/Caches/BraveSoftware/Brave-Browser-Beta`. Tested and operational. Brave Stable and Beta maintain fully independent profile stores.
- **Google Chrome (Stable)**: Bundle ID `com.google.Chrome`, user data root `~/Library/Application Support/Google/Chrome`, cache root `~/Library/Caches/Google/Chrome`. Tested and operational.
- **Chromium, Edge, Vivaldi, Brave Nightly**: Definitions and path heuristics exist, but application bundles were not present on this machine; classified as **Supported-but-untested**.
- **Firefox**: Not installed on the machine; no adapter exists.
- **Safari**: Conservative detection only; no adapter exists and no mutation will ever be supported without verified safety guarantees.
- **Chromium Storage Mechanics**:
  - Metadata lives in `<user data root>/Local State` under `profile.info_cache`, keyed by on-disk directory name.
  - The mutable display name lives under `profile.info_cache.<dir>.name`.
  - On macOS, per-profile caches are stored at `<cache root>/<ProfileDirectory>/{Cache,Code Cache}`, outside the profile directory.

---

## 4. Codebase Structure

```text
Cargo.toml                  # Dependencies: clap, ratatui, crossterm, serde, plist, thiserror
src/
├── main.rs                 # Binary entry point -> calls cli::run()
├── lib.rs                  # Core library re-exports
├── error.rs                # Typed Error enum (NotFound, Malformed, Unsupported, etc.)
├── domain/                 # Core data structures and identifiers
│   ├── browser.rs          # BrowserInstallId, BrowserKind, Channel, SupportLevel, BrowserInstall
│   ├── profile.rs          # ProfileId, BrowserProfile, StorageBreakdown, ProfileStoreSnapshot
│   ├── capability.rs       # BrowserCapabilities struct and disabled reason helpers
│   └── health.rs           # Severity, HealthFinding
├── browsers/               # Browser adapter abstractions and implementations
│   ├── mod.rs              # BrowserAdapter trait and discover_all()
│   └── chromium/           # Chromium family adapter, Local State parser, discovery
│       ├── mod.rs          # ChromiumAdapter declaration
│       ├── discovery.rs    # App bundle and user data root scanning
│       ├── local_state.rs  # Local State JSON parsing
│       └── profile.rs      # BrowserAdapter trait impl for Chromium
├── doctor/                 # Pure health analysis
│   └── mod.rs              # analyze(install, snapshot) -> Vec<HealthFinding>
├── fs/                     # Filesystem helpers
│   ├── mod.rs
│   └── size.rs             # Disk usage calculations
├── platform/               # Platform-specific integrations
│   ├── mod.rs
│   └── macos/              # macOS bundle IDs and default paths
├── cli/                    # Clap-derive command line interface
│   └── mod.rs              # CLI command handling
└── tui/                    # Ratatui terminal user interface
    └── mod.rs              # Three-pane dashboard event loop
```

---

## 5. Architectural Invariants

1. **Presentation Layer Isolation**: The TUI and CLI must never perform direct browser-specific filesystem logic, path construction, or metadata parsing. They interact solely through the core library and `BrowserAdapter`.
2. **Stable Identifiers vs. Labels**:
   - `BrowserInstallId`: Derived from `BrowserKind` slug and normalized user data path (`format!("{}:{}", kind.slug(), user_data_root.display())`).
   - `ProfileId`: Derived from `BrowserInstallId` and the physical directory name (`format!("{}/{}", install.as_str(), directory)`).
   - Display names are purely human-facing labels and are never used as identifiers or map keys.
3. **Explicit Capabilities**: Adapters declare capabilities via `BrowserCapabilities`. The UI disables unsupported actions upfront rather than failing at runtime.
4. **Pure Doctor Engine**: The doctor does not execute I/O. It consumes an immutable `ProfileStoreSnapshot` and produces a list of `HealthFinding` items deterministically.

---

## 6. Safety and Privacy Commitments

- **Privacy**: ProfileMux inspects only high-level structural metadata, directory entries, and file sizes. It **never** reads or extracts passwords (`Login Data`), session tokens/cookies (`Cookies`), browsing histories (`History`), or form entries. Zero telemetry and zero network calls.
- **Future Mutation Lifecycle**: When mutation is introduced in v0.2+, it must follow the strict 8-stage lifecycle (Inspect, Preflight, Build Plan, Confirm, Metadata Backup, Execute, Validate, Commit) with automated rollback on failure.
- **No Process Killing**: ProfileMux will never issue `SIGKILL` or kill browser processes; structural mutations will require the user to close the browser cleanly first.
- **OS Trash Only**: Deletions will move directories to the OS Trash, never executing irreversible `rm -rf`.
