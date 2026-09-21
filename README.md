# ProfileMux (`pmux`)

ProfileMux is a local-first, cross-browser profile manager for power users, written in Rust with a Ratatui terminal user interface (TUI) and a clap-derive command-line interface (CLI) built on top of a single shared core library. It provides centralized profile discovery, metadata inspection, per-profile disk and cache footprint analysis, and structural health diagnostics across multiple browser installations without requiring browser-specific scripts or manual directory navigation.

## Status

ProfileMux is currently at **v0.1** and is strictly **READ-ONLY**.

- **Implemented**: Browser installation discovery, profile listing, profile metadata inspection, per-profile disk size measurement (core, cache, code cache, GPU cache), structural doctor health checks, the CLI commands (`pmux browsers`, `pmux profiles`, `pmux profile show`, `pmux profile open`, `pmux doctor`), and the interactive three-pane TUI.
- **Not implemented**: Profile creation, cloning, display name renaming, directory renaming, profile deletion, cache cleanup, profile launching, configuration templates, custom avatars, dry-run mode, the transaction execution engine, and adapters for Firefox and Safari.

Unimplemented features return `Error::Unsupported` in the core library and are disabled in the user interface.

## Installation and Building

Building ProfileMux requires Rust 1.80 or later.

```bash
# From the repository root, build the release binary
cargo build --release

# The compiled binary is located at target/release/pmux
./target/release/pmux --help
```

## CLI Usage

Running `pmux` without subcommands launches the interactive TUI. Subcommands provide scriptable and terminal-based inspection.

### Discover installed browsers

```bash
pmux browsers
```

Sample output:
```text
ID                                                    KIND        CHANNEL   SUPPORT     PROFILES  PATH
brave:~/Library/Application Support/BraveSoftware/Brave-Browser       brave       Stable    Read-only   3         /Applications/Brave Browser.app
brave-beta:~/Library/Application Support/BraveSoftware/Brave-Browser-Beta  brave-beta  Beta      Read-only   1         /Applications/Brave Browser Beta.app
chrome:~/Library/Application Support/Google/Chrome    chrome      Stable    Read-only   2         /Applications/Google Chrome.app
```

### List profiles

```bash
pmux profiles
```

Sample output:
```text
BROWSER     DIRECTORY       DISPLAY NAME    REGISTERED  SIZE (TOTAL)  LAST ACTIVE
brave       Default         Personal        yes         1.42 GB       2026-09-20 18:32
brave       Profile 1       Work            yes         824.10 MB     2026-09-21 09:15
brave       Profile 2       Research        yes         312.45 MB     2026-09-14 11:04
brave-beta  Default         Beta Tester     yes         450.20 MB     2026-09-19 14:10
chrome      Default         Default         yes         2.10 GB       2026-09-21 16:45
chrome      Profile 1       Client Demo     yes         510.30 MB     2026-09-18 20:00
```

### Show profile details

```bash
pmux profile show brave/Default
```

Sample output:
```text
Profile:        Personal
ID:             brave:~/Library/Application Support/BraveSoftware/Brave-Browser/Default
Browser:        Brave Browser (Stable)
Directory:      Default
Path:           /Users/alice/Library/Application Support/BraveSoftware/Brave-Browser/Default
Cache Path:     /Users/alice/Library/Caches/BraveSoftware/Brave-Browser/Default
Registered:     true
Directory Exists: true
Avatar:         Stock (chrome://theme/IDR_PROFILE_AVATAR_26)
Last Active:    2026-09-20 18:32:11 UTC

Storage Breakdown:
  Core:         1.18 GB
  Cache:        184.20 MB
  Code Cache:   42.10 MB
  GPU Cache:    15.30 MB
  Total:        1.42 GB
```

### Open profile folder in file manager

```bash
pmux profile open brave/Default
```

Sample output:
```text
Revealed /Users/alice/Library/Application Support/BraveSoftware/Brave-Browser/Default in file manager.
```

### Run health doctor

```bash
pmux doctor
```

Sample output:
```text
[✓] brave: Brave Browser (Stable)
    Healthy: 3 profiles registered, all directories present.
[!] brave-beta: Brave Browser Beta (Beta)
    Warning: Found 1 orphan cache directory with no matching profile:
      - /Users/alice/Library/Caches/BraveSoftware/Brave-Browser-Beta/Profile 2
[✓] chrome: Google Chrome (Stable)
    Healthy: 2 profiles registered, all directories present.
```

## Terminal User Interface (TUI)

Launching `pmux` without subcommands opens a three-pane interactive terminal dashboard rendered via Ratatui:

1. **Left pane (Browsers)**: Lists all detected browser installations, their release channels, support levels, and profile counts.
2. **Center pane (Profiles)**: Lists profiles associated with the selected browser installation, showing on-disk directory names, display names, and health status glyphs.
3. **Right pane (Inspector & Doctor)**: Shows comprehensive profile details, metadata flags, storage breakdown (core, cache, code cache, GPU cache), and diagnostic findings from the health doctor.

### Keybindings

| Key | Action |
| --- | --- |
| `j` / `Down` | Move selection down in active pane |
| `k` / `Up` | Move selection up in active pane |
| `Tab` | Cycle focus to next pane |
| `BackTab` / `Shift+Tab` | Cycle focus to previous pane |
| `s` | Run or refresh disk storage scan for current selection |
| `o` | Reveal selected profile directory in system file manager |
| `r` | Refresh profile store snapshot from disk |
| `?` | Toggle keybinding help overlay |
| `q` / `Esc` | Quit ProfileMux |

## Feature Status

| Capability | Category | Status | Notes |
| --- | --- | --- | --- |
| Browser Detection | Discovery | Read-only | Detects app bundles, data directories, and channels on macOS |
| Profile Listing | Inspection | Read-only | Reads registered profiles from browser metadata |
| Profile Details | Inspection | Read-only | Shows paths, avatars, registration status, and activity timestamps |
| Storage Breakdown | Inspection | Read-only | Measures core directory, HTTP cache, code cache, and GPU cache |
| Health Doctor | Diagnostics | Read-only | Pure diagnostic analysis of snapshots (missing dirs, orphan caches) |
| Reveal in File Manager | Navigation | Read-only | Opens profile path in system file manager (`open_folder`) |
| Launch Profile | Lifecycle | Not implemented | Planned for future release (`BrowserCapabilities::launch = false`) |
| Create Profile | Mutation | Not implemented | Planned for future release (`BrowserCapabilities::create = false`) |
| Rename Display Name | Mutation | Not implemented | Planned for future release (`BrowserCapabilities::rename_display_name = false`) |
| Rename Directory | Mutation | Not implemented | Planned for future release (`BrowserCapabilities::rename_directory = false`) |
| Clone Profile | Mutation | Not implemented | Planned for future release (`BrowserCapabilities::clone = false`) |
| Delete Profile | Mutation | Not implemented | Planned for future release (`BrowserCapabilities::delete = false`) |
| Cache Cleanup | Maintenance | Not implemented | Planned for future release (`BrowserCapabilities::clean_cache = false`) |
| Custom Avatars | Customization | Not implemented | Planned for future release (`BrowserCapabilities::custom_avatar = false`) |
| Transaction Engine & Rollback | Safety | Not implemented | Planned execution architecture for safe mutation in v0.2+ |
| Firefox Adapter | Integration | Not implemented | No adapter implementation exists |
| Safari Adapter | Integration | Not implemented | Detection only; no adapter implementation exists |

## Privacy

ProfileMux operates exclusively at the filesystem and structural container level.

- ProfileMux **is not** a password extractor, cookie viewer, or session token exporter.
- It **never reads or parses** the internal contents of credential stores (`Login Data`), session databases (`Cookies`), browsing histories (`History`), bookmark databases, or web storage files.
- It inspects only structural metadata (e.g. `Local State` profile lists), directory paths, filesystem timestamps, and disk sizes.
- All operations are strictly local: ProfileMux makes zero network calls and includes no analytics, telemetry, or remote reporting.
