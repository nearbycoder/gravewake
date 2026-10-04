# Gravewake creature models

Twelve original creatures authored in Blender 4.5.3 LTS with `scripts/build-enemies.py`. No downloaded models or external character rigs were used. Editable, assembled source: `assets/blender/gravewake-creatures.blend`; toggle the twelve named collections to inspect each design.

The skull has Boolean-cut orbital and nasal cavities, a separate cheek arch and mandible, and individual teeth. The skeleton includes curved ribs, sternum, scapulae, perforated pelvis, shaped paired limb bones, staggered fingers, and metatarsal feet. Species add articulated wings, crawler legs, forged armor, reliquaries, cowl/robe folds, a kite shield, lanterns, and a curved reaping blade.

Runtime files use a `GVM1` header, little-endian vertex count, and records containing a rig-group integer plus twelve floats: position, normal, color, engine material, and UV. Coordinates are Y-up, facing +Z. Twenty-two local segment frames are driven by the engine's shared hit-test pose. Missing parts are omitted; the same full-detail geometry supplies detached physics pieces. These are segmented skeletal animations, not soft-body skin or cloth simulation.

Each creature has three geometry tiers. Full detail is used within eight meters and in close review; simplified Blender exports are used beyond eight and fourteen meters. `manifest.json` records counts. Near geometry is unchanged by distance simplification. The renderer keeps indexed geometry on the GPU and updates only segment transforms/status per visible enemy. Bone shading uses subdued pore variation; metal and cloth use the game material palettes.

Rebuild with `./scripts/build-creatures.sh`. Blender is an authoring dependency only; meshes are embedded in the executable.
