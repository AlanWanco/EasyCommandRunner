# EasyCommandRunner GPUI v1.0.0

GPUI becomes the primary implementation. Historical Qt/PyQt branches and Releases remain available and unchanged.

## Highlights

- Native Rust + GPUI Kit UI: configuration sidebar, open tabs, editable command templates, parameter drag ordering and live preview.
- Dark/light modes, accents, English/Chinese, typography and reduced motion. Page changes slide and fade without replacing live input/IME entities.
- Offline SVG picker, random icons and cached custom SVG; explicit Save persists edits and icons.
- Real shell execution, Unicode stdout/stderr, stop controls, system completion notifications and tray close/exit protection.
- Run log defaults to tail-follow. Scrolling up pauses; the bottom-arrow resumes. Detached/reattached views share the pause state.
- Qt-compatible command configurations, backups, restore confirmation and import/export.

## Packages and caveats

Windows x86_64/ARM64 Portable ZIP, macOS ARM64 DMG, Linux x86_64/ARM64 AppImage.

Windows ZIP is not an installer; extract the complete folder. macOS requires Apple Silicon and the currently tested macOS 14+ baseline; its ad-hoc signature is not Developer ID signing or notarization. Linux requires Ubuntu 24.04-compatible glibc 2.39+, system display/GPU drivers and desktop tray support. All packages remain unsigned by a public publisher identity. See the runtime README inside the package before use.

External CLI tools are not included. Back up configuration before switching from Qt; Qt may discard GPUI-only settings when it saves again. Notifications depend on OS permissions/Do Not Disturb. Linux/Windows ARM64 CI checks are not exhaustive physical-device acceptance. Log copy/save-file controls are not yet implemented.

Release is published only after all five native build/test/package jobs succeed. Verify downloads against SHA256SUMS.txt; per-package build-info.json records the immutable source commit and release build input. No debug binaries are published.
