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

**Resolved for this round (orchestrator, 2026-10-06):** Linux may be listed
as supported, with a note that it was tested on one CachyOS machine with an
AMD Radeon 8060S. `gilrs` and its `libudev` dependency are acceptable. Release
artifacts and tag workflows (#6) are out of scope this round.

## Round 1 results

All five scoped items shipped on the `improvements` branch. Native checks ran
on the CachyOS / Radeon 8060S / KDE Wayland machine described above.

| Item | Verification |
| --- | --- |
| A. Linux support | `--smoke`, `--benchmark` and a real window close (sent via a KWin script) exit with status 0 and leave no core dump. KWin reports the window class `gravewake`. Legacy settings migrated from `~/Library/Application Support/Gravewake` into `~/.local/share/gravewake` under a temporary `HOME`. `scripts/package-linux.sh` built a tarball whose extracted binary passed `--smoke` outside the repo, and whose `install.sh` / `--uninstall` were exercised under a temporary `HOME`. Unit tests cover path resolution and migration priority. The Ubuntu CI job has not run, because nothing was pushed. |
| B. Preferences | Unit tests cover round-trip, partial files and clamping. Custom values survived a real launch and quit. The shader review still drives the moved journal controls through egui, and the journal was captured at 1440×900 and 960×600 (`docs/media/improvements/journal-preferences*.jpg`). The FOV change was not checked visually in a live arena: no review fixture exercises a non-default FOV. |
| C. Fresh seed | A unit test shows fixed-seed openings repeat while fresh-seed openings and seeds differ. Smoke and reviews still use the fixed seed. |
| D. Combat feedback | Unit tests check arc bearings in all four quadrants and under rotation, merging and the four-arc cap, a real enemy strike, and markers from bullet, projectile, melee-weapon and bash hits (not powers), with one kill tick per volley. Two new text-review fixtures were captured natively (`docs/media/improvements/hud-*.jpg`). |
| E. Gamepad | Unit tests cover deadzones, arena mapping, cursor movement, clamping, clicks and back, and steering. `--smoke --gamepad` tears, reveals, selects and equips the pack via the virtual cursor and the A button (`docs/media/improvements/gamepad-cursor-pack.jpg`). gilrs initialised without errors in a normal launch. **No physical controller was tested.** The attached 8BitDo receiver exposed no joystick device, so real sticks, triggers, hot-plugging and macOS IOKit input are unverified. |

Final state: 94 unit tests pass. `--smoke`, `--smoke --gamepad`,
`--shader-review` and `--text-review` (normal and `--review-small`) pass.

Notes from verification:
- Two of about 17 `--smoke --gamepad` runs stalled during heavy machine load
  (load average 30–48): one at a 600 s timeout and one at 400 s. Both stalled
  after renderer start-up and before the first frame was captured, which is
  before any controller-specific code runs, and smoke mode does not create a
  gilrs context. Ten back-to-back re-runs passed, and a stall never recurred,
  so no stack was captured. In the same window, a small-window text review
  segfaulted after 61 captures in the Wayland clipboard worker, with no panic,
  while KWin logged "failed to read client connection" for a client. It passed
  when re-run. I suspect compositor or desktop contention on the shared
  machine, but the root cause is not established.
- The final binary's `--benchmark` exited with status 0 and no core dump:
  259, 242 and 93 FPS for the three optimized scenes, under shared load.
- A panic inside the event loop (only seen when a smoke assertion failed) still
  segfaults on Wayland while unwinding, because `exiting` is skipped. Normal
  exits are clean. A panic hook that releases the window would close this gap.

Not verified this round: no macOS build or run of these changes (the
new gilrs dependency uses IOKit there; macOS CI should catch compile
problems once pushed); Windows; X11; NVIDIA.

Deferred to later rounds: everything ranked #6 onward. Release artifacts (#6)
remain an owner decision, including Developer ID notarization for macOS.

## Round 2 scope

Round 1 was merged to `main` on 2026-10-06. This round takes the next items
that most help a player and can be verified on this Linux machine. It also
closes two loose ends from round 1.

### A. Clean exit after a crash, plus an FOV capture (round 1 follow-ups)

A panic inside the event loop unwinds past `exiting`. The App is then dropped
after the Wayland connection, so the panic message is followed by a segfault
in the clipboard worker. The FOV slider was also never checked visually in a
live arena.

Acceptance criteria:
- A panic during play prints its message and exits with Rust's panic status
  (101), not SIGSEGV (139), and leaves no core dump.
- A text-review fixture captures the arena at a non-default FOV.

Verification: a temporary local panic injection (not committed) run natively,
checked with its exit status and `coredumpctl`. The new fixture is captured
and inspected.

### B. Positional audio and attack telegraph cues (backlog #9)

Today every sound is mono and centred, and enemies wind up their special
attacks silently, so threats behind you are invisible and inaudible.

Acceptance criteria:
- World sounds from enemies (special-attack wind-ups, blasts, melee swings)
  are panned with equal power by bearing relative to the player's view, and
  attenuated by distance. Sounds behind the player are slightly softened.
- Each special-attack family plays a distinct wind-up cue when its visible
  warning starts: caster bolts, slams and plague bursts, dives, summons and
  blinks.
- The player's own weapon, UI and pickup sounds are unchanged.

Verification: unit tests for the pan/gain function (front, left, right,
behind, near and far) and for warnings emitting a cue at the enemy's position.
The sound-bank bounds test covers the new cues, and their exported WAVs get
spectrogram images. No human listening test is possible here, and the report
will say so.

### C. Weapon comparison on cards (part of backlog #7)

Pack and Collector offers show raw stats, so the main shop decision needs
mental arithmetic.

Acceptance criteria:
- Pack choices and the Collector's offer show an estimated sustained damage
  per second (pellets, magazine and reload included; melee per swing). They
  also show the difference from the equipped card, in green or red ink.
- The equipped card, armory cards and binding view are unchanged.

Verification: unit tests for the estimate (pistol, shotgun, melee and upgraded
cards) and the delta sign. Native pack and Collector captures at 1440×900 and
960×600 via the text review.

### D. Run records and varied wave order (part of backlog #8)

Every run plays the same per-descent enemy order (`pool[(n + wave * 3) %
len]`), and the game keeps no history.

Acceptance criteria:
- Reinforcements draw from each descent's existing pool through a seeded
  shuffle bag. Each species keeps the same share of a wave as before, so
  balance is preserved, but the order changes between runs. Tithekeeper
  descents still open with the boss.
- `records.json` stores runs started, deepest descent, most souls in a run,
  victories and the fastest victory. Records update on death, victory and each
  cleared descent. The title shows them once a run has been played, and the
  ending screen marks new records. Save-disabled modes never write records.

Verification: unit tests that composition counts match the old round-robin
per bag, that order differs across seeds and repeats for one seed, that the
twelve-descent progression test still passes, and that records update and
round-trip without writing in save-disabled games. Title and ending captures
come from the text review, plus a real launch and close with a temporary
`HOME` to confirm the file is written and read.

### Deferred this round

- **Instanced ragdoll sections (#10).** Corpse fragments are arbitrary
  clipped meshes, transformed on the CPU and re-uploaded every frame. Doing
  this properly needs a new instanced GPU path for fragment meshes, which is
  too large and risky to combine with this round. The worst benchmark scene
  ran at 93 FPS here.
- Key rebinding (#11), music (#13), Windows validation (#12), the browser
  build (#15), and first-run contextual tips (the rest of #7).
- Owner decisions, unchanged: release downloads and tag workflows, macOS
  signing and notarization, and licences.

## Round 2 results

All four scoped items shipped on `improvements-2`. The round 1 CI run on `main`
passed both jobs, including the macOS job's build and unit tests with the new
gilrs dependency; the game itself has still not been run on a Mac. Native
checks below ran on the same CachyOS / Radeon 8060S / KDE Wayland machine.

| Item | Verification |
| --- | --- |
| A. Clean exit after a panic, plus FOV capture | A temporary injected panic (not committed) exited with status 101, printed the panic message and left no core dump. Before the fix, panics ended in SIGSEGV (139) from the clipboard worker. The new `hud-field-of-view-90` text-review fixture shows the wider view next to the default (`round2/hud-field-of-view-{70,90}.jpg`). Wide angles stretch objects near the edges, such as the moon, as expected for a rectilinear projection. |
| B. Positional audio and wind-up cues | Unit tests cover equal-power panning (ahead centred at the unpanned level, hard left and right, softer behind, centred when facing, falling with distance) and each special-attack family's cue at the position where its warning starts. The sound-bank bounds test covers the five new cues. `--export-audio` spectrograms show distinct shapes: rising cast, low slam, falling dive, steady summon bell, blink whoosh (`round2/warning-cue-spectrograms.jpg`). In the survival review's recorded stereo track, 20 of 124 active 100 ms windows have more than 20% left/right imbalance. That supports panning in real play but doesn't prove it, since some existing sounds already had stereo reflections. **No human listening test was possible.** |
| C. Weapon comparison on cards | Unit tests cover single-shot, pellet, burst and melee estimates, rarity and both upgrade paths, and a finite positive value for all 33 weapons. Native text-review captures of the pack (1440×900 and 960×600) and the Collector show green, red and "same as equipped" lines (`round2/pack-dps-comparison*.jpg`, `round2/collector-dps-comparison.jpg`). The estimate is single-target, so splash and elemental weapons look weaker than they are; DEVELOPMENT.md says so. |
| D. Shuffled waves and records | Unit tests: two bag passes contain each species exactly twice, one seed repeats its order, other seeds vary it, and boss descents open with the Tithekeeper. Records update through a new run, a descent, death and victory; practice is ignored; and the atomic write and read-back work. The twelve-descent combat test still passes. Text-review captures cover the title and death screens (`round2/title-records.jpg`, `round2/death-new-records.jpg`). In a real launch with a temporary `HOME`, `records.json` was read and shown on the title, captured with KWin's screenshot tool (`round2/title-records-live-launch.jpg`), and the window then closed cleanly with status 0. |

Final state: 99 unit tests pass. `--smoke`, `--shader-review`,
`--survival-review`, `--world-review`, `--benchmark` and `--text-review`
(normal and `--review-small`) pass.

Notes:
- **Intermittent startup stall, still unexplained.** Across both rounds,
  about 5 of roughly 60 `--smoke --gamepad` launches stalled after renderer
  start-up and before the first frame. In this round it happened with the
  machine nearly idle, so load is not the cause. It has not happened in about
  35 plain `--smoke` launches, but the gamepad variant was always the second
  of a pair, and no controller code runs before the stall. Stacks could not be
  captured: Yama blocks attaching to a running process, and five runs under
  gdb as the parent did not stall. The cause could be a startup race between
  back-to-back Wayland windows or something in the game, and needs testing on
  another machine. When the gamepad smoke did run, it passed every time.
- Benchmark under shared load this round: 124, 118 and 72 FPS for the
  optimized 12-enemy, 48-enemy and corpse scenes. Earlier today the same scenes
  ran at 259, 242 and 93 FPS. Simulation and mesh CPU times were unchanged
  (about 0.1 ms and 0.76 ms in the 12-enemy scene). The difference was
  surface acquisition (6.4 ms vs 1.75 ms per frame), i.e. compositor
  presentation, not game code.

Deferred, with reasons:
- Instanced ragdoll sections (#10). This needs a new GPU path for clipped
  fragment meshes, which is too risky to combine with this round. Now that the
  benchmark shows presentation varying by time of day, it should be measured
  with GPU timestamps first.
- First-run contextual tips (rest of #7), key rebinding (#11), music (#13),
  Windows (#12) and the browser build (#15).
- Owner decisions: release downloads and tag workflows, macOS signing and
  notarization, and licences.

## Round 3 scope

Round 2 was merged and pushed on 2026-10-06. This round takes four items. Two
are the deferred player-facing work that matters most in the first hour:
controls that fit the player's keyboard, and a first run that explains itself.
One adds the missing music. One builds a tool to catch the unexplained startup
stall. Instanced ragdolls (#10) stay deferred for the reason given in round 2.

### A. Key rebinding (backlog #11)

Keys are hard-coded `KeyCode`s, and labels always read WASD, even on AZERTY
keyboards where the physical keys are Z Q S D.

Acceptance criteria:
- Forward, back, left, right, sprint, dodge, reload, melee and Ember Bolt can
  each be bound to a key or to the right, middle, back or forward mouse button.
  Firing stays on the left mouse button. Escape, F6, F7, F8 and F11 stay
  reserved, and the journal says so if you try them.
- The Hunter's Journal gets a Preferences page and a Controls page. On the
  Controls page you click an action, press the new key, and the binding is
  saved to `settings.json`. A key already used by another action swaps with it.
  Escape (or controller B) cancels, and **Restore default keys** resets every
  binding.
- Key labels follow the player's layout: a binding shows the character the key
  produced when it was bound or last pressed. Every in-game prompt (the opening
  banner, tips and the journal) uses the current labels.
- Older `settings.json` files without bindings load with the defaults.
  Controller mapping is unchanged.

Verification: unit tests for the swap rule, reserved keys, the default
round-trip, and loading a legacy or corrupt bindings file. Text-review captures
of both journal pages and the waiting-for-key state at 1440×900 and 960×600. A
real launch with a temporary `HOME`: rebind a key with synthetic input
(`ydotool` or `wtype`, if they work here), quit, relaunch, and check
`settings.json` and the journal. If synthetic keyboard input isn't possible,
the report will say the live rebinding wasn't exercised by hand.

### B. First-run field tips (rest of backlog #7)

The only guidance is a seven-second banner across the top-centre HUD stack.
Souls, level-ups, the Collector, the Binding and the chalice are explained only
by incidental notices.

Acceptance criteria:
- A short sequence of contextual tips, each shown once: moving, sprinting and
  dodging at the start of the first run; reloading and melee the first time the
  magazine runs low; souls when the first soul drops; damage arcs the first time
  you're hurt; the Collector, packs and the Binding on the first shop visit; and
  Ember Bolt when a chalice is bound.
- Tips appear in a panel in the lower part of the screen, clear of the reticle,
  the top-centre stack and the vitality plate. They use the current key labels
  and queue rather than overlap. The opening banner moves into the same place
  and is replaced by the first tip.
- Seen tips are saved in `settings.json`. A **Field tips** toggle in the
  journal turns them off, and turning it back on shows them all again. Review,
  smoke and practice modes never record tips as seen.

Verification: unit tests that each trigger queues its tip once, that the seen
state persists and resets with the toggle, and that no tip is recorded when
saving is disabled. Text-review captures of the movement tip and the shop tip
at both sizes.

### C. Startup-stall watchdog (round 1 and 2 open issue)

About 5 of roughly 60 `--smoke --gamepad` launches have stalled before the
first frame, and no stack has been captured: Yama blocks attaching to a running
process, and runs under gdb didn't stall.

Acceptance criteria:
- In smoke, review and benchmark modes only, a watchdog thread records
  start-up milestones (window, renderer, first frame and later frames). If no
  progress is made for 120 seconds, it prints the last milestone and aborts, so
  systemd-coredump keeps a core with every thread's stack. Normal play never
  starts the watchdog.
- A script runs repeated `--smoke` and `--smoke --gamepad` launches with logs
  under `captures/stall/`, and it collects the `coredumpctl` stack of any stall.

Verification: a temporary injected stall (not committed) must produce the abort,
the milestone message and a readable core with stacks. Then run the loop for as
many launches as time allows. If a stall is caught, its stack is analysed and
either fixed (if the fix is small and clear) or documented. If none is caught,
the report says how many launches ran.

### D. Adaptive music (backlog #13)

There is no music, only a synthesized drone and wind loop.

Acceptance criteria:
- A sparse synthesized score in three synchronized layers: a calm layer (low
  organ and choir pads with a distant bell) for the title and the Collector, a
  pressure layer (a heartbeat drum and a bowed ostinato) that rises with the
  number of living enemies and with low health, and a boss layer while the
  Tithekeeper is alive. Layers crossfade over a few seconds and never restart
  mid-phrase.
- A separate **Music volume** slider is saved in `settings.json`. At zero no
  music is mixed. Smoke and review runs keep music silent unless a review
  records audio.
- The sound bank stays embedded and generated at start-up, with no new files
  or dependencies. Start-up time grows by no more than about 150 ms here.

Verification: unit tests that the layers are finite, bounded, loop without a
click at the seam and stay synchronized, and that the mix targets follow the
game state. An `--export-audio` render of each layer and a one-minute mix, with
spectrogram images. The start-up timing is measured. **No human listening test
is possible here**, so whether it sounds good is an owner decision, and the
report will say so.

### Deferred this round

- Instanced ragdoll sections (#10): unchanged reason from round 2.
- Windows validation (#12): a Windows CI job could go red with nothing here to
  test it on.
- Elite modifiers (rest of #8): balance needs playtests.
- The browser build (#15), HUD scale, release downloads, macOS signing and
  licences (owner decisions).

## Round 3 results

All four scoped items shipped on `improvements-3`. Native checks ran on the
same CachyOS / Radeon 8060S / KDE Wayland machine. Every native run used a
throwaway `XDG_DATA_HOME`. `~/.local/share/gravewake` didn't exist before or
after the round.

| Item | Verification |
| --- | --- |
| C. Stall watchdog (`b7ae034`) | A temporary injected stall (not committed) with a 15 s limit aborted with status 134. It printed `no progress for 15 s after "renderer ready" (frame 0)`, and `coredumpctl` kept a core whose stacks showed the main thread asleep inside `run_app`. A unit test covers the limit and the report. `scripts/stall-hunt.sh` then ran **242 smoke launches** (121 plain, 121 controller). **No stall occurred**, so its cause is still unknown, but the watchdog is in place for the next one. The hunt did catch a different flaky failure: 3 of the first 61 controller runs failed the pack step. See the note below. |
| A. Key rebinding (`42d8310`) | Unit tests cover swaps, reserved and unknown inputs, layout-aware labels, round-trips, partial files, damaged entries (which reset only the bindings), and the journal's waiting state. Text-review captures cover both journal pages, the waiting state, an AZERTY swap at 1440×900 and 960×600, and the longest labels on the HUD (`round3/controls-*.jpg`, `round3/hud-rebound-mouse-keys.jpg`). In a real launch, a seeded `settings.json` (AZERTY glyphs, melee on the right mouse button) loaded, was written back unchanged on quit, and the window closed with status 0. **Pressing keys in the live game wasn't exercised**: no synthetic keyboard tool is installed, and kernel-level input on this shared desktop could reach another session's window. The review fixtures call the same `Game::bind` path that real key events use. |
| B. Field tips (`177ac95`) | Unit tests: each tip appears once at its moment, later tips queue, the Collector tip waits for the shop and the bolt tip for the arena, the switch hides tips and replays them, the seen state persists, and smoke, review and practice runs never record tips. Captures of the movement tip, the bolt tip under a notice (960×600) and the Collector tip (`round3/hud-tip-*.jpg`, `round3/collector-tip.jpg`). **The triggers weren't seen in a live hand-played run**: starting a run needs a click that couldn't be sent to the real window. |
| D. Adaptive music (`8c50901`) | Unit tests: the layers are finite, bounded, stereo, equally long and audible. The loop seam is no larger than the music's own 99th-percentile sample step, and with the tail fold disabled the test fails (0.068 vs 0.020). The mix follows the title, the crowd, low vitality, the boss and the shop, and the crossfade has no jumps. Spectrograms of each layer and a 64 s adaptive mix show the chord changes every 8 s and the drum figures (`round3/music-spectrograms.jpg`). In a live launch, the game's own PipeWire stream was recorded on the title: the music faded in after about 3 s and played at about −32 dBFS RMS at default volume. Rendering takes 1.1–1.5 s on a background thread, so the window isn't delayed. **No one has listened to it.** Whether it sounds good is for the owner to judge. |

Also fixed this round:
- **Controller smoke flakiness.** Desktop pointer motion over the window
  (when it opens under the mouse, or when other windows move) reset the
  controller's virtual cursor mid-click in scripted runs. Scripted runs now
  ignore desktop pointer motion and log it once. In the second hunt (120
  launches, 60 controller), every run logged such motion and none failed. In
  the first hunt, before the fix, 3 of 61 controller runs failed. That supports
  the fix, but with failures this rare it doesn't prove it.
- The pause screen said "CMD + Q / QUIT" on Linux. Only macOS shows it now.
- The shader review's journal clicks follow the moved controls through shared
  layout constants.

Final state: 114 unit tests pass. `--smoke`, `--smoke --gamepad`,
`--text-review` (normal and `--review-small`, 86 captures across 83 fixtures),
`--shader-review`, `--armory-review`, `--survival-review`, `--world-review` and
`--benchmark` pass with status 0. Benchmark under shared load: 110, 106 and
80 FPS for the optimized 12-enemy, 48-enemy and corpse scenes (round 2: 124,
118 and 72).

Not verified: macOS (the music thread and rodio source are platform-neutral,
and CI builds and tests there after the push), Windows, X11 and NVIDIA. Also
unverified: a listening test, live key presses, and live tip triggers.

Deferred, with reasons:
- Instanced ragdoll sections (#10): it still needs a new GPU path. Measure it
  with GPU timestamps first.
- Windows validation (#12): nothing here to test on.
- Elite modifiers (rest of #8): balance needs playtests.
- Controller remapping: bindings cover keyboard and mouse only.
- Key labels on non-QWERTY layouts: a key shows the layout's own character
  only after it has been pressed once (winit has no layout query), so an
  untouched AZERTY Q position reads "Q" until it's pressed.
- Browser build (#15) and HUD scale.
- Owner decisions: release downloads and tag workflows, macOS signing and
  notarization, licences, and whether the synthesized score fits the game or
  should be replaced by composed music.

## Round 4 scope

Round 3 was merged and pushed on 2026-10-06. Rounds 1–3 covered the platform,
preferences, feedback, audio, rebinding, tips and music. This round fixes two
gaps a player meets in the first hour: prompts that ignore the controller,
and deaths that don't say what happened. It also takes on the long-deferred
corpse rendering path. The README gains a "Status and known issues" section.
The riskiest item (C) goes last, so A and B ship even if it slips.

### A. Controller-aware prompts

Controller players see keyboard prompts everywhere: the field tips ("W A S D
to move… Shift to sprint"), the HUD hints (reload, strike, dodge, Ember Bolt)
and the level-up line ("PRESS 1, 2 OR 3").

Acceptance criteria:
- The game tracks the last input device used. A controller button, a trigger
  or a stick pushed past its deadzone switches prompts to controller labels. A
  key press, a mouse click or real mouse movement switches them back. Stick
  noise inside the deadzone doesn't switch. The choice isn't saved.
- In controller mode, the HUD hints, the level-up line, the opening banner and
  every field tip name controller inputs, using the same Xbox names as the
  README table (left stick, LT, A, X, B, Y, D-pad). Keyboard prompts are
  unchanged.

Verification: unit tests for device switching (including deadzone noise), the
label for every action on both devices, and every tip's text on both devices.
Text-review captures of the controller HUD, the movement tip and the level-up
screen at 1440×900 and 960×600. `--smoke --gamepad` must still pass.

### B. Death recap and run summary

The ending screen shows only the descent, kills and time. Damage from all
sources is summed each tick, so the game can't say what killed you, which
matters with flyers, casters and off-screen blasts.

Acceptance criteria:
- Each hit on the player records its source: the creature and the attack
  (strike, bolt, slam, plague burst, blast). The death screen names the killing
  blow (for example "Slain by a Grave Crawler's strike") and the creature that
  dealt the most damage over the run.
- Death and victory screens show a run summary: kills, headshots, damage
  dealt, damage taken, soul level, the equipped weapon and the run time.
- The statistics are saved with the run. Older saves load with zeros. Practice
  never adds to them.
- Both screens fit at 1440×900 and 960×600.

Verification: unit tests that each attack family records the right cause, that
the killing blow and the top source are reported, that hits and kills add to
the statistics, that a legacy save loads, and that practice leaves the run
untouched. Text-review captures of the death and victory screens at both
sizes.

### C. GPU-resident corpse sections (backlog #10)

Corpse sections keep fixed body-space vertices, but each frame the CPU
transforms every vertex and re-uploads them (about 527k vertices with 14
corpses). That makes the corpse benchmark the slowest scene.

Acceptance criteria:
- In the optimized renderer, each body piece (intact sections and fracture
  fragments) uploads its vertices to a resident GPU buffer once, when it first
  appears. Each frame writes one transform per visible piece. Frustum culling,
  the end-of-life shrink and ground shadows are unchanged. The buffer reuses
  space as pieces expire, and compacts when it fills.
- The CPU reference path (benchmark reference scenes, `--model-cpu`) is
  unchanged.
- The corpse scene renders the same as the CPU path: a pixel difference of the
  two captures is reviewed, and the mean difference is under 1/255.
- In the corpse benchmark, mesh CPU time falls by at least 80%. Frame rates
  are reported with the machine's load.

Verification: unit tests that the instance transform reproduces the CPU
expansion (rotation, translation, shrink and normals) and that the allocator
appends, reuses space and compacts without losing a live piece. A native
`--benchmark` before and after, paired captures of the CPU and GPU paths with
a difference image, and the anatomy review (fractures, explosions and expiry)
must pass.

### D. Status and known issues in the README

The README has no single place that says what is verified, what isn't, and
what is known to be wrong.

Acceptance criteria: a "Status and known issues" section listing the tested
platforms, what has never been verified (macOS runs since round 1, Windows,
X11, NVIDIA, a physical controller, a listening test), and known limitations.

### Deferred this round

- Windows validation (#12): nothing here to test on.
- Elite modifiers (rest of #8): balance needs playtests.
- Controller remapping, HUD scale and the browser build (#15).
- Owner decisions, unchanged: release downloads and tag workflows, macOS
  signing and notarization, licences, and whether the synthesized score stays.

## Round 4 results

All four scoped items shipped on `improvements-4`. Native checks ran on the
same CachyOS / Radeon 8060S / KDE Wayland machine, with a throwaway
`XDG_DATA_HOME` for every run. `~/.local/share/gravewake` didn't exist before
or after the round. Load averages on the shared machine were 19–29 during the
round. Timing- and input-sensitive runs waited for a load below 24.

| Item | Verification |
| --- | --- |
| A. Controller-aware prompts (`27ef2b0`, `2e84ed2`) | Unit tests check every action's controller label against the real arena mapping (pressing the named button performs the action), that stick drift, light trigger pressure and held buttons don't switch prompts while presses, trigger pulls and stick pushes do, and every tip's text on both devices. New text-review fixtures show the controller HUD with the movement tip and a melee weapon, the controller opening reminder and the controller level-up line at 1440×900 and 960×600 (`round4/hud-controller-*.jpg`, `round4/powers-controller-960x600.jpg`). The practice notices name Start instead of Escape after the controller is used. `--smoke --gamepad` still passes. **The switch itself hasn't been seen with a real controller**: none is exposed on this machine, so the live gilrs-to-prompt path is unverified. |
| B. Death recap and run summary (`cc28d3f`) | Unit tests: a strike, a bolt, a slam and a burst each record the right creature and attack; the blow that empties vitality is the one named; damage taken stops at the vitality left; simultaneous hits share the loss by raw damage; the top source and its share are right; bullet and projectile hits add damage dealt (without overkill) and headshots; statistics survive a save round-trip; a save without them loads with zeros; and practice leaves the run's statistics unchanged. Text-review captures of a late death, a victory and a first-descent death at both sizes (`round4/death-recap.jpg`, `round4/victory-summary.jpg`, `round4/death-first-descent-960x600.jpg`). The recap wasn't seen after a hand-played death. |
| C. GPU-resident corpse sections (`059a90c`, `f613d8d`) | A unit test runs `scene::dynamic` both ways on the benchmark's corpse scene and checks that, for every visible vertex, the instance transform reproduces the CPU expansion (position, normal and all other attributes), including two shrinking pieces. Another drives the buffer through appends, holes, compaction, growth, 400 steps of churn and a device limit, checking that no live piece is lost or overlapped. In the benchmark (three alternating runs per path, one binary, load 18–23), corpse-scene mesh CPU time fell from 5.5–7.4 ms to 0.65–0.90 ms (−87 to −88%), submit time from 2.7–4.0 ms to 0.3–0.8 ms, and per-frame dynamic vertices from 527,310 to 41,790. In the two busiest pairs, presentation capped every scene and frame rates moved little. In the third pair, the corpse scene ran at 46% of the same run's 12-enemy frame rate with CPU expansion and 80% with resident sections, and its p95 fell from 30.1 ms to 3.6 ms. Absolute frame rates varied about threefold between runs because of the shared desktop, so they aren't compared directly. Images: the benchmark's corpse capture differs by at most 2/255 on 0.009% of pixels. In the anatomy review, every frame before the fracture scene has an MSE under 0.005 between the paths. From there, two runs of the same path also diverge at the same frame, so the review's physics isn't deterministic (pre-existing, cause not investigated). The anatomy review passes on both paths. |
| D. Status and known issues (`145d289`) | A new README section lists what was tested, what never was (Windows, X11, NVIDIA/Intel, a physical controller, a listening test, live key presses and live tip triggers) and the known limitations. |

Final state: 122 unit tests pass (114 before the round). `--smoke` and
`--smoke --gamepad` (twice each), `--text-review` (normal and
`--review-small`, 90 captures across 87 fixtures), `--shader-review`,
`--armory-review`, `--survival-review`, `--world-review`, `--anatomy-review`
(on both corpse paths) and `--benchmark` all exited with status 0 on the final
binary. The final benchmark (load 19) measured 275, 257 and 223 FPS for the
optimized 12-enemy, 48-enemy and corpse scenes; round 3 measured 110, 106 and
80 under heavier load.

Notes:
- The anatomy review isn't deterministic from run to run: two runs of the
  same build first differ in its fracture scene (frame 503 of 720), and their
  logged joint gaps and final speeds differ. Rendering can't affect physics,
  so this predates the round. The benchmark's corpse scene is deterministic
  (repeat captures are identical).
- The repository isn't rustfmt-clean on `main` (34 differing hunks), so this
  round didn't reformat files. New code follows the surrounding style.
- The dead-code warning (backlog #16) remains. Clearing it means removing
  three embedded meshes and shifting asset indices, which isn't worth the risk
  without a reason to touch that file.

Not verified: macOS (CI builds and runs the unit tests after the push; the new
shader stage uses only standard WGSL vertex inputs), Windows, X11, NVIDIA and
Intel GPUs, a physical controller (including the live prompt switch), and
a hand-played death to see the recap.

Deferred, with reasons:
- Windows validation (#12): nothing here to test on.
- Elite modifiers (rest of #8): balance needs playtests.
- Controller remapping, HUD scale and the browser build (#15).
- The anatomy review's nondeterminism: worth a look if exact replays matter,
  but it doesn't affect play.
- Owner decisions, unchanged: release downloads and tag workflows, macOS
  signing and notarization, licences, and whether the synthesized score stays.

## Round 5 scope

Round 4 was merged and pushed on 2026-10-06. This round takes four items.
Two help a player survive and read the fight: a warning for special attacks
winding up out of view, and a HUD size option. One closes the longest-standing
controller gap, fixed button mapping. The last clears the dead-code warning.
Controller remapping is the largest item and goes last, so the others ship even
if it slips. A time-boxed look at the anatomy review's nondeterminism follows
if time allows, and is reported either way.

### A. Off-screen attack warnings

Every special attack has a visible warning (a ground circle, a sigil, a
glowing caster) and a positioned wind-up sound. Both fail when the attacker is
behind the player: the circle is out of view, and the sound doesn't help
players who play muted or can't hear it. The death recap (round 4) exists
largely because of off-screen dives, casts and blasts.

Acceptance criteria:
- While a creature is winding up an attack aimed at the player (Cinder Skull
  and Ash Cantor casts, Bell Gargoyle and Gloamwing dives, Tithe Reaper
  blinks, and Tithekeeper slams or Plague Vessel bursts when the player is
  inside or near the marked circle), and it is outside the horizontal view, an
  amber pointer around the reticle shows its bearing. The pointer is outside
  the red damage arcs, so the two can't be confused.
- In-view wind-ups show no pointer; their own warning is visible. Bone Shepherd
  summons, which don't target the player, show no pointer.
- Pointers grow as the attack nears and disappear when it lands or is
  interrupted. At most four show at once, the most urgent first. **Reduce
  flashes** stops their pulsing.

Verification: unit tests that each attack family produces (or, for summons,
doesn't produce) a pointer, that in-view and out-of-range slams don't, that the
bearing is right in all four quadrants, and that the cap keeps the most urgent.
A text-review fixture with pointers and damage arcs together at 1440×900 and
960×600, inspected by eye.

### B. HUD size

The HUD is laid out on a 1440×900 canvas and scales only with the window, so
it can't be enlarged on a large TV or a high-resolution laptop, or shrunk to
see more of the arena.

Acceptance criteria:
- A **HUD size** switch on the journal's Preferences page cycles 80, 90, 100,
  115 and 130% and is saved in `settings.json`. Older files load at 100%.
- Each HUD group scales around its own anchor: the status panel from the top
  left, the descent and boss panels from the top centre, vitality from the
  bottom left, ammunition from the bottom right, the reticle, hit marks, arcs
  and warnings from the centre, and the field-tip panel from the bottom
  centre. Menus, cards and the journal are unchanged.
- At 130% in a 960×600 window, HUD groups don't overlap each other.

Verification: unit tests for the setting's round-trip, clamping and the anchor
transform. Text-review captures of a busy HUD (all powers, boss, tip, arcs) at
80% and 130% at 1440×900 and 960×600, inspected for overlaps, plus the
journal with the new switch at both sizes.

### C. Controller remapping

Controller buttons are fixed. The README lists it as a known limitation, and
prompts (round 4) already name buttons, so remapping must keep them right.

Acceptance criteria:
- The journal's Controls page splits into **Keyboard** and **Controller**
  pages. On the Controller page, Fire, Sprint, Dodge, Reload, Melee and
  Ember Bolt each have a main and a second button. Defaults match today's
  mapping (RT; LT + left-stick click; A; X; B + RB; Y + LB).
- Any of A, B, X, Y, LB, RB, LT, RT and either stick click can be bound.
  Start and the D-pad stay reserved (pause, power choice and the menu cursor),
  and in menus A still selects and B still goes back. A button already in use
  swaps with the slot being changed. A change that would leave an action with
  no button is refused with a note. **Restore default buttons** resets them.
- Choosing a slot waits for the next button press (with the controller or the
  mouse). Start, Escape or a click elsewhere cancels. The press that chose the
  slot doesn't bind it.
- Bindings are saved in `settings.json`; older or damaged entries load the
  defaults without affecting other preferences. Prompts (HUD hints, tips, the
  opening reminder) name the bound buttons.

Verification: unit tests for the arena mapping under custom bindings, swaps,
reserved buttons, the refusal rule, trigger edges, prompt labels, round-trips
and damaged entries, and the waiting state (the choosing press doesn't bind).
Text-review captures of the Controller page, its waiting state and a remapped
HUD prompt at both sizes. `--smoke --gamepad` must still pass. **No physical
controller is available**, so live button presses stay unverified.

### D. Dead-code warning (backlog #16)

`environment_assets::Asset::{Wall, Buttress, Gate}` embed three meshes (about
1 MB) that the world never draws. Remove the variants and their
`include_bytes!`; the exported files stay in `assets/` with their Blender
source. Acceptance: `cargo build` and `cargo test` produce no warnings, and
the world review's captures are unchanged.

### Deferred this round

- Windows validation (#12): nothing here to test on.
- Elite modifiers (rest of #8): balance needs playtests.
- Focus-based controller menu navigation: the virtual cursor works, and a
  focus layer would touch every screen.
- The browser build (#15).
- Owner decisions, unchanged: release downloads and tag workflows, macOS
  signing and notarization, licences, and whether the synthesized score stays.

## Round 5 results

All four scoped items shipped on `improvements-5`, plus two fixes found while
verifying them. Native checks ran on the same CachyOS / Radeon 8060S / KDE
Wayland machine, with a throwaway `XDG_DATA_HOME` for every run.
`~/.local/share/gravewake` didn't exist before or after the round. Load
averages on the shared machine were 16–45. Timing- and input-sensitive runs
waited for a load below 24.

| Item | Verification |
| --- | --- |
| D. Dead-code warning (`9463e70`) | The three unused meshes are no longer embedded. `cargo build` and `cargo test` print no warnings, and the executable is about 1 MB smaller. All nine world-review captures are pixel-identical before and after (ImageMagick AE = 0). |
| A. Off-screen attack warnings (`9a8e1de`) | Unit tests: every attack family aimed at the player shows a pointer when it winds up behind the player; summons, in-view casts and a burst the player has left don't; bearings are right in all four quadrants and under rotation; urgency rises through the warning, and a dive in flight is 1; four at most, the most urgent kept. The `hud-offscreen-warnings` fixture (1440×900 and 960×600) shows pointers to the right, behind and behind-left, sized by urgency, next to a damage arc, and none for an in-view cast (`round5/hud-offscreen-warnings*.jpg`). The scope wrongly named the Iron Penitent; the slam is the Tithekeeper's. |
| B. HUD size (`c08c235`, `9f9392a`) | **Range changed from the plan:** 80, 90, 100 and 110%, not up to 130%. At 1440 design units, the bottom row (vitality, armor, dodge and weapon) only fits up to about 115%, and at 115% a damage arc pointing straight ahead reaches the boss bar. The reticle group keeps its size so it can't run into the boss bar or the notes. Unit tests cover the round-trip, clamping, the cycle, and, in a real egui frame at three window sizes, that each anchor stays put while distances scale. Busy-HUD fixtures (every power, boss bar under the last-threat bearing, chalice, the longest field note under a notice, a damage arc, an off-screen dive) at 80, 100 and 110% at both sizes show no panel overlaps (`round5/hud-size-*.jpg`). The journal switch fits at 960×600. Arena field notes now end above the weapon and chalice panels: before, a two-line note in a 960×600 window overlapped the weapon panel's corner by a few pixels. At 80% in a 960×600 window, HUD text is about 11 points. |
| C. Controller remapping (`8350a68`) | Unit tests: each prompt names a button that performs its action, with default and remapped bindings (including fire); swaps; an emptied main slot taking the second button; reserved Start and D-pad refused; leaving an action bare refused with no change; every bindable button usable; serde round-trip; damaged entries (reserved, empty, too many, duplicate or unknown) restore the defaults without touching other preferences; files from before remapping load the defaults; trigger pulls become presses and releases at the threshold; the waiting state keeps waiting through refusals and reports swaps. Captures of the Controller page, the waiting state and a remapped page with its note, and a HUD and opening reminder with remapped prompts, at both sizes (`round5/controller-*.jpg`, `round5/hud-controller-remapped.jpg`). `--smoke --gamepad` passes. **No physical controller was available**, so live button presses on the Controller page are unverified. That the choosing press can't bind relies on egui registering a click when A is released; a test checks that a release-only frame binds nothing. |
| Watchdog fix (`269a176`) | Steps inside a frame no longer count as watchdog progress, and a lost or outdated surface no longer counts as a presented frame. A temporary injected outdated surface (not committed) now aborts with status 134, names the step, logs the surface error once and leaves a core; before, those failed frames counted as progress. |
| Physics determinism (`af9ddeb`) | The anatomy review's nondeterminism (open since round 4) came from Parry's hash maps, which use a hasher seeded randomly for each process unless Rapier's `enhanced-determinism` feature is on. Two baseline runs first differed at capture 287 (round 4 saw 503). With the feature, three runs gave identical captures for all 720 frames and identical logs, and the CPU and GPU corpse paths now produce identical logs, with a worst normalised RMSE of 0.00056 over every tenth frame of the whole review, fracture scenes included. In alternating benchmark runs (load 20–29), corpse-scene simulation time was 2.18 and 2.08 ms with the feature and 2.11 and 2.09 ms without, so no measurable cost. `Cargo.lock` gains four dependency edges, to crates it already pinned. |

Final state: 128 unit tests pass (122 before the round), with no warnings.
On the final binary, `--smoke` and `--smoke --gamepad` (twice each),
`--text-review` (normal and `--review-small`, 98 captures across 95
fixtures), `--shader-review`, `--armory-review`, `--survival-review` and
`--world-review` exited with status 0. `--anatomy-review` (three runs, plus
one on the CPU corpse path) and `--benchmark` (twice) ran on the same code.
Benchmark (four runs, load 20–29): 72–167 FPS for 12 enemies, 82–175 for 48
and 106–165 for the corpse scene. The spread comes from the shared desktop's
presentation, not game code.

Notes:
- **One controller smoke stall.** Early in the round, a `--smoke --gamepad`
  launch started immediately after a plain smoke run stalled after start-up
  with no captures, and `timeout` killed it at 600 s (status 124). Its
  watchdog never printed or aborted, so the whole process, watchdog thread
  included, stopped making progress. That looks like the process being
  stopped or hung outside the game loop, not a game-loop stall, but the cause
  isn't established and no stack was kept. An immediate rerun passed in
  9 s, a 30-launch stall hunt (15 plain, 15 controller) passed every run, and
  so did the four final smoke runs.
- The watchdog's abort test left one core dump (about 79 MB) in
  systemd-coredump's store; it will be rotated out normally.

Not verified: macOS (CI builds and runs the unit tests after the push), Windows,
X11, NVIDIA and Intel GPUs, a physical controller (including live remapping),
and how 80% and 110% HUD sizes feel in a hand-played run.

Deferred, with reasons:
- HUD sizes above 110%: they need a different bottom-row layout.
- Windows validation (#12): nothing here to test on.
- Elite modifiers (rest of #8): balance needs playtests.
- Focus-based controller menu navigation and the browser build (#15).
- Owner decisions, unchanged: release downloads and tag workflows, macOS
  signing and notarization, licences, and whether the synthesized score stays.

## Round 6 scope

Round 5 was merged and pushed on 2026-10-07. Rounds 1–5 made the game
playable with a controller, but its menus still need the stick-driven cursor,
and nothing happens when the controller drops out mid-fight. This round takes
five items: two for controller players, one for players on large screens, one
for the shop decision, and one aimed at the stall that has never been caught.
The two layout-heavy items (D and E) go last, so the smaller ones ship even if
those slip.

### A. Pause when the controller disconnects

If the controller you're playing with runs out of battery or is unplugged
mid-fight, the game shows a notice but keeps running, so the player takes
damage they can't answer.

Acceptance criteria:
- When the controller driving the game disconnects during arena play and the
  last input came from a controller, the game pauses (as losing focus does),
  releases held inputs and shows "CONTROLLER DISCONNECTED". Another
  controller's disconnect, or a disconnect while playing with keyboard and
  mouse, doesn't pause. Smoke and review runs never pause this way.

Verification: unit tests for the decision (driving vs other controller,
controller vs keyboard device, arena vs menus) and that the game ends up
paused with no held fire or movement. **No physical controller** is exposed
here, so the gilrs disconnect event itself can't be produced live.

### B. Honest damage estimate on cards

The "EST. DPS" line on pack and Collector cards counts only direct hits, so
burning and venom weapons look weaker than they are, and splash, chain and
piercing weapons don't say they hit more than one target.

Acceptance criteria:
- The estimate adds burn (18 per second for 3 s after each hit) and venom
  (12 per second, 1.8 s per hit, up to 6 s) damage over a full magazine and
  reload cycle, single target.
- Cards for weapons that hit several targets (splash, chain lightning,
  piercing) say so after the estimate, within the card at 960×600.

Verification: unit tests that burn and venom estimates match a simulated
magazine cycle, that other weapons are unchanged, and that every one of the
33 weapons gets a finite estimate. Text-review captures of a pack with a burn
weapon and an area weapon at both sizes.

### C. A watchdog that can't be silenced by its own output

In round 5, one controller smoke run hung with the watchdog silent: the whole
process stopped. One way that happens is the watchdog thread blocking on its
own report, for example when stdout and stderr are a pipe that has stopped
draining. The stall hunt also kills a hung run without recording what state
it was in.

Acceptance criteria:
- The watchdog writes its report without waiting on stderr (and to a file in
  the capture folder) and aborts within a few seconds of the limit even if
  stderr is blocked.
- `scripts/stall-hunt.sh`, when its outer timeout fires, records the process
  state, kernel wait channel and per-thread states from `/proc`, then sends
  SIGABRT so a core is kept, before falling back to SIGKILL.

Verification: a temporary injected stall (not committed) run with stdout and
stderr into a pipe that is never read: before the change the process hangs
past the limit, after it the process aborts. The hunt script's timeout path is
exercised with a temporary short timeout. A short hunt then runs, and the
report gives the count.

### D. Controller menu navigation with the D-pad

Menus use a free virtual cursor, which is slow and fiddly for picking small
buttons such as the upgrade medallions and journal rows.

Acceptance criteria:
- In menus, a D-pad press moves the controller cursor to the nearest control
  in that direction (buttons, cards' actions, upgrade medallions, the
  equipped card, journal sliders and rows). The first D-pad press in a screen
  picks the control nearest the screen centre. A still selects and B still
  goes back, and the left stick still moves the cursor freely.
- Holding the D-pad repeats after a short delay. On a slider, left and right
  adjust its value in twentieths, and up and down leave it.
- Controls hidden behind a dialog or the journal can't be reached. The
  level-up screen keeps the D-pad for choosing powers, and the Controller
  page's waiting state is unchanged.

Verification: unit tests for the directional choice (straight, diagonal,
nothing in that direction, first press), repeat timing and slider steps. The
controller smoke run drives part of its pack flow with D-pad presses only.
Text-review captures of the snapped cursor in the shop and the journal at both
sizes. Live presses on a physical controller stay unverified.

### E. Larger HUD sizes: 120% and 130%

Round 5 stopped at 110% because the bottom row ran out of room and the boss
bar met the damage arcs.

Acceptance criteria:
- **HUD size** cycles 80, 90, 100, 110, 120 and 130%. At 100% and below,
  and at 110%, the layout is unchanged.
- Above 110%, a compact layout applies: armor moves into the vitality plate,
  the chalice bar narrows to the plate, the dodge readout narrows, the
  Tithekeeper's health joins the descent panel, and the top-centre panels
  narrow so they clear the status panel.
- With the busiest HUD (every power, boss, chalice, a long note, a damage arc
  pointing ahead and an off-screen warning), no HUD panels overlap at 120%
  and 130% in 1440×900 and 960×600 windows.

Verification: unit tests for the setting's cycle and clamping and, in a real
egui frame, that the compact panels' rectangles don't intersect at both
sizes. Busy-HUD captures at 120% and 130% at both window sizes, inspected by
eye. If 130% can't be made to fit, it's dropped and the report says why.

### Deferred this round

- Elite enemy variants (rest of #8): balance needs playtests.
- Windows validation (#12) and the browser build (#15).
- Owner decisions, unchanged: release downloads and tag workflows, macOS
  signing and notarization, licences, and whether the synthesized score stays.

## Round 6 results

All five scoped items shipped on `improvements-6`. Native checks ran on the
same CachyOS / Radeon 8060S / KDE Wayland machine, with a throwaway
`XDG_DATA_HOME` for every run. `~/.local/share/gravewake` didn't exist before
or after the round. Load averages on the shared machine were 14–57 during the
round. Input-sensitive runs used the controller smoke run, which counts frames
rather than time.

| Item | Verification |
| --- | --- |
| A. Pause on controller disconnect (`bc7c734`) | A unit test: losing the controller you're fighting with pauses the run, clears held movement, sprint and fire, and shows "CONTROLLER DISCONNECTED / PAUSED"; a disconnect while playing with keyboard and mouse, or in a menu, the shop, the level-up screen or the title, changes nothing. `Pad::poll` flags only the driving controller's disconnect. **No physical controller is exposed here**, so the gilrs disconnect event itself wasn't produced live. |
| B. Honest card estimate (`e89a55e`) | Unit tests: Dragonbreath's burn matches a hand calculation over its two-shell cycle (16.8 burn damage per second, not the full 18, because its reload outlasts the burn); the Needler, Widow Fangs and Plague Censer keep venom up the whole cycle (+12); every other weapon's estimate is unchanged, and all 33 are finite and positive. A second test lets a burning and a poisoned creature tick through the real creature update, and the health lost matches the shared constants (54 and 72). The rarity line now names each weapon's trait (BURN, VENOM, SPLASH, CHAIN, PIERCE, FROST, DRAIN, PULL, with "+ SPLASH" for bursting occult shots). Pack and Collector captures at both sizes (`round6/pack-traits-and-burn-estimate*.jpg`, `round6/collector-venom-splash-offer*.jpg`): the trait fits the rarity line at 960×600, and Dragonbreath rose from 52 to 69 DPS. **Changed from the plan:** the area note is on the rarity line, not after the estimate, which had no room. |
| C. Watchdog that can't be silenced (`fee48cc`) | A temporary injected stall (not committed) that keeps writing to a stdout/stderr pipe nobody reads, with a 10 s limit: before the change, the watchdog never reported and an outer `timeout` killed the run at 60 s (status 124), which is exactly the round 5 symptom. After it, the run aborted with status 134 two seconds after the limit, wrote `captures/watchdog-<pid>.txt` and left a core. The hunt script's timeout path was exercised by stopping a run with SIGSTOP under a 20 s limit: it saved the `/proc` state (every thread `T (stopped)`, wait channel `do_signal_stop`), sent SIGABRT and SIGCONT, and kept a core whose gdb stacks were saved. A 30-launch hunt (15 plain, 15 controller) then passed every run. Whether a blocked pipe caused round 5's hang isn't known. |
| D. D-pad menu navigation (`856160c`) | Unit tests: the directional choice in a row, upward, a diagonal fallback, the edge (no move), skipping the control under the cursor, and preferring a control in line over a closer one off to the side; the first press landing nearest the centre; repeats at 0.4 s and then every 0.12 s, stopping at the last control; A clicking where the cursor rests; slider steps of a twentieth, clamped at the track's end, and leaving a slider vertically; the stick-only fallback with no targets. `--smoke --gamepad` now reaches REVEAL ALL, the middle card's CHOOSE CARD and TAKE & EQUIP with D-pad presses alone and fails if a press lands anywhere else; it passed in every run (4 in the final check, 15 in the hunt). Review fixtures show the cursor after D-pad presses in the Collector and after two slider steps (field of view 70° → 73°) at both sizes (`round6/pad-*.jpg`). **Live presses on a physical controller are unverified.** |
| E. HUD 120% and 130% (`fd926e2`) | The setting's cycle (now six sizes) and clamping are tested. A new test draws the busiest HUD through `ui::draw` in a real egui frame at all six sizes in 1440×900, 960×600 and 1920×1080 windows and checks every panel rectangle against every other and against the damage arcs' reach: none overlap. Disabling the compact layout makes it fail at 120% (the boss panel reaches the arcs). Captures of the busiest HUD at 120% and 130% at both sizes, with an arc pointing straight ahead (`round6/hud-size-1{2,3}0-busy*.jpg`), inspected by eye: nothing collides. The same test showed that from 110% (and, by a fraction of a pixel, at 100% in 960×600) a field note or notice can reach the circle an arc pointing behind you sweeps, so hit marks, arcs and off-screen warnings now draw after notes and notices. |

Final state: 135 unit tests pass (128 before the round), with no warnings. On
the final binary, `--smoke` and `--smoke --gamepad` (twice each),
`--text-review` (normal and `--review-small`, 103 captures across 100
fixtures), `--shader-review`, `--armory-review`, `--survival-review`,
`--world-review` and `--benchmark` exited with status 0. The text review gained
`GRAVEWAKE_REVIEW_ONLY` to capture a subset of fixtures.

Notes:
- The benchmark ran at load 17–28 with presentation dominating every frame
  (surface acquire about 31 ms): 31, 28 and 27 FPS for the optimized 12-enemy,
  48-enemy and corpse scenes. Simulation (0.10 ms) and mesh CPU (0.58 ms) in
  the 12-enemy scene match earlier rounds, so the drop is the shared desktop,
  not game code. In the same period, smoke runs took 25–46 s instead of
  round 5's 8–16 s, whether output went to a file or a pipe.
- The two watchdog checks left two cores (about 80 MB each) in
  systemd-coredump's store; they'll be rotated out normally.

Not verified: macOS (CI builds and runs the unit tests after the push), Windows,
X11, NVIDIA and Intel GPUs, a physical controller (D-pad navigation, the
disconnect pause and everything from earlier rounds), and how 120% and 130%
feel on a real TV.

Deferred, with reasons:
- Elite enemy variants (rest of #8): balance needs playtests.
- Windows validation (#12): nothing here to test on. The browser build (#15).
- A true focus highlight for controller menus: the D-pad now jumps between
  controls, but the only marker is the cursor ring and the control's hover glow.
- Owner decisions, unchanged: release downloads and tag workflows, macOS
  signing and notarization, licences, and whether the synthesized score stays.
