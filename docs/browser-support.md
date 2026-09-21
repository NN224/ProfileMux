# Browser Support Matrix

This document defines the current browser support matrix, verified test environments, and support tier definitions for ProfileMux.

## Support Matrix

Columns represent operational capabilities:
- **Detect**: Discovery of application bundle, channel, and profile root directory.
- **List profiles**: Enumeration of registered profiles from browser metadata.
- **Details**: Metadata extraction (display name, avatar type, last active time, path resolution).
- **Sizes**: Recursive measurement of core profile directory and external caches.
- **Doctor**: Read-only health analysis for orphan directories, missing folders, and parse errors.
- **Launch**: Launching the browser using a specific profile.
- **Create**: Creating a new profile on disk and in browser metadata.
- **Rename name**: Renaming a profile's human-visible display name.
- **Rename directory**: Renaming the underlying on-disk profile folder.
- **Clone**: Duplicating an existing profile into a new profile directory.
- **Delete**: Safely removing a profile directory and its metadata entries.

Values are strictly marked as **Yes**, **No**, or **Untested**.

| Browser / Channel | Detect | List profiles | Details | Sizes | Doctor | Launch | Create | Rename name | Rename directory | Clone | Delete |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Brave Browser (Stable)** | Yes | Yes | Yes | Yes | Yes | No | No | No | No | No | No |
| **Brave Browser Beta** | Yes | Yes | Yes | Yes | Yes | No | No | No | No | No | No |
| **Brave Browser Nightly** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Google Chrome (Stable)** | Yes | Yes | Yes | Yes | Yes | No | No | No | No | No | No |
| **Google Chrome Beta** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Google Chrome Dev** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Google Chrome Canary** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Chromium** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Microsoft Edge (Stable)** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Microsoft Edge Beta** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Microsoft Edge Dev** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Microsoft Edge Canary** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Vivaldi** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Opera** | Untested | Untested | Untested | Untested | Untested | No | No | No | No | No | No |
| **Mozilla Firefox (Stable)** | No | No | No | No | No | No | No | No | No | No | No |
| **Mozilla Firefox Developer**| No | No | No | No | No | No | No | No | No | No | No |
| **Mozilla Firefox Nightly** | No | No | No | No | No | No | No | No | No | No | No |
| **Apple Safari** | Untested | No | No | No | No | No | No | No | No | No | No |

## Tested-On Verification (macOS)

The following installations were directly verified on the macOS development environment at time of writing:

1. **Brave Browser (Stable)**
   - Version: `153.1.95.104`
   - Bundle ID: `com.brave.Browser`
   - Application Path: `/Applications/Brave Browser.app`
   - User Data Root: `~/Library/Application Support/BraveSoftware/Brave-Browser`
   - Cache Root: `~/Library/Caches/BraveSoftware/Brave-Browser`
   - Status: Detected and read successfully.

2. **Brave Browser Beta**
   - Version: `154.1.97.44`
   - Bundle ID: `com.brave.Browser.beta`
   - Application Path: `/Applications/Brave Browser Beta.app`
   - User Data Root: `~/Library/Application Support/BraveSoftware/Brave-Browser-Beta`
   - Cache Root: `~/Library/Caches/BraveSoftware/Brave-Browser-Beta`
   - Status: Detected and read successfully. Brave Stable and Brave Beta maintain entirely independent user data and cache stores.

3. **Google Chrome (Stable)**
   - Version: `153.0.8010.52`
   - Bundle ID: `com.google.Chrome`
   - Application Path: `/Applications/Google Chrome.app`
   - User Data Root: `~/Library/Application Support/Google/Chrome`
   - Cache Root: `~/Library/Caches/Google/Chrome`
   - Status: Detected and read successfully.

### Supported-But-Untested Installations

Definitions and default path mappings exist in the codebase for **Chromium**, **Microsoft Edge** (Stable, Beta, Dev, Canary), **Vivaldi**, and **Brave Nightly**. On the development machine, residual data directories were observed for some of these browsers, but no valid application bundles were installed. Accordingly, they are classified as **Supported-but-untested** rather than verified.

### Non-Chromium Browsers

- **Mozilla Firefox** (Stable, Developer Edition, Nightly): Not installed on the development machine. No Firefox adapter currently exists in the codebase; all operations are marked **No**.
- **Apple Safari**: Conservative detection may locate Safari, but no adapter exists. Mutation will never be supported for Safari unless explicitly designed and verified due to macOS sandbox constraints and proprietary profile storage.

## Chromium Profile Storage Notes

For all Chromium-based browsers on macOS:
- Profile registration and metadata live in `<user data root>/Local State` as a JSON document under the key `profile.info_cache`.
- Entries in `profile.info_cache` are keyed by the physical on-disk directory name (e.g. `"Default"`, `"Profile 1"`).
- The human-readable profile label is stored in the mutable `name` field within each directory entry.
- On macOS, per-profile HTTP and code caches are located outside the profile directory in `<cache root>/<ProfileDirectory>/Cache` and `<cache root>/<ProfileDirectory>/Code Cache`. ProfileMux scans both locations when calculating storage breakdowns.

## Support Level Definitions

The `SupportLevel` enum (`src/domain/browser.rs`) categorizes what ProfileMux can perform for an installation:

1. **`SupportLevel::Full`**
   - Complete support for discovery, read-only inspection, disk size breakdown, health doctor diagnostics, and the upcoming transaction-safe mutation operations (create, rename, clone, delete, cache clean).
   - *Target state for verified browsers in v0.2+.*

2. **`SupportLevel::Partial`**
   - Discovery, profile inspection, and health diagnostics work reliably, but only a subset of mutation operations are supported due to browser limitations or partial testing.

3. **`SupportLevel::ReadOnly`**
   - Discovery, profile listing, metadata extraction, recursive storage measurements, and health doctor checks are supported. All mutation operations are prohibited and return `Error::Unsupported`.
   - *Current operational status for verified Chromium browsers in v0.1.*

4. **`SupportLevel::Unsupported`**
   - A browser definition or bundle identifier is recognized by ProfileMux, but no operational adapter exists or the browser platform is fundamentally incompatible. All profile queries and mutations return `Error::Unsupported`.
