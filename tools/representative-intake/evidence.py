"""Collect proposed Velox viewer evidence without changing approved registries."""
import argparse
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

import intake

ROOT = Path(__file__).resolve().parents[2]
COMPARISONS = {
    "svg-chromium":("official-chromium.rgba.zlib","native-chromium.rgba.zlib","canonical"),
    "svg-rust":("official-rust.rgba.zlib","native-rust.rgba.zlib","rust"),
    "pdf-pdfium":("official-chromium.rgba.zlib","native-pdfium.rgba.zlib","canonical"),
    "pdf-pdfjs":("official-chromium.rgba.zlib","native-pdfjs.rgba.zlib","pdfjs"),
    "png-canvas":("official-png.rgba.zlib","native.png","canonical"),
}


def producer_paths(reference):
    paths = ["tools/representative-intake/" + name for name in ["evidence.py", "svg-viewer.mjs", "svg-viewer.sh", "intake.py", "reference.mjs", "reference.sh", "compare/src/main.rs", "compare/Cargo.toml", "compare/Cargo.lock"]]
    paths += ["tools/reference-oracle/scripts/stage4-viewer-assets.mjs", "tools/reference-oracle/scripts/svg-viewer.mjs", "xtask/src/conformance_candidate.rs", "Cargo.lock", "Cargo.toml", "xtask/Cargo.toml", "tools/svg-viewer-rust/src/main.rs", "tools/svg-viewer-rust/Cargo.lock", "tools/svg-viewer-rust/Cargo.toml", "tools/pdf-harness/pdfium/render.py", "tools/pdf-harness/pdfjs/render.mjs", "tools/pdf-harness/run-pdfium.sh", "tools/pdf-harness/run-pdfjs.sh", "tools/pdf-harness/pdfium.lock.json", "tools/pdf-harness/pdfjs/package-lock.json", "tests/conformance/comparator-profiles/canonical-visual-v1.json"]
    paths += [str(p.relative_to(ROOT)) for p in (ROOT / "crates/excalidraw-render-compare/src").glob("*.rs")]
    paths += [f"tests/conformance/viewer-profiles/{name}.json" for name in ["canonical-svg-chromium-v1", "canonical-svg-pure-rust-v1", "canonical-pdf-pdfium-v1", "canonical-pdf-pdfjs-v1"]]
    paths += [r["path"] for r in reference["sources"]]
    return sorted(set(paths))


def run(*args):
    result = subprocess.run(list(map(str, args)), cwd=ROOT, capture_output=True, text=True, timeout=600)
    intake.require(result.returncode == 0, f"command failed: {args}\n{result.stdout}\n{result.stderr}")
    return result.stdout


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def bind(path):
    return {"path": relative(path), **intake.descriptor(path.read_bytes())}


def regions(scene, plan, reference):
    # Velox's authored elements are unrotated. Retain source anchoring, include
    # endpoints and stroke margin; do not fit crops to native disagreement.
    live = [e for e in scene["elements"] if not e.get("isDeleted", False)]
    intake.require(all(e.get("angle", 0) == 0 for e in live), "rotated source crops need a reviewed bounds adapter")
    min_x = min(e["x"] for e in live)
    min_y = min(e["y"] for e in live)
    padding, scale = reference["options"]["exportPadding"], reference["options"]["exportScale"]
    width, height = reference["width"], reference["height"]
    crops = []
    for region in plan["regions"]:
        if region["id"] == "whole":
            continue
        selected = [e for e in live if e["id"] in region["element_ids"]]
        points = [(e["x"] + x, e["y"] + y) for e in selected
                  for x, y in (e.get("points") or [[0, 0], [e["width"], e["height"]]])]
        left = max(0, math.floor((min(p[0] for p in points) - min_x + padding - 4) * scale))
        top = max(0, math.floor((min(p[1] for p in points) - min_y + padding - 4) * scale))
        right = min(width, math.ceil((max(p[0] for p in points) - min_x + padding + 4) * scale))
        bottom = min(height, math.ceil((max(p[1] for p in points) - min_y + padding + 4) * scale))
        intake.require(right > left and bottom > top, "empty anchored crop")
        crops.append({"id": region["id"], "class": region["class"], "x": left, "y": top, "width": right-left, "height": bottom-top})
    return {"width": width, "height": height,
            "whole_class": next(r["class"] for r in plan["regions"] if r["id"] == "whole"), "crops": crops}


def collect(bundle, reference_root, native, output, repeat=False):
    intake.verify(bundle)
    intake.verify_capture(bundle, native)
    reference = intake.decode((reference_root / "reference.json").read_bytes())
    plan = intake.decode((bundle / "plan.json").read_bytes())
    scene = intake.decode((bundle / "scene.excalidraw").read_bytes())
    intake.require(reference["scene_sha256"] == intake.descriptor((bundle / "scene.excalidraw").read_bytes())["sha256"], "reference scene changed")
    intake.require(reference["plan_sha256"] == intake.descriptor((bundle / "plan.json").read_bytes())["sha256"], "reference plan changed")
    intake.require([r["variant"] for r in reference["rows"]] == [v["id"] for v in plan["variants"]], "reference variant inventory")
    source_paths = producer_paths(reference)
    sources = [bind(ROOT / p) for p in source_paths]
    for source in reference["sources"]:
        intake.require(bind(ROOT / source["path"])["sha256"] == source["sha256"], "reference producer changed")
    input_bindings = [bind(path) for path in [bundle / "manifest.json", bundle / "plan.json", bundle / "scene.excalidraw", reference_root / "reference.json", native / "capture.json"]]
    old_files = None
    if repeat:
        previous = intake.decode((output / "viewers.json").read_bytes())
        intake.require(previous["approval"] is None and previous["eligible_executions"] == [], "cannot repeat approved collection")
        old_files = previous["files"]
        for name, expected in old_files.items():
            intake.require(intake.descriptor((output / name).read_bytes()) == expected, "previous collection was modified")
    else:
        output.mkdir()
    def retain(path, data):
        if repeat and path.exists():
            intake.require(path.read_bytes() == data, f"repeat would change retained file: {path}")
        else:
            path.write_bytes(data)
    captures, judgments = [], []
    for ref in reference["rows"]:
        variant = ref["variant"]
        directory = output / variant
        directory.mkdir(exist_ok=repeat)
        (directory / "assets").mkdir(exist_ok=repeat)
        for name, expected in ref["files"].items():
            data = (reference_root / variant / name).read_bytes()
            intake.require(intake.descriptor(data) == expected, "reference file changed")
            intake.require(name in ("official.svg", "official.png", "official-png.rgba.zlib"), "unexpected reference file")
            retain(directory / name, data)
        for fmt in intake.FORMATS:
            retain(directory / f"native.{fmt}", (native / f"{variant}-{fmt}.{fmt}").read_bytes())
            retain(directory / f"native-{fmt}-report.json", (native / f"{variant}-{fmt}-report.json").read_bytes())
        region_plan = regions(scene, plan, ref)
        retain(directory / "regions.json", intake.encode(region_plan))
        run("node", "tools/reference-oracle/scripts/stage4-viewer-assets.mjs", relative(directory))
        run("cargo", "run", "--locked", "-p", "xtask", "--", "conformance", "candidate-prepare-assets", relative(directory))
        width, height = ref["width"], ref["height"]
        specs = [
            (f"{kind}-chromium", ["bash", "tools/representative-intake/svg-viewer.sh", "render", relative(directory / f"{kind}.svg"), None, relative(directory / f"{kind}-assets.json")])
            for kind in ("official", "native")]
        specs += [(f"native-{viewer}", ["bash", f"tools/pdf-harness/run-{viewer}.sh", "render", relative(directory / "native.pdf"), None, str(width), str(height)]) for viewer in ("pdfium", "pdfjs")]
        specs += [(f"{kind}-rust", ["cargo", "run", "--locked", "--manifest-path", "tools/svg-viewer-rust/Cargo.toml", "--", "render", relative(directory / f"{kind}-rust.svg"), None, relative(directory / f"{kind}-assets.json")]) for kind in ("official", "native")]
        for name, command in specs:
            repeated = []
            for repeat_index in range(2):
                raster = directory / f"{name}-{repeat_index}.rgba"
                summary = json.loads(run(*[relative(raster) if arg is None else arg for arg in command]))
                actual_width, actual_height = summary["width"], summary["height"]
                data = raster.read_bytes()
                intake.require(len(data) == actual_width * actual_height * 4, "viewer length differs")
                raw = directory / f"{name}-{repeat_index}-raw.rgba.zlib"
                raster.unlink() # temporary process output is never retained over an old observation
                retain(raw, zlib.compress(data, 9))
                summary["raw_capture"] = {"file":raw.name, **intake.descriptor(data)}
                # usvg rounds fractional dimensions; clip excess right/bottom
                # pixels at origin without resizing/re-rasterizing the image.
                if name.endswith("-rust"):
                    intake.require(width <= actual_width <= width+1 and height <= actual_height <= height+1, "Rust viewport differs")
                    data = b"".join(data[y*actual_width*4:y*actual_width*4+width*4] for y in range(height))
                else:
                    intake.require((actual_width, actual_height) == (width, height), "viewer dimensions differ")
                if name.endswith("-chromium"):
                    intake.require(summary["platform_fonts"] and all(f["isCustomFont"] for f in summary["platform_fonts"]), "ambient or absent fonts")
                repeated.append((data, summary))
            intake.require(repeated[0][0] == repeated[1][0], "viewer raster does not repeat")
            raster = directory / f"{name}.rgba.zlib"
            retain(raster, zlib.compress(repeated[0][0], 9))
            captures.append({"variant": variant, "viewer": name, "raster": bind(raster), "canonical_pixels":intake.descriptor(repeated[0][0]), "runs": [r[1] for r in repeated]})
        for name, (left, right, mode) in COMPARISONS.items():
            report = directory / f"{name}-comparison.json"
            with tempfile.TemporaryDirectory() as temporary:
                candidate_report = Path(temporary) / "report.json"
                run("cargo", "run", "--locked", "--release", "--manifest-path", "tools/representative-intake/compare/Cargo.toml", "--",
                    directory / left, directory / right, mode, directory / "regions.json", candidate_report)
                retain(report, candidate_report.read_bytes())
            judgments.append({"variant": variant, "comparison": name, "report": bind(report), "result": json.loads(report.read_bytes())})
    intake.require(sources == [bind(ROOT / p) for p in source_paths], "producer changed during collection")
    intake.require(input_bindings == [bind(ROOT / b["path"]) for b in input_bindings], "inputs changed during collection")
    intake.verify_capture(bundle, native)
    files = {str(p.relative_to(output)): intake.descriptor(p.read_bytes()) for p in sorted(output.rglob("*")) if p.is_file()}
    files.pop("viewers.json", None)
    if old_files is not None:
        intake.require(all(files.get(name)==expected for name,expected in old_files.items()), "repeat changed retained artifacts/judgments")
    record = {"schema": "excalidraw-render/representative-intake-viewers/v1", "status": "pending-review", "approval": None,
              "eligible_executions": [], "sources": sources, "inputs": input_bindings, "files": files, "reference": bind(reference_root / "reference.json"),
              "native": bind(native / "capture.json"), "captures": captures, "judgments": judgments,
              "viewport_adapter": "truncate fractional viewport at origin; never resize or align; proposed, not approved"}
    (output / "viewers.json").write_bytes(intake.encode(record))
    print(json.dumps({"passed": sum(j["result"]["passed"] for j in judgments), "comparisons": len(judgments), "approval": None}))


def verify(bundle, reference_root, native, output):
    intake.verify(bundle)
    intake.verify_capture(bundle, native)
    record = intake.decode((output / "viewers.json").read_bytes())
    reference = intake.decode((reference_root / "reference.json").read_bytes())
    intake.require([s["path"] for s in record["sources"]] == producer_paths(reference), "incomplete producer inventory")
    intake.require(record["schema"] == "excalidraw-render/representative-intake-viewers/v1" and record["approval"] is None and record["eligible_executions"] == [], "evidence cannot grant approval")
    expected_inputs = [bind(path) for path in [bundle / "manifest.json", bundle / "plan.json", bundle / "scene.excalidraw", reference_root / "reference.json", native / "capture.json"]]
    intake.require(record["inputs"] == expected_inputs and record["reference"] == bind(reference_root / "reference.json") and record["native"] == bind(native / "capture.json"), "evidence input bindings changed")
    for binding in record["inputs"] + record["sources"]:
        relative(ROOT / binding["path"])
        intake.require(bind(ROOT / binding["path"]) == binding, f"stale binding {binding['path']}")
    actual = {str(p.relative_to(output)): intake.descriptor(p.read_bytes()) for p in sorted(output.rglob("*")) if p.is_file() and p != output / "viewers.json"}
    intake.require(actual == record["files"], "viewer file inventory or digest changed")
    plan = intake.decode((bundle / "plan.json").read_bytes())
    reference = intake.decode((reference_root / "reference.json").read_bytes())
    scene = intake.decode((bundle / "scene.excalidraw").read_bytes())
    expected = [(v["id"], name) for v in plan["variants"] for name in COMPARISONS]
    intake.require([(j["variant"], j["comparison"]) for j in record["judgments"]] == expected, "incomplete judgment inventory")
    for row in reference["rows"]:
        path = output / row["variant"] / "regions.json"
        intake.require(json.loads(path.read_bytes()) == regions(scene, plan, row), "crop plan detached from source anchors")
        for name, expected_descriptor in row["files"].items():
            intake.require(name in ("official.svg", "official.png", "official-png.rgba.zlib"), "unexpected reference file")
            intake.require(intake.descriptor((output / row["variant"] / name).read_bytes()) == expected_descriptor, "copied reference changed")
        for fmt in intake.FORMATS:
            intake.require((output / row["variant"] / f"native.{fmt}").read_bytes() == (native / f"{row['variant']}-{fmt}.{fmt}").read_bytes(), "copied native changed")
    expected_captures = [(v["id"], name) for v in plan["variants"] for name in ["official-chromium", "native-chromium", "native-pdfium", "native-pdfjs", "official-rust", "native-rust"]]
    intake.require([(c["variant"],c["viewer"]) for c in record["captures"]] == expected_captures, "incomplete viewer inventory")
    for capture in record["captures"]:
        path = output / capture["variant"] / f"{capture['viewer']}.rgba.zlib"
        intake.require(bind(path) == capture["raster"] and len(capture["runs"]) == 2, "invalid raster capture")
        pixels = zlib.decompress(path.read_bytes())
        intake.require(intake.descriptor(pixels) == capture["canonical_pixels"], "decoded raster digest changed")
        ref = next(r for r in reference["rows"] if r["variant"] == capture["variant"])
        width,height = ref["width"],ref["height"]
        raw_runs = []
        for index, receipt in enumerate(capture["runs"]):
            raw_name = f"{capture['viewer']}-{index}-raw.rgba.zlib"
            raw = zlib.decompress((path.parent / raw_name).read_bytes())
            intake.require(receipt["raw_capture"] == {"file":raw_name, **intake.descriptor(raw)}, "raw receipt detached")
            w,h = receipt["width"],receipt["height"]
            intake.require(type(w) is int and type(h) is int and len(raw)==w*h*4, "invalid receipt dimensions")
            raw_runs.append(raw)
            if capture["viewer"].endswith("-rust"):
                intake.require(width<=w<=width+1 and height<=h<=height+1, "invalid Rust grid")
                clipped=b"".join(raw[y*w*4:y*w*4+width*4] for y in range(height))
                intake.require(clipped==pixels, "Rust clip differs")
            else:
                intake.require((w,h)==(width,height) and raw==pixels, "viewer grid differs")
                if capture["viewer"].endswith("-chromium"):
                    intake.require(receipt["browser_version"]=="149.0.7827.55" and receipt["lifecycle_updates"]==2 and receipt["animation_offset_microseconds"]==0, "browser receipt identity")
                    intake.require(receipt["platform_fonts"] and all(f["isCustomFont"] is True for f in receipt["platform_fonts"]), "font audit failed")
                    intake.require(receipt["rgba_sha256"]==intake.descriptor(raw)["sha256"], "browser receipt digest")
                else:
                    intake.require(receipt["sha256"]==intake.descriptor(raw)["sha256"] and receipt["byte_length"]==len(raw), "PDF receipt digest")
                    intake.require(receipt["pixel_format"]=="straight-alpha-srgb-rgba8" and receipt["layout"]=="top-left-row-major", "PDF pixel profile")
                    if capture["viewer"].endswith("pdfium"):
                        intake.require(receipt["pdfium"]=="152.0.7947.0" and receipt["pypdfium2"]=="5.12.1", "PDFium identity")
                    else: intake.require(receipt["pdfjs"]=="6.1.200", "PDF.js identity")
        intake.require(raw_runs[0]==raw_runs[1], "raw repeats differ")
        if capture["viewer"].endswith("-chromium"):
            intake.require(capture["runs"][0]["container_namespace"] != capture["runs"][1]["container_namespace"], "browser process isolation missing")
    with tempfile.TemporaryDirectory() as temporary:
        for judgment in record["judgments"]:
            directory = output / judgment["variant"]
            left,right,mode = COMPARISONS[judgment["comparison"]]
            retained = directory / f"{judgment['comparison']}-comparison.json"
            intake.require(bind(retained)==judgment["report"] and json.loads(retained.read_bytes())==judgment["result"], "comparison report detached")
            report = Path(temporary) / "report.json"
            run("cargo", "run", "--locked", "--release", "--manifest-path", "tools/representative-intake/compare/Cargo.toml", "--",
                directory / left, directory / right, mode, directory / "regions.json", report)
            intake.require(json.loads(report.read_bytes()) == judgment["result"], "stored judgment differs from actual canonical comparison")
    print(json.dumps({"status":"verified-observations", "approval":None,"eligible_executions":[]}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("reference", type=Path)
    parser.add_argument("native", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--repeat-unapproved", action="store_true", help="re-execute viewers; require every existing artifact/judgment byte unchanged")
    args = parser.parse_args()
    for path in (args.bundle, args.reference, args.native, args.output):
        relative(path)
    intake.require(not (args.verify and args.repeat_unapproved), "choose verify or repeat")
    if args.verify: verify(args.bundle, args.reference, args.native, args.output)
    else: collect(args.bundle, args.reference, args.native, args.output, args.repeat_unapproved)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, TypeError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
