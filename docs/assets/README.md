# Assets

| File | Role |
| --- | --- |
| `profilemux-tui.png` | The screenshot used in `README.md`. 2320x1244 (2x), PNG for maximum rendering compatibility — GitHub notes that SVG images can fail to render for some Firefox users. |
| `profilemux-tui.svg` | The vector source of the same capture, kept for regeneration and for higher-fidelity viewing. |

Both are the same capture: the terminal output of `pmux` running against this
machine's actual browser installations, replayed into a static image.

The capture is sanitized. Profile display names and the home directory are
replaced with neutral placeholders (`Master`, `Work`, `Client A`,
`/Users/owner`) so no real account, client or path is published. Everything
else — the layout, panes, glyphs, storage figures, health status and action
bar — is the application's own output, unmodified.
