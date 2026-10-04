"""Build Gravewake's OFL-licensed UI face from the existing title letterforms.

Run with Python + fonttools. No system font installation is required.
Alphabetic outlines/metrics are deliberately unchanged: GRAVEWAKE must remain
identical to the title the player approved. Numerals and interface punctuation
are adapted for the HUD. The renamed derivative retains its source license.
"""
from pathlib import Path
import hashlib
import json
from fontTools.ttLib import TTFont
from fontTools.pens.ttGlyphPen import TTGlyphPen

ROOT = Path(__file__).resolve().parents[1]
DIR = ROOT / "assets/fonts"
SOURCE = DIR / "PirataOne-Regular.ttf"
OUTPUT = DIR / "GravewakeGothic-Regular.ttf"
assert hashlib.sha256(SOURCE.read_bytes()).hexdigest() == "5347a2e155589ecf667d4b766613c8ee003edde9f83717fd24c09599a4b1ecc0"
font = TTFont(SOURCE, recalcTimestamp=False)
cmap = font.getBestCmap()
glyf = font["glyf"]
metrics = font["hmtx"].metrics
title = {c: (glyf[cmap[ord(c)]].compile(glyf), metrics[cmap[ord(c)]]) for c in "GRAVEWAKE"}

# Common advances and centered source outlines prevent counters from changing
# width as ammunition, health, timers, currency and damage values change.
for c in "0123456789":
    name = cmap[ord(c)]
    glyph = glyf[name]
    glyph.recalcBounds(glyf)
    offset = round((480 - (glyph.xMax - glyph.xMin)) / 2 - glyph.xMin)
    for i, (x, y) in enumerate(glyph.coordinates):
        glyph.coordinates[i] = (x + offset, y)
    glyph.recalcBounds(glyf)
    metrics[name] = (480, glyph.xMin)

def polygon(pen, points):
    pen.moveTo(points[0])
    for p in points[1:]:
        pen.lineTo(p)
    pen.closePath()

def lozenge(x, y, radius):
    return [(x-radius, y), (x, y+radius), (x+radius, y), (x, y-radius)]

def glyph(code, name, width, contours):
    pen = TTGlyphPen(None)
    for points in contours:
        polygon(pen, points)
    value = pen.glyph()
    if name not in glyf.glyphs:
        font.setGlyphOrder(font.getGlyphOrder() + [name])
    glyf[name] = value
    value.recalcBounds(glyf)
    metrics[name] = (width, value.xMin)
    for table in font["cmap"].tables:
        if table.isUnicode():
            table.cmap[code] = name

# Cut-stone punctuation echoes the pointed terminals of the title, with enough
# ink for small HUD timers and decimal statistics. ASCII remains fully covered.
glyph(ord(":"), "colon", 230, [lozenge(115, 150, 57), lozenge(115, 470, 57)])
glyph(ord("·"), "periodcentered", 240, [lozenge(120, 310, 57)])
glyph(ord("•"), "bullet", 320, [lozenge(160, 310, 85)])
right_arrow = [(45,265),(350,265),(350,145),(560,330),(350,515),(350,395),(45,395)]
glyph(0x2192, "gravewakeArrowRight", 610, [right_arrow])
glyph(0x2190, "gravewakeArrowLeft", 610, [[(610-x,y) for x,y in reversed(right_arrow)]])
glyph(0x2191, "gravewakeArrowUp", 610, [[(y-25,x+25) for x,y in reversed(right_arrow)]])
glyph(0x2193, "gravewakeArrowDown", 610, [[(y-25,635-x) for x,y in right_arrow]])

# Old font signatures and build timestamps no longer describe this derivative.
for tag in ("DSIG", "FFTM"):
    if tag in font:
        del font[tag]
names = font["name"]
names.names = [n for n in names.names if n.nameID in (0, 13, 14)]
values = {
    1: "Gravewake Gothic", 2: "Regular", 3: "GravewakeGothic-1.000",
    4: "Gravewake Gothic", 5: "Version 1.000", 6: "GravewakeGothic-Regular",
    8: "Gravewake", 9: "Based on letterforms by Rodrigo Fuenzalida and Nicolas Massi; interface adaptations for Gravewake.",
    10: "The Gravewake title letterforms with tabular combat numerals and angular interface punctuation. Derived from Pirata One under SIL OFL 1.1.",
    16: "Gravewake Gothic", 17: "Regular",
}
for name_id, value in values.items():
    names.setName(value, name_id, 3, 1, 0x409)
    names.setName(value, name_id, 1, 0, 0)
font["head"].fontRevision = 1.0
font["head"].modified = font["head"].created
font["hhea"].advanceWidthMax = max(w for w, _ in metrics.values())
font["OS/2"].recalcAvgCharWidth(font)
font.save(OUTPUT)

# Reopen the delivered bytes, checking title fidelity and runtime coverage.
result = TTFont(OUTPUT)
result_cmap = result.getBestCmap()
for c, (outline, metric) in title.items():
    name = result_cmap[ord(c)]
    assert result["glyf"][name].compile(result["glyf"]) == outline
    assert result["hmtx"].metrics[name] == metric
assert all(c in result_cmap for c in range(32, 127))
assert {result["hmtx"].metrics[result_cmap[ord(c)]][0] for c in "0123456789"} == {480}
for name in result.getGlyphOrder():
    result["glyf"][name].getCoordinates(result["glyf"])
license_text = (DIR / "pirataone-OFL.txt").read_text()
(DIR / "gravewake-gothic-OFL.txt").write_text(
    "Gravewake Gothic is a modified version of Pirata One.\n"
    "Alphabetic outlines retained; HUD numerals, punctuation, symbols and naming adapted for Gravewake (2026).\n\n"
    + license_text
)
(DIR / "gravewake-gothic-build.json").write_text(json.dumps({
    "family": "Gravewake Gothic", "source": SOURCE.name,
    "output": OUTPUT.name, "sha256": hashlib.sha256(OUTPUT.read_bytes()).hexdigest(),
    "unicode_characters": len(result_cmap), "title_outlines_and_metrics_preserved": True,
    "tabular_digit_advance": 480, "units_per_em": result["head"].unitsPerEm,
    "license": "SIL OFL 1.1", "system_installation_required": False,
}, indent=2) + "\n")
print(f"Built {OUTPUT}: {len(result_cmap)} Unicode characters; exact title outlines; tabular figures.")
