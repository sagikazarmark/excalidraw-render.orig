"""Attribute a PDF comparison failure to viewer rasterization of sub-pixel sketch strokes (#84).

Checks, for one collected viewer variant directory, that the native PDF strokes
exactly the native SVG's sketch-fill geometry (same number sequence, same
width), and measures mean coverage of that fill in a patch for the official
raster, the native SVG raster and both PDF viewer rasters. Observations only;
it grants no exception.
"""
import argparse
import json
from pathlib import Path
import re
import sys
import zlib

import intake


def sketch_numbers_svg(svg, width):
    paths = [m.group(0) for m in re.finditer(r"<path[^>]*>", svg) if f'stroke-width="{width:g}"' in m.group(0)]
    intake.require(len(paths) == 1, f"expected one SVG sketch path at stroke-width {width:g}, found {len(paths)}")
    return re.findall(r"-?[\d.]+", re.search(r' d="([^"]*)"', paths[0])[1])


def sketch_numbers_pdf(pdf, width):
    found = []
    for match in re.finditer(rb"stream\r?\n", pdf):
        end = pdf.find(b"endstream", match.end())
        try:
            content = zlib.decompress(pdf[match.end():end]).decode("latin-1")
        except zlib.error:
            continue
        for start in [m.end() for m in re.finditer(fr"(?m)^{re.escape(f'{width:g}')} w\n", content)]:
            body = content[start:content.index("Q\n", start)].split("SCN\n", 1)[1]
            operations = [line for line in body.split("\n") if line and line != "S"]
            found.append(([token for line in operations for token in line.split()[:-1]], body.count("S\n")))
    intake.require(len(found) == 1, f"expected one PDF sketch fill at {width:g} w, found {len(found)}")
    return found[0]


def coverage(raster, width, patch, background, paint):
    x0, y0, w, h = patch
    red = sum(raster[(y * width + x) * 4] for y in range(y0, y0 + h) for x in range(x0, x0 + w)) / (w * h)
    return round((background - red) / (background - paint), 4)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("variant", type=Path, help="collected viewers/<variant> directory")
    parser.add_argument("--stroke-width", type=float, required=True, help="sketch stroke width in path units (strokeWidth / 2)")
    parser.add_argument("--patch", type=int, nargs=4, required=True, metavar=("X", "Y", "W", "H"), help="pure sketch-fill patch")
    parser.add_argument("--red", type=int, nargs=2, required=True, metavar=("BACKGROUND", "PAINT"), help="red channel of background and fill paint")
    parser.add_argument("--control", type=Path, help="optional extra raster (.rgba.zlib) to measure")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    directory = args.variant
    svg_numbers = sketch_numbers_svg((directory / "native.svg").read_text(), args.stroke_width)
    pdf_numbers, strokes = sketch_numbers_pdf((directory / "native.pdf").read_bytes(), args.stroke_width)
    regions = intake.decode((directory / "regions.json").read_bytes())
    width = regions["width"]
    rasters = {name: directory / f"{name}.rgba.zlib" for name in ("official-chromium", "native-chromium", "native-pdfium", "native-pdfjs")}
    if args.control:
        rasters["control"] = args.control
    record = {
        "schema": "excalidraw-render/pdf-subpixel-attribution/v1", "approval": None,
        "inputs": {name: intake.descriptor((directory / name).read_bytes()) for name in ("native.svg", "native.pdf", "regions.json")},
        "stroke_width": args.stroke_width,
        "geometry": {"svg_numbers": len(svg_numbers), "pdf_numbers": len(pdf_numbers),
                     "identical": svg_numbers == pdf_numbers, "pdf_stroke_operations": strokes},
        "patch": args.patch,
        "coverage": {name: coverage(zlib.decompress(path.read_bytes()), width, args.patch, *args.red) for name, path in rasters.items()},
    }
    if args.control:
        record["control"] = {"path": str(args.control), **intake.descriptor(args.control.read_bytes())}
    args.output.write_bytes(intake.encode(record))
    print(json.dumps({"geometry": record["geometry"], "coverage": record["coverage"]}))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
