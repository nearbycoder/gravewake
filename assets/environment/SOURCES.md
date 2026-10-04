# Mournhollow cemetery assets

All geometry in this directory is original and authored in Blender for Gravewake. No external meshes or model downloads were used.

- Editable source: `assets/blender/mournhollow-environment.blend`
- Reproducible authoring script: `scripts/build-environment.py`
- Runtime format: `ENV1` header, little-endian vertex count, then triangle-list vertices with 12 floats: position XYZ, unit normal XYZ, linear color RGB, engine material index, UV.
- Engine coordinates: Y up. Blender source coordinates: Z up. The script performs the rotation during export.
- `manifest.json` records counts, bounds, and exported byte sizes.

The set includes three light distant spruce variants, a close spruce with separated needle bundles, secondary twigs and exposed roots, three carved grave silhouettes, staggered bevelled masonry and spear railings, a stepped buttress, a fitted arch/gate with curved iron tracery, and a hollow bowl brazier with swept supports. Each asset remains a named collection of editable mesh objects and bevel modifiers in Blender.

The game places these assets at the established fire, grave, wall and tree locations. Detailed collision trees use 9,018 triangles; distant trees use 2,926 triangles. Geometry is decoded once into the static world buffer. Existing stone/bark/leaf/metal material atlas shading supplies weathering without a new runtime texture dependency.
