#!/bin/bash
# Build a portable Linux tarball: dist/gravewake-linux-<arch>.tar.gz
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_root"
cargo_bin="$(command -v cargo || true)"
if [[ -z "$cargo_bin" ]]; then cargo_bin="$HOME/.cargo/bin/cargo"; fi
"$cargo_bin" build --release --locked
arch="$(uname -m)"
name="gravewake-linux-$arch"
stage="$project_root/dist/$name"
rm -rf "$stage"
mkdir -p "$stage/fonts" "$stage/audio"
cp target/release/gravewake "$stage/gravewake"
cp assets/linux/gravewake.desktop "$stage/gravewake.desktop"
cp assets/linux/gravewake-256.png "$stage/gravewake.png"
cp assets/fonts/*OFL.txt assets/fonts/SOURCES.md "$stage/fonts/"
cp assets/fonts/GravewakeGothic-Regular.ttf assets/fonts/gravewake-gothic-build.json "$stage/fonts/"
cp assets/audio/SOURCES.md "$stage/audio/SOURCES.md"
cat > "$stage/install.sh" <<'INSTALL'
#!/bin/bash
# Install for the current user under ~/.local; pass --uninstall to remove.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bin="$HOME/.local/bin"
apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
icons="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps"
if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$bin/gravewake" "$apps/gravewake.desktop" "$icons/gravewake.png"
  echo "Removed Gravewake. Saves remain in ${XDG_DATA_HOME:-$HOME/.local/share}/gravewake."
  exit 0
fi
mkdir -p "$bin" "$apps" "$icons"
install -m 755 "$here/gravewake" "$bin/gravewake"
install -m 644 "$here/gravewake.png" "$icons/gravewake.png"
sed "s|^Exec=.*|Exec=$bin/gravewake|" "$here/gravewake.desktop" > "$apps/gravewake.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$apps" || true
echo "Installed Gravewake to $bin/gravewake and added it to your application menu."
INSTALL
chmod +x "$stage/install.sh"
# The binary needs the glibc it was linked against, or newer.
glibc="$(objdump -T "$stage/gravewake" | grep -o 'GLIBC_[0-9.]*' | sort -V | tail -1 | cut -d_ -f2)"
cat > "$stage/README.txt" <<README
Gravewake — The Hollow Tithe (Linux build)

Run ./gravewake from this folder, or ./install.sh to add it to your
application menu (./install.sh --uninstall removes it).

Requires a Vulkan driver (Mesa RADV/ANV or a vendor driver), Wayland or X11,
and ALSA/PipeWire audio (libasound.so.2). This build needs glibc $glibc or
newer; on older distributions, build from source instead.
Saves: \${XDG_DATA_HOME:-~/.local/share}/gravewake/.
Font and audio notices are in fonts/ and audio/.
Source: https://github.com/nearbycoder/gravewake
README
tar -C "$project_root/dist" -czf "$project_root/dist/$name.tar.gz" "$name"
echo "Built dist/$name.tar.gz"
