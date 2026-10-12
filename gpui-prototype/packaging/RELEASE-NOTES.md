# EasyCommandRunner GPUI v1.1.0

Quality-of-life update to the GPUI release. The historical Qt/PyQt branches and published GPUI v1.0.1 Release are unchanged.

## Changes since v1.0.1

- Configuration editing now auto-saves the complete workspace after a short idle debounce, including all command tabs, sidebar groups, ordering, layout and applied preferences. Applying settings saves the complete configuration; pending edits are flushed before exit. Save failures remain visible and do not silently discard changes. The manual Save action remains available.
- Sidebar organization: ungrouped configurations appear alongside folders; groups and configurations can be reordered or moved between groups. Row-local selection and hover highlights stay attached to their actual rows, including when the list does not scroll.
- Drag-and-switch polish: clearer sidebar drop targets and lower-cost page switching.
- Run logs retain each run's launch-command snapshot after completion, so it can still be copied from docked or detached logs without copying output text.
- Pasting a file path into a command field places the caret at the end and scrolls long paths into view.

## Packages and caveats

Five native packages: Windows x86_64/ARM64 Portable ZIP, macOS ARM64 DMG, Linux x86_64/ARM64 AppImage. Windows ZIP is not an installer; extract the whole folder. No external command-line tools (including N_m3u8DL-RE, ffmpeg or Python) are bundled.

Windows CI and offscreen UI tests are not a substitute for physical-device acceptance. Linux requires Ubuntu 24.04-compatible glibc 2.39+, working display/GPU drivers and desktop tray support. macOS requires Apple Silicon; its ad-hoc signature is not Developer ID signing or notarization. Packages are not signed by a public publisher. Notifications depend on OS permissions and Do Not Disturb.

Third-party tools that emit legacy-encoded output may still display incorrectly. N_m3u8DL-RE `data:`/`skd:` key-handling errors are unrelated to these UI changes and are not fixed here. Back up `config.json` before switching between Qt and GPUI; Qt may discard GPUI-only settings and icons when saving. Verify downloads against `SHA256SUMS.txt`; each `build-info.json` records the immutable source commit and binary/package hashes. No debug binaries are published.
