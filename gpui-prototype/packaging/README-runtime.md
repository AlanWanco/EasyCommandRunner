# EasyCommandRunner GPUI runtime notes

This package contains the Rust/GPUI application, not the historical Qt version.
No command-line tools (ffmpeg, yt-dlp, Python, etc.) are bundled. Install the tools
used by your configurations separately and ensure their paths are correct.

- Windows x86_64/ARM64: Windows 10/11 with working DirectX graphics drivers. Unzip
  the entire directory and run EasyCommandRunner.exe. ARM64 requires native ARM64
  Windows. The Portable ZIP is not an installer. App settings still use APPDATA,
  unless ECR_DATA_DIR is set. Microsoft system UCRT components are not bundled.
- macOS ARM64: Apple Silicon, macOS 14 or later for the currently tested baseline.
  Drag EasyCommandRunner.app from the DMG to Applications. It has an ad-hoc
  integrity signature, not a Developer ID signature or Apple notarization.
  Gatekeeper may require an explicit user decision. Do not disable system security.
- Linux x86_64/ARM64: Ubuntu 24.04 or a compatible system with glibc >= 2.39,
  X11 or Wayland, and working graphics drivers. chmod +x the AppImage before use.
  AppImage does not bundle glibc or your GPU/display driver. If FUSE is unavailable,
  use APPIMAGE_EXTRACT_AND_RUN=1. GTK/AppIndicator runtime libraries are staged in
  the AppImage; tray availability still depends on the desktop/status extension.

System notifications depend on OS permission and Do Not Disturb settings. On macOS
launch the .app, not a binary copied out of it. Linux/Windows ARM64 have CI tests,
not a claim of exhaustive physical-device acceptance.

Configuration is stored in the system user-data directory, or ECR_DATA_DIR. Click
Save / Ctrl+S (Cmd+S on macOS) to persist command edits and tab icons. Switching from
Qt can read its compatible command data; saving again from Qt may discard GPUI-only
settings/icons. Back up your configuration before switching application versions.

The application's MIT license and icon notices are included. Rust dependency license
metadata/source URLs are included in dependency-notices.json when built in CI.
Dynamic Linux system libraries retain their respective licenses; packaged notices
are under usr/share/doc inside the AppImage. SHA256SUMS.txt and the package-specific
build-info.json accompany every Release. The latter pins the source commit and binary
hash. All packages are unsigned by a public publisher identity.
