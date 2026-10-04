# Original Gravewake architecture

These ten modular assets were authored in Blender for the expanded Mournhollow burial grounds. There are no downloaded meshes or third-party asset dependencies. All textures come from the game's existing weathered material system; the ENV1 streams retain local box-projected UVs and per-face or smooth normals.

Rebuild with `Blender --background --python scripts/build-landmarks.py`. The named, editable source collections are in `assets/blender/mournhollow-landmarks.blend`.

| Asset | Placement contract |
| --- | --- |
| Chapel facade | 18 m wide; 6 m clear central doorway. Side masonry is solid to 5.4 m; carved arch, broken gable and pinnacles rise to 12.2 m. |
| Chapel wall | 22 m wide, 7.45 m tall. A continuous 1.4 m sill supports open lancet windows with Y tracery. Do not substitute this for a full-height collision wall. |
| Curtain wall | 22 m wide, 4.6 m tall, 1.4 m deep. Continuous masonry, coping, battered buttresses and climbing ivy; suitable for scaling to solid collision rectangles. |
| Cloister bay | 8 m wide, 8.4 m tall, 1.86 m deep. Ground passage remains 5.1 m clear. |
| Bell tower | Solid 6.8 m square footing. Open belfry, cast verdigris bell, overlapping slate spire and iron finial rise to 20.1 m. |
| Memorial fountain | Dry basin, 2.8 m radius and 2.18 m overall height. Ground obstacle. |
| Ruin wall | Approximately 6.2 m wide; jagged courses rise to 3.6 m. |
| Rubble | Scattered fallen masonry and a fractured column; ankle-height visual debris. |
| Funerary urn | 1.87 m tall with open bowl, stone pedestal and cast handles. |
| Cemetery obelisk | 4.35 m tall tapered stone memorial with recessed tablet and bronze inlay. |

`manifest.json` contains exact exported bounds, triangle counts, byte sizes and collision notes. Every individual asset stays below 20,000 triangles. Native unit checks cover finite vertex data, normalized transformed normals, geometry budgets and the actual exported clear doorways. `captures/architecture-source` contains authoring inspection renders; the native environment review is the final in-game appearance.
