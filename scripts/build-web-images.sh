#!/usr/bin/env bash
# Make the browser build's WebP copies of the large PNG art in assets/web/.
# The desktop game embeds the PNGs; the web build embeds these instead
# (`image_asset!` in src/main.rs), which cuts its download by about 20 MB.
# Art is lossy at quality 90; the weapon surface map holds roughness,
# height and metalness data, so it stays lossless. Rerun after changing
# any of the PNGs, and commit the results.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p assets/web
for name in card-back card-front material-atlas armory-pack button-plaque house-backdrop \
            gravewake-emblem weapons/weathered-color; do
  magick "assets/$name.png" -quality 90 -define webp:method=6 -define webp:alpha-quality=95 \
    "assets/web/$(basename "$name").webp"
done
magick assets/weapons/weathered-surface.png -define webp:lossless=true -define webp:method=6 \
  -define webp:exact=true assets/web/weathered-surface.webp
magick assets/gravewake-emblem.png -resize 64x64 -strip web/icon.png
du -b assets/web/*.webp web/icon.png
