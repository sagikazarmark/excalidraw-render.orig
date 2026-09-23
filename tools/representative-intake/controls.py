"""Classify comparison results against identical-input rasterizer controls (#84, #195).

For one collected viewer variant directory, every comparison's regions are
measured next to control pairs that render *identical input* with different
canonical rasterizers or reference authorities:

- `official-chromium` vs `official-rust`: the official SVG in both SVG viewers;
- `official-chromium` vs `official-png`: the official SVG vs the official canvas;
- `native-pdfium` vs `native-pdfjs`: the same native PDF in both PDF viewers
  (PDF comparisons only).

A region is `within` when its changed pixels do not exceed and its SSIM is not
below the worst control of that region; otherwise `beyond`. `within` means the
difference is no larger than the references' own disagreement, so it is budget
calibration evidence (#195), not a renderer defect. `beyond` still needs a
diagnosis. `--tiles N` adds an N-pixel grid of geometry/image regions, so
differences inside a large region can be localized. Observations only.
"""
import argparse
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

import evidence
import intake

ROOT = Path(__file__).resolve().parents[2]
MODES = {"svg-chromium": "canonical", "svg-rust": "rust", "pdf-pdfium": "canonical", "pdf-pdfjs": "pdfjs", "png-canvas": "canonical"}
CANDIDATES = {"svg-chromium": ("official-chromium", "native-chromium"), "svg-rust": ("official-rust", "native-rust"),
              "pdf-pdfium": ("official-chromium", "native-pdfium"), "pdf-pdfjs": ("official-chromium", "native-pdfjs"),
              "png-canvas": ("official-png", "native.png")}
CONTROLS = {"svg": [("official-chromium", "official-rust"), ("official-chromium", "official-png")],
            "pdf": [("official-chromium", "official-rust"), ("official-chromium", "official-png"), ("native-pdfium", "native-pdfjs")],
            "png": [("official-png", "official-chromium"), ("official-png", "official-rust")]}


def ssim(bits):
    return struct.unpack(">d", bytes.fromhex(bits))[0]


def raster(directory, name):
    return directory / (name if name.endswith(".png") else f"{name}.rgba.zlib")


def compare(directory, left, right, mode, regions):
    with tempfile.TemporaryDirectory() as temporary:
        plan, report = Path(temporary) / "regions.json", Path(temporary) / "report.json"
        plan.write_bytes(intake.encode(regions))
        evidence.run("cargo", "run", "--locked", "--release", "--manifest-path", "tools/representative-intake/compare/Cargo.toml", "--",
                     raster(directory, left), raster(directory, right), mode, plan, report)
        result = json.loads(report.read_bytes())
    return {"whole": result["whole"], **{crop["id"]: crop["region"] for crop in result["crops"]}}


def tiled(regions, size):
    tiles = [{"id": f"tile-{x}-{y}", "class": "geometry/image", "x": x, "y": y,
              "width": min(size, regions["width"] - x), "height": min(size, regions["height"] - y)}
             for y in range(0, regions["height"], size) for x in range(0, regions["width"], size)]
    return {**regions, "crops": regions["crops"] + tiles}


def classify(directory, comparisons, tiles):
    regions = intake.decode((directory / "regions.json").read_bytes())
    if tiles:
        regions = tiled(regions, tiles)
    classes = {crop["id"]: crop["class"] for crop in regions["crops"]}
    classes["whole"] = regions["whole_class"]
    cache, rows = {}, []
    for comparison in comparisons:
        fmt = comparison.split("-")[0]
        measured = compare(directory, *CANDIDATES[comparison], MODES[comparison], regions)
        controls = []
        for pair in CONTROLS[fmt]:
            if pair not in cache:
                cache[pair] = compare(directory, *pair, "canonical", regions)
            controls.append((pair, cache[pair]))
        for region, value in measured.items():
            if value["changed_pixels_pass"] and value["ssim_pass"]:
                continue
            envelope_changed = max(control[region]["changed_pixels"] for _, control in controls)
            envelope_ssim = min(ssim(control[region]["ssim_bits"]) for _, control in controls)
            within = value["changed_pixels"] <= envelope_changed and ssim(value["ssim_bits"]) >= envelope_ssim
            rows.append({"comparison": comparison, "region": region, "class": classes[region],
                         "changed_pixels": value["changed_pixels"], "ssim": ssim(value["ssim_bits"]),
                         "control_changed_pixels": envelope_changed, "control_ssim": envelope_ssim,
                         "controls": {f"{a} vs {b}": {"changed_pixels": c[region]["changed_pixels"], "ssim": ssim(c[region]["ssim_bits"])}
                                      for (a, b), c in controls},
                         "verdict": "within" if within else "beyond"})
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("variant", type=Path, help="collected viewers/<variant> directory")
    parser.add_argument("--comparison", action="append", choices=list(MODES), help="default: all")
    parser.add_argument("--tiles", type=int, help="add a grid of geometry/image tiles of this size")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    rows = classify(args.variant, args.comparison or list(MODES), args.tiles)
    if args.output:
        args.output.write_bytes(intake.encode({"schema": "excalidraw-render/identical-input-controls/v1", "approval": None,
                                               "variant": str(args.variant), "tiles": args.tiles, "failing_regions": rows}))
    for row in rows:
        if not row["region"].startswith("tile-") or row["verdict"] == "beyond":
            print(f"{row['verdict']:6s} {row['comparison']:12s} {row['region']:26s} {row['changed_pixels']:>7} / {row['ssim']:.6f}"
                  f"   controls {row['control_changed_pixels']:>7} / {row['control_ssim']:.6f}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
