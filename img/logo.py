"""Draws the MSG icon files from the same ball-and-stick model as src/logo.rs (keep ATOMS, BONDS and BEST the same).

    python img/logo.py

Writes msg_icon_{16,32,48,128,256,512}.png, msg_icon.ico (for the exe, build.rs) and msg_logo.svg next to itself.
Needs Pillow. The colours are the dark theme's (src/theme.rs); the background is transparent.
"""
import math
from pathlib import Path

from PIL import Image, ImageDraw

ATOMS = [
    ((-1.288, -0.181, 0.062), "C"),
    ((-0.472, 0.397, 0.062), "C"),
    ((0.344, -0.181, 0.062), "C"),
    ((1.160, 0.397, 0.062), "C"),
    ((1.976, -0.181, 0.062), "C"),
    ((-1.286, -0.945, 0.500), "O"),
    ((-2.008, 0.074, -0.375), "O"),
    ((1.160, 0.945, 0.838), "N"),
    ((2.588, -0.079, -0.562), "O"),
    ((2.083, -0.792, 0.687), "O"),
    ((-2.588, 0.719, -0.838), "Na"),
]
BONDS = [(0, 1), (1, 2), (2, 3), (3, 4), (0, 5), (0, 6), (3, 7), (4, 8), (4, 9)]
REACH = 2.82
BEST = 0.6
TILT = 0.45
COLOUR = {"C": (0xB8, 0xBC, 0xC8), "O": (0xFF, 0x96, 0x78), "N": (0xD9, 0xA7, 0xFF), "Na": (0xFF, 0xB3, 0x6B)}
SIZE = {"C": 0.4, "O": 0.37, "N": 0.37, "Na": 0.46}
BOND = (0x77, 0x7F, 0x90)
RIM = (0x17, 0x19, 0x1F, 128)
SIZES = [16, 32, 48, 128, 256, 512]


def project(angle=None):
    """The atoms turned and tipped: (x, y, z, scale, kind) with the bond length as the unit, and the drawing order far to
    near, where a bond is two halves, each just behind its own atom, so that no stick crosses a ball."""
    angle = BEST if angle is None else angle
    sa, ca = math.sin(angle), math.cos(angle)
    st, ct = math.sin(TILT), math.cos(TILT)
    atoms = []
    for (x, y, z), kind in ATOMS:
        x, z = x * ca + z * sa, z * ca - x * sa
        y, z = y * ct - z * st, y * st + z * ct
        s = 1 / (1 - z * 0.08)
        atoms.append((x * s, -y * s, z, s, kind))
    order = [(a[2], n, None) for n, a in enumerate(atoms)]
    order += [(atoms[a][2] - 0.001, a, b) for a, b in BONDS] + [(atoms[b][2] - 0.001, b, a) for a, b in BONDS]
    order.sort(key=lambda item: item[0])
    return atoms, order


def fit(atoms, side, margin=0.04):
    """The scale and offset that fill a side×side canvas with this pose."""
    xs = [x + d * SIZE[k] * s for x, y, z, s, k in atoms for d in (-1, 1)]
    ys = [y + d * SIZE[k] * s for x, y, z, s, k in atoms for d in (-1, 1)]
    unit = side * (1 - 2 * margin) / max(max(xs) - min(xs), max(ys) - min(ys))
    return unit, side / 2 - (max(xs) + min(xs)) / 2 * unit, side / 2 - (max(ys) + min(ys)) / 2 * unit


def shade(colour, depth):
    """Far atoms a little darker."""
    k = 0.72 + 0.28 * depth
    return tuple(int(c * k) for c in colour)


def shapes(side):
    """What to draw on a side×side canvas, far to near: ("line", (x1, y1, x2, y2), width, colour) and
    ("ball", (x, y), radius, colour)."""
    atoms, order = project()
    unit, dx, dy = fit(atoms, side)
    at = lambda n: (atoms[n][0] * unit + dx, atoms[n][1] * unit + dy)
    out = []
    for z, n, other in order:
        depth = min(1, max(0, (z + REACH) / (2 * REACH)))
        (x, y), s, kind = at(n), atoms[n][3], atoms[n][4]
        if other is not None:
            ox, oy = at(other)
            out.append(("line", (x, y, (x + ox) / 2, (y + oy) / 2), unit * 0.22 * s, shade(BOND, depth)))
        else:
            out.append(("ball", (x, y), unit * SIZE[kind] * s, shade(COLOUR[kind], depth)))
    return unit, out


def render(size, scale=4):
    side = size * scale
    unit, out = shapes(side)
    im = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    draw = ImageDraw.Draw(im)
    for what, where, measure, colour in out:
        if what == "line":
            x1, y1, x2, y2 = where
            draw.line([(x1, y1), (x2, y2)], fill=colour, width=max(1, round(measure)))
        else:
            (x, y), r = where, measure
            draw.ellipse([x - r, y - r, x + r, y + r], fill=colour, outline=RIM, width=max(1, round(unit * 0.05)))
            if r >= 3 * scale:
                h, hx, hy = r * 0.3, x - r * 0.32, y - r * 0.32
                draw.ellipse([hx - h, hy - h, hx + h, hy + h], fill=(255, 255, 255, 76))
    return im.resize((size, size), Image.LANCZOS)


def svg(side=512):
    unit, out = shapes(side)
    lines = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {side} {side}">',
             "\t<!-- MSG logo: monosodium glutamate as a ball-and-stick model, drawn by img/logo.py. -->"]
    for what, where, measure, colour in out:
        colour = "#%02x%02x%02x" % colour
        if what == "line":
            x1, y1, x2, y2 = where
            lines.append(f'\t<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" stroke="{colour}" stroke-width="{measure:.1f}"/>')
        else:
            (x, y), r = where, measure
            lines.append(f'\t<circle cx="{x:.1f}" cy="{y:.1f}" r="{r:.1f}" fill="{colour}" stroke="#17191f" stroke-opacity="0.5" stroke-width="{unit * 0.05:.1f}"/>')
            lines.append(f'\t<circle cx="{x - r * 0.32:.1f}" cy="{y - r * 0.32:.1f}" r="{r * 0.3:.1f}" fill="#fff" fill-opacity="0.3"/>')
    lines.append("</svg>")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    here = Path(__file__).parent
    pictures = {size: render(size) for size in SIZES}
    for size, im in pictures.items():
        im.save(here / f"msg_icon_{size}.png")
    pictures[256].save(here / "msg_icon.ico", sizes=[(s, s) for s in SIZES if s <= 256], append_images=[pictures[s] for s in SIZES if s < 256])
    (here / "msg_logo.svg").write_text(svg(), encoding="utf-8")
    print("wrote", ", ".join(f"msg_icon_{s}.png" for s in SIZES), "msg_icon.ico and msg_logo.svg")
