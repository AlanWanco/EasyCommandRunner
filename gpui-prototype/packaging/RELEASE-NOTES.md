# EasyCommandRunner GPUI v1.0.1

Maintenance update to the GPUI v1.0.0 release. The historical Qt/PyQt branches and Releases are unchanged.

## Changes since v1.0.0

- Windows tray: theme-aware menu with a rounded popup, more actions (show/hide, run/stop, logs, save, settings, theme and quit), keyboard navigation and a native high-contrast fallback. Hover margins, painted borders and last-row scrolling have been adjusted for custom DPI; the rounded Win32 clipping region now uses the actual client-area origin. The Windows x86_64 tray at 115% scaling was manually accepted for the latest fix; other devices, scales and multiple-monitor setups remain subject to user validation.
- Sidebar: in the expanded list, a short marker occupies its own leading slot for every configuration open in the top tab strip (not only the selected tab). All expanded rows reserve the same slot so closing a tab removes only its marker, without shifting icons or labels. The collapsed list hides the markers and centers the SVG icons. Collapsed icons and highlights share a center axis. Hover temporarily opens a panel over the editor without shifting inputs or IME geometry; leaving closes it, while clicking the toggle pins it open. The thin scrollbar overlays the list edge rather than reserving row width.
- Command editing: clicking the option header alphabetically sorts parameters, with enabled and disabled rows sorted separately when enabled-first is active. Sorting changes the command's argument order and does not silently save edits.
- Run logs: while a run is active, an icon beside Stop previews and copies the exact command captured at launch in both docked and detached views. This is not a log export or log-text copy feature.
- Windows shell: quote handling protects literal ampersands inside quoted arguments, including HLS URLs, without changing deliberate shell operators. Process-output decoding still assumes UTF-8; legacy-encoded output from third-party tools may display incorrectly.
- Parameter dragging: fixes a panic when dragging above or below an overflowing parameter list and continues edge scrolling while the pointer is held there; scrolling stops on release.

## Packages and caveats

Five native packages: Windows x86_64/ARM64 Portable ZIP, macOS ARM64 DMG, Linux x86_64/ARM64 AppImage. Windows ZIP is not an installer; extract the whole folder. No external command-line tools (including N_m3u8DL-RE, ffmpeg or Python) are bundled. A source's `data:`/`skd:` key-handling errors are not fixed by this UI release.

macOS requires Apple Silicon and the currently tested macOS 14+ baseline; its ad-hoc signature is not Developer ID signing or notarization. Linux requires Ubuntu 24.04-compatible glibc 2.39+, working display/GPU drivers and desktop tray support. Packages are not signed by a public publisher. Notifications depend on OS permissions and Do Not Disturb. CI tests on Windows ARM64/Linux are not exhaustive physical-device acceptance. Log-text copy and save-to-file controls are not yet implemented.

Back up `config.json` before switching between Qt and GPUI; Qt may discard GPUI-only settings/icons on save. Publication requires every native build, test and package check to pass. Verify downloads against `SHA256SUMS.txt`; each `build-info.json` records the immutable source commit and binary/package hashes. No debug binaries are published.
