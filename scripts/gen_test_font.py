"""Build tests/fixtures/boxes.ttf, the tiny font the tests draw labels with.

Every glyph is a plain box, so the file is our own work with no third-party
outlines in it. Digits and 'x' are 600 units wide, the CJK character U+53E3
(a square ring) is a full 1000-unit em; there is no glyph for 'Z', which the
tests use to check the missing-glyph note. The line height (ascent 900,
descent 300) is 1200 units, larger than the em, so the tests can tell an em
size from ab_glyph's height-based scale. Run with fontTools installed:

    python3 scripts/gen_test_font.py
"""

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen


def box(pen, x0, y0, x1, y1, clockwise=True):
    pts = [(x0, y0), (x0, y1), (x1, y1), (x1, y0)]
    if not clockwise:
        pts.reverse()
    pen.moveTo(pts[0])
    for p in pts[1:]:
        pen.lineTo(p)
    pen.closePath()


def glyph(draw):
    pen = TTGlyphPen(None)
    draw(pen)
    return pen.glyph()


def main():
    names = [".notdef", "space", "x", "ring"] + [f"d{i}" for i in range(10)]
    cmap = {ord(" "): "space", ord("x"): "x", 0x53E3: "ring"}
    cmap.update({ord(str(i)): f"d{i}" for i in range(10)})
    glyphs = {
        ".notdef": glyph(lambda p: box(p, 50, 0, 450, 700)),
        "space": glyph(lambda p: None),
        "x": glyph(lambda p: box(p, 100, 0, 500, 500)),
        "ring": glyph(lambda p: (box(p, 80, -60, 920, 780), box(p, 200, 60, 800, 660, False))),
    }
    for i in range(10):
        glyphs[f"d{i}"] = glyph(lambda p: box(p, 80, 0, 520, 700))
    metrics = {n: (600, 0) for n in names}
    metrics["ring"] = (1000, 80)
    metrics[".notdef"] = (500, 50)

    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder(names)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=900, descent=-300)
    fb.setupNameTable({"familyName": "placehold boxes", "styleName": "Regular"})
    fb.setupOS2(sTypoAscender=900, sTypoDescender=-300, sTypoLineGap=0, usWinAscent=900, usWinDescent=300)
    fb.setupPost()
    out = Path(__file__).resolve().parent.parent / "tests" / "fixtures" / "boxes.ttf"
    fb.save(str(out))
    print(out, out.stat().st_size, "bytes")


if __name__ == "__main__":
    main()
