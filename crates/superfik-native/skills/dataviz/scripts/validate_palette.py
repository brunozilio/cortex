#!/usr/bin/env python3
"""
Validate a categorical chart palette against the computable data-viz checks.

Design-system-agnostic: feed it ANY palette's hex values plus the mode and
surface, and it computes \u2014 never eyeballs \u2014
the five checks that can be measured from color alone:

  2. Lightness band   \u2014 OKLCH L within the mode's band
  3. Chroma floor     \u2014 OKLCH C >= floor (below it a hue reads as gray)
  4. CVD separation   \u2014 OKLab \u0394E (×100) between slots under simulated protan/deutan
                        (tritan reported); adjacent pairs by default, --pairs all
                        for scatter/bubble/maps
  4b. Normal-vision floor \u2014 worst OKLab \u0394E (×100) on the active pairlist
      (adjacent by default; all pairs with --pairs all) under unsimulated vision;
                        full-color readers must be able to tell neighbors apart too
  5. Contrast vs surface \u2014 WCAG ratio of each mark against the chart surface

Checks 1 (fixed hue order) and 6 (values resolve to real ramp steps) are
structural rules the skill enforces, not measurable from hexes alone.

Usage:
  python validate_palette.py "#2a78d6,#eb6834,#1baf7a,#eda100,#e87ba4,#008300,#6250d6,#e34948" --mode light
  python validate_palette.py "#256abf,#199e70,..." --mode dark --surface "#1a1a19"

Exit code 0 unless a check hard-FAILs; 1 on any FAIL. WARN bands do not fail:
adjacent CVD in the 6\u20138 floor band, and contrast in the sub-3:1 relief band, are
reported as WARNs and still exit 0 (each is legal only with mandatory secondary
encoding: direct labels, gaps, or texture). The normal-vision floor is a hard
gate: a worst unsimulated pair below 15 FAILs the run.
"""
import sys, math, json, argparse, re

BAND = {"light": (0.43, 0.77), "dark": (0.48, 0.67)}   # OKLCH L
CHROMA_FLOOR = 0.10                                     # OKLCH C
CVD_TARGET, CVD_FLOOR = 8.0, 6.0                        # OKLab Delta E×100, min(protan, deutan), adjacent pairs
NORMAL_FLOOR = 15.0                                     # OKLab Delta E×100, worst pair on the active pairlist, unsimulated vision
CONTRAST_MIN = 3.0                                      # WCAG vs surface
DEFAULT_SURFACE = {"light": "#fcfcfb", "dark": "#1a1a19"}

MACHADO = {
    "protan": [[0.152286, 1.052583, -0.204868],
               [0.114503, 0.786281, 0.099216],
               [-0.003882, -0.048116, 1.051998]],
    "deutan": [[0.367322, 0.860646, -0.227968],
               [0.280085, 0.672501, 0.047413],
               [-0.011820, 0.042940, 0.968881]],
    "tritan": [[1.255528, -0.076749, -0.178779],
               [-0.078411, 0.930809, 0.147602],
               [0.004733, 0.691367, 0.303900]]}

def hex2srgb(h):
    h = h.strip().lstrip("#")
    return tuple(int(h[i:i+2], 16) / 255 for i in (0, 2, 4))

_WS = (" \t\n\v\f\r\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006"
       "\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000")

def strip_ws(v):
    return v.strip(_WS)

def split_colors(raw):
    return [c for c in (strip_ws(s) for s in (raw or "").split(",")) if c]

def is_hex_color(v):
    return re.fullmatch(r"#?[0-9a-fA-F]{6}", v) is not None

def s2lin(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

def lin2s(c):
    c = max(0.0, min(1.0, c))
    return 12.92 * c if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055

def lin(h):
    return tuple(s2lin(c) for c in hex2srgb(h))

def relative_luminance(h):
    r, g, b = lin(h)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b

def contrast(h1, h2):
    a, b = sorted((relative_luminance(h1), relative_luminance(h2)), reverse=True)
    return (a + 0.05) / (b + 0.05)

def _apca_y(h):
    r, g, b = (c ** 2.4 for c in hex2srgb(h))
    y = 0.2126729 * r + 0.7151522 * g + 0.072175 * b
    return y if y >= 0.022 else y + (0.022 - y) ** 1.414

def apca(text, background):
    t, b = _apca_y(text), _apca_y(background)
    if abs(b - t) < 0.0005:
        return 0.0
    c = 1.14 * (b ** 0.56 - t ** 0.57 if b > t else b ** 0.65 - t ** 0.62)
    return 0.0 if abs(c) < 0.1 else (c - (0.027 if c > 0 else -0.027)) * 100

def lin2oklab(r, g, b):
    l = 0.4122214708*r + 0.5363325363*g + 0.0514459929*b
    m = 0.2119034982*r + 0.6806995451*g + 0.1073969566*b
    s = 0.0883024619*r + 0.2817188376*g + 0.6299787005*b
    l, m, s = l ** (1/3), m ** (1/3), s ** (1/3)
    L = 0.2104542553*l + 0.7936177850*m - 0.0040720468*s
    a = 1.9779984951*l - 2.4285922050*m + 0.4505937099*s
    bb = 0.0259040371*l + 0.7827717662*m - 0.8086757660*s
    return L, a, bb

def lin2oklch(r, g, b):
    L, a, bb = lin2oklab(r, g, b)
    return L, math.hypot(a, bb)   # (L, C)

def oklch(h):
    return lin2oklch(*lin(h))

def simulate(h, kind):
    r, g, b = lin(h)
    M = MACHADO[kind]
    sr = M[0][0]*r + M[0][1]*g + M[0][2]*b
    sg = M[1][0]*r + M[1][1]*g + M[1][2]*b
    sb = M[2][0]*r + M[2][1]*g + M[2][2]*b
    return (max(0.0, min(1.0, sr)), max(0.0, min(1.0, sg)), max(0.0, min(1.0, sb)))

def deltaE(h1, h2, kind=None):
    a = lin2oklab(*(simulate(h1, kind) if kind else lin(h1)))
    b = lin2oklab(*(simulate(h2, kind) if kind else lin(h2)))
    return 100 * math.dist(a, b)

def _jn(v):
    return int(v) if isinstance(v, float) and v.is_integer() else v

def validate(palette, mode, surface, pairs="adjacent"):
    lo, hi = BAND[mode]
    report, ok = [], True

    offband = [(c, _jn(round(oklch(c)[0], 3))) for c in palette if not (lo <= oklch(c)[0] <= hi)]
    if offband: ok = False
    report.append(("Lightness band", not offband,
                   f"all {len(palette)} inside L {lo}\u2013{hi}" if not offband
                   else f"outside band: {json.dumps(offband, separators=(',', ':'))}"))

    lowc = [(c, _jn(round(oklch(c)[1], 3))) for c in palette if oklch(c)[1] < CHROMA_FLOOR]
    if lowc: ok = False
    report.append(("Chroma floor", not lowc,
                   f"all {len(palette)} >= {CHROMA_FLOOR}" if not lowc
                   else f"below floor (reads gray): {json.dumps(lowc, separators=(',', ':'))}"))

    n = len(palette)
    pairlist = ([(i, j) for i in range(n) for j in range(i+1, n)] if pairs == "all"
                else [(i, i+1) for i in range(n-1)])
    label = "all-pairs" if pairs == "all" else "adjacent"
    worst = None
    for kind in ("protan", "deutan"):
        for i, j in pairlist:
            d = deltaE(palette[i], palette[j], kind)
            if worst is None or d < worst[0]:
                worst = (d, kind, palette[i], palette[j])
    tri = min((deltaE(palette[i], palette[j], "tritan") for i, j in pairlist), default=99)
    wd = worst[0] if worst else 99
    cvd_state = "pass" if wd >= CVD_TARGET else ("floor" if wd >= CVD_FLOOR else "fail")
    if cvd_state == "fail": ok = False
    report.append(("CVD separation", cvd_state,
                   f"worst {label} {worst[3]}\u2194{worst[2]} \u0394E {wd:.1f} ({worst[1]}) · "
                   f"tritan {tri:.1f}" if worst else "n/a"))

    nworst = None
    for i, j in pairlist:
        d = deltaE(palette[i], palette[j])
        if nworst is None or d < nworst[0]:
            nworst = (d, palette[i], palette[j])
    nd = nworst[0] if nworst else 99
    nor_state = "pass" if nd >= NORMAL_FLOOR else "fail"
    if nor_state == "fail": ok = False
    report.append(("Normal-vision floor", nor_state,
                   f"worst {label} {nworst[2]}\u2194{nworst[1]} \u0394E {nd:.1f} (normal)"
                   + ("" if nd >= NORMAL_FLOOR else
                      f" \u2014 below {NORMAL_FLOOR:.0f}, hard to tell apart even with full color vision")
                   if nworst else "n/a"))

    low = [(c, _jn(round(contrast(c, surface), 2))) for c in palette if contrast(c, surface) < CONTRAST_MIN]
    report.append(("Contrast vs surface", "pass" if not low else "relief",
                   f"all {len(palette)} >= {CONTRAST_MIN:g}:1" if not low
                   else f"below {CONTRAST_MIN:g}:1 \u2014 relief required (visible labels or table view): {json.dumps(low, separators=(',', ':'))}"))
    return report, ok

ORDINAL_MIN_DL = 0.06          # min OKLCH delta L between adjacent steps
ORDINAL_LIGHT_FLOOR = 2.0      # lightest step: WCAG contrast vs surface

def validate_ordinal(palette, mode, surface):
    """Ordered categories (funnel stages, size tiers, time buckets rendered as
    discrete marks) take a one-hue ramp, not categorical hues. The categorical
    checks FAIL a correct ramp by design (it spans the lightness band; light
    steps drop below the chroma floor). The ordinal checks instead verify the
    ramp reads *as a ramp*: one hue, monotone lightness with visible gaps
    between steps, and a lightest step that still clears the surface."""
    report, ok = [], True
    Ls = [oklch(c)[0] for c in palette]

    order = sorted(range(len(Ls)), key=Ls.__getitem__)
    mono = order == list(range(len(Ls))) or order == list(range(len(Ls)))[::-1]
    if not mono: ok = False
    report.append(("Lightness monotone", mono,
                   "steps read light\u2192dark" if mono
                   else f"out of order \u2014 L values {json.dumps([_jn(round(l,3)) for l in Ls], separators=(',', ':'))}"))

    gaps = [abs(Ls[i+1] - Ls[i]) for i in range(len(Ls)-1)]
    thin = [(palette[i], palette[i+1], _jn(round(g,3))) for i, g in enumerate(gaps) if g < ORDINAL_MIN_DL]
    if thin: ok = False
    report.append(("Adjacent \u0394L", not thin,
                   f"all gaps >= {ORDINAL_MIN_DL}" if not thin
                   else f"steps too close: {json.dumps(thin, separators=(',', ':'))}"))

    lightest = max(palette, key=lambda c: oklch(c)[0]) if mode == "light" else min(palette, key=lambda c: oklch(c)[0])
    cr = contrast(lightest, surface)
    if cr < ORDINAL_LIGHT_FLOOR: ok = False
    report.append(("Light-end contrast", cr >= ORDINAL_LIGHT_FLOOR,
                   f"{lightest} at {cr:.2f}:1 vs surface"
                   + ("" if cr >= ORDINAL_LIGHT_FLOOR else f" \u2014 below {ORDINAL_LIGHT_FLOOR:g}:1 floor")))

    hues = []
    for c in palette:
        _, a, bb = lin2oklab(*lin(c))
        hues.append(math.degrees(math.atan2(bb, a)) % 360)
    spread = (max(hues) - min(hues)) if hues else 0
    if spread > 180: spread = 360 - spread
    one_hue = spread <= 40
    if not one_hue: ok = False
    report.append(("Single hue", one_hue,
                   f"hue spread {spread:.0f}°" + ("" if one_hue else " \u2014 >40°, not a one-hue ramp")))
    return report, ok

def main():
    ap = argparse.ArgumentParser(description="Validate a categorical chart palette (the data-viz six checks).")
    ap.add_argument("palette", help="comma-separated hex values, in slot order")
    ap.add_argument("--mode", choices=["light", "dark"], default="light")
    ap.add_argument("--surface", default=None, help="chart surface hex (defaults per mode)")
    ap.add_argument("--pairs", choices=["adjacent", "all"], default="adjacent",
                    help="adjacent: stacks/bars/lines (default). all: scatter/bubble/maps/"
                         "small-multiples, where any two marks can sit side by side.")
    ap.add_argument("--ordinal", action="store_true",
                    help="ordered categories (funnel, tiers, buckets) \u2014 validate as a "
                         "one-hue ramp instead of the categorical checks.")
    a = ap.parse_args()
    palette = split_colors(a.palette)
    if not palette:
        print('usage: python validate_palette.py "#hex,#hex,..." [--mode light|dark] [--surface #hex] [--pairs adjacent|all] [--ordinal]', file=sys.stderr)
        sys.exit(2)
    raw_surface = strip_ws(a.surface) if a.surface is not None else ""
    surface = raw_surface or DEFAULT_SURFACE[a.mode]
    bad_hex = [c for c in [*palette, surface] if not is_hex_color(c)]
    if bad_hex:
        print(f"invalid hex value(s): {', '.join(bad_hex)} \u2014 expected #rrggbb", file=sys.stderr)
        sys.exit(2)

    report, ok = (validate_ordinal(palette, a.mode, surface) if a.ordinal
                  else validate(palette, a.mode, surface, a.pairs))
    glyph = {True: "PASS", False: "FAIL", "pass": "PASS", "floor": "WARN", "fail": "FAIL", "relief": "WARN"}
    kind = "ordinal ramp" if a.ordinal else "categorical"
    print(f"\nPalette ({a.mode}, surface {surface}, {kind}): {len(palette)} slots")
    for name, state, detail in report:
        print(f"  [{glyph[state]:4}] {name:22} {detail}")
    verdict = "ALL CHECKS PASS" if ok else "FAILED \u2014 fix the marked checks"
    if a.ordinal:
        print(f"\n  \u2192 {verdict}"
              "  (ordinal: one hue, monotone L, visible step gaps, light end clears surface)")
    else:
        print(f"\n  \u2192 {verdict}"
              "  (CVD in the 6\u20138 floor band is legal ONLY with secondary encoding:"
              " direct labels, gaps, or texture)")
        print("  scope: categorical palettes only. For a lone status/text color check"
              " text contrast with this script's apca() or contrast(); for a sequential ramp, lightness monotonicity.\n")
    sys.exit(0 if ok else 1)

if __name__ == "__main__":
    main()
