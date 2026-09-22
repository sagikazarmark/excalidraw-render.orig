"""Prepare representative intake review material; not an approval consumer."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import sys
import tempfile


SCHEMA = "excalidraw-render/representative-intake/v1"
FORMATS = ["svg", "png", "pdf"]
FILES = ["scene.excalidraw", "LICENSE", "PROVENANCE.json", "plan.json"]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def decode(data):
    def members(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f"duplicate JSON member: {key}")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=members,
                      parse_constant=lambda token: require(False, f"non-finite JSON: {token}"))


def encode(value):
    # Tool-owned JSON only; scene bytes and source number spelling stay opaque.
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False) + "\n").encode()


def descriptor(data):
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def read(root, name):
    path = root / name
    require(not path.is_symlink(), f"symlink is not an immutable intake file: {name}")
    return path.read_bytes()


def census(scene):
    counts = {}
    live = [element for element in scene["elements"] if not element.get("isDeleted", False)]
    for element in live:
        kind = element["type"]
        counts[kind] = counts.get(kind, 0) + 1
    return {"live_elements": len(live), "element_types": counts,
            "bound_text": sum(e.get("type") == "text" and bool(e.get("containerId")) for e in live),
            "bound_arrows": sum(e.get("type") == "arrow" and bool(e.get("startBinding") or e.get("endBinding")) for e in live),
            "font_families": sorted({e["fontFamily"] for e in live if e["type"] == "text"}),
            "assets": len(scene["files"])}


def validate_plan(plan, scene, provenance):
    require(set(plan) == {"schema", "case_id", "source_identity", "purpose", "sanitization", "variants", "formats", "applicability", "regions"}, "unknown or missing intake plan field")
    require(set(plan["sanitization"]) == {"operation", "review_notes"}, "invalid sanitization fields")
    require(plan["schema"] == "excalidraw-render/representative-intake-plan/v1", "unsupported plan schema")
    require(re.fullmatch(r"representative\.[a-z0-9-]+\.v[1-9][0-9]*", plan["case_id"]), "invalid case identity")
    require(plan["source_identity"] == provenance["identity"], "source identity mismatch")
    require(plan["purpose"].strip(), "missing representative purpose")
    require(plan["sanitization"]["operation"] == "reviewed-no-change", "only reviewed-no-change intake is supported")
    require(plan["sanitization"]["review_notes"].strip(), "missing sanitization review notes")
    require(plan["formats"] == FORMATS, "intake requires SVG, PNG and PDF")
    require(plan["variants"], "missing variants")
    ids = set()
    for variant in plan["variants"]:
        require(set(variant) == {"id", "color_mode", "include_background", "padding", "scale", "metadata"}, "unknown or missing variant option")
        require(re.fullmatch(r"[a-z][a-z0-9-]*", variant["id"]) and variant["id"] not in ids, "invalid or duplicate variant")
        ids.add(variant["id"])
        require(variant["color_mode"] in ("light", "dark"), "invalid color mode")
        for key in ("include_background", "metadata"):
            require(type(variant[key]) is bool, f"invalid {key}")
        for key in ("padding", "scale"):
            value = variant[key]
            require(type(value) in (int, float) and math.isfinite(value), f"invalid {key}")
        require(0 <= variant["padding"] <= 100 and 0 < variant["scale"] <= 4, "request outside intake envelope")
    require(set(plan["applicability"]) == {"frame_exports", "assets", "sizing"}, "incomplete applicability decisions")
    require(all(isinstance(v, str) and v.strip() for v in plan["applicability"].values()), "empty applicability reason")
    elements = {e["id"] for e in scene["elements"] if not e.get("isDeleted", False)}
    region_ids = set()
    for region in plan["regions"]:
        require(set(region) == {"id", "class", "element_ids"}, "invalid region fields")
        require(re.fullmatch(r"[a-z][a-z0-9-]*", region["id"]) and region["id"] not in region_ids, "invalid or duplicate region")
        region_ids.add(region["id"])
        require(region["class"] in ("geometry/image", "typography", "mixed-representative"), "invalid visual class")
        require(isinstance(region["element_ids"], list) and set(region["element_ids"]) <= elements, "unknown crop element")
        require(region["id"] == "whole" or region["element_ids"], "crop needs source anchors")
    require("whole" in region_ids, "missing whole-scene judgment")


def materialize(files):
    provenance = decode(files["PROVENANCE.json"])
    plan = decode(files["plan.json"])
    scene = decode(files["scene.excalidraw"])
    require(scene["type"] == "excalidraw" and scene["version"] == 2, "scene envelope")
    require(isinstance(scene["elements"], list) and isinstance(scene["appState"], dict) and isinstance(scene["files"], dict), "scene envelope")
    require(provenance["scene"]["file"] == "scene.excalidraw" and provenance["license"]["text"] == "LICENSE", "source file names must be local fixed names")
    require(descriptor(files["scene.excalidraw"]) == {key: provenance["scene"][key] for key in ("bytes", "sha256")}, "upstream scene digest/length mismatch")
    upstream = provenance["upstream"]
    require(re.fullmatch(r"[0-9a-f]{40}", upstream["commit"]), "upstream commit must be immutable")
    require(upstream["repository"].startswith("https://github.com/") and upstream["permalink"] == f'{upstream["repository"]}/blob/{upstream["commit"]}/{upstream["path"]}', "upstream permalink mismatch")
    require(provenance["license"]["spdx"].strip() and files["LICENSE"].strip(), "missing source license")
    validate_plan(plan, scene, provenance)
    return {"schema": SCHEMA, "case_id": plan["case_id"], "status": "pending-review", "approval": None,
            "eligible_executions": [], "files": {name: descriptor(data) for name, data in files.items()},
            "source": upstream, "license": provenance["license"]["spdx"],
            "sanitization": {**plan["sanitization"], "original_sha256": descriptor(files["scene.excalidraw"])["sha256"],
                             "retained_sha256": descriptor(files["scene.excalidraw"])["sha256"], "sign_off": None},
            "census": census(scene),
            "proposed_executions": [f'{plan["case_id"]}:{v["id"]}:{f}' for v in plan["variants"] for f in FORMATS],
            "pending": ["sanitization-and-applicability-review", "reference-and-viewer-evidence", "independent-semantics-reports-notices",
                        "resource-outcomes", "canonical-crops", "exact-subject-approval", "registry-and-target-integration"]}


def prepare(source, plan, output):
    files = {name: read(source, name) for name in FILES if name != "plan.json"}
    files["plan.json"] = plan.read_bytes()
    manifest = materialize(files)
    # Validate everything before creating any destination, and never overwrite
    # an existing bundle. The manifest is written last as the completion marker.
    output.mkdir()
    for name, data in files.items():
        (output / name).write_bytes(data)
    (output / "manifest.json").write_bytes(encode(manifest))
    return manifest


def verify(root):
    require({p.name for p in root.iterdir()} == {*FILES, "manifest.json"}, "intake file inventory changed")
    files = {name: read(root, name) for name in FILES}
    manifest = materialize(files)
    require(read(root, "manifest.json") == encode(manifest), "intake manifest or file binding changed")
    return manifest


def request_args(variant, fmt):
    args = ["--format", fmt, "--compatibility-revision", "1", "--padding", str(variant["padding"]),
            "--scale", str(variant["scale"]), "--report-format", "json"]
    if variant["color_mode"] == "dark":
        args.append("--dark-mode")
    if not variant["include_background"]:
        args.append("--no-background")
    if variant["metadata"]:
        args += ["--embed-scene", "--embed-provenance", "--embed-diagnostics"]
    return args


def execution_row(variant, fmt):
    key = f'{variant["id"]}-{fmt}'
    return {"variant": variant["id"], "format": fmt, "fresh_process_runs": 2,
            "request_args": request_args(variant, fmt), "artifact": f"{key}.{fmt}", "report": f"{key}-report.json"}


def validate_descriptor(value):
    require(isinstance(value, dict) and set(value) == {"bytes", "sha256"}, "invalid file descriptor")
    require(type(value["bytes"]) is int and value["bytes"] > 0, "invalid descriptor length")
    require(isinstance(value["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", value["sha256"]), "invalid descriptor digest")


def validate_report(artifact, report, fmt):
    parsed = decode(report)
    require(set(parsed) == {"schema_version", "status", "renderer_version", "compatibility_revision", "excalidraw_release",
                            "reference_profile_digest", "format", "artifact", "error", "diagnostics"}, "invalid report envelope")
    require(type(parsed["schema_version"]) is int and parsed["schema_version"] == 1 and parsed["status"] == "success" and parsed["error"] is None, "invalid success report")
    require(type(parsed["compatibility_revision"]) is int and parsed["compatibility_revision"] == 1 and parsed["excalidraw_release"] == "v0.18.1", "unexpected compatibility identity")
    require(isinstance(parsed["renderer_version"], str) and parsed["renderer_version"], "missing renderer version")
    require(isinstance(parsed["reference_profile_digest"], str) and re.fullmatch(r"sha256:[0-9a-f]{64}", parsed["reference_profile_digest"]), "invalid profile identity")
    require(parsed["format"] == fmt and isinstance(parsed["diagnostics"], list), "invalid report format/diagnostics")
    for diagnostic in parsed["diagnostics"]:
        require(set(diagnostic) == {"code", "severity", "format", "effects", "element_id", "json_pointer", "message", "details"}, "invalid diagnostic envelope")
        require(diagnostic["format"] == fmt and diagnostic["severity"] in ("info", "warning"), "invalid diagnostic format/severity")
        require(isinstance(diagnostic["code"], str) and diagnostic["code"] and isinstance(diagnostic["message"], str), "invalid diagnostic identity")
        require(diagnostic["element_id"] is None or isinstance(diagnostic["element_id"], str), "invalid diagnostic element identity")
        require(isinstance(diagnostic["json_pointer"], str) and (not diagnostic["json_pointer"] or diagnostic["json_pointer"].startswith('/')), "invalid diagnostic pointer")
        require(isinstance(diagnostic["effects"], list) and diagnostic["effects"] and set(diagnostic["effects"]) <= {"input_repair", "visual", "semantic", "metadata"}, "invalid diagnostic effects")
        require(diagnostic["effects"] == [effect for effect in ("input_repair", "visual", "semantic", "metadata") if effect in diagnostic["effects"]], "diagnostic effects must be unique and ordered")
        # Details are retained byte-for-byte observations. Their revision-specific
        # semantic validation belongs to the existing renderer report contracts,
        # not a second registry maintained in this preparation tool.
        require(diagnostic["details"] is None or isinstance(diagnostic["details"], dict), "invalid diagnostic details container")
    media = {"svg": "image/svg+xml", "png": "image/png", "pdf": "application/pdf"}
    digest = descriptor(artifact)
    require(parsed["artifact"] == {"media_type": media[fmt], "byte_length": digest["bytes"], "sha256": "sha256:" + digest["sha256"]}, "report detached from requested artifact")
    require((fmt == "svg" and b"<svg" in artifact[:1000]) or (fmt == "png" and artifact.startswith(b"\x89PNG\r\n\x1a\n")) or (fmt == "pdf" and artifact.startswith(b"%PDF-")), "wrong artifact carrier")


def capture(bundle, renderer, output, timeout):
    manifest = verify(bundle)
    inputs = {name: read(bundle, name) for name in FILES}
    require(materialize(inputs) == manifest, "intake changed before capture")
    intake_binding = descriptor(encode(manifest))
    tool_binding = descriptor(Path(__file__).read_bytes())
    renderer = renderer.resolve(strict=True)
    binary = descriptor(renderer.read_bytes())
    plan = decode(inputs["plan.json"])
    require(not output.exists(), "capture destination already exists")
    rows, files = [], {}
    keys = [(v, f) for v in plan["variants"] for f in FORMATS]
    first = {}
    # Both passes use fresh CLI processes. Reverse variant/format order on the
    # second pass, and retain observations rather than inventing expectations.
    with tempfile.TemporaryDirectory(prefix="representative-capture-") as temporary:
        scene = Path(temporary) / "scene.excalidraw"
        scene.write_bytes(inputs["scene.excalidraw"])
        for run, entries in enumerate((keys, list(reversed(keys)))):
            for variant, fmt in entries:
                key = f'{variant["id"]}-{fmt}'
                artifact = Path(temporary) / f"{run}-{key}.{fmt}"
                report = Path(temporary) / f"{run}-{key}-report.json"
                process = subprocess.run([str(renderer), str(scene),
                                          "--output", str(artifact), "--report-output", str(report),
                                          *request_args(variant, fmt)], capture_output=True, timeout=timeout)
                require(process.returncode == 0, f"render failed for {key}: {process.stderr.decode(errors='replace')}")
                observed = (artifact.read_bytes(), report.read_bytes())
                validate_report(*observed, fmt)
                if run == 0:
                    first[key] = observed
                else:
                    require(first[key] == observed, f"artifact/report bytes changed between fresh processes: {key}")
    require(descriptor(renderer.read_bytes()) == binary, "renderer changed during capture")
    require(verify(bundle) == manifest, "intake changed during capture")
    require(descriptor(Path(__file__).read_bytes()) == tool_binding, "tool changed during capture")
    for variant, fmt in keys:
        key = f'{variant["id"]}-{fmt}'
        artifact, report = first[key]
        files[f"{key}.{fmt}"] = artifact
        files[f"{key}-report.json"] = report
        rows.append(execution_row(variant, fmt))
    record = {"schema": "excalidraw-render/representative-intake-capture/v1", "case_id": manifest["case_id"],
              "status": "observed-unapproved", "eligible_executions": [], "approval": None,
              "intake_manifest": intake_binding, "renderer_binary": binary,
              "tool": tool_binding, "rows": rows,
              "files": {name: descriptor(data) for name, data in files.items()}}
    output.mkdir()
    for name, data in files.items():
        (output / name).write_bytes(data)
    (output / "capture.json").write_bytes(encode(record))
    return record


def verify_capture(bundle, output):
    manifest = verify(bundle)
    record = decode(read(output, "capture.json"))
    require(set(record) == {"schema", "case_id", "status", "eligible_executions", "approval", "intake_manifest", "renderer_binary", "tool", "rows", "files"}, "invalid capture envelope")
    for key in ("intake_manifest", "renderer_binary", "tool"):
        validate_descriptor(record[key])
    require(record["schema"] == "excalidraw-render/representative-intake-capture/v1" and record["case_id"] == manifest["case_id"], "capture identity changed")
    require(record["status"] == "observed-unapproved" and record["approval"] is None and record["eligible_executions"] == [], "capture cannot grant approval")
    require(record["intake_manifest"] == descriptor(read(bundle, "manifest.json")), "capture intake binding changed")
    require(record["tool"] == descriptor(Path(__file__).read_bytes()), "capture tool binding changed")
    plan = decode(read(bundle, "plan.json"))
    expected_rows = []
    expected_files = set()
    for variant in plan["variants"]:
        for fmt in FORMATS:
            key = f'{variant["id"]}-{fmt}'
            artifact, report = f"{key}.{fmt}", f"{key}-report.json"
            expected_files.update((artifact, report))
            expected_rows.append(execution_row(variant, fmt))
            data = read(output, artifact)
            validate_report(data, read(output, report), fmt)
    require(record["rows"] == expected_rows and set(record["files"]) == expected_files, "capture execution inventory changed")
    require({p.name for p in output.iterdir()} == expected_files | {"capture.json"}, "capture file inventory changed")
    for name in expected_files:
        require(descriptor(read(output, name)) == record["files"][name], f"capture file changed: {name}")
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("prepare")
    command.add_argument("source", type=Path)
    command.add_argument("plan", type=Path)
    command.add_argument("output", type=Path)
    command = commands.add_parser("verify")
    command.add_argument("bundle", type=Path)
    command = commands.add_parser("capture")
    command.add_argument("bundle", type=Path)
    command.add_argument("renderer", type=Path)
    command.add_argument("output", type=Path)
    command.add_argument("--timeout", type=float, default=180, help="maximum seconds per render process")
    command = commands.add_parser("verify-capture")
    command.add_argument("bundle", type=Path)
    command.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        result = prepare(args.source, args.plan, args.output)
    elif args.command == "verify":
        result = verify(args.bundle)
    elif args.command == "capture":
        require(math.isfinite(args.timeout) and 0 < args.timeout <= 1800, "invalid process timeout")
        result = capture(args.bundle, args.renderer, args.output, args.timeout)
    else:
        result = verify_capture(args.bundle, args.output)
    print(encode({"case_id": result["case_id"], "status": result["status"], "eligible_executions": []}).decode(), end="")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
