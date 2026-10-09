# Gameplay trailer

`media/gravewake-trailer.mp4` is a 45-second, 1920 × 1080, 30 fps H.264/AAC
cut (about 25 MB, `+faststart`). It was recorded on October 8, 2026, after
twelve improvement rounds, from the native renderer (wgpu on Vulkan) on Linux,
inside a private virtual KWin, at the **Ultra** Graphics Fidelity step.

The footage comes from the game's scripted review modes. They step the
simulation a fixed 1/60 s per frame and save every second frame, so each
recording is a steady 30 fps whatever a frame costs to draw; gameplay plays at
its recorded speed, without time remapping. Encounters are staged,
deterministic review fixtures rather than one continuous player run, and the
menu, HUD and journal shots are the text review's native fixtures held as
stills. Nothing is prerendered or taken from reference footage. The edit adds
captions and the opening, chapter and closing cards; the anatomy shots carry
the review's own scene names in the game's notice bar.

The trailer shows mixed-enemy combat with automatic powers, the shader fire
and brazier shadows Ultra adds, a break-open shotgun reload, location damage
and mesh fracture, a pack torn, revealed and equipped through the real UI with
its menu sounds, the Collector's next-descent preview, a level-up over the
blurred arena, a first-sighting creature note, the pause ledger, the journal's
Display page, the death recap and the title's chronicle.

Sound is the game's own: the reviews record their sound effects in sync, and
with `--capture-full-mix` also the adaptive score (following the game's state
frame by frame, as the live mixer does), the ambience and the menu ticks and
clacks, at the levels a player hears with the default volumes. The stills
carry the score's calm layer. There is no narration or external music. The
edit is raised to −16 LUFS ahead of a fast limiter, which leaves about
−18.5 LUFS with true peaks near −2 dBTP once encoded.

`media/trailer-manifest.json` records every shot's source, in-point, duration,
caption and the fidelity step. `media/trailer-poster.jpg` is the Ultra title
screen with a play label, `media/gameplay-preview.gif` an eight-second,
640-wide excerpt, and the README gallery and hero image come from the same
captures.

The README links the poster to the file in the repository. An earlier cut was
also uploaded as a GitHub attachment for an inline player; that upload still
holds the October 4 cut, so it is no longer linked.

## Reproduce

Requires Linux with KDE Plasma 6 (`kwin_wayland`, `dbus-run-session`), Rust,
FFmpeg and Python with Pillow. From the repository root:

```sh
scripts/capture-trailer.sh                 # FIDELITY=high for another step
python3 -m venv .venv-media
.venv-media/bin/pip install Pillow
.venv-media/bin/python scripts/build-trailer.py
```

`capture-trailer.sh` builds the release binary and runs the motion, survival,
anatomy and text reviews, each in its own directory under `captures/trailer/`
with a throwaway `XDG_DATA_HOME`, inside `scripts/nested-kwin.sh` on a
2560 × 1440 virtual output, with `--capture-size 1920x1080` and
`--capture-fidelity`. Only scripted runs read those flags. It then encodes
each review with its sound track and exports the score. `build-trailer.py`
cuts the shots, adds captions in Gravewake Gothic, normalises loudness,
encodes the trailer (failing above 45 MB), and writes the poster, the GIF
(at most 10 MB), the gallery and the manifest into `docs/media/`.

Raw frames (several gigabytes) and intermediate edits stay in the ignored
`captures/` directory; only the curated exports are tracked.
