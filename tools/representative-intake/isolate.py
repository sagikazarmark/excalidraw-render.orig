"""Bounds-preserving paint isolation for the Velox PNG investigation (#84).

Derived diagnostic scenes are not sanitized replacements or conformance cases.
Only stroke/background paint is hidden; source order, text, geometry and bindings
remain intact. Reference and native viewBoxes must match the original exactly.
"""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import struct
import xml.etree.ElementTree as ET

import intake

ROOT = Path(__file__).resolve().parents[2]
INTAKE = ROOT / "tests/conformance/intake/velox-design-v1"
ORIGINAL = ROOT / "tests/conformance/proposals/velox84-evidence-v1"
IMAGE = "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9"
RECTANGLE = "TQgY1veWt3TS0QhLqucyH"
ARROW = "zmn0HQb9Lpi-dAcRCscJX"
CONTROLS = ("geometry", "text", "rectangles", "arrows", "client-rectangle", "client-arrow",
            "connection-rectangle", "diff-rectangle", "json-rectangle", "rectangles-fill", "rectangles-stroke", "minimal-rectangle")
PRODUCERS = ["tools/representative-intake/isolate.py", "tools/representative-intake/intake.py",
             "tools/representative-intake/reference.mjs", "tools/representative-intake/compare/Cargo.toml",
             "tools/representative-intake/compare/Cargo.lock", "tools/representative-intake/compare/src/main.rs",
             "tools/reference-oracle/src/browser-entry.js", "tools/reference-oracle/package-lock.json",
             "tests/oracle/reference-profiles/revision-1.json", "tests/oracle/reference-profiles/revision-1-assets.json"]


def bindings():
    paths = PRODUCERS + [str(p.relative_to(ROOT)) for p in (ROOT / "crates/excalidraw-render/src").rglob("*.rs")]
    paths += [str(p.relative_to(ROOT)) for p in (ROOT / "crates/excalidraw-render-compare/src").glob("*.rs")]
    paths += ["Cargo.toml", "Cargo.lock", "crates/excalidraw-render/Cargo.toml", "crates/excalidraw-render-cli/src/main.rs",
              "tests/conformance/intake/velox-design-v1/scene.excalidraw", "tests/conformance/intake/velox-design-v1/plan.json"]
    paths += [str((ORIGINAL / "viewers" / mode / "regions.json").relative_to(ROOT)) for mode in ("light", "dark")]
    paths += [str((ORIGINAL / "reference/reference.json").relative_to(ROOT))]
    return [{"path":p, **intake.descriptor((ROOT / p).read_bytes())} for p in sorted(set(paths))]


def execute(*args):
    process = subprocess.run(list(map(str, args)), cwd=ROOT, capture_output=True, text=True, timeout=300)
    intake.require(process.returncode == 0, f"command failed: {args}\n{process.stdout}\n{process.stderr}")
    return process.stdout


def local(path):
    return str(path.resolve().relative_to(ROOT))


def visible(element, control):
    return {"geometry": element["type"] != "text", "text": element["type"] == "text",
            "rectangles": element["type"] == "rectangle", "arrows": element["type"] == "arrow",
            "client-rectangle": element["id"] == RECTANGLE,
            "client-arrow": element["id"] == ARROW,
            "connection-rectangle": element["id"] == "GzZ5DxFQBXtgobCm4Ty_x",
            "diff-rectangle": element["id"] == "8NX6tIYP2wHnSL-Ik3OhC",
            "json-rectangle": element["id"] == "m9ZSH5z498ffbdFB4VeIg",
            "rectangles-fill": element["type"] == "rectangle",
            "rectangles-stroke": element["type"] == "rectangle",
            "minimal-rectangle": element["id"] == RECTANGLE}[control]


def transform(source, control):
    scene = copy.deepcopy(source)
    if control == "minimal-rectangle":
        rectangle = copy.deepcopy(next(e for e in source["elements"] if e["id"] == RECTANGLE))
        rectangle["boundElements"] = None
        min_x,min_y = min(e["x"] for e in source["elements"]),min(e["y"] for e in source["elements"])
        max_x = max(e["x"]+e["width"] for e in source["elements"])
        max_y = max(e["y"]+e["height"] for e in source["elements"])
        anchor = copy.deepcopy(rectangle)
        anchor.update(id="bounds-anchor",x=min_x,y=min_y,width=max_x-min_x,height=max_y-min_y,
                      strokeColor="transparent",backgroundColor="transparent",roundness=None,index="Zz")
        scene["elements"]=[anchor,rectangle]
        return scene
    for element in scene["elements"]:
        if not visible(element, control):
            element["strokeColor"] = element["backgroundColor"] = "transparent"
        elif control == "rectangles-fill": element["strokeColor"] = "transparent"
        elif control == "rectangles-stroke": element["backgroundColor"] = "transparent"
    return scene


def prepare(output):
    intake.verify(INTAKE)
    source = intake.decode((INTAKE / "scene.excalidraw").read_bytes())
    plan = intake.decode((INTAKE / "plan.json").read_bytes())
    output.mkdir()
    for control in CONTROLS:
        directory = output / control
        directory.mkdir()
        scene = transform(source, control)
        hidden = [e["id"] for e in source["elements"] if not visible(e, control)]
        (directory / "scene.excalidraw").write_bytes(intake.encode(scene))
        (directory / "plan.json").write_bytes(intake.encode(plan))
        (directory / "derivation.json").write_bytes(intake.encode({
            "control": control, "source": intake.descriptor((INTAKE / "scene.excalidraw").read_bytes()),
            "operation": "two-element bounds-anchor reduction" if control == "minimal-rectangle" else "hide excluded paint; fill/stroke controls additionally hide rectangle stroke/fill",
            "hidden_elements": hidden, "scene": intake.descriptor((directory / "scene.excalidraw").read_bytes()),
            "approval": None, "eligible_executions": [],
        }))
    print(json.dumps({"controls": list(CONTROLS), "status": "diagnostic-inputs"}))


def verify_inputs(output):
    source = intake.decode((INTAKE / "scene.excalidraw").read_bytes())
    for control in CONTROLS:
        directory = output / control
        scene = intake.decode((directory / "scene.excalidraw").read_bytes())
        expected = transform(source, control)
        intake.require(scene == expected, f"{control} changed more than paint visibility")
        derivation = intake.decode((directory / "derivation.json").read_bytes())
        intake.require(derivation["scene"] == intake.descriptor((directory / "scene.excalidraw").read_bytes()), "stale derived scene")
        intake.require(derivation["source"] == intake.descriptor((INTAKE / "scene.excalidraw").read_bytes()), "stale original scene")
        intake.require((directory / "plan.json").read_bytes() == intake.encode(intake.decode((INTAKE / "plan.json").read_bytes())), "derived request changed")


def collect(output, renderer, selected):
    intake.require(not (output / "isolation.json").exists(), "collection already completed; use a new directory")
    intake.require(len(set(selected))==len(selected), "duplicate selected control")
    intake.require(all(not (output / control / "reference").exists() for control in selected), "control already collected; use a new directory")
    verify_inputs(output)
    renderer = renderer.resolve(strict=True)
    binary = intake.descriptor(renderer.read_bytes())
    sources = bindings()
    original_reference = intake.decode((ORIGINAL / "reference/reference.json").read_bytes())
    execute("docker", "run", "--rm", "--network", "none", "--platform", "linux/amd64",
            "--mount", f"type=bind,src={ROOT},dst=/workspace", "--workdir", "/workspace/tools/reference-oracle",
            "--user", f"{os.getuid()}:{os.getgid()}", "--env", "HOME=/tmp", "--env", "REFERENCE_ORACLE_CANONICAL=1", IMAGE,
            "bash", "-eu", "-c", "npm run build && node scripts/profile.mjs verify")
    results = []
    for control in selected:
        directory = output / control
        execute("docker", "run", "--rm", "--init", "--ipc=host", "--network", "none", "--platform", "linux/amd64",
                "--user", f"{os.getuid()}:{os.getgid()}", "--env", "HOME=/tmp",
                "--env", "REFERENCE_ORACLE_CANONICAL=1", "--mount", f"type=bind,src={ROOT},dst=/workspace",
                "--workdir", "/workspace/tools/reference-oracle", IMAGE, "node", "../representative-intake/reference.mjs",
                local(directory), local(directory / "reference"))
        reference = intake.decode((directory / "reference/reference.json").read_bytes())
        plan = intake.decode((directory / "plan.json").read_bytes())
        for variant, ref, original in zip(plan["variants"], reference["rows"], original_reference["rows"]):
            mode = variant["id"]
            intake.require(all(ref[key] == original[key] for key in ("variant", "width", "height", "svg_width", "svg_height", "view_box")), "reference bounds or phase changed")
            first = {}
            for repeat in range(2):
                for fmt in ("png", "svg") if repeat == 0 else ("svg", "png"):
                    artifact = directory / f"{mode}-{repeat}.{fmt}"
                    report = directory / f"{mode}-{repeat}-{fmt}-report.json"
                    execute(renderer, directory / "scene.excalidraw", "--output", artifact,
                            "--report-output", report, *intake.request_args(variant, fmt))
                    data = artifact.read_bytes(), report.read_bytes()
                    intake.validate_report(*data, fmt)
                    if repeat == 0: first[fmt] = data
                    else: intake.require(data == first[fmt], "native artifact/report repeat differs")
                    if fmt == "svg":
                        intake.require(ET.fromstring(data[0]).attrib["viewBox"] == original["view_box"], "native bounds changed")
            region_plan = ORIGINAL / "viewers" / mode / "regions.json"
            report_path = directory / f"{mode}-comparison.json"
            execute("cargo", "run", "--locked", "--release", "--manifest-path", "tools/representative-intake/compare/Cargo.toml", "--",
                    directory / "reference" / mode / "official-png.rgba.zlib", directory / f"{mode}-0.png", "canonical", region_plan, report_path)
            result = intake.decode(report_path.read_bytes())
            focused_plan = intake.decode(region_plan.read_bytes())
            focused_plan["crops"] = [{"id":"client-top-stroke-nontext","class":"geometry/image","x":1250,"y":26,"width":200,"height":15}]
            focused_path = directory / f"{mode}-nontext-regions.json"
            focused_path.write_bytes(intake.encode(focused_plan))
            focused_report = directory / f"{mode}-nontext-comparison.json"
            execute("cargo", "run", "--locked", "--release", "--manifest-path", "tools/representative-intake/compare/Cargo.toml", "--",
                    directory / "reference" / mode / "official-png.rgba.zlib", directory / f"{mode}-0.png", "canonical", focused_path, focused_report)
            results.append({"control": control, "mode": mode, "result": result,"nontext":intake.decode(focused_report.read_bytes())})
    verify_inputs(output)
    intake.require(intake.descriptor(renderer.read_bytes()) == binary, "renderer changed")
    intake.require(bindings() == sources, "producer source changed")
    record = {"schema": "excalidraw-render/velox-png-isolation/v1", "approval": None, "eligible_executions": [],
              "renderer_binary": binary, "sources":sources,"controls":selected,
              "source": intake.descriptor((INTAKE / "scene.excalidraw").read_bytes()), "results": results,
              "files": {str(p.relative_to(output)): intake.descriptor(p.read_bytes()) for p in sorted(output.rglob("*")) if p.is_file()}}
    (output / "isolation.json").write_bytes(intake.encode(record))
    for result in results:
        binding = next(c["region"] for c in result["result"]["crops"] if c["id"] == "client-binding")
        print(json.dumps({"control":result["control"],"mode":result["mode"],"changed":binding["changed_pixels"],
            "ssim":struct.unpack(">d",bytes.fromhex(binding["ssim_bits"]))[0],"ssim_pass":binding["ssim_pass"]}))


def verify(output):
    verify_inputs(output)
    record = intake.decode((output / "isolation.json").read_bytes())
    intake.require(record["schema"] == "excalidraw-render/velox-png-isolation/v1" and record["approval"] is None and record["eligible_executions"] == [], "diagnosis cannot grant approval")
    intake.require(record["sources"] == bindings(), "diagnosis source binding changed")
    files = {str(p.relative_to(output)): intake.descriptor(p.read_bytes()) for p in sorted(output.rglob("*")) if p.is_file() and p.name != "isolation.json"}
    intake.require(files == record["files"], "diagnosis inventory or bytes changed")
    intake.require(record["controls"] and len(set(record["controls"]))==len(record["controls"]) and set(record["controls"]) <= set(CONTROLS), "invalid control inventory")
    intake.require([(r["control"],r["mode"]) for r in record["results"]] == [(c,m) for c in record["controls"] for m in ("light","dark")], "incomplete results")
    originals = intake.decode((ORIGINAL / "reference/reference.json").read_bytes())["rows"]
    profile_digest = "sha256:" + intake.descriptor((ROOT / "tests/oracle/reference-profiles/revision-1.json").read_bytes())["sha256"]
    for result in record["results"]:
        directory = output / result["control"]
        mode = result["mode"]
        reference = intake.decode((directory / "reference/reference.json").read_bytes())
        intake.require(reference["scene_sha256"] == intake.descriptor((directory / "scene.excalidraw").read_bytes())["sha256"] and reference["plan_sha256"] == intake.descriptor((directory / "plan.json").read_bytes())["sha256"], "reference input detached")
        intake.require(reference["approval"] is None and reference["eligible_executions"] == [] and reference["browser"] == "149.0.7827.55" and reference["network"] == "none", "invalid reference identity")
        for source in reference["sources"]:
            intake.require(intake.descriptor((ROOT / source["path"]).read_bytes())["sha256"] == source["sha256"], "reference producer changed")
        ref = next(r for r in reference["rows"] if r["variant"]==mode)
        original = next(r for r in originals if r["variant"]==mode)
        intake.require(ref["options"] == original["options"] and ref["fresh_browser_runs"] == 2, "reference request/repeat differs")
        for name, expected in ref["files"].items():
            intake.require(name in ("official.svg", "official.png", "official-png.rgba.zlib"), "unknown reference output")
            intake.require(intake.descriptor((directory / "reference" / mode / name).read_bytes()) == expected, "reference file binding changed")
        intake.require(all(ref[k]==original[k] for k in ("width","height","svg_width","svg_height","view_box")), "reference grid changed")
        for fmt in ("svg","png"):
            first = (directory / f"{mode}-0.{fmt}").read_bytes()
            intake.require(first==(directory / f"{mode}-1.{fmt}").read_bytes(), "native repeat changed")
            report = (directory / f"{mode}-0-{fmt}-report.json").read_bytes()
            intake.require(report==(directory / f"{mode}-1-{fmt}-report.json").read_bytes(), "report repeat changed")
            intake.validate_report(first, report, fmt)
            intake.require(intake.decode(report)["reference_profile_digest"] == profile_digest, "native profile changed")
            if fmt=="svg": intake.require(ET.fromstring(first).attrib["viewBox"]==original["view_box"], "native grid changed")
        intake.require(result["result"] == intake.decode((directory / f"{mode}-comparison.json").read_bytes()), "binding judgment detached")
        intake.require(result["nontext"] == intake.decode((directory / f"{mode}-nontext-comparison.json").read_bytes()), "nontext judgment detached")
        for key, plan_path in (("result", ORIGINAL / "viewers" / mode / "regions.json"), ("nontext", directory / f"{mode}-nontext-regions.json")):
            comparison = result[key]
            intake.require(comparison["candidate"] == intake.descriptor((directory / f"{mode}-0.png").read_bytes()), "comparison candidate detached")
            intake.require(comparison["reference"] == intake.descriptor((directory / "reference" / mode / "official-png.rgba.zlib").read_bytes()), "comparison reference detached")
            intake.require(comparison["plan"] == intake.descriptor(plan_path.read_bytes()) and comparison["mode"] == "canonical", "comparison plan detached")
    print(json.dumps({"status":"verified-diagnostic-observations","approval":None,"controls":record["controls"]}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "verify-inputs", "collect", "verify", "reconcile-replay"))
    parser.add_argument("output", type=Path)
    parser.add_argument("--renderer", type=Path)
    parser.add_argument("--control", choices=CONTROLS, action="append")
    parser.add_argument("--replay", type=Path, help="fresh completed collection whose bytes must equal the retained collection")
    args = parser.parse_args()
    local(args.output)
    if args.command == "prepare": prepare(args.output)
    elif args.command == "verify-inputs": verify_inputs(args.output)
    elif args.command == "verify": verify(args.output)
    elif args.command == "reconcile-replay":
        intake.require(args.replay is not None, "fresh replay required")
        local(args.replay)
        verify(args.replay)
        fresh = intake.decode((args.replay / "isolation.json").read_bytes())
        old = intake.decode((args.output / "isolation.json").read_bytes())
        intake.require(old["approval"] is None and old["eligible_executions"] == [], "cannot reconcile approved evidence")
        intake.require(old["results"] == fresh["results"] and old["renderer_binary"] == fresh["renderer_binary"], "replay changed results or executable")
        files = {str(p.relative_to(args.output)): intake.descriptor(p.read_bytes()) for p in sorted(args.output.rglob("*")) if p.is_file() and p.name != "isolation.json"}
        intake.require(files == old["files"] == fresh["files"], "replay changed retained bytes")
        (args.output / "isolation.json").write_bytes(intake.encode(fresh))
        verify(args.output)
    else:
        intake.require(args.renderer is not None, "renderer required")
        collect(args.output, args.renderer, args.control or CONTROLS)


if __name__ == "__main__":
    try: main()
    except (ValueError, OSError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
