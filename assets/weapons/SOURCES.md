# Weathered arsenal

These models and textures were authored for Gravewake in Blender 4.5.3 LTS using `scripts/build-weathered-armory.py`. No third-party weapon meshes or texture photographs were used.

## Editable source and runtime assets

- `assets/blender/weathered-armory.blend`: seven named collections for the Iron toggle pistol, Double shotgun, Butcher Cleaver, Grave Revolver, Slugbreaker pump shotgun, Repeater, and scoped Longrifle; editable meshes, UVs, procedural materials, and packed baked images.
- `iron-pistol.dvm`, `double-barrel.dvm`, `cleaver.dvm`, `grave-revolver.dvm`, `slug-pump.dvm`, `repeater.dvm`, `longrifle.dvm`: native triangle meshes exported from Blender with UVs, corner normals, material IDs, and mechanical part groups.
- `weathered-color.png`: 1024 x 1024 sRGB atlas, four 512-pixel tiles. Rusty iron and oxidized brass are the lower row; aged walnut and oil-soaked leather are the upper row.
- `weathered-surface.png`: corresponding linear atlas. Red = roughness, green = micro-height, blue = metalness.
- `0-3-color/surface.png`: individual editable bake outputs.

Blender material nodes generate multi-scale corrosion, pits, wood grain, dirt and scuffing. Their four-dimensional periodic noise avoids hard tile borders in iron/brass/leather. All fixtures use stable object-space projected UVs. Split, area-weighted corner normals preserve broad milled faces and continuous barrel highlights. Zero-area pole triangles are omitted on export. The surface maps control the game's restrained highlights and micro-bump response. Existing procedural gun, melee, bow and occult meshes share these maps, so all 33 weapons receive the same aged treatment.

Rig groups are 0 fixed receiver/grip, 1 break-action barrels/fore-end, 2 sliding action, 3 detachable magazine, 4 hammers, 5 revolver cylinder/crane, 6 pump fore-end/action bars, and 7 operating lever. Shells and hands remain animated by the existing engine. Asset data and maps are embedded; Blender is not required to play.

## Rebuild

Run `./scripts/build-weathered-armory.sh`, then rebuild the Rust app. This preserves the existing atlas; pass `--rebake` to regenerate both shared atlases from the node materials. Studio geometry inspection renders can be reproduced by running Blender in background with `--python scripts/review-weathered-armory.py`; native gameplay captures must still be checked separately. The script accepts `BLENDER_BIN` when Blender is installed elsewhere. The authoring tool used here is a separate local copy at `~/.local/share/dark-veil-tools/Blender.app`, downloaded from the [official Blender release server](https://download.blender.org/release/Blender4.5/blender-4.5.3-macos-arm64.dmg).

The seven replacement meshes contain roughly 1,900–8,800 triangles each (exact counts in `manifest.json`). Other weapon families still use the runtime-authored geometry with this shared material atlas.
