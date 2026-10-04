#!/bin/zsh
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_root"
cargo_bin="$(command -v cargo || true)"
if [[ -z "$cargo_bin" ]]; then cargo_bin="$HOME/.cargo/bin/cargo"; fi
"$cargo_bin" build --release --locked
bundle="$project_root/dist/Gravewake.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
mkdir -p "$bundle/Contents/Resources/fonts"
cp assets/fonts/*OFL.txt assets/fonts/SOURCES.md "$bundle/Contents/Resources/fonts/"
cp assets/fonts/GravewakeGothic-Regular.ttf assets/fonts/gravewake-gothic-build.json "$bundle/Contents/Resources/fonts/"
cp target/release/gravewake "$bundle/Contents/MacOS/gravewake"
cp assets/Gravewake.icns "$bundle/Contents/Resources/Gravewake.icns"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Gravewake</string>
<key>CFBundleDisplayName</key><string>Gravewake — The Hollow Tithe</string>
<key>CFBundleIdentifier</key><string>com.nearby.gravewake</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleExecutable</key><string>gravewake</string>
<key>CFBundleIconFile</key><string>Gravewake</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$bundle"
echo "Built $bundle"
