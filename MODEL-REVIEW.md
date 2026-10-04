# Blender model overhaul — 2026-10-04

The delivered pass replaces all twelve enemy designs, seven weapon assemblies, and eleven cemetery assets with original Blender-authored geometry. It preserves the game's segmented animations, anatomical hit regions, dismemberment, and physical debris.

## What changed

- Creatures: recessed eye/nasal cavities, separate jaw/teeth, curved ribs and sternum, pelvis openings, paired limb bones, hands/feet, connected necks, folded garments, cupped wings, thicker armor, shield, and curved scythe. In-game review found and corrected robe clipping.
- Weapons: Iron, Double, Cleaver, Grave Revolver, Slugbreaker, Repeater, and Longrifle. Shaped stocks/receivers, smooth fitted metal surfaces, open bores/breeches, seated shells, separate mechanical actions, restrained rust and grain. First-person placement exposes more of the assembly above the HUD.
- Scenery: irregular conifers with individual near needle bundles, carved grave silhouettes, bevelled masonry/buttresses, fitted Gothic gate, and curved bowl braziers.

The other 26 weapon designs retain their runtime geometry with the revised shared material maps. First-person hands retain their existing procedural meshes. Cloth is shaped geometry, not simulated fabric. The foliage remains stylized and angular, especially at distance.

## Reproducible evidence

`cargo run --release -- --model-review` writes 17 actual native-renderer captures to `captures/models/after`. Matched previous-model captures are retained in `captures/models/before`. The gallery includes every creature, the environment, first-person pistol/shotgun/reload, and a 48-creature crowd. Fixtures never read or write player progress/preferences.

`--model-cpu` renders full-detail reference geometry to a separate gallery. All twelve close portraits matched the GPU path: at least 99.89% of pixels were within two RGB levels. See `captures/models/parity.json` for per-image differences. Distant crowd views intentionally use lower-detail GPU meshes, so they are not an identical-detail comparison.

At this model pass, all 43 automated tests passed. The native smoke run completed combat → reward → pack tear/reveal/equip → upgrade → next round with a successful save roundtrip. The subsequent physics pass replaced the six-section corpse with ten articulated bodies and procedural skull/torso fractures, using these same authored meshes. Its current native proof and validation are documented in `RAGDOLL-REVIEW.md`.

## Rendering measurements

Development machine: Apple M4 Max, Metal, 1440 × 900. All figures below describe the 48-model fixture with animated rigs and frozen AI; each uses 240 frames after 60 warm-up frames and excludes capture/readback.

| Measurement | Result |
| --- | --- |
| Native presented frame mean / p95 | 16.68 / 20.37 ms |
| Native presented rate | 59.97 FPS |
| Native CPU mesh assembly | 0.617 ms |
| Native drawable acquisition wait | 14.60 ms |
| Offscreen serial CPU + GPU frame mean / p95 | 4.65 / 5.89 ms |

The offscreen diagnostic uses the identical rendered scene and explicitly waits for GPU completion each frame. It bypasses drawable/compositor pacing and demonstrates rendering headroom; it is **not** a gameplay FPS claim. Reproduce with `--model-review --model-offscreen`. Reports are in `captures/models/after/manifest.json` and `captures/models/after-offscreen/manifest.json`.

Creature meshes stay in indexed GPU buffers; only segment matrices/status are uploaded during animation. Three Blender detail levels keep full geometry near the player and reduce it beyond eight and fourteen meters. Full-detail meshes also supply ragdolls. Existing physics and enemy-count budgets remain in place.

## Editable sources

- `assets/blender/gravewake-creatures.blend` — twelve assembled, named creature collections and rig-frame reference.
- `assets/blender/weathered-armory.blend` — seven mechanical weapon assemblies and materials.
- `assets/blender/mournhollow-environment.blend` — cemetery assets.

Authoring recipes, geometry manifests and source notes live in `scripts/` and the corresponding `assets/enemies`, `assets/weapons` and `assets/environment` directories. Blender is only needed to author/rebuild; runtime assets are embedded in the app.
