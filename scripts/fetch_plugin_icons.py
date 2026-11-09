#!/usr/bin/env -S uv run --script
"""Refresh bundled plugin marks from their owners: uv run scripts/fetch_plugin_icons.py.

Sources are reviewed URLs from HTML heads, web manifests, and brand pages.
Raster originals are embedded losslessly in SVG for the existing icon renderer;
they are never enlarged. SVG marks stay vectors. No network is needed at runtime.
"""

import base64
import hashlib
import io
import json
import re
import urllib.request
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

from PIL import Image

BRANDS = Path(__file__).resolve().parents[1] / "crates/superfik-ui/src/brands"

def download(source):
    request = urllib.request.Request(source["url"], headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = response.read()
    if "member" in source:
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            data = archive.read(source["member"])
    if "inline_viewbox" in source:
        matches = re.findall(rb"<svg\b.*?</svg>", data, re.DOTALL)
        expected = f'viewBox="{source["inline_viewbox"]}"'.encode()
        data = next(svg for svg in matches if expected in svg)
    return data

def svg_document(data, source):
    if source["format"] == "svg":
        root = ET.fromstring(data)
        if root.tag.split("}")[-1] != "svg":
            raise ValueError("Expected an SVG document")
        if "viewbox" in source:
            root.set("viewBox", source["viewbox"])
        viewbox = root.get("viewBox", "").split()
        if len(viewbox) != 4:
            raise ValueError("SVG needs an explicit viewBox")
        width, height = map(float, viewbox[2:])
        root.set("width", str(width))
        root.set("height", str(height))
        root.set("x", str((max(width, height) - width) / 2))
        root.set("y", str((max(width, height) - height) / 2))
        root.set("color", source.get("color", "#171717"))
        for element in root.iter():
            if "style" in element.attrib:
                declarations = element.attrib["style"].split(";")
                element.set("style", ";".join(d for d in declarations if "color(display-p3" not in d))
        for style in list(root.iter("{http://www.w3.org/2000/svg}style")):
            css = style.text or ""
            if ":root" in css:
                default = re.search(r":root\s*\{([^}]+)\}", css)
                if default:
                    for name, value in re.findall(r"([\w-]+)\s*:\s*([^;]+)", default[1]):
                        root.set(name, value.strip())
                root.remove(style)
        root.set("overflow", "hidden")
        ET.register_namespace("", "http://www.w3.org/2000/svg")
        body = ET.tostring(root, encoding="unicode")
        size = max(width, height)
        resolution = "vector"
    else:
        image = Image.open(io.BytesIO(data))
        if image.format == "ICO":
            image = image.ico.getimage(max(image.ico.sizes(), key=lambda size: size[0] * size[1]))
        image = image.convert("RGBA")
        if "crop" in source:
            image = image.crop(tuple(source["crop"]))
        width, height = image.size
        if min(width, height) < 144:
            raise ValueError(f"Raster icon too small: {width}x{height}")
        output = io.BytesIO()
        image.save(output, format="PNG", optimize=True)
        encoded = base64.b64encode(output.getvalue()).decode()
        size = max(width, height)
        body = (f'<image x="{(size - width) / 2}" y="{(size - height) / 2}" '
                f'width="{width}" height="{height}" href="data:image/png;base64,{encoded}"/>')
        resolution = f"{width}x{height}"
    tile = source.get("tile", {})
    inset = tile.get("inset", 0)
    background = tile.get("background", "white")
    scale = (100 - 2 * inset) / size
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">'
            f'<defs><clipPath id="tile"><rect width="100" height="100" rx="22.5"/></clipPath></defs>'
            f'<g clip-path="url(#tile)"><rect width="100" height="100" fill="{background}"/>'
            f'<g transform="translate({inset} {inset}) scale({scale})">{body}</g></g></svg>\n'), resolution

def main():
    sources = json.loads((BRANDS / "sources.json").read_text())
    results = {}
    for key, source in sources.items():
        data = download(source)
        svg, resolution = svg_document(data, source)
        results[key] = svg
        source["resolution"] = resolution
        source["sha256"] = hashlib.sha256(data).hexdigest()
        print(f"{key}: {resolution}")
    for key, svg in results.items():
        (BRANDS / f"{key}.svg").write_text(svg)
    (BRANDS / "sources.json").write_text(json.dumps(sources, indent=2) + "\n")

if __name__ == "__main__":
    main()
