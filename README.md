# ProfileMux (`pmux`)

ProfileMux is a local-first Chromium browser profile manager for macOS, written in Rust with a Ratatui terminal user interface (TUI) and a clap-derive command-line interface (CLI) built on a single shared core library. It provides centralized profile discovery, metadata inspection, disk and cache footprint analysis, structural health diagnostics, profile launching, and transaction-safe profile mutations across Chromium-family browser installations.

## Status

ProfileMux is at **v1**. It supports full Chromium-family profile management on macOS.

### Implemented and Live-Validated
The following capabilities are implemented and live-validated on macOS:
- Browser discovery by application bundle identifier
- Profile listing and metadata inspection
- Recursive disk and cache size measurement (core directory, HTTP cache, code cache, GPU cache)
- Health doctor diagnostics (`pmux doctor`)
- Profile launching (`pmux profile launch`)
- Profile creation (`pmux profile create`)
- Profile creation from a template with an explicit clone policy
- Profile cloning (`pmux profile clone`)
- Display-name renaming (`pmux profile rename --name <NAME>`)
- Custom profile avatar assignment (`pmux profile avatar`)
- Safe profile deletion to the macOS Trash (`pmux profile delete`)
- Cache-only cleanup (`pmux cache clean`)
- Dry-run simulation for every structural operation (`--dry-run`)
- Rollback-capable filesystem transaction engine (`Transaction`)
- Full CLI command suite
- Three-pane TUI with interactive modal dialogs for every action

### Experimental Capabilities
Marked experimental in `BrowserCapabilities`:
- **Custom profile avatar**: Supported and validated on Brave Browser and Brave Browser Beta only. Chrome does not claim this capability.
- **Profile directory renaming**: Implemented for all Chromium adapters via `pmux profile rename --directory <DIR>`, but marked experimental due to deep internal path references in extension and browser state.
- **Extension copying during template creation or clone**: The default policy is `none` (no extensions copied). Two optional policies exist: `copy` (copies installed extension payloads from `Extensions`) and `copy-settings` (copies `Extensions`, `Local Extension Settings`, and `Sync Extension Settings`). However, Chromium signs extension registrations in `Secure Preferences` with a per-profile message authentication code (MAC). ProfileMux deliberately does not forge or recompute these signatures, so copied extensions may be detected as tampered with and dropped by the browser on next launch.

### Not Implemented and Out of Scope
- Firefox and Safari adapters
- Windows and Linux support
- Permanent (non-Trash) deletion
- Configuration files
- Plugin systems
- Standalone audit-log subsystems

## Installation and Building

Building ProfileMux requires Rust 1.80 or later.

```bash
cargo build --release
./target/release/pmux --help
```

## CLI Usage

Running `pmux` without subcommands opens the interactive TUI. Subcommands provide scriptable and terminal-based inspection and mutation.

### Profile Selectors
Commands that accept a `<selector>` argument resolve targets in the following order:
1. Full `ProfileId` (e.g. `brave:~/Library/Application Support/BraveSoftware/Brave-Browser/Default`)
2. Browser slug and directory (e.g. `brave/Default`, `chrome/Profile 1`)
3. Profile display name (e.g. `Personal`, `Work`)

If a display name is ambiguous across installed browsers or profiles, ProfileMux aborts and lists all matching candidates instead of guessing.

### Discover installed browsers

```bash
pmux browsers
```

Sample output:
```text
NAME                     CHANNEL  SLUG        VERSION         BUNDLE ID                  USER DATA ROOT                                                    PROFILES  SUPPORT
Brave Browser            Stable   brave       153.1.95.104    com.brave.Browser          /Users/alice/Library/Application Support/BraveSoftware/Brave-Browser   3         Full
Brave Browser Beta       Beta     brave-beta  154.1.97.44     com.brave.Browser.beta     /Users/alice/Library/Application Support/BraveSoftware/Brave-Browser-Beta 1      Full
Google Chrome            Stable   chrome      153.0.8010.52   com.google.Chrome          /Users/alice/Library/Application Support/Google/Chrome             2         Full
```

### List profiles

```bash
pmux profiles --sizes
```

Sample output:
```text
BROWSER     DISPLAY NAME    DIRECTORY    STATUS  SIZE
brave       Personal        Default      [✓]     1.42 GB
brave       Work            Profile 1    [✓]     824.10 MB
brave       Research        Profile 2    [✓]     312.45 MB
brave-beta  Beta Tester     Default      [✓]     450.20 MB
chrome      Default         Default      [✓]     2.10 GB
chrome      Client Demo     Profile 1    [✓]     510.30 MB
```

### Show profile details

```bash
pmux profile show brave/Default
```

Sample output:
```text
Display Name:      Personal
Directory:         Default
Browser:           Brave Browser (Stable)
Absolute Path:     /Users/alice/Library/Application Support/BraveSoftware/Brave-Browser/Default
Cache Path:        /Users/alice/Library/Caches/BraveSoftware/Brave-Browser/Default
Avatar:            icon: chrome://theme/IDR_PROFILE_AVATAR_26, custom picture: no
Last Active:       1726857131
Registered:        yes
Directory Exists:  yes
Running:           no
Sizes:
  Total:           1.42 GB
  Core:            1.18 GB
  Cache:           184.20 MB
  Code Cache:      42.10 MB
  GPU Cache:       15.30 MB
```

### Launch a profile

```bash
pmux profile launch brave/Default
```

### Reveal profile folder in file manager

```bash
pmux profile open brave/Default
```

### Create a new profile

Create an empty profile:
```bash
pmux profile create --browser brave --name "Staging"
```

Create a profile using an existing profile as a configuration template:
```bash
pmux profile create --browser brave --name "Testing" --template "Personal" --extensions none
```

Supported flags:
- `--browser <slug>`: Target browser identifier (`brave`, `chrome`, etc.)
- `--name <NAME>`: Profile display name
- `--directory <DIR>`: Custom directory name (sanitized automatically if omitted)
- `--template <NAME-or-DIR>`: Source profile to copy preferences from
- `--avatar <PATH>`: Path to image file for custom avatar (Brave only)
- `--extensions <none|copy|copy-settings>`: Extension copy policy (default: `none`)
- `--open`: Launch browser with the new profile immediately after creation
- `--dry-run`: Output the execution plan without modifying disk
- `--close-browser`: Gracefully quit running browser instances via AppleScript before mutating

### Clone an existing profile

```bash
pmux profile clone brave/Default --name "Personal Copy"
```

Supported flags match `profile create`, replacing `--browser` and `--template` with the positional `<selector>` argument for the source profile.

### Dry-run inspection

Passing `--dry-run` to any mutation command produces the rendered `OperationPlan`:

```bash
pmux profile clone brave/Default --name "Staging" --dry-run
```

Output:
```text
CLONE PROFILE

Browser:   Brave Browser (Stable)
Profile:   Staging

Steps:
  Create profile directory: /Users/alice/Library/Application Support/BraveSoftware/Brave-Browser/Staging
  Register profile `Staging` in Local State info_cache and profiles_order
  Copy allowed configuration from source profile `Default`

Copy:
  Preferences

Exclude:
  Bookmarks
  Extensions
  Cookies
  Login Data
  History
  Sessions
  Web Data
  Network state
  Account identity (GAIA)
  Local Storage
  Service Worker
  Top Sites

Requires the browser to be fully closed.
No files changed.
```

### Rename a profile

Rename display name only (instant, browser may remain open):
```bash
pmux profile rename brave/Default --name "Primary Work"
```

Rename profile directory on disk (experimental, requires browser closed):
```bash
pmux profile rename brave/Profile-1 --directory "Work" --close-browser
```

### Assign custom avatar (Brave only)

```bash
pmux profile avatar brave/Default ~/Pictures/avatar.png
```

Scales images down to 256x256 PNG format, installs `Google Profile Picture.png` inside the profile folder, and configures `Local State` avatar fields.

### Delete a profile

Safely moves the profile folder and external cache directories to `~/.Trash` and deregisters the profile from `Local State`:

```bash
pmux profile delete brave/Profile-2
```

Prompt:
```text
DELETE PROFILE

Browser:   Brave Browser (Stable)
Profile:   Research

Steps:
  Send profile directory to Trash: /Users/alice/Library/Application Support/BraveSoftware/Brave-Browser/Profile 2
  Send cache directory to Trash: /Users/alice/Library/Caches/BraveSoftware/Brave-Browser/Profile 2
  Remove profile `Profile 2` from Local State info_cache and profiles_order

Reclaimed: 312.45 MB

Requires the browser to be fully closed.
Delete profile 'Research'? [y/N]: 
```

To skip interactive confirmation in automation:
```bash
pmux profile delete brave/Profile-2 --yes --close-browser
```

### Clean cache directories

Removes HTTP cache, code cache, GPU cache, and service worker cache storage without touching user data or credentials:

```bash
pmux cache clean brave/Default --dry-run
pmux cache clean brave/Default --yes
```

### Run health doctor

```bash
pmux doctor
```

Sample output:
```text
Brave Browser (Stable)
  ✓ Healthy
Brave Browser Beta (Beta)
  ! [orphan-cache-dir] Cache directory exists for unregistered profile
      /Users/alice/Library/Caches/BraveSoftware/Brave-Browser-Beta/Profile 2
Google Chrome (Stable)
  ✓ Healthy
Summary: 0 broken, 1 warning across 3 browser(s)
```

## Terminal User Interface (TUI)

Launching `pmux` without subcommands opens the interactive three-pane dashboard:

1. **Left pane (Browsers)**: Lists detected browser installations, release channels, support status, and profile counts.
2. **Center pane (Profiles)**: Lists profiles for the active browser with display names, directory names, and health status indicators.
3. **Right pane (Details)**: Shows comprehensive profile metadata, process status, storage breakdowns, and doctor findings.

### Action Bar
The bottom action bar displays available operations (actions unsupported by the active browser are dimmed):
```text
N New   C Clone   R Rename   D Delete   L Launch   A Avatar   O Folder   X Clean   H Doctor   / Search   ? Help   Q Quit
```

### Keybindings

| Key | Action |
| --- | --- |
| `Up` / `k` | Move selection up in active pane |
| `Down` / `j` | Move selection down in active pane |
| `Tab` | Cycle focus forward (Browsers -> Profiles -> Details) |
| `Shift+Tab` / `BackTab` | Cycle focus backward |
| `h` / `Left` | Focus left pane |
| `l` / `Right` | Focus right pane |
| `Enter` / `L` | Launch selected profile |
| `i` | Open fullscreen profile details overlay |
| `b` | Open fullscreen browser details overlay |
| `F5` | Re-scan storage sizes for selected profile |
| `N` | Open New Profile dialog |
| `C` | Open Clone Profile dialog |
| `R` | Open Rename Profile dialog |
| `D` | Open Delete Profile confirmation dialog |
| `A` | Open Set Avatar dialog |
| `O` | Reveal profile folder in Finder |
| `X` | Open Clean Cache confirmation dialog |
| `H` | View Doctor health findings overlay |
| `/` | Filter profiles by name or directory (fuzzy subsequence matching) |
| `Esc` | Clear filter / close modal dialog or overlay |
| `?` | Toggle keybinding help overlay |
| `q` / `Ctrl-C` | Quit ProfileMux |

## Feature Status

| Capability | Category | Status | Notes |
| --- | --- | --- | --- |
| Browser Detection | Discovery | Stable | Detects macOS Chromium application bundles, bundle IDs, channels, and roots |
| Profile Listing | Inspection | Stable | Reads registered profiles from `Local State` |
| Profile Details | Inspection | Stable | Displays paths, avatars, registration state, process state, and timestamps |
| Storage Breakdown | Inspection | Stable | Measures core directory, HTTP cache, code cache, and GPU cache sizes |
| Health Doctor | Diagnostics | Stable | Identifies missing directories, orphan caches, duplicate IDs, and corruption |
| Profile Launching | Lifecycle | Stable | Spawns browser with `--user-data-dir` and `--profile-directory` |
| Profile Creation | Mutation | Stable | Allocates directory, writes profile metadata, registers in `Local State` |
| Template Profile Creation | Mutation | Stable | Copies preferences and settings with private data excluded |
| Profile Cloning | Mutation | Stable | Clones profile under new identity with explicit copy policy |
| Rename Display Name | Mutation | Stable | Updates display name in `Local State`; directory remains untouched |
| Delete Profile | Mutation | Stable | Moves profile and cache directories to `~/.Trash` and deregisters |
| Cache Cleanup | Maintenance | Stable | Prunes verified cache directories; preserves credentials, history, cookies |
| Custom Profile Avatar | Customization | Experimental | Supported on Brave and Brave Beta only; Chrome does not claim it |
| Rename Profile Directory | Mutation | Experimental | Renames on-disk folder and updates `Local State` references |
| Extension Copying | Mutation | Experimental | Copies extension files; Chromium `Secure Preferences` MAC may drop them |
| Rollback Transactions | Safety | Stable | `Transaction` records inverse operations; rolls back on error or `Drop` |
| Running Browser Guard | Safety | Stable | Prevents structural writes while browser holds root open; graceful quit |
| Firefox Adapter | Integration | Not implemented | Out of scope |
| Safari Adapter | Integration | Not implemented | Out of scope |
| Windows / Linux | Platform | Not implemented | Out of scope |
| Permanent Deletion | Safety | Not implemented | Out of scope; ProfileMux only moves to Trash |

## Privacy

ProfileMux operates strictly at the structural container level.

- ProfileMux **is not** a password extractor, cookie viewer, or session-token exporter.
- It **never reads or parses** the internal contents of credential databases (`Login Data`), session cookies (`Cookies`), browsing histories (`History`), bookmark databases, or web storage stores.
- It reports names, paths, sizes, and metadata only.
- All operations are completely local: ProfileMux makes zero network calls and includes no analytics, telemetry, or remote crash reporting.
