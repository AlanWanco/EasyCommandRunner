#!/usr/bin/env bash
set -euo pipefail
umask 022
cd "$(dirname "$0")/.."
version="${RELEASE_VERSION:?RELEASE_VERSION is required}"
sha="${SOURCE_SHA:?SOURCE_SHA is required}"
binary="${ECR_RELEASE_BINARY:-target/release/easy-command-runner-gpui-prototype}"
dist="${ECR_DIST_DIR:-dist}"
mkdir -p "$dist"
dist="$(cd "$dist" && pwd)"
name="$(python3 scripts/release_support.py name --version "$version" --platform macos --arch arm64)"
[[ "$(uname -m)" == arm64 ]] || { echo 'Native Apple Silicon packaging required' >&2; exit 1; }
file "$binary" | grep -F arm64
otool -L "$binary"
unexpected="$(otool -L "$binary" | tail -n +2 | awk '{print $1}' | grep -Ev '^(/System/Library/|/usr/lib/)' || true)"
[[ -z "$unexpected" ]] || { echo "Unbundled dylibs: $unexpected" >&2; exit 1; }
work="$(mktemp -d "${TMPDIR:-/tmp}/ecr-gpui-dmg.XXXXXX")"
trap 'rm -rf "$work"' EXIT
python3 scripts/release_support.py prepare --version "$version" --platform macos --arch arm64 \
  --source-sha "$sha" --binary "$binary" --output "$work/notes"
app="$work/image/EasyCommandRunner.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/EasyCommandRunner"
chmod 755 "$app/Contents/MacOS/EasyCommandRunner"
cp ../resources/app_icon.icns "$app/Contents/Resources/app_icon.icns"
cp -R "$work/notes/." "$app/Contents/Resources/"
python3 - "$app/Contents/Info.plist" "$version" <<'PY'
import plistlib
import sys
from pathlib import Path
info = {
    'CFBundleName': 'EasyCommandRunner', 'CFBundleDisplayName': 'EasyCommandRunner',
    'CFBundleExecutable': 'EasyCommandRunner', 'CFBundlePackageType': 'APPL',
    'CFBundleIdentifier': 'com.sleepykanata.EasyCommandRunner.GPUI',
    'CFBundleShortVersionString': sys.argv[2], 'CFBundleVersion': sys.argv[2],
    'CFBundleIconFile': 'app_icon.icns', 'LSMinimumSystemVersion': '14.0',
    'NSHighResolutionCapable': True,
}
Path(sys.argv[1]).write_bytes(plistlib.dumps(info))
PY
plutil -lint "$app/Contents/Info.plist"
codesign --force --sign - --identifier com.sleepykanata.EasyCommandRunner.GPUI "$app"
codesign --verify --deep --strict "$app"
ln -s /Applications "$work/image/Applications"
cp "$work/notes/README-runtime.md" "$work/image/README-runtime.md"
hdiutil create -volname "EasyCommandRunner GPUI $version" -srcfolder "$work/image" \
  -ov -format UDZO "$dist/$name"
hdiutil verify "$dist/$name"
# Read-only mount to verify the actual disk image, not merely its staging tree.
mkdir "$work/mount"
hdiutil attach -readonly -nobrowse -mountpoint "$work/mount" "$dist/$name"
set +e
test -x "$work/mount/EasyCommandRunner.app/Contents/MacOS/EasyCommandRunner" && \
  test -f "$work/mount/EasyCommandRunner.app/Contents/Resources/app_icon.icns" && \
  test "$(readlink "$work/mount/Applications")" = /Applications
verified=$?
hdiutil detach "$work/mount"
detached=$?
set -e
[[ "$verified" -eq 0 && "$detached" -eq 0 ]]
python3 scripts/release_support.py stamp --version "$version" --package "$dist/$name" --info "$work/notes/build-info.json"
