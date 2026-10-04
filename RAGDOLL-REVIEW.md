# Articulated bodies and procedural fracture

The current pass replaces stiff arm/leg corpses with a ten-body humanoid ragdoll: torso, skull, two upper arms, two forearms/hands, two thighs and two calves/feet. Nine constrained joints retain a connected body; elbow and knee hinges limit bending, and neck/shoulder/hip joints limit rotation. The authored mesh, equipment and exact animated pose transfer into physics at impact. Head-only species retain their own shaped physical body.

## Combat behavior

- Head, arm and leg shots sever the corresponding anatomical region. Detached arms and legs retain a working elbow or knee. Living enemies keep their injury consequences: weaker attacks, limp, or crawl. Death never regenerates a missing part.
- Both sides of an anatomical sever receive bone-rim and dark-marrow caps. Heavy weapon impacts can clip the actual skull or torso mesh into two to four capped physical fragments. The clipping preserves surface attributes and handles separate bones and concave cut contours. This is geometric skull/torso fracture; ordinary limb removal follows authored anatomical divisions.
- Impact position controls the applied force and rotation. A severing hit applies its local force to the detached part; the remaining corpse inherits momentum without receiving that same local hit a second time.
- Subsequent bullets, melee strikes and blasts move existing remains. Rays hit the nearest physical surface; blast pressure acts on each exposed collider surface with distance falloff. Static cover blocks both. These interactions do not create extra kills or souls.
- Shotgun pellets trace the same visible pose during a volley while respecting newly missing parts, preventing intermediate knockback from moving later impact points off the rendered body.

## Stability and budgets

Rapier advances bodies at a fixed 120 Hz, independently of render refresh. Continuous collision detection, anatomical joint limits, damping, friction and sleeping keep motion bounded. The collision world includes the expanded arena's floor, walls, pillars and outer boundaries. The shared pool allows 144 physical sections, evicts complete oldest body groups and removes remains after 18 seconds. Head-only species and fractured sections use bounded convex hulls; intact limbs use capsule shapes.

Transient pose snapshots, impulses and fracture state are excluded from serialized saves. Persistent injury flags remain compatible with existing runs. Review and benchmark modes use disposable game state and do not write the player's run or preferences.

## Reproduce the native proof

```sh
./scripts/capture-anatomy.sh
```

The 24-second, 1440 × 900 Metal capture uses actual `Game::fire` ray traces and projectiles at a 60 Hz game update, encoded at 30 FPS with synchronized game audio. Six scenes show head detachment, an articulated severed arm, limp/crawl, an ordinary ten-body collapse, shotgun mesh fracture and a second grenade moving existing debris.

Outputs:

- `captures/anatomy-review.mp4`: native footage, not an animation rendered separately from gameplay.
- `captures/anatomy/manifest.json`: per-scene body/joint counts, fragment counts, pose-handoff error, joint-anchor separation, knee/elbow motion, settling speeds and the subsequent blast's measured velocity change.
- `captures/anatomy/frame-*.png`: source frames for visual inspection.

The final native review passed all six scenes. Pose handoff error was zero for intact sections. The ordinary corpse had ten bodies/nine joints, with a maximum joint-anchor gap of 1.06 mm and mean final body speed of 0.0011 m/s. Shotgun and grenade impacts generated two and three clipped fragments respectively. The subsequent grenade changed existing debris velocity by up to 8.13 m/s; its articulated pieces stayed connected with a 0.42 mm maximum anchor gap. The largest gap anywhere in the six scenes was 8.3 mm. Final rendered frames were inspected for detachment, crawling, knee/elbow collapse, mesh fracture and the second blast; this also caught and corrected a duplicate headshot impulse that previously caused an exaggerated somersault.

`cargo test --locked` passed all 83 tests after this pass. The automated suite covers anatomical hit detection and missing-part persistence; frame-consistent shotgun rays; fragment volume and cap topology; all twelve creature meshes; finite physics, joint limits, floor collision, sleeping and expiry; cover-aware secondary impacts; headshot impulse isolation; pool bounds; and save/progression compatibility. The six-scene review also runs as a CPU regression through the same combat and physics paths.

The 1440 × 900 release benchmark on the Apple M4 Max averaged 60.0 FPS with 24 enemies and 14 complete articulated corpses (140 physical sections, 126 joints). Mean simulation time was 0.774 ms; p95 frame interval was 17.82 ms. This steady workload excludes spawn/fracture creation cost and is not a worst-case combat guarantee. See `PERFORMANCE.md` and `captures/performance/ragdoll.json`.

The final native smoke test passed combat → reward → pack tear/reveal/equip → upgrade → next round, including its disposable save roundtrip. Formatting checks passed for changed Rust files. `scripts/package-macos.sh` rebuilt `dist/Gravewake.app`; its signature verification passed and its executable UUID matches the tested release binary.
