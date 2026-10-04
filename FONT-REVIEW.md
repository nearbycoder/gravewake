# Gravewake Gothic review

The game now uses one embedded font family for its title, menus, cards, prose,
tooltips, combat text and HUD. Gravewake Gothic is an OFL-licensed adaptation of
the title's existing Pirata One face. The alphabetic letterforms are retained;
the title's outlines and advances are verified byte-for-byte during the build.
The customizations are tabular combat numerals, angular punctuation, separators,
arrow symbols and the font's own family metadata. Attribution and the complete
source license ship with the font.

The build script validates all ASCII characters, 386 Unicode mappings, equal
digit advances and every output glyph. Current UI text is covered without
requiring fallback. Unsupported future characters can still use egui fallback.

Native review generated 75 screenshots covering 72 fixtures at each of
1440 × 900 and 960 × 600. The galleries and exact capture lists are in
`captures/legibility/normal/manifest.json` and
`captures/legibility/small/manifest.json`. Representative title, HUD, armory,
binding, collector, settings, power-selection and revealed-pack screens were
visually inspected. Button labels were increased to 22 design units; generic
labels and prose have an 18-unit floor, with a 13.5 logical-point minimum at
smaller window sizes. Positions retain pixel snapping and contrasting surfaces.

The real-font specimen is `captures/typography/gravewake-gothic-specimen.png`.
The standalone font is `assets/fonts/GravewakeGothic-Regular.ttf`; its checksum
and build facts are recorded in `assets/fonts/gravewake-gothic-build.json`.

Validation: `cargo test --locked` passed all 84 tests. The release app was rebuilt
and its ad-hoc signature verified with `codesign --verify --deep --strict`.
The release executable and packaged executable have matching Mach-O UUIDs.
The final native smoke run passed combat, rewards, pack tearing/reveal/selection,
equip, upgrades, next-round transition and save roundtrip. The packaged app was
reopened and its title and armory typography visually confirmed.
