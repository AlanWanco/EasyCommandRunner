#!/usr/bin/env bash
set -euo pipefail
umask 022
cd "$(dirname "$0")/.."
version="${RELEASE_VERSION:?RELEASE_VERSION is required}"
sha="${SOURCE_SHA:?SOURCE_SHA is required}"
arch="${1:?Use x86_64 or arm64}"
case "$arch:$(uname -m)" in x86_64:x86_64) deploy_arch=x86_64 ;; arm64:aarch64) deploy_arch=aarch64 ;; *) echo 'Native matching Linux architecture required' >&2; exit 1 ;; esac
binary="${ECR_RELEASE_BINARY:-target/release/easy-command-runner-gpui-prototype}"
dist="${ECR_DIST_DIR:-dist}"
mkdir -p "$dist"
dist="$(cd "$dist" && pwd)"
name="$(python3 scripts/release_support.py name --version "$version" --platform linux --arch "$arch")"
work="$(mktemp -d "${TMPDIR:-/tmp}/ecr-gpui-appimage.XXXXXX")"
trap 'rm -rf "$work"' EXIT
appdir="$work/AppDir"
python3 scripts/release_support.py prepare --version "$version" --platform linux --arch "$arch" \
  --source-sha "$sha" --binary "$binary" --output "$work/notes"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/doc/EasyCommandRunner" "$appdir/usr/lib/gdk-pixbuf-loaders"
cp "$binary" "$appdir/usr/bin/EasyCommandRunner"
chmod 755 "$appdir/usr/bin/EasyCommandRunner"
cp -R "$work/notes/." "$appdir/usr/share/doc/EasyCommandRunner/"
cat > "$work/EasyCommandRunner.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=EasyCommandRunner
Comment=Rust and GPUI command runner
Exec=EasyCommandRunner
Icon=EasyCommandRunner
Categories=Utility;
Terminal=false
X-AppImage-Version=$version
EOF
# Fetch tool metadata through the authenticated build API and verify the exact
# asset digest before executing it. Record the tool identity in the package.
# continuous is moving: source is pinned, tool digest is recorded, not falsely
# advertised as a deterministic toolchain across future rebuilds.
gh api repos/linuxdeploy/linuxdeploy/releases/tags/continuous > "$work/linuxdeploy-release.json"
python3 - "$work/linuxdeploy-release.json" "$deploy_arch" "$work/tool.env" <<'PY'
import json
import sys
from pathlib import Path
release = json.loads(Path(sys.argv[1]).read_text())
asset = next(a for a in release['assets'] if a['name'] == f'linuxdeploy-{sys.argv[2]}.AppImage')
digest = asset.get('digest') or ''
if not digest.startswith('sha256:') or len(digest) != 71:
    raise SystemExit('linuxdeploy asset has no verifiable SHA256 digest; refusing to execute')
Path(sys.argv[3]).write_text(f"{asset['browser_download_url']}\n{digest[7:]}\n")
PY
url="$(sed -n '1p' "$work/tool.env")"
expected="$(sed -n '2p' "$work/tool.env")"
curl --fail --location --retry 3 --proto '=https' --proto-redir '=https' "$url" --output "$work/linuxdeploy"
printf '%s  %s\n' "$expected" "$work/linuxdeploy" | sha256sum --check -
chmod +x "$work/linuxdeploy"
export APPIMAGE_EXTRACT_AND_RUN=1
"$work/linuxdeploy" --help > "$work/linuxdeploy-help.txt"
for option in --library --executable --icon-filename --custom-apprun --output; do
  grep -F -- "$option" "$work/linuxdeploy-help.txt" >/dev/null || {
    echo "Downloaded linuxdeploy does not support $option" >&2; exit 1;
  }
done
cp "$work/linuxdeploy-release.json" "$appdir/usr/share/doc/EasyCommandRunner/linuxdeploy-build-tool.json"
# AppIndicator and pixbuf image loaders use dlopen; NEEDED cannot discover them.
multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
indicator="$(readlink -f "/usr/lib/$multiarch/libayatana-appindicator3.so.1")"
test -f "$indicator"
loader_dir="/usr/lib/$multiarch/gdk-pixbuf-2.0/2.10.0/loaders"
query="/usr/lib/$multiarch/gdk-pixbuf-2.0/gdk-pixbuf-query-loaders"
test -x "$query"
# Force GTK runtime libraries too; an exclusion policy must not silently turn
# this into a package requiring the host's GTK installation.
args=(--library "$indicator" --library "/usr/lib/$multiarch/libgtk-3.so.0" \
  --library "/usr/lib/$multiarch/libgdk-3.so.0" --executable "$query")
for loader in "$loader_dir/"*.so; do
  test -f "$loader"
  cp "$loader" "$appdir/usr/lib/gdk-pixbuf-loaders/"
  args+=(--library "$loader")
done
"$work/linuxdeploy" --appdir "$appdir" --executable "$appdir/usr/bin/EasyCommandRunner" \
  --desktop-file "$work/EasyCommandRunner.desktop" --icon-file ../resources/app_icon.png \
  --icon-filename EasyCommandRunner "${args[@]}"
test -f "$appdir/usr/share/applications/EasyCommandRunner.desktop"
test -n "$(find "$appdir/usr/share/icons/hicolor" -name EasyCommandRunner.png -print -quit)"
test -f "$appdir/usr/lib/libayatana-appindicator3.so.1"
test -f "$appdir/usr/lib/libgtk-3.so.0"
test -f "$appdir/usr/lib/libgdk-3.so.0"
mkdir -p "$appdir/usr/share/glib-2.0/schemas"
cp /usr/share/glib-2.0/schemas/*.xml "$appdir/usr/share/glib-2.0/schemas/"
glib-compile-schemas "$appdir/usr/share/glib-2.0/schemas"
# Keep Debian copyright/source notices for every staged shared library, including
# dlopen loaders absent from ldconfig. Handle usr-merge and :architecture owners.
notices="$appdir/usr/share/doc/EasyCommandRunner/linux-dependency-packages.tsv"
printf 'binary-package\tversion\tsource-package\tsource-version\n' > "$notices"
for lib in "$appdir/usr/lib/"*.so* "$appdir/usr/lib/gdk-pixbuf-loaders/"*.so "$query"; do
  original="$(find "/usr/lib/$multiarch" "/lib/$multiarch" "$loader_dir" "$(dirname "$query")" \
    -maxdepth 1 -name "$(basename "$lib")" -print -quit)"
  [[ -n "$original" ]] || { echo "No system origin for bundled library: $lib" >&2; exit 1; }
  original="$(readlink -f "$original")"
  owner="$(dpkg-query -S "$original" | awk -F ': /' 'NR == 1 {print $1}')"
  package="${owner%%:*}"
  test -f "/usr/share/doc/$package/copyright" || { echo "Missing license notice: $package" >&2; exit 1; }
  if [[ ! -d "$appdir/usr/share/doc/$package" ]]; then
    mkdir -p "$appdir/usr/share/doc/$package"
    cp "/usr/share/doc/$package/copyright" "$appdir/usr/share/doc/$package/"
    dpkg-query -W -f='${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\n' "$owner" >> "$notices"
  fi
done
cp packaging/AppRun "$work/AppRun"
chmod +x "$work/AppRun"
# Use a separate source path: copying AppRun onto itself can fail in deployers.
(cd "$work" && OUTPUT="$name" "$work/linuxdeploy" --appdir "$appdir" --output appimage --custom-apprun "$work/AppRun")
test -f "$work/$name"
cp "$work/$name" "$dist/$name"
chmod +x "$dist/$name"
# Extract the final AppImage for structural and dynamic dependency verification.
(cd "$work" && env -u APPIMAGE_EXTRACT_AND_RUN "$dist/$name" --appimage-extract >/dev/null)
extracted="$work/squashfs-root"
test -x "$extracted/AppRun"
cmp -s "$work/AppRun" "$extracted/AppRun"
test -x "$extracted/usr/bin/EasyCommandRunner"
test -x "$extracted/usr/bin/gdk-pixbuf-query-loaders"
test -f "$extracted/usr/lib/libayatana-appindicator3.so.1"
test -f "$extracted/usr/lib/libgtk-3.so.0"
test -f "$extracted/usr/lib/libgdk-3.so.0"
for dependency in "$extracted/usr/bin/EasyCommandRunner" "$extracted/usr/lib/libayatana-appindicator3.so.1" "$extracted/usr/lib/gdk-pixbuf-loaders/"*.so; do
  LD_LIBRARY_PATH="$extracted/usr/lib" ldd "$dependency" > "$work/dependencies.txt"
  if grep -F 'not found' "$work/dependencies.txt"; then
    echo "Packaged dependency has missing shared libraries: $dependency" >&2; exit 1
  fi
done
if objdump -p "$extracted/usr/bin/EasyCommandRunner" | grep -E 'NEEDED[[:space:]]+libQt'; then
  echo 'Qt dependency in GPUI package' >&2; exit 1
fi
python3 scripts/release_support.py stamp --version "$version" --package "$dist/$name" --info "$work/notes/build-info.json"
