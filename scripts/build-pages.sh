#!/usr/bin/env bash
# Build the browser version for GitHub Pages into dist/pages/ (gitignored),
# or the folder given as the first argument. The folder is the whole site:
# index.html at its root, the gzipped WebAssembly game and asset pack, its
# JavaScript glue, an icon and .nojekyll. Serve it under any path (the live site is
# https://nearbycoder.github.io/gravewake/); every URL in it is relative.
#
#   scripts/build-pages.sh [out-dir]
#
# Needs the wasm32-unknown-unknown Rust target. wasm-bindgen-cli, at the
# version Cargo.lock pins, is installed into target/tools on first use.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-dist/pages}"
jobs="${JOBS:-8}"

version=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p')
bindgen=target/tools/bin/wasm-bindgen
if ! "$bindgen" --version 2>/dev/null | grep -qF " $version"; then
  echo "Installing wasm-bindgen-cli $version into target/tools"
  nice -n 10 cargo install wasm-bindgen-cli --version "$version" --root target/tools --locked -j "$jobs"
fi

nice -n 10 cargo build --release --locked --target wasm32-unknown-unknown -j "$jobs"

rm -rf "$out"
mkdir -p "$out"
"$bindgen" --target web --no-typescript --out-dir "$out" --out-name gravewake \
  target/wasm32-unknown-unknown/release/gravewake.wasm
# GitHub Pages can't send Content-Encoding for this file, so it is gzipped
# here and unpacked in the page with the browser's DecompressionStream.
gzip -9 -n -c "$out/gravewake_bg.wasm" > "$out/gravewake_bg.wasm.gz"
rm "$out/gravewake_bg.wasm"

# The embedded files (`asset_bytes!` and the WebP art) go in an asset pack
# beside the module rather than inside it (src/web.rs reads it): a browser
# compiling a module holds several copies of its data.
python3 - "$out/gravewake-assets.bin" <<'PACK'
import glob, re, struct, sys
names = set()
for source in glob.glob("src/*.rs"):
    text = open(source, encoding="utf-8").read()
    names.update(re.findall(r'asset_bytes!\(\s*"\.\./assets/([^"]+)"', text))
names.update(p[len("assets/"):] for p in glob.glob("assets/web/*.webp"))
names = sorted(names)
header, data = bytearray(b"GWPK" + struct.pack("<I", len(names))), bytearray()
for name in names:
    blob = open("assets/" + name, "rb").read()
    encoded = name.encode()
    header += struct.pack("<H", len(encoded)) + encoded + struct.pack("<II", len(data), len(blob))
    data += blob
open(sys.argv[1], "wb").write(header + data)
print(f"Asset pack: {len(names)} files, {len(data) / 1048576:.1f} MB")
PACK
gzip -9 -n -c "$out/gravewake-assets.bin" > "$out/gravewake-assets.bin.gz"
rm "$out/gravewake-assets.bin"

bytes=$(stat -c %s "$out/gravewake_bg.wasm.gz")
asset_bytes=$(stat -c %s "$out/gravewake-assets.bin.gz")
commit=$(git rev-parse --short HEAD)
if ! git diff --quiet HEAD -- src shaders assets Cargo.toml Cargo.lock web 2>/dev/null; then
  commit="$commit+changes"
fi
sed -e "s/__WASM__/gravewake_bg.wasm.gz?v=$commit/" \
    -e "s/__WASM_BYTES__/$bytes/" \
    -e "s/__ASSETS__/gravewake-assets.bin.gz?v=$commit/" \
    -e "s/__ASSET_BYTES__/$asset_bytes/" \
    -e "s/__VERSION__/$commit/" \
    web/index.html > "$out/index.html"
# The page loads gravewake.js by a fixed name; tag it too so a new build
# never runs old glue from the browser cache.
sed -i "s#\"./gravewake.js\"#\"./gravewake.js?v=$commit\"#" "$out/index.html"
cp web/icon.png "$out/icon.png"
touch "$out/.nojekyll"

echo "Built $out for $commit:"
du -ab "$out" | sort -n | awk '{printf "  %8.1f MB  %s\n", $1/1048576, $2}'
