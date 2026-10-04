# Audio sources

Recorded gunshots: **The Free Firearm Sound Library**, Ben Jaszczak, Brian Nelson, Kevin Heras, Matthew Nanney. CC0 (no rights reserved).
https://opengameart.org/content/the-free-firearm-sound-library

- `shotgun-a.wav`, `shotgun-b.wav`: near stereo Mossberg Model 190 12-gauge recordings, source `Mossberg/N_30P.wav`, trimmed at 1.690s and 4.954s (1.35s each).
- `pistol.wav`: near stereo 1911 recording, `1911/A_42P.wav`, trimmed at 0.935s for 0.85s.

Reload Foley: **Shotgun Reload Sound Effects**, zer0_sol. CC0.
https://opengameart.org/content/shotgun-reload-sound-effects

- `shell-in.wav`: `Shell in Chamber.mp3`, 0.505–0.905s.
- `rack.wav`: `Rack.mp3`, 0.605–1.085s.

Sources checked October 4, 2026. Original downloads are in `reference/audio/`. Preparation is reproducible with `scripts/prepare-audio.py`: trims, 45Hz high-pass, short edge fades, peak normalization, conversion to stereo 48kHz PCM. Runtime adds subtle pitch variation and a quiet designed impact layer. Other Foley, foil tearing, card motion and ambience are authored in `src/audio.rs`.
