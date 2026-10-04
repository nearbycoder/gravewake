# Articulated ragdoll performance

After the ragdoll/fracture pass, the release benchmark was rerun on the Apple M4 Max with Metal at 1440 × 900 and full Hollowlight effects. The optimized 24-enemy fixture plus 14 complete articulated corpses (140 physical sections, 126 joints) averaged **60.0 FPS**, with a **17.82 ms p95** frame interval. Mean simulation time was **0.774 ms**, and dynamic mesh construction took **2.609 ms**. Physics now advances at 120 Hz. The 12- and 48-enemy scenarios averaged 60.0 and 60.1 FPS respectively. Source: `captures/performance/ragdoll.json`.

Each scenario measures 300 frames after 120 warm-up frames, excluding screenshot readbacks. This fixture measures steady rendering and physics after bodies spawn; it does not measure the cost of simultaneous new mesh fractures or guarantee every combat frame. The current ten-section bodies fill the 144-piece pool with 14 complete corpses; this differs from the older six-section baseline below. MacOS drawable acquisition dominates the current visible measurements, despite AutoNoVsync being requested.

# Expanded world performance

The current 96 × 96 m world was measured on the Apple M4 Max, Metal, a release build and full Hollowlight effects. The native world tour moves through the districts using the real game update loop, then simulates a scenario initialized with 48 enemies in the eastern cloister. AI and body physics advance; player health is protected and the fixture does not fire. This is a bounded scenario, not a worst-case guarantee for every late-game power combination.

| Current visible scenario | Result |
| --- | ---: |
| Native resolution | 1440 × 900 |
| Mean visible frame rate | 61.5 FPS |
| Mean frame interval | 16.26 ms |
| p95 frame interval | 18.88 ms |
| Mean simulation CPU time | 0.109 ms |
| Mean dynamic mesh CPU time | 0.706 ms |
| Mean surface acquisition time | 14.28 ms |

239 measured frames exclude 60 warm-up frames and screenshot readbacks. AutoNoVsync is requested, but macOS presentation still dominates this visible run. Source: `captures/world/normal/manifest.json`. The tour also verified 184.9 metres of continuous movement, six arrival stops, wall contact and final-threat orientation.

Static world triangles are now indexed into spatial visibility batches with conservative bounds. The renderer retains distant detail but skips batches outside the camera view. Eighteen braziers use the six nearest point lights, preserving the shader's light-loop budget. Soft glows and ground shadows render after opaque surfaces without depth writes, fixing fog artefacts.

Use `cargo run --release --locked -- --world-review` to reproduce the visible run. Add `--world-offscreen` to bypass drawable acquisition and wait for each GPU submission to complete. That mode measures serial simulation/rendering latency; its reciprocal throughput is **not visible display FPS**. Add `--review-small` for the 960 × 600 legibility pass. See `WORLD-REVIEW.md` for scene captures and movement validation.

---

# Earlier arena performance baseline

Measured on the local Apple M4 Max using Metal, a release build, 1440 × 900, full Hollowlight effects and AutoNoVsync. These are average rendered FPS, not a promise about monitor refresh or every individual frame. macOS presentation scheduling and other desktop activity affect results.

| Scenario | Original release FPS | Optimized FPS | Original mesh CPU time | Optimized mesh CPU time |
| --- | ---: | ---: | ---: | ---: |
| 12 enemies | 120.0 | 120.0 | 2.90 ms | 0.91 ms |
| 48 enemies | 56.4 | 119.2 | 10.26 ms | 2.08 ms |
| 24 enemies + 144 ragdoll sections | 65.3 | 118.7 | 7.42 ms | 2.93 ms |

Original measurements: `captures/performance/before.json`. Final measurements: `captures/performance/final.json`. A same-window comparison in the final executable also runs the CPU reference path with shared primitive caching and allocation reuse already applied: it measured 67.2 FPS for 48 enemies and 88.2 FPS for the ragdoll scene, versus 119.2 and 118.7 FPS with instancing and visibility checks. Treat the original-to-final comparison as a local benchmark, with the paired run providing an additional controlled comparison.

## Changes

- Full-detail enemy ellipsoids (skulls, joints, eye sockets, armor and emissive details) share one GPU vertex buffer. Each shape uploads a transform, radii, color and material instead of hundreds of expanded vertices. Mesh topology, enemy counts and shader quality are retained.
- Cached primitive trigonometry and reused frame allocations. Per-triangle transformed normals are computed once instead of once per corner.
- Conservative camera-frustum checks skip rendering offscreen enemies, body pieces, particles and orbs. Their simulation, attacks, physics, collision and pickups continue normally; attack warnings remain visible when their area is on screen.
- Bullet traces reject enemies outside a conservative bounding sphere before constructing detailed animated hit capsules.
- Unlocked presentation is the default, with safe platform fallback. F7 toggles VSync, F8 toggles the FPS counter. Both controls are also in Settings & Controls and persist in `performance.json` alongside the existing save files.
- Occluded/minimized windows suspend continuous rendering and wake when visible, rather than consuming GPU time behind other windows.

## Reproduce

Run `./scripts/benchmark.sh`. It uses disposable game state and does not write the player's run or settings. Each of three deterministic workloads warms up for 120 frames and measures the next 300, first through the CPU reference renderer and then the optimized renderer, in the same window. Six reference screenshots are taken during warmup, outside measured samples. JSON results are written to `captures/performance/latest.json`; set `GRAVEWAKE_BENCH_LABEL` to preserve another named result.

Frame timing includes presentation/acquisition and therefore is not an isolated GPU timer. Mesh and simulation timings are CPU measurements; submit time includes upload, command encoding, interface rendering, and presentation. `max_vertices` counts CPU-expanded dynamic vertices, excluding the shared instanced topology.

Matched reference/optimized crowd screenshots have a 63.84 dB full-frame PSNR; visible geometry, textures and lighting were also inspected. Small differences arise from equivalent floating-point transforms and draw order. No resolution scaling or lower-detail models were used for these results.

Validation also includes the existing combat, progression, dismemberment, physics, and weapon tests, plus geometry equivalence, frustum bounds, frame allocation reuse, and non-mutating render tests. The native shader review exercises the VSync and FPS controls through actual UI clicks.
