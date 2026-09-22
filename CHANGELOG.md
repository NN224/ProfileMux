# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Per-profile browser theme control (System, Dark, and Brave Ultra Dark).
- Experimental web-content Force Dark as a ProfileMux launch policy.
- Appearance dialog (`T`) in the TUI for configuring browser theme and web dark mode.
- `pmux profile appearance` CLI command supporting `--theme`, `--web-dark`, `--dry-run`, and `--close-browser`.
- `Browser Theme:` and `Web Dark Mode:` rows in the TUI Details pane and `pmux profile show`.

## [1.1.0] - 2026-09-22

### Added
- Profile account email display in the TUI Details pane and in `pmux profile show`, read from the browser's own `Local State` profile metadata.
- Built-in update checking with `pmux update --check` and `pmux update`.
- Non-blocking update-availability indicator and a `U` action in the TUI.

### Fixed
- Enter now activates the selected button in every TUI dialog; previously the dialog handlers re-read an already-taken dialog and silently did nothing.

### Infrastructure
- Automated macOS release workflow producing architecture-verified binaries and SHA-256 checksums.

## [1.0.0] - 2026-09-21

First release with Chromium profile discovery, inspection, creation, template cloning, renaming, avatars, deletion to Trash, cache cleanup, doctor and dry-run.

[1.1.0]: https://github.com/NN224/ProfileMux/releases/tag/v1.1.0
[1.0.0]: https://github.com/NN224/ProfileMux/releases/tag/v1.0.0
