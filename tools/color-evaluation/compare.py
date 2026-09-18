#!/usr/bin/env python3
"""Verify lock identities and report exact #185 differences, with no tolerances."""
import argparse
import json
from pathlib import Path
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--record", action="store_true")
args = parser.parse_args()
here = Path(__file__).resolve().parent
native = json.loads((here / "expected-native.json").read_text())
wasm = json.loads((here / "expected-wasm.json").read_text())
root = here.parent.parent
production = {(p["name"], p["version"]): p.get("checksum") for p in tomllib.loads((root / "Cargo.lock").read_text())["package"] if "source" in p}
additional = []
for p in tomllib.loads((here / "Cargo.lock").read_text())["package"]:
    if "source" not in p:
        continue
    key = p["name"], p["version"]
    if key in production:
        assert p["checksum"] == production[key], key
    else:
        additional.append(f"{key[0]} {key[1]}")
assert sorted(additional) == ["by_address 1.2.1", "color 0.3.3", "fast-srgb8 1.0.0", "palette 0.7.6", "palette_derive 0.7.6"]

lines = ["# Color conversion evaluation matrix", "", "Order everywhere: current / palette 0.7.6 / color 0.3.3.", "", "## Raw numeric comparison (native)", "", "| Operation | Samples | Palette bitwise changes | Color bitwise changes | Palette maximum absolute channel deltas | Color maximum absolute channel deltas |", "| --- | ---: | ---: | ---: | --- | --- |"]
for row in native["numeric"]:
    lines.append(f"| {row['operation']} | {row['samples']} | {row['changed'][0]} | {row['changed'][1]} | {row['max_abs_channel_delta'][0]} | {row['max_abs_channel_delta'][1]} |")
lines += ["", "## Native/browser-WASM equality", "", "| Section | Exact equality |", "| --- | --- |"]
for key in native:
    equal = native[key] == wasm[key]
    lines.append(f"| {key} | {equal} |")
    if key != "numeric":
        assert equal, f"cross-target canonical/artifact observations changed: {key}"
lines += ["", "Numeric stream equality by implementation (all input/output binary64 bits):", "", "| Operation | Current | Palette | Color |", "| --- | --- | --- | --- |"]
for a, b in zip(native["numeric"], wasm["numeric"]):
    equality = [x == y for x, y in zip(a["stream_sha256"], b["stream_sha256"])]
    assert all(equality[1:]), f"candidate numeric portability regressed: {a['operation']}"
    lines.append(f"| {a['operation']} | {' | '.join(map(str, equality))} |")
lines += ["", "## Canonical RGBA changes", "", "| Corpus | Samples | Palette | Color |", "| --- | ---: | ---: | ---: |"]
css = native["css"]
counts = [sum(r["rgba"][0] != r["rgba"][i] for r in css) for i in [1, 2]]
lines.append(f"| CSS cases | {len(css)} | {counts[0]} | {counts[1]} |")
for key in ["half_byte", "lab_sweep"]:
    r = native[key]
    lines.append(f"| {key} | {r['samples']} | {r['changed'][0]} | {r['changed'][1]} |")
lines += ["", "Every differing fixed CSS case (exact RGBA8, no tolerance):", "", "| CSS | Current | Palette | Color |", "| --- | --- | --- | --- |"]
for r in css:
    if r["rgba"][0] != r["rgba"][1] or r["rgba"][0] != r["rgba"][2]:
        lines.append(f"| `{r['css']}` | {r['rgba'][0]} | {r['rgba'][1]} | {r['rgba'][2]} |")
lines += ["", "## Canonical-boundary swatch artifacts", "", "| Format | Samples (light + dark) | Palette changed bytes | Color changed bytes |", "| --- | ---: | ---: | ---: |"]
for fmt in ["Svg", "Png", "Pdf"]:
    rows = [o for r in native["artifacts"] for o in r["observations"] if o["format"] == fmt]
    counts = [sum(o["outputs"][0]["sha256"] != o["outputs"][i]["sha256"] for o in rows) for i in [1, 2]]
    lines.append(f"| {fmt} | {len(rows)} | {counts[0]} | {counts[1]} |")
lines += ["", "New registry packages relative to production: " + ", ".join(sorted(additional)) + ".", ""]
reference = json.loads((here / "expected-reference.json").read_text())
refs = {r["css"]: r for r in reference["rows"]}
lines += ["## Supplemental pinned Chromium reference", "", "Chrome/ChromeDriver 149.0.7827.55; sRGB canvas and SVG image raster, light mode.", "These are browser probes, not approved Excalidraw reference exports or full parity judgments.", "", "| CSS | Current RGBA | Palette RGBA | Color RGBA | Browser canvas pixel | Browser SVG pixel |", "| --- | --- | --- | --- | --- | --- |"]
for r in native["artifacts"]:
    ref = refs[r["css"]]
    lines.append(f"| `{r['css']}` | {r['rgba'][0]} | {r['rgba'][1]} | {r['rgba'][2]} | {ref['canvas']} | {ref['svg']} |")
divergent = [r for r in reference["rows"] if r["canvas"] != r["svg"]]
lines += ["", f"Canvas/SVG pixels differ on {len(divergent)}/{len(refs)} unique reference probes.", "", "These comparisons retain exact channels. At low alpha, raster unpremultiplication", "can differ from canonical unpremultiplied RGB; inspect the swatch PNG center", "pixels in the observation JSON instead of treating this table as a tolerance gate.", ""]

# Literal source-line accounting for the current production arrangement of the
# evaluated substitution, not a prediction of net code savings from a library.
source = (root / "crates/excalidraw-render/src/color.rs").read_text().split("#[cfg(test)]\nmod tests")[0]
kernel = (root / "crates/excalidraw-render/src/color/conversion.rs").read_text()
def function(name):
    start = kernel.index(f"fn {name}(")
    end = kernel.index("\n}\n", start) + 3
    return kernel[start:end]
names = ["decode_srgb", "encode_srgb", "adapt_d50_to_d65", "adapt_d65_to_d50", "srgb_to_xyz_d65", "srgb_to_lab_d50", "lab_d50_to_srgb", "xyz_d65_to_linear_srgb"]
replaced = sum(len(function(n).splitlines()) for n in names)
lines += ["## Source-line accounting", "", f"Eight replaced numeric functions: {replaced} production lines (including signatures).", "", "| Function | Production lines |", "| --- | ---: |"]
for name in names:
    lines.append(f"| `{name}` | {len(function(name).splitlines())} |")
lines += ["", f"Non-test color policy module: {len(source.splitlines())} lines.", f"Numeric conversion module: {len(kernel.splitlines())} lines.", f"Shared two-candidate adapter: {len((here / 'src/adapter.rs').read_text().splitlines())} lines.", "", "Also replaced: rectangular Oklab mix conversion arms. Wide-RGB matrices/transfers,", "polar conversions, CSS admission, missing components, alpha, gamut mapping,", "mix arithmetic and final quantization remain.", ""]
matrix = "\n".join(lines)
if args.record:
    (here / "matrix.md").write_text(matrix)
else:
    assert (here / "matrix.md").read_text() == matrix, "matrix is stale"
print(matrix)
