# Improvement plan

This plan was written on 2026-10-06 from a fresh clone of `main` (`ff7a722`).
It is based on a source audit and native runs on Linux, not on general advice.
The second phase implements the round scope at the end. Everything else is a
ranked backlog.

## Baseline on Linux

Machine: CachyOS Linux, AMD Radeon 8060S (RADV, Mesa 26.2.4), Wayland, Rust
1.96.1. The machine was shared and heavily loaded (load average about 45 on 32
cores) during these runs. Timings are indicative only.

| Check | Result |
| --- | --- |
| `cargo test --locked` (Rust 1.96.1, no toolchain upgrade needed) | **84 passed**, 0 failed. One dead-code warning (`environment_assets::Asset::{Wall, Buttress, Gate}`). |
| `cargo build --release --locked` | Builds. wgpu selects **Vulkan** on the iGPU. Mailbox, Fifo and Immediate present modes are available. |
| `gravewake --smoke` | **`SMOKE PASS`**: combat → reward → pack tear/reveal/equip → upgrade → next round, save roundtrip. Eight captures were inspected and render correctly: title, arena, collector, pack, binding, armory and bestiary. **The process then segfaults on exit (status 139).** |
| `gravewake --benchmark` | 12 enemies: 284 FPS. 48 enemies: 277 FPS (p95 8.2 ms). 24 enemies + 14 corpses: **78 FPS** (mesh CPU 6.5 ms, p95 14.3 ms). The same exit segfault followed. |

**Exit crash root cause** (from `coredumpctl`): `drop_in_place<gravewake::App>`
drops egui-winit's `smithay_clipboard::Clipboard`. Its worker thread then calls
`wl_proxy_destroy` on a Wayland connection that has already been torn down.
Every Linux/Wayland exit crashes this way, including **Quit Game** and closing
the window, because they share the same `event_loop.exit()` → drop path. The run
saves before exit, so progress is not lost. Even so, a crash on every quit
leaves core dumps and looks broken.

So the game already runs and plays on Linux. The gaps are that it crashes on
quit, files saves in the wrong place, and isn't claimed or packaged.

## What the audit found

- **Linux save location.** `Game::support_path()` always uses
  `$HOME/Library/Application Support`, so on Linux saves land in
  `~/Library/Application Support/Gravewake/`. That directory is foreign there
  and invisible to backup tools. It should use `$XDG_DATA_HOME` (default
  `~/.local/share`) on Linux and `%APPDATA%` on Windows.
- **Preferences are not saved.** `sensitivity` and `volume` reset to 0.0025 and
  40% on every launch (`game.rs` new). Only VSync/FPS (`performance.json`) and
  Hollowlight (`graphics.json`) persist. The README implies all Settings &
  Controls values persist.
- **Every run starts the same way.** `Run::default()` uses `seed: 948271`, and
  `new_run()` never reseeds. The first eight spawn positions, the first spawn
  batches and any shop or pack roll made before the first spread shot are
  identical in every run. Wave composition is a fixed round-robin
  (`pool[(n + wave * 3) % len]` in `survival.rs`), so each descent's enemy order
  never changes.
- **Combat readability.** Player damage is summed from every source into a
  single `hurt` value that drives a full-screen vignette. Nothing shows where an
  off-screen hit came from, which matters in a 96 m arena with flyers and
  ranged casters. The crosshair only widens with `flash` (firing). There is no
  hit or kill confirmation at the reticle; floating damage numbers exist but
  appear at the target.
- **Audio is non-positional.** `Audio::play(event, volume)` is mono. Enemies
  are silent apart from the player's own `hurt` cue, and there is no music;
  ambience is a synthesized drone and wind loop.
- **Onboarding.** A single banner ("WASD Move / Mouse Fire / R Reload /
  E Melee / Esc Pause") shows for the first 7 seconds of a run. It omits sprint,
  and it sits in the top-centre stack with the descent counter, compass and boss
  bar, covering the crowd (see `captures/performance/optimized-2.png`). Souls,
  level-ups, the Collector's prices and the chalice are explained only by
  incidental notices.
- **Input.** Keyboard and mouse only, with hard-coded `KeyCode`s and no
  rebinding. Physical keys keep AZERTY usable, but labels always say WASD. No
  FOV option (fixed 70° vertical), no invert-Y. The UI is a custom egui canvas
  driven entirely by pointer clicks.
- **Card decisions.** Pack and offer cards show raw stats but no comparison
  with the equipped weapon, so the one meaningful choice in the shop needs
  mental arithmetic.
- **Performance hotspot.** Living enemies are GPU-instanced, but ragdoll
  sections are CPU-expanded every frame (527k dynamic vertices with 14 corpses).
  That scene is the only benchmark well below the others on this GPU.
- **Distribution.** CI (`.github/workflows/build.yml`) runs only on
  `macos-14`. There is no release workflow, no Linux/Windows artifact, and the
  window sets no Wayland `app_id`, so desktops show a generic icon.

## Ranked backlog

Impact is for a real player. Effort is S (under half a day), M (a day or two)
or L (several days). Risk covers regressions to the existing, well-tested
macOS build.

| # | Improvement | Impact | Effort | Risk |
| --- | --- | --- | --- | --- |
| 1 | **Linux as a supported platform**: fix the Wayland exit segfault, use XDG save paths with migration, set an `app_id` plus `.desktop`/icon, add a `scripts/package-linux.sh` tarball and an Ubuntu CI job, and update the README | High: opens a second platform that already nearly works | M | Low: platform-gated paths. macOS behaviour is unchanged. |
| 2 | **Persist all preferences**: sensitivity and volume, plus new FOV and invert-Y options, in one versioned `settings.json` that reads the legacy files | High: a bug every returning player hits | S | Low |
| 3 | **Fresh seed per run**: seed `new_run()` from time and keep deterministic seeds for tests and reviews. Optionally show the seed on the ending screen. | High for a roguelite: the opening varies | S | Low: tests pin seeds explicitly |
| 4 | **Combat readability**: directional damage arcs around the reticle, plus a hit marker (body, head, kill variants) and a short kill tick | High: survivability and feel | S–M | Low: HUD only |
| 5 | **Gamepad support** via `gilrs`: twin-stick movement and look with deadzones, triggers for fire, face buttons for dodge/reload/melee/Ember Bolt, Start to pause, and a stick-driven virtual cursor so every egui menu (shop, packs, binding, armory) works through synthetic pointer events | High: removes the "keyboard and mouse required" limitation | M | Medium: new dependency (needs `libudev` on Linux). Menu navigation by virtual cursor is less polished than focus-based navigation. |
| 6 | **Release pipeline**: a tag-triggered GitHub Actions workflow building a macOS `.app` zip and a Linux x86_64 tarball, with checksums, as draft artifacts. The owner decides whether to publish. | High for reach (no Rust toolchain needed) | M | Low to the code. **Needs an owner decision** (see below). |
| 7 | **Onboarding pass**: a first-run (persisted) contextual tip sequence for sprint/dodge, souls and level-ups, the Collector, and the chalice. Move the opening banner below the reticle area. Add "vs. equipped" stat deltas on pack/offer cards. | Medium–High: first five minutes | M | Low |
| 8 | **Run variety and records**: weighted random wave composition per descent (same introduction schedule) and occasional elite modifiers; best descent, fastest clear and total souls shown on the title and ending screens | Medium–High: replayability | M | Medium: balance needs playtesting |
| 9 | **Positional audio**: stereo pan and distance attenuation for enemy, impact and explosion cues; attack-telegraph sounds for off-screen dives, blinks and casts | Medium | M | Low |
| 10 | **Instanced ragdoll sections**: reuse the living-enemy instancing path for intact corpse sections and keep CPU expansion only for fractured fragments | Medium: late fights on weaker GPUs | M | Medium: touches the renderer and anatomy handoff |
| 11 | **Key rebinding**, with a remappable action table that also feeds gamepad glyphs | Medium (accessibility) | M | Low |
| 12 | **Windows validation**: a CI build job plus `%APPDATA%` paths (shared with #1). Not testable on this machine. | Medium | S for CI, unknown for runtime | Unknown |
| 13 | **Music**: a sparse adaptive score (calm at the Collector, rising with wave pressure and the boss) | Medium | M–L (composition) | Low |
| 14 | **HUD scale and reduced-flash options** (hurt vignette, muzzle flash and bloom intensity) | Low–Medium | S | Low |
| 15 | **Browser/WebGPU build** | Medium | L | High: rodio, threads and embedded asset size. Not this round. |
| 16 | Remove the dead-code warning | Trivial | S | None |

Balance is deliberately not ranked. The tests cover a full twelve-descent run,
but this audit had no long manual playtests to support specific tuning claims.
Item 8's records would also give the owner data for later balance passes.

## Proposed scope for this round

Five items, chosen for player impact per unit of risk. Item 5 is the largest
and goes last, so the earlier items ship even if it slips.

### A. Linux as a supported platform (backlog #1)

Acceptance criteria:
- `gravewake --smoke`, `--benchmark` and a normal **Quit Game** exit with
  status 0 on Wayland. Fix by dropping egui-winit state (and its clipboard)
  before the window and event loop are torn down.
- Saves and preferences live in `$XDG_DATA_HOME/gravewake/` (default
  `~/.local/share/gravewake/`) on Linux. Any existing
  `~/Library/Application Support/Gravewake/` files are copied once and never
  overwritten, following the existing Dark Veil migration rules. macOS paths are
  unchanged.
- The window sets a Wayland/X11 `app_id`/class. `scripts/package-linux.sh`
  produces `dist/gravewake-linux-x86_64.tar.gz` with the binary, `.desktop`
  entry, icon, licences and a README snippet.
- CI gains an `ubuntu-latest` job running `cargo test --locked` and a release
  build. The README lists Linux (tested: AMD RADV, Wayland) alongside macOS,
  states what was not tested (X11, NVIDIA), and keeps Windows and browser marked
  as unvalidated.

Verification: unit tests for path resolution and migration (temporary
`HOME`/`XDG_DATA_HOME`); native `--smoke` and `--benchmark` runs with exit
codes checked; and a `coredumpctl` check that no new gravewake dumps appear.
Unpack the tarball into a temporary directory and launch it from there. Running
the CI job requires pushing, so the workflow is only reviewed locally (with
`actionlint` if available) until the owner pushes.

### B. Persist all preferences, plus FOV and invert-Y (backlog #2)

Acceptance criteria:
- Sensitivity, volume, vertical FOV (60–90°, default today's 70°) and invert-Y persist across launches in one
  `settings.json` with atomic writes.
- Existing `performance.json`/`graphics.json` values are read on first launch.
  Review and smoke modes never write preferences, matching current rules.
- The Hunter's Journal shows the new controls without breaking the existing
  layout at 960×600.

Verification: a unit roundtrip and legacy-read test. Change values through the
journal sliders, quit and relaunch natively, and confirm they persist. Run
`--text-review --review-small` to check the journal layout.

### C. Fresh seed per run (backlog #3)

Acceptance criteria:
- `new_run()` seeds from system time mixed with a counter. Tests, smoke and
  reviews keep fixed seeds, and existing saves keep their stored seed.
- Two consecutive new runs differ in initial spawn layout and first pack
  contents.

Verification: a unit test that two seeded runs diverge and a fixed seed
reproduces exactly. The existing 84 tests and the smoke run must stay green.

### D. Combat readability: damage direction and hit markers (backlog #4)

Acceptance criteria:
- Each damage source (melee contact, projectile, hazard, blast) records its
  world direction. The HUD draws a fading arc around the reticle pointing
  toward it, relative to the current yaw, and stacks up to four arcs.
- Successful hits flash a reticle hit marker, with distinct head and kill
  variants and a short kill tick sound. Melee and projectile hits count too.
- Both respect a new "reduce flashes" toggle (backlog #14, trimmed to this
  feature).

Verification: unit tests mapping attacker bearing to arc angle for all four
quadrants and checking that marker state is set by bullet, projectile and melee
paths. Add native captures from a small review fixture showing arcs and markers
and inspect them.

### E. Gamepad support (backlog #5)

Acceptance criteria:
- With a standard controller, the player can start a run, fight, dodge,
  reload, melee, cast, choose powers, pause, shop, open and equip a pack, and
  buy an upgrade without touching the mouse or keyboard.
- Arena play uses the left stick to move and the right stick to look, with
  radial deadzones and a separate stick sensitivity. Menus use a visible virtual
  cursor that emits egui pointer events, with face buttons mapped to
  click/back.
- Hot-plugging works, and the game runs unchanged without a controller or
  without udev access.
- Controls are listed in the journal and the README.

Verification: unit tests for the input mapping, deadzones and virtual-cursor
event synthesis. A scripted smoke variant drives the shop and pack flow through
the synthetic gamepad path. **Hands-on testing needs the owner:** an 8BitDo
Pro 3 receiver is attached here, but the pad was not exposed as a joystick
during the audit, so real button presses could not be verified.

## Decisions for the owner

1. **Release artifacts (#6).** Should a tag-triggered workflow build
   downloadable macOS and Linux binaries, and should they be published as
   GitHub Releases? Apple notarization needs a Developer ID that only the owner
   holds. Without it, macOS downloads stay ad-hoc signed and Gatekeeper will
   warn.
2. **Claiming Linux.** Is it acceptable to list Linux as supported, tested only
   on AMD/RADV/Wayland, in the README and on the public game page?
3. **Gamepad dependency.** `gilrs` adds a `libudev` runtime dependency on
   Linux. The alternative is reading evdev directly, which is more code for
   the same result.
