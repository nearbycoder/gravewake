# Gravewake — The Hollow Tithe

A native Rust gothic arena roguelite: answer the cracked funeral bell, survive Mournhollow, and pay the Collector's tithe in souls. Gravewake has its own name, bell-and-thorn identity, oxblood pack artwork and card backs. It began as a study of the supplied arena/card-shop references; source attribution remains below. Reference images and videos remain in the local working archive but are excluded from the public repository. Geometry, animation, interface interactions, and audio playback are implemented in Rust. The fidelity pass adds painted textures and a generated dealer/table background in `assets/`; these are embedded in the executable.

## Play

Double-click **Play Gravewake.command**, or open **dist/Gravewake.app** after packaging.

Choose **Quit Game** on the title screen. During a run, press **Esc** and choose **Save & Quit Game**; use **Continue Your Descent** next time to resume. **Cmd+Q** and the window close button also save and exit. Quitting weapon practice preserves your existing run.

```sh
./scripts/package-macos.sh
open "dist/Gravewake.app"
```

For development (if Cargo is not on your PATH, use `~/.cargo/bin/cargo`):

```sh
cargo run
cargo test
cargo run -- --smoke
```

The smoke run launches the actual Metal renderer, shoots through a fixed eight-enemy encounter using the normal combat rules, checks the 90-gold reward, buys a pack, uses real egui pointer events to tear it open, reveal the cards, select and equip one, purchases an upgrade, checks serialization, and enters the next round. It saves screenshots to `captures/` and never writes the player's save file.

## The loop

Start with a common Worn Iron pistol. Survive twelve descents through Mournhollow. Each new run draws its own random seed, so spawn positions, Collector draws and pack contents differ between runs; a saved run keeps its seed. Tests, the smoke run and review captures use a fixed seed so they stay reproducible. Between descents, The Collector restores 25 vitality and pays 90–140 gold. Spend it on random weapons, a visible weapon offer, a Hollow Chalice, armor, or Armory Packs. Pack prices increase by 15 gold per purchase and reset at the next shop.

Packs contain three cards. Turn them over individually or reveal all, select one, then take and equip it. Common, uncommon, rare, and legendary cards have different power and treatments. All cards use the standard finish. The armory contains 33 weapons across sidearms, scatterguns, longarms, ordnance, occult implements and melee. See [the complete armory](ARMORY.md) for all 30 additions and their mechanics.

Pack choices and the Collector's offer show an estimated sustained damage per second, counting pellets, burst fire, a full magazine and its reload (melee: damage per swing). The line is inked green when the card beats your equipped weapon and red when it is weaker. The estimate is single-target: splash, piercing, chains, elemental effects and soul powers are not included, so area weapons such as the Gravedigger are worth more against crowds than the figure suggests.

Click the equipped card or **Upgrade Card** to open The Binding. Three paths each contain five sequential upgrades: damage, fire/reload speed, and mana regeneration. Completing one path unlocks Soul Siphon, which heals on damage. Upgrades belong to the card and persist across rounds. Switching to a newly drawn weapon equips that new card's upgrades.

Twelve enemy archetypes appear, including Gloamwing dive hunters, six-legged Grave Crawlers, armored Iron Penitents, exploding Plague Vessels, levitating Ash Cantors, Bell Gargoyles, summoning Bone Shepherds, and blinking Tithe Reapers. The Tithekeeper returns on descents 4, 8 and 12 with increasing strength. Enemies have animated head, torso, arm, and leg hit regions. Headshots can detach skulls; severed arms weaken attacks and drop the weapon arm; losing one leg causes a limp and losing both causes a crawl. Deaths create Rapier ragdolls using the surviving body sections.

## Expanded Mournhollow grounds

The arena now spans 96 × 96 metres, about 7.5 times its former nominal playable footprint. Five connected districts provide an open court, ruined chapel, cloister, bell sanctuary and grave orchard. Ten original Blender architecture modules, worn connecting paths, soil and moss shading, smoother wooded banks and 18 braziers replace the close perimeter cage. Major geometry shares collision with player movement, enemy navigation, shots and body physics.

A district label, compass and final-threat bearing help with orientation. Reinforcements stay near the player throughout the larger map, and the final soul drops return promptly. Static geometry is indexed and divided into visibility batches; only the closest six braziers contribute point lights. See [the expansion and native verification](WORLD-REVIEW.md) for captures, traversal checks and reproduction commands.

## Controls

| Input | Action |
| --- | --- |
| W A S D | Move |
| Mouse | Look |
| Left mouse (hold) | Fire |
| R | Reload |
| Shift | Sprint |
| Space | Dodge, with a short invulnerable interval |
| E | Melee |
| Q | Ember Bolt, after binding the Hollow Chalice |
| Escape | Pause / back |
| F11 | Toggle fullscreen |

Mouse sensitivity, invert look, field of view and sound volume are available in Settings & Controls and persist in `settings.json` beside the run save. Missing or out-of-range values fall back to defaults or are clamped. Losing window focus pauses combat and releases the pointer.

## Save data

The active run is saved atomically to:

```text
~/Library/Application Support/Gravewake/run.json
```

On Linux the folder is `$XDG_DATA_HOME/gravewake/` (normally `~/.local/share/gravewake/`). Linux builds that predate this used the macOS path above under the home directory; those files are imported once, preferring them over any older Dark Veil copies.

On first launch, legacy run, graphics and performance files are copied from `~/Library/Application Support/Dark Veil/` into the Gravewake folder. Originals remain untouched, and existing Gravewake files are never overwritten. If copying fails, loading falls back to the old files.

Pausing, closing the window, finishing a descent, and making purchases save progress. Continue restores position, enemies, health, gold, weapon affixes, upgrades, soul level, collected powers, uncollected orbs, queued level choices, and remaining reinforcements. Body-part damage and missing limbs are saved; older saves load with intact anatomy. Temporary visual particles and rigid-body debris are not serialized.

## Engine

- **wgpu / Metal**: custom forward renderer, procedural mesh batches, 18 braziers with six nearby point lights, moon lighting, distance fog, a mipmapped material atlas, an HDR scene target, fire bloom, contact shadows, and a final vignette composite.
- **winit**: native window, raw mouse input, pointer capture, and focus handling.
- **Rapier 3D**: fixed-step articulated ragdolls with ten bodies, elbow/knee hinges and limited neck/shoulder/hip joints; procedural fracture debris, impact-point impulses, continuous collision detection, friction, sleeping and world collisions. Player locomotion shares the world layout's boundaries, wall and pillar collision shapes.
- **egui**: custom-painted HUD, shop, card treatments, card reveals, upgrade paths, collection, and bestiary. Weapon and chalice previews are rendered offscreen on the GPU using the same textured geometry and materials as gameplay.
- **rodio**: recorded CC0 firearm reports and reload Foley, layered with authored impact, foil, card and ambient sounds. Audio device failure is nonfatal.
- **serde**: versioned JSON save data with atomic replacement.

The lockfile pins the dependency set used for this build. Typography uses **Gravewake Gothic**, a custom adaptation of the title font, throughout headings, menus, cards, descriptions, tooltips and HUD numbers. Its alphabetic letterforms preserve the title design; custom tabular figures and angular punctuation support the interface. Rebuild the standalone TTF with `scripts/build-gravewake-font.py` (requires FontTools). Font credits and SIL Open Font Licenses are in `assets/fonts/` and included in the app bundle. The executable embeds this font, its WGSL shaders, seven art assets and the audio samples; it needs no loose files alongside the app.

## Identity

**Gravewake: The Hollow Tithe** uses a cracked bronze funeral bell, a crown of thorns and a verdigris soul flame. The arena is **Mournhollow**, the merchant is **the Collector**, and the recurring boss is **the Tithekeeper**. The app bundle, Dock icon, launcher, window title, menus, pack wrapper and card backs carry the new identity. The name is a creative choice, not a trademark-clearance claim.

Regenerate the macOS icon from `assets/gravewake-emblem.png` using `./scripts/build-icon.sh`. Packaging installs `assets/Gravewake.icns` in the app. Previous branded art and prompts are archived in `reference/legacy-branding/`; they are not embedded in the executable. Original third-party reference materials and credits keep their historical names.

## Scope and references

The supplied archive contained 17 images and two reference documents, but no original code, mesh assets, or embedded videos. Four video files were subsequently supplied and copied into `reference/videos/`. `reference/INSTRUCTIONS.md` is historical reference material containing quoted prompts; it is not an executable project instruction file.

The reference game is **Dark Veil — The Blackwood**, by Thomas Ricouard (@Dimillian). See `reference/SOURCES.md` for the original post and media URLs. The shop uses an imagegen-cleaned background plate based on the supplied shop screenshot. Its cards, text, models, buttons, pack reveals, and upgrade interactions are rendered separately. The arena is entirely real-time 3D. See `assets/PROMPTS.md` for the generated asset paths and prompts.

This version targets native macOS. Browser/WebGPU deployment, the reference game's full item/affix catalogue, and exact visual or balance parity are future work. Detached limbs and corpses are physical, while living enemies use procedural animation with localized recoil and injury poses. Conventional gunfire uses hit tests; bolts, arrows, grenades and occult orbs travel as simulated projectiles. No video was copied into the game as a substitute for playable content.

## Visual fidelity pass

The revised version replaces the flat shop blockout with a cleaned dealer/velvet/coin/candle background; aged engraved fronts and bell-and-thorn backs; a metallic foil booster wrapper; textured GPU-rendered item previews; hover glints; staggered reveal animations; and a card that lifts from the deck into its upgrade screen. The arena now has irregular layered pine branches, painted bark/stone/earth/bone/metal materials with mip filtering, rounded skulls with sockets and teeth, articulated fingers and toes, tattered cloth, an antlered Warden, hollow gun barrels, and warm fire halos.

`cargo test` covers a full twelve-descent run through the Tithekeeper to victory, card economics, upgrade persistence, shooting, death/victory state separation, and physical debris collision/expiry. `cargo run -- --smoke` writes eight inspectable native-renderer screenshots including the sealed pack and revealed selection.

## Motion and sound pass

The pack uses a 2-second sequence: foil tension, a jagged progressive tear, curling detached strip, foil fragments, staggered card emergence, fan-out and eased landing. Full card surfaces (including text and GPU weapon previews) rotate in perspective through the reveal. Taking a card animates it into the deck. Interaction waits for the relevant movement to finish.

The double barrel now opens on a mechanical hinge, ejects two spent casings, seats two fresh shells one at a time, and snaps closed. The support hand moves between the fore-end and the breech. Pistol and repeater reloads remove and replace their magazines, then rack their actions. Reload durations are weapon-specific and scale with upgrades. Foley cues share the same normalized timeline; ammunition refills only after completion. Recoil has a fast impulse and damped recovery independent of muzzle flash, with view kick, mouse sway, moving fingers/hands, brief multi-part flashes and fading muzzle smoke. Skeletons use lifted swing feet, bent knees, counter-swinging arms, strike follow-through and hit flinch.

Shotgun and handgun reports use actual stereo firearm recordings; see [audio source credits](assets/audio/SOURCES.md). Shell insertion and closing cues also use recorded Foley. Samples are cached before play and vary subtly between shots. Original large audio downloads stay local and are ignored by Git.

To reproduce the 19-second motion review (the normal renderer and UI handlers, with a disposable staged run):

```sh
./scripts/capture-motion.sh
```

The script writes `captures/motion-review.mp4`, including synchronized audio from the same sound bank used by the game. It exercises pack tear, reveals, equip, double-barrel shots/reload, pistol reload and repeater reload, and never modifies the player's save. `cargo test` also checks reload event ordering at 30 and 144 updates/second, ammo timing, and audio bounds/tails. This remains procedural character animation, not the reference's original rigs or motion data.

## Menu polish and expanded armory

Menu buttons now use an engraved burgundy leather and brass plate with preserved decorative end caps, hover lighting, press travel, and serif labels. Settings is a parchment Hunter's Journal with matching sliders; pause, confirmation and bestiary details use the same material treatment. Upgrade nodes are textured gilded medallions, and the vitality HUD uses an engraved frame.

The Armory browser provides six category tabs, animated model cards, rarity previews, descriptions, combat statistics, and a practice action for every weapon. Practice keeps the active run in memory and disables save writes until returning to the catalog. It can be entered from the title or shop. Press Escape to leave practice.

Run `cargo run -- --armory-review` to render the journal, pause, confirmation, six weapon families, and a melee practice interaction through the real native UI. Tests exercise all 33 weapons, trait mechanics, burst/reload behavior, loot coverage, save serialization and practice restoration. The new plate and its built-in imagegen prompt are documented in `assets/PROMPTS.md`.

## Firearm modeling pass

The initial firearm pass added assembled models in `src/guns.rs`: smoothly shaded barrel profiles with recessed bores, rounded receivers, shaped walnut stocks/grips, curved trigger guards, inset side locks, scroll inlay, screws, ejection ports, sight ribs and distinct family silhouettes. The revolver has an exposed six-chamber cylinder; the Gatling has a six-barrel bank and drum; the blunderbuss has a flared bell; the longrifle has a mounted scope; elemental reservoirs have metal cages. Barrel groups, receiver parts and actions retain separate reload transforms. Muzzle effects follow each firearm's actual length.

Steel, brass, walnut and matte leather have separate shader treatments with metal highlights, subtle patina and wood grain. Armory thumbnails fit projected model bounds and use a more revealing three-quarter view. Most weapon silhouettes remain procedural Rust meshes. The weathered arsenal pass below adds Blender-authored replacement assemblies and baked material maps. Blender is an authoring tool only; the game needs no external modeling software.

`cargo run --release -- --gun-review` writes 44 native-renderer captures to `captures/guns/`: all 33 weapon previews, nine first-person views and two reload poses. The mesh test checks finite positions, unit normals and a vertex budget through six reload poses. The normal motion-review capture also exercises firing and reloads with the new assemblies.

## Hollowlight custom shaders

The custom WGSL world and composite shaders now add:

- Depth-reconstructed contact occlusion, height-integrated drifting ground mist, and depth-clipped warm scattering around braziers.
- Damp flagstone highlights and torch reflections on blued steel and brass, cool bone rim lighting, and local muzzle illumination.
- Gentle foliage wind, emissive pulses, and animated ember/frost/venom treatments on affected enemy surfaces.
- Bloom at three scales, conservative edge smoothing, warm/cool color grading and filmic highlight compression. The interface is drawn afterward and stays sharp.

**F6** toggles the treatment. **Settings & Controls → Hollowlight** adjusts intensity from 0 to 100%. The preference is saved separately from the run in `~/Library/Application Support/Gravewake/graphics.json`; 0 uses the previous composite treatment and skips the new depth effects. These are stylized shader effects, not ray-traced reflections or volumetric shadow maps.

`cargo run --release -- --shader-review` captures baseline/full/half intensity, firelight, muzzle lighting, elemental effects, the live settings control and a resized window. It tests the actual slider through egui input and reports frame intervals excluding setup/capture frames. On the development M4 Max, the 1440×900 release review remained near the 60 Hz presentation cap with both the original and Hollowlight treatments (roughly 16.7 ms/frame); this is an observed scene result, not a GPU-only timing or a guarantee for every machine. The review uses a disposable run and never writes user saves or preferences.

## Location-based damage and body physics

`src/anatomy.rs` shares the animated skeletal pose between hit detection and rendering, including Warden scale, floating Cinder Skulls, attack swings, recoil, and the injured stance. Bullets use nearest capsule intersections; projectiles sweep their traveled segment to avoid tunneling. Missing parts have no hit volume. Head hits deal double health damage, limb hits deal 80%, and structural damage accumulates independently. Limb thresholds scale with enemy health and cap at 120 damage. Ordinary enemies die on decapitation; the Warden can continue headless.

A surviving enemy switches to the remaining arm after disarm. One or two lost arms reduce attack damage to 55% or 20% and slow attack cadence. One lost leg reduces movement to 48%; two reduce it to 20%, with an eased collapse, planted crawling hands, and a raised neck. Melee swings choose a body region from their aim; explosive direct impacts retain the hit region and splash reaches the nearest surviving body surface.

Sever events preserve the exact animated pose at impact: skull sockets/teeth, antlers, fingers, toes, and the Skirmisher's sword carry into physics without a preset death pose. A complete humanoid has ten physical sections and nine joints, including separate upper/lower arms and legs. Knees and elbows hinge within anatomical limits; neck, shoulders and hips have limited rotation. Detached arms and legs keep their elbow/knee joints. Missing limbs stay missing, and capped bone/marrow surfaces cover the wounds on both sides of a sever.

Heavy shotgun, ordnance, hand-cannon, slug and heavy-melee impacts can fracture skull or torso geometry along procedurally chosen planes, producing two to four capped mesh fragments. Ordinary limb detachment follows the authored anatomical sections. Subsequent shots, melee strikes and explosions kick or tumble existing remains, with walls blocking these impulses; they do not award additional kills or experience. Head-only creatures use their own shaped collider. The fixed-step solver supports continuous collision detection and sleeping. The pool is capped at 144 physical sections, evicts complete old groups, and expires remains after 18 seconds.

```sh
./scripts/capture-anatomy.sh
```

This records a 24-second native Metal review using real weapon fire: decapitation, articulated disarm, limp/crawl, a complete ten-body collapse, heavy mesh fracture, and a second explosion moving existing debris. It writes `captures/anatomy-review.mp4` with synchronized game audio and a diagnostic manifest, and never writes player saves or settings. Assertions verify pose handoff, independent joint motion, missing-part persistence, finite motion and the body budget. Unit tests also cover rotated/scaled hit regions, cover, fast projectiles, injury consequences, save compatibility, capped fracture geometry, settling and expiry. See `RAGDOLL-REVIEW.md` for this pass's verification.

The supplied `reference/videos/ragdoll.mp4` informed this pass. The linked browser reference at https://dark-veil.dimillian.chatgpt.site/ loaded into its arena, but its pointer-capture control was blocked in the in-app browser, so combat comparison used the supplied footage.

## Weathered arsenal and Blender assets

All 33 weapons use an aged material palette: pitted iron with localized brown corrosion, oxidized brass, worn walnut, and oil-darkened leather. Four 512-pixel Blender material bakes form an albedo atlas and a linear roughness/height/metalness atlas. The shader uses those maps for muted highlights and surface pitting, including in armory previews. UVs are attached to the model rather than the camera, so aiming and reloads do not move the texture across its surface. Environmental materials retain their existing treatment.

Bone and arsenal shading now use a darker, muted palette to match the arena. Worn metal has softer highlights driven by moonlight and nearby flames, reduced camera fill, and less albedo amplification; the armory retains its separate inspection lighting. Skeletons use weathered ash-ivory with a restrained silhouette rim. Ragdolls retain their authored colors instead of being recolored pale on death. Matched native captures are saved in `captures/material-tone/before` and `captures/material-tone/after`.

Seven weapons now have Blender-authored assemblies: Iron pistol, Double shotgun, Butcher Cleaver, Grave Revolver, Slugbreaker, Repeater, and Longrifle. They use shaped receivers and stocks, smoothly shaded barrels, fitted locks, dark open bores and distinct moving actions. The revolver cylinder, shotgun hinge, pump, magazines, and repeater lever preserve their mechanical groups. The other 26 designs retain their existing geometry and receive the revised shared textured finish. Rarity remains visible in small inset jewels.

Editable source: `assets/blender/weathered-armory.blend`. Rebuild with `./scripts/build-weathered-armory.sh`; source/material details are in `assets/weapons/SOURCES.md`. The exported meshes and texture maps are embedded in the app. Asset tests check finite geometry and UVs, mechanical groups, stable reload UVs, and weathered material coverage for every weapon.

## Souls and survival

Every defeated enemy drops a green soul orb; stronger enemies are worth more experience and bosses drop a gold orb worth 60. Drops bob above the ground, persist until collected, and accelerate toward you within pickup range. On a cleared wave, the remaining souls sweep toward you before the shop opens, so no experience is lost. The HUD shows soul level, experience, active powers, live enemies, and approaching reinforcements.

Leveling freezes combat and offers three distinct engraved power folios. Click one or press **1 / 2 / 3**. Ten powers each have five ranks: weapon damage, attack recovery, pickup radius, maximum health, healing on pickup, automatic lightning, orbiting blades, frost pulses, retaliatory thorns, and movement speed. Multiple earned levels queue separate choices. Fully ranked powers leave the pool; after all fifty ranks, further levels restore health and grant armor. These powers persist for the run, independently of weapon-card upgrades.

Reinforcements arrive in batches, with new enemy types introduced over the opening four descents. Waves contain 19–74 enemies before summons; no more than 48 living enemies can crowd the arena. Ranged attacks travel as dodgeable projectiles. Dives, blasts, summons and blinks have visible warnings. Ground creatures and low flyers separate and navigate shared obstacle footprints; the higher aerial poses retain their flight animation. After descent twelve, **The Tithe Never Ends** continues the build with stronger waves (reinforcement quota capped at 180).

`cargo test` covers all twelve species, twelve-descent combat progression, soul drops without duplication, collection and final-wave vacuum, saved/queued choices, caps and legacy defaults, automatic powers, ranged warnings, summon bounds, and existing weapons/anatomy/physics. `./scripts/capture-survival.sh` produces `captures/survival-review.mp4` from the native Metal renderer, visits all twelve bestiary entries, renders a 48-enemy stress scene, then records combat, soul collection and power choices through real UI handlers. The capture uses a disposable fourth-descent showcase with three initial automatic powers and never writes the player's save.

## Performance

The renderer now instances repeated enemy shapes on the GPU, caches primitive geometry, reuses frame buffers, and skips offscreen rendering while preserving gameplay simulation. Unlocked presentation is the default. **F7** toggles VSync; **F8** shows the FPS counter. Both are also in Settings & Controls and are saved between launches. Minimized/occluded windows suspend continuous rendering.

Run `./scripts/benchmark.sh` for a repeatable native benchmark. See [performance results and methodology](PERFORMANCE.md) for measured frame times, matched reference images, and limitations.

## Controllers

`src/gamepad.rs` polls controllers through gilrs (evdev/udev on Linux, IOKit on macOS) and maps them in pure functions that tests can drive without hardware. In the arena, sticks have radial deadzones, and the look stick uses a squared response; its turn rate follows the Aim sensitivity slider and invert-look setting. Menus use a virtual cursor that sends ordinary egui pointer events, so every screen (shop, packs, the Binding, Armory, journal) works without a separate navigation layer; moving the mouse hands control back to it. The most recently used controller drives the game, connections and disconnections show a notice, and a missing or inaccessible device leaves keyboard and mouse play unchanged. Linux builds need `libudev`.

`cargo run --release -- --smoke --gamepad` runs the normal smoke test but tears, reveals, selects and equips the pack by steering the controller cursor with synthetic stick input and pressing A. No physical controller has been tested yet.

## Positional audio

Enemy sounds come from where they happen. Special-attack wind-ups, blasts and melee swings are panned with equal power by bearing relative to your view, attenuated with distance, and slightly softened behind you, so a threat out of sight can still be heard and located. Each special attack has its own wind-up cue, starting with its visible warning: a rising intake before caster bolts, a low growl before slams and plague bursts, a falling screech before dives, a hollow bell before a Bone Shepherd summons, and a reversed whoosh before a Tithe Reaper blinks. Your own weapons, pickups and interface sounds stay centred. The cues are synthesized like the other designed sounds; `--export-audio` writes them to `captures/audio/`.

![Spectrograms of the five wind-up cues](docs/media/improvements/round2/warning-cue-spectrograms.jpg)

## Combat feedback

Your own weapon hits flash a marker around the reticle: ivory for a body hit, gold for a headshot, and a larger red mark with a short tick for a kill. Pellets and splash that land together show the strongest result and tick once. Automatic powers do not trigger markers. When you take damage, a red arc around the reticle points toward each source (the striking enemy, a blast's caster, or the direction a projectile came from) and fades over 1.2 seconds; up to four arcs show at once. **Reduce flashes** in Settings & Controls softens the full-screen hurt vignette and muzzle lighting to 35%.

![Kill marker and damage arcs](docs/media/improvements/hud-kill-and-damage-arcs.jpg)

## Text legibility review

HUD numbers and labels have dedicated opaque dark surfaces, with armor separate from the ornamental vitality plate. Card text is printed in dark ink on a neutral parchment field independent of rarity. Folios soften their background engraving behind reading areas. Every control uses Gravewake Gothic, including compact labels and numbers; long card names and prose wrap using measured font widths. Generic labels and paragraphs have an 18-design-unit floor, with a minimum of 13.5 logical points after window scaling. The title glyph shapes and spacing are preserved exactly.

`cargo run --release -- --text-review` captures the actual native UI in 78 screenshots across 75 fixtures: every weapon, every creature, every power description, all four rarities, stressed HUD values, reload/melee, hit markers and damage arcs, a 90° field of view, pack opening, upgrade tooltips, settings, confirmation, pause and endings. Add `--review-small` for 960×600. Results and a manifest go to `captures/legibility/normal` or `captures/legibility/small`. Review fixtures freeze combat and never load or write player saves.

## Blender creature and cemetery overhaul

All twelve creatures now use original Blender meshes: recessed skull cavities, separate mandibles and teeth, curved ribcages, shaped pelvis and joints, articulated hands and feet, thicker forged armor, folded garments, cupped wing membranes, lantern cages, and curved blades. Their meshes share the existing anatomical pose, so headshots, dismemberment, injury poses and ragdolls use the same body parts visible in gameplay. Animation remains a rigid segmented rig; garments do not use cloth simulation.

The cemetery uses eleven Blender assets for conifers, carved grave markers, masonry, buttresses, a fitted pointed arch with iron tracery, and hollow bowl braziers. Near trees have individual needle bundles and gaps between twigs. Collision and fire locations are preserved.

Editable sources are `assets/blender/gravewake-creatures.blend`, `assets/blender/mournhollow-environment.blend`, and `assets/blender/weathered-armory.blend`. Rebuild creatures with `./scripts/build-creatures.sh`, scenery with Blender's `--background --python scripts/build-environment.py`, and weapons with `./scripts/build-weathered-armory.sh`. Asset manifests and provenance live in the corresponding `assets/enemies`, `assets/environment`, and `assets/weapons` directories.

Creature geometry stays in indexed GPU buffers. Only rig transforms and combat state are uploaded each frame; lower-detail Blender exports are selected beyond eight and fourteen meters. Full geometry remains available for close views and detached body parts.

`cargo run --release -- --model-review` produces twelve close creature portraits, an environment overview, three weapon views, and a 48-enemy rendering stress capture in `captures/models/after`. The manifest reports 240 timed frames after 60 warm-up frames, excluding screenshots. AI is frozen in this diagnostic; it is a rendering comparison, not a full gameplay benchmark. `--model-cpu` exercises the full-detail CPU fallback in a separate gallery. Review fixtures never load or save player progress.
