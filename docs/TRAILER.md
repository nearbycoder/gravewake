# Gameplay trailer

The showcase is edited from Gravewake's native Metal renderer. It uses the
current font, models, material treatment, UI and body physics. Gameplay portions
run at their recorded 30 fps without time remapping. Encounters use deterministic
review fixtures rather than a continuous player run. The armory inserts are
native UI stills; the opening and closing cards are editorial graphics.

The trailer includes combat against mixed enemies, automatic powers, a shotgun
reload, location-based damage, physical mesh fracture, three weapon families,
pack tearing/reveal/equip, and a soul-power choice. Sound comes from the game;
there is no external soundtrack or reference-game footage.

`media/gravewake-trailer.mp4` is the full-quality 1280 × 720 H.264/AAC export.
`media/trailer-manifest.json` records every source, in-point and duration.
The README uses an eight-second animated preview linked to the full-quality
video. The edit also produces a smaller attachment-ready copy under `captures/`.
The repository retains a static poster as an alternative cover.

## Reproduce

Requires macOS with a graphical session, Rust, FFmpeg, and Python with Pillow.
Run these commands from the repository root:

```sh
cargo build --release --locked
./scripts/capture-survival.sh
./scripts/capture-anatomy.sh
./target/release/gravewake --motion-review
ffmpeg -y -framerate 30 -i captures/motion/frame-%04d.png \
  -i captures/motion-review.wav -c:v libx264 -crf 19 -pix_fmt yuv420p \
  -c:a aac -b:a 192k -shortest -movflags +faststart captures/motion-review.mp4
./target/release/gravewake --text-review
./target/release/gravewake --text-review --quit-review-title
mkdir -p captures/title-cleanup
cp captures/quit/title/00-title.png captures/title-cleanup/title-normal.png
python3 -m venv .venv-media
.venv-media/bin/pip install Pillow
.venv-media/bin/python scripts/build-trailer.py
```

The captures use disposable game state. Large raw frame sequences and temporary
edits remain under the ignored `captures/` directory. Only curated exports are
tracked. Native screenshots retain their full UI; the hero image is cropped from
the start screen and the trailer poster adds a play label.
