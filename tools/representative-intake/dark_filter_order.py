"""Test whether a dark official raster is the theme filter applied after compositing (#84).

The reference's dark SVG carries `invert(93%) hue-rotate(180deg)` on its root,
so the browser filters the finished light image. The renderer lowers the
filter into each color before compositing. This applies the exact filter,
rounded or floored, to the official light raster and histograms its per-pixel
maximum channel difference from the official dark raster, and the native dark
raster's difference from it, over the whole image and one region. Observations
only.
"""
import argparse
import collections
from pathlib import Path
import sys
import zlib

import intake


def theme_filter(rgb, quantize):
    red, green, blue = (0.93 * (255 - c) + 0.07 * c for c in rgb)
    rotated = (-0.574 * red + 1.430 * green + 0.144 * blue,
               0.426 * red + 0.430 * green + 0.144 * blue,
               0.426 * red + 1.430 * green - 0.856 * blue)
    return tuple(quantize(min(255.0, max(0.0, v))) for v in rotated)


def histogram(pixels, width, region, predicate):
    whole, inside = collections.Counter(), collections.Counter()
    x0, y0, w, h = region
    for index in range(0, len(pixels) // 4):
        y, x = divmod(index, width)
        delta = predicate(index * 4)
        whole[delta] += 1
        if x0 <= x < x0 + w and y0 <= y < y0 + h:
            inside[delta] += 1
    return {str(k): v for k, v in sorted(whole.items())}, {str(k): v for k, v in sorted(inside.items())}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("viewers", type=Path, help="collected viewers directory with light/ and dark/")
    parser.add_argument("--region", required=True, help="crop id from dark/regions.json")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    raster = lambda variant, name: zlib.decompress((args.viewers / variant / f"{name}.rgba.zlib").read_bytes())
    light, dark, native = raster("light", "official-chromium"), raster("dark", "official-chromium"), raster("dark", "native-chromium")
    regions = intake.decode((args.viewers / "dark" / "regions.json").read_bytes())
    crop = next(c for c in regions["crops"] if c["id"] == args.region)
    region = (crop["x"], crop["y"], crop["width"], crop["height"])
    record = {"schema": "excalidraw-render/dark-filter-order/v1", "approval": None, "region": crop,
              "inputs": {f"{v}/{n}.rgba.zlib": intake.descriptor((args.viewers / v / f"{n}.rgba.zlib").read_bytes())
                         for v, n in (("light", "official-chromium"), ("dark", "official-chromium"), ("dark", "native-chromium"))}}
    for name, quantize in (("round", lambda v: int(v + 0.5)), ("floor", int)):
        cache = {}
        def delta(i):
            key = bytes(light[i:i + 3])
            filtered = cache.get(key) or cache.setdefault(key, theme_filter(key, quantize))
            return max(abs(filtered[c] - dark[i + c]) for c in range(3))
        whole, inside = histogram(light, regions["width"], region, delta)
        record[f"filtered_light_{name}_vs_official_dark"] = {"whole": whole, "region": inside}
    whole, inside = histogram(native, regions["width"], region,
                              lambda i: max(abs(native[i + c] - dark[i + c]) for c in range(3)))
    record["native_dark_vs_official_dark"] = {"whole": whole, "region": inside}
    args.output.write_bytes(intake.encode(record))
    for key in ("filtered_light_round_vs_official_dark", "filtered_light_floor_vs_official_dark", "native_dark_vs_official_dark"):
        exact = record[key]["region"].get("0", 0)
        total = sum(record[key]["region"].values())
        print(f"{key}: region exact {exact}/{total}, max delta {max(map(int, record[key]['region']))}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, StopIteration) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
