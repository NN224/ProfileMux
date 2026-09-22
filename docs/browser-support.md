# Browser Support Matrix

This document defines the browser support matrix, verified test environments, and live validation results for ProfileMux 1.0.0.

## Support Matrix

Columns represent operational capabilities:
- **Detect**: Discovery of application bundle, channel, bundle identifier, and profile root directory.
- **List**: Enumeration of registered profiles from browser metadata.
- **Details**: Metadata extraction (display name, avatar information, account email, last active timestamp, path resolution).
- **Sizes**: Recursive measurement of core profile directory and external caches.
- **Doctor**: Health analysis for orphan directories, missing folders, duplicate names, and parse errors.
- **Launch**: Launching the browser using a specific profile via `--user-data-dir` and `--profile-directory`.
- **Create**: Creating a new profile on disk and in browser metadata.
- **Clone**: Duplicating an existing profile or template with explicit privacy exclusions.
- **Rename name**: Renaming a profile's human-visible display name.
- **Avatar**: Installing a custom profile picture.
- **Rename directory**: Renaming the physical on-disk profile folder.
- **Delete**: Safely moving a profile and its caches to macOS Trash and deregistering from metadata.
- **Clean cache**: Removing verified cache directories while preserving user data and credentials.
- **Browser theme**: Per-profile browser UI colour scheme (System, Dark).
- **Ultra Dark**: Brave-specific darker UI theme variant.
- **Force Dark**: Launch-time automatic darkening of web contents via ProfileMux launch policy.

Values are strictly marked as **Yes**, **No**, **Experimental**, or **Untested**.

| Browser / Channel | Detect | List | Details | Sizes | Doctor | Launch | Create | Clone | Rename name | Avatar | Rename directory | Delete | Clean cache | Browser theme | Ultra Dark | Force Dark |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Brave Browser (Stable)** | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Experimental | Yes | Yes | Yes | Yes | Experimental |
| **Brave Browser Beta** | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Experimental | Yes | Yes | Yes | Yes | Experimental |
| **Brave Browser Nightly** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Google Chrome (Stable)** | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | No | Experimental | Yes | Yes | Yes | No | Experimental |
| **Google Chrome Beta** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Google Chrome Dev** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Google Chrome Canary** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Chromium** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Microsoft Edge (Stable)** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Microsoft Edge Beta** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Microsoft Edge Dev** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Microsoft Edge Canary** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |
| **Vivaldi** | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested | Untested |

### Appearance Preferences and Launch Switches

Browser UI colour scheme is a per-profile preference stored in each profile's `Preferences` file at `browser.theme.color_scheme2` (written together with `browser.theme.follows_system_colors`, without which the browser resets the scheme on the next launch), an integer where `0` is System, `1` is Light, and `2` is Dark (an absent key defaults to System). Brave's darker UI variant (Ultra Dark) is a per-profile preference in the same file at `brave.darker_mode`, a boolean existing only in Brave builds. Setting `brave.darker_mode` alone does not put Brave into dark mode; ProfileMux therefore writes the dark colour scheme (`browser.theme.color_scheme2 = 2`) as well when Ultra Dark is selected.

Web-content Force Dark is not a per-profile preference. Chromium stores `chrome://flags` state in `browser.enabled_labs_experiments` inside `Local State`, which is browser-wide and cannot express a per-profile choice. ProfileMux implements per-profile Force Dark as its own launch policy instead, adding `--enable-features=WebContentsForceDark` when it launches that profile. Consequently, Force Dark applies only to browser windows ProfileMux itself launches, does not require the browser to be closed to change, and windows opened directly from the macOS Dock or Finder will not have it.

## Live Validation

Live validation was performed directly on the development machine. Each browser's real application binary was driven against an isolated temporary user data root created with Chromium's own `--user-data-dir` override. In this manner, no real user profile was touched, and the production safety preflight was never bypassed.

### Verified Installations (macOS)

1. **Brave Browser (Stable)**
   - Version: `153.1.95.104`
   - Bundle ID: `com.brave.Browser`
   - Application Path: `/Applications/Brave Browser.app`
   - User Data Root: `~/Library/Application Support/BraveSoftware/Brave-Browser`
   - Cache Root: `~/Library/Caches/BraveSoftware/Brave-Browser`

2. **Brave Browser Beta**
   - Version: `154.1.97.44`
   - Bundle ID: `com.brave.Browser.beta`
   - Application Path: `/Applications/Brave Browser Beta.app`
   - User Data Root: `~/Library/Application Support/BraveSoftware/Brave-Browser-Beta`
   - Cache Root: `~/Library/Caches/BraveSoftware/Brave-Browser-Beta`

3. **Google Chrome (Stable)**
   - Version: `153.0.8010.52`
   - Bundle ID: `com.google.Chrome`
   - Application Path: `/Applications/Google Chrome.app`
   - User Data Root: `~/Library/Application Support/Google/Chrome`
   - Cache Root: `~/Library/Caches/Google/Chrome`

### Completed Validation Sequence
For each of the three verified installations above, the live validation suite executed and confirmed:
1. **Browser initialization**: Spawning the real binary to initialize an isolated temporary root.
2. **Create profile**: Programmatic creation and registration of a new profile.
3. **Create from template**: Cloning with verified exclusions of private data (cookies, credentials, history, session state).
4. **Launch by real binary**: Spawning the browser into the created profile, followed by verification that the profile remained registered and intact afterwards.
5. **Cache-only cleanup**: Removing the verified cache locations while confirming Cookies, Preferences, History and Login Data survived, with the reclaimed byte count matching the pre-flight estimate.
6. **Display-name rename**: Updating the display name while verifying the underlying directory name remained unchanged.
7. **Doctor diagnostics**: Running health doctor with zero broken findings reported.
8. **Delete to Trash**: Moving profile directories to `~/.Trash` with metadata deregistration.
9. **Running browser guard**: Verifying that mutation operations are strictly refused while a browser instance holding the target user data root is active.

### Custom Avatar Validation
- Custom avatar installation was live-validated on **Brave Browser (Stable)** and **Brave Browser Beta**.
- **Google Chrome (Stable)** has full, live-validated Chromium profile management except the custom avatar, which the Chrome adapter deliberately does not claim and which was therefore never validated. The capability is not claimed for Chrome.

### Implemented and Not Locally Validated Browsers
Static definitions, bundle IDs, and data path mappings exist in `src/browsers/chromium/discovery.rs` for:
- **Chromium**
- **Microsoft Edge** (Stable, Beta, Dev, Canary)
- **Vivaldi**
- **Brave Browser Nightly**
- **Google Chrome** (Beta, Dev, Canary)

These browsers have discovery definitions and share the same adapter code but are not installed on the validation machine, so they are implemented and not locally validated — never verified, never supported.

### Non-Chromium Browsers
There is no adapter for Firefox or Safari; neither is implemented.

## Support Level Definitions

The `SupportLevel` enum (`src/domain/browser.rs`) categorizes runtime support for an installation:

1. **`SupportLevel::Full`**
   - Full support for discovery, profile inspection, disk measurement, doctor diagnostics, launching, and safe mutations (create, clone, rename display name, delete, clean cache).
   - Assigned to verified Chromium browsers (Brave Stable, Brave Beta, Chrome Stable).

2. **`SupportLevel::Partial`**
   - Discovery and inspection work, but only a subset of mutations are enabled due to browser capability limitations or experimental status.

3. **`SupportLevel::ReadOnly`**
   - Discovery, listing, metadata extraction, storage measurement, and doctor checks are available. Structural mutations are prohibited.

4. **`SupportLevel::Unsupported`**
   - The browser is recognized or detected, but no operational adapter exists. All operations return `Error::Unsupported`.
