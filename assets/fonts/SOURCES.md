# Gravewake Gothic

**Gravewake Gothic** is the single active font throughout the game: title, headings, card names, prose, buttons, tooltips, settings, HUD values and combat text. It is embedded in the executable and also shipped as `GravewakeGothic-Regular.ttf` in the app's Resources/fonts directory. No system installation or runtime download is needed.

This is an original UI adaptation of **Pirata One**, not a claim of entirely new alphabetic letterforms. The title's uppercase outlines and advances are preserved exactly. Source alphabetic forms remain consistent across the app. The derivative adds centered tabular digits (480 units each), angular timer punctuation, lozenge separators and four matching arrow glyphs. It covers 386 Unicode characters. The family and PostScript names have been changed to Gravewake Gothic; original attribution remains in the font and accompanying license.

- Original letterforms: Rodrigo Fuenzalida and Nicolas Massi, Pirata One. Retained source: `PirataOne-Regular.ttf`.
- License: SIL Open Font License 1.1. Read `gravewake-gothic-OFL.txt` for derivative notes and the complete original copyright/license notice.
- Rebuild: `python3 scripts/build-gravewake-font.py` from the project root, with FontTools installed. The script verifies the source SHA-256, preserves title outlines/metrics, checks ASCII coverage, checks equal digit advances and reopens every output glyph.
- Output checksum and verification facts: `gravewake-gothic-build.json`.

The native UI uses this one face at every semantic text role. Small labels and paragraphs have increased size floors for blackletter readability; pixel-snapped positions, dark reading surfaces and a one-device-pixel shadow preserve contrast. Egui's bundled fallback is retained only for characters outside the font's supported set.

## Retained historical sources

The old Cinzel, Source Sans 3 and Alegreya files remain for provenance, but are no longer embedded or selected by the game. Their original OFL files remain alongside them. Pirata One remains the reproducible source for Gravewake Gothic. The retained source checksums below refer to the unmodified originals, not the new derived font.

## SHA-256

- `Alegreya.ttf`: `ba5564634b93a8f8ba57b48cd4f1ae7417d2b4656fbac779028679b00de3cf12`
- `Cinzel-Bold.ttf`: `3c072961cb1efc76942063a0120c944c2ee848a33d7bc0c3b54fa73beecd9a02`
- `Cinzel.ttf`: `f4d83d34d1f6c741193e4acf4b3dff9531e5a67b6aa65228d00a7db72a4e0f34`
- `PirataOne-Regular.ttf`: `5347a2e155589ecf667d4b766613c8ee003edde9f83717fd24c09599a4b1ecc0`
- `SourceSans3-Bold.ttf`: `9214b9d95e4231c609802815c2646c98174e2102d0d37f88978a7f8e71006e6a`
- `SourceSans3-Semibold.ttf`: `a3f4f8dcf343a8f24dc61951de93f3ba1558b15cd250ba24af8a40e957081b7d`
- `SourceSans3.ttf`: `042fe2cc0b933e328410d7acbd0aa6a1873dca5aef81875f4bc214b08825c7b9`
