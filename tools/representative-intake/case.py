"""Registered representative conformance cases (#84).

A case directory `tests/conformance/cases/representative/<name>/` holds:

- `case.json`: identity, the intake bundle it renders (bound by manifest
  digest), registered variants and formats, semantic expectations, exact-ceiling
  tolerance exceptions, the retained-file inventory and the approval record;
- `reference/`: the pinned reference collection the comparisons need
  (`reference.json` and, per variant, `official.svg`, `official.png` and
  `official-png.rgba.zlib`);
- `expected-report-<format>.json`: independently authored report expectations
  (`{"schema_version":1,"diagnostics":[...]}`, the focused-case form).

`verify` checks the retained bytes and approval subject offline. With
`--renderer` it renders the intake twice in fresh processes (exact repeat bytes,
reports, dimensions, semantics including mandatory notices) and checks every
declared resource outcome (a failing render, no artifact, a repeated report
matching its authored fields). With `--viewers` it also runs every canonical
viewer through `evidence.py` and requires each registered comparison to pass
its budget or a matching exception. An exception that no failing region uses
is rejected as dead. Nothing here grants approval; `record-approval` binds a
maintainer's #85 comment to the exact subject.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

import controls
import evidence
import intake

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "excalidraw-render/representative-case/v1"
REGISTRY = ROOT / "tests/conformance/cases/representative/registry.json"
COMPARISON_FORMATS = {"svg-chromium": "svg", "svg-rust": "svg", "pdf-pdfium": "pdf", "pdf-pdfjs": "pdf", "png-canvas": "png"}
VIEWER_PROFILES = {"svg-chromium": "canonical-svg-chromium-v1", "svg-rust": "canonical-svg-pure-rust-v1",
                   "pdf-pdfium": "canonical-pdf-pdfium-v1", "pdf-pdfjs": "canonical-pdf-pdfjs-v1", "png-canvas": None}
REFERENCE_FILES = ("official.svg", "official.png", "official-png.rgba.zlib")
APPROVAL_KEYS = {"approved_by", "approved_at", "approval_url", "approval_subject_sha256"}
ISSUE_85 = "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-"


def relative(path):
    return str(Path(path).resolve().relative_to(ROOT))


def inventory(directory):
    return {str(p.relative_to(directory)): intake.descriptor(p.read_bytes())
            for p in sorted(directory.rglob("*")) if p.is_file() and p.name != "case.json"}


def subject(case):
    """SHA-256 of the canonical case record without its approval."""
    body = {key: value for key, value in case.items() if key != "approval"}
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def materialize(output, bundle, reference, case_id, formats, semantics, reports, exceptions, evidence_record, resource_outcomes=()):
    """Create a new case directory from a verified intake bundle and collection."""
    manifest = intake.verify(bundle)
    plan = intake.decode((bundle / "plan.json").read_bytes())
    intake.require(manifest["case_id"] == case_id, "case id differs from the intake bundle")
    intake.require(set(formats) <= set(intake.FORMATS) and formats, "invalid registered formats")
    output.mkdir()
    (output / "reference").mkdir()
    shutil.copyfile(reference / "reference.json", output / "reference/reference.json")
    for variant in plan["variants"]:
        target = output / "reference" / variant["id"]
        target.mkdir()
        for name in REFERENCE_FILES:
            shutil.copyfile(reference / variant["id"] / name, target / name)
    for fmt in formats:
        (output / f"expected-report-{fmt}.json").write_bytes(json.dumps(reports[fmt], separators=(",", ":")).encode())
    comparisons = [name for name, fmt in COMPARISON_FORMATS.items() if fmt in formats]
    record = {"schema": SCHEMA, "case_id": case_id, "layer": "representative", "compatibility_revision": 1,
              "originating_issue": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/84",
              "intake": {"path": relative(bundle), **intake.descriptor((bundle / "manifest.json").read_bytes())},
              "evidence_record": {"path": relative(evidence_record), **intake.descriptor(evidence_record.read_bytes())},
              "variants": [variant["id"] for variant in plan["variants"]], "formats": formats, "comparisons": comparisons,
              "judgment_rule": JUDGMENT_RULE, "semantic_expectations": semantics, "exceptions": exceptions,
              "resource_outcomes": list(resource_outcomes),
              "files": inventory(output), "approval": None}
    validate(record, output)
    (output / "case.json").write_bytes(intake.encode(record))
    print(json.dumps({"case_id": case_id, "subject": f"sha256:{subject(record)}", "approval": None}))


def validate(case, directory):
    intake.require(case["schema"] == SCHEMA and case["layer"] == "representative", "invalid case schema")
    intake.require(case.get("judgment_rule") == JUDGMENT_RULE, "unsupported judgment rule")
    intake.require(case["files"] == inventory(directory), "retained case files changed")
    bundle = ROOT / case["intake"]["path"]
    intake.require(intake.descriptor((bundle / "manifest.json").read_bytes()) == {k: case["intake"][k] for k in ("bytes", "sha256")},
                   "intake bundle changed")
    intake.verify(bundle)
    plan = intake.decode((bundle / "plan.json").read_bytes())
    intake.require(case["variants"] == [v["id"] for v in plan["variants"]], "variant inventory differs from the plan")
    intake.require(case["comparisons"] == [n for n, f in COMPARISON_FORMATS.items() if f in case["formats"]], "comparison inventory")
    for exception in case["exceptions"]:
        intake.require(set(exception) == {"variant", "comparison_id", "region", "max_changed_pixels", "minimum_ssim_bits",
                                          "reason", "approving_issue", "viewer_profile"}, "invalid exception fields")
        intake.require(exception["variant"] in case["variants"] and exception["comparison_id"] in case["comparisons"], "exception outside the case")
        intake.require(exception["viewer_profile"] == VIEWER_PROFILES[exception["comparison_id"]], "exception viewer profile")
        intake.require(re.fullmatch(r"[0-9a-f]{16}", exception["minimum_ssim_bits"]) and exception["reason"].strip(), "invalid exception ceiling")
    for outcome in case.get("resource_outcomes", []):
        intake.require(set(outcome) == {"id", "variant", "format", "overrides", "expected_report"}, "invalid resource outcome fields")
        intake.require(outcome["variant"] in case["variants"] and outcome["format"] in intake.FORMATS, "resource outcome outside the case")
        intake.require(set(outcome["overrides"]) <= {"scale", "padding"}, "unsupported resource outcome override")
        intake.require(outcome["expected_report"]["status"] == "failure" and outcome["expected_report"]["artifact"] is None,
                       "a resource outcome expects a failed render")
    keys = [(e["variant"], e["comparison_id"], e["region"]) for e in case["exceptions"]]
    intake.require(len(keys) == len(set(keys)), "duplicate exception")
    approval = case["approval"]
    if approval is not None:
        intake.require(set(approval) == APPROVAL_KEYS and approval["approval_subject_sha256"] == f"sha256:{subject(case)}",
                       "approval does not bind the exact case subject")
        intake.require(approval["approval_url"].startswith(ISSUE_85) and approval["approval_url"][len(ISSUE_85):].isdigit(), "approval URL")


def notices(artifact):
    """The font and license notice identities an SVG or PDF artifact's manifest
    carries (PDF streams inflated); per-artifact subset identities excluded."""
    text = artifact
    if artifact.startswith(b"%PDF-"):
        inflated = []
        for match in re.finditer(rb"stream\r?\n", artifact):
            try:
                inflated.append(zlib.decompress(artifact[match.end():artifact.find(b"endstream", match.end())]))
            except zlib.error:
                continue
        text = artifact + b"".join(inflated)
    return sorted({identity.decode() for identity in re.findall(rb'"asset_identity":"((?:font|license)\.[^"]+)"', text)})


def semantics(artifacts, expectations):
    """Semantic checks on native artifacts: exact dimensions and document facts."""
    for key, (artifact, _) in artifacts.items():
        variant, fmt = key
        expected = expectations[variant]
        if fmt == "svg":
            root = re.match(rb"<svg[^>]*>", artifact).group(0).decode()
            size = [re.search(fr'\b{n}="([^"]+)"', root).group(1) for n in ("width", "height")]
            intake.require(size == expected["svg_size"], f"{key}: SVG size {size}")
            intake.require((b"<!-- payload-start -->" in artifact) == expected["embedded_scene"], f"{key}: embedded scene")
            intake.require(artifact.count(b"<text") >= expected["min_text_elements"], f"{key}: selectable text")
            intake.require(artifact.count(b"<a ") == expected["links"], f"{key}: links")
            dark = b'filter="invert(93%) hue-rotate(180deg)"' in re.match(rb"<svg[^>]*>", artifact).group(0)
            intake.require(dark == expected["root_theme_filter"], f"{key}: root theme filter")
            intake.require(notices(artifact) == expected["notices"], f"{key}: mandatory notices {notices(artifact)}")
        elif fmt == "png":
            intake.require(list(struct.unpack(">II", artifact[16:24])) == expected["png_size"], f"{key}: PNG size")
        else:
            intake.require(notices(artifact) == expected["notices"], f"{key}: mandatory notices {notices(artifact)}")
            box = re.search(rb"/MediaBox \[0 0 ([\d.]+) ([\d.]+)\]", artifact)
            intake.require([box.group(1).decode(), box.group(2).decode()] == expected["pdf_media_box"], f"{key}: PDF MediaBox")


JUDGMENT_RULE = "canonical-budget-or-identical-input-controls-or-exception/v1"


def control_envelopes(collection, variant, comparison):
    """Per-region worst case of the identical-input controls for a comparison
    (`controls.CONTROLS`): changed pixels and SSIM of the canonical viewers and
    reference authorities rendering the same input."""
    directory = collection / variant
    regions = intake.decode((directory / "regions.json").read_bytes())
    measured = [controls.compare(directory, left, right, "canonical", regions)
                for left, right in controls.CONTROLS[COMPARISON_FORMATS[comparison]]]
    return {region: (max(m[region]["changed_pixels"] for m in measured),
                     min(controls.ssim(m[region]["ssim_bits"]) for m in measured)) for region in measured[0]}


def judge(case, collection):
    """Each failing region passes by its identical-input control envelope or a
    matching exact-ceiling exception, in that order; an exception the controls
    or the budget already cover is dead."""
    record = intake.decode((collection / "viewers.json").read_bytes())
    exceptions = {(e["variant"], e["comparison_id"], e["region"]): e for e in case["exceptions"]}
    used, controlled, failures, envelopes = set(), [], [], {}
    for judgment in record["judgments"]:
        key = (judgment["variant"], judgment["comparison"])
        if judgment["comparison"] not in case["comparisons"]:
            continue
        result = judgment["result"]
        regions = [("whole", result["whole"])] + [(crop["id"], crop["region"]) for crop in result["crops"]]
        for region, measured in regions:
            if measured["changed_pixels_pass"] and measured["ssim_pass"]:
                continue
            ssim = controls.ssim(measured["ssim_bits"])
            if key not in envelopes:
                envelopes[key] = control_envelopes(collection, *key)
            envelope_changed, envelope_ssim = envelopes[key][region]
            if measured["changed_pixels"] <= envelope_changed and ssim >= envelope_ssim:
                controlled.append((*key, region))
                continue
            exception = exceptions.get((*key, region))
            if exception and measured["changed_pixels"] <= exception["max_changed_pixels"] \
                    and ssim >= controls.ssim(exception["minimum_ssim_bits"]):
                used.add((*key, region))
            else:
                failures.append({"variant": key[0], "comparison": key[1], "region": region,
                                 "changed_pixels": measured["changed_pixels"], "ssim": ssim,
                                 "control_changed_pixels": envelope_changed, "control_ssim": envelope_ssim})
    dead = sorted(set(exceptions) - used)
    intake.require(not failures, f"comparisons fail: {failures}")
    intake.require(not dead, f"dead exceptions: {dead}")
    return {"within_controls": len(controlled), "exceptions_applied": len(used)}


def resource_outcomes(case, bundle, renderer, temporary):
    """Render each declared outcome twice in fresh processes: the CLI must fail,
    write no artifact, repeat its report exactly, and match every authored field
    of the expected report."""
    plan = intake.decode((bundle / "plan.json").read_bytes())
    scene = temporary / "outcome-scene.excalidraw"
    scene.write_bytes((bundle / "scene.excalidraw").read_bytes())
    for outcome in case["resource_outcomes"]:
        variant = next(v for v in plan["variants"] if v["id"] == outcome["variant"])
        arguments = intake.request_args({**variant, **outcome["overrides"]}, outcome["format"])
        reports = []
        for run in range(2):
            artifact = temporary / f"{outcome['id']}-{run}.{outcome['format']}"
            report = temporary / f"{outcome['id']}-{run}-report.json"
            process = subprocess.run([str(renderer), str(scene), "--output", str(artifact), "--report-output", str(report),
                                      *arguments], capture_output=True, timeout=900)
            intake.require(process.returncode != 0, f"{outcome['id']}: render succeeded")
            intake.require(not artifact.exists(), f"{outcome['id']}: a failed render wrote an artifact")
            reports.append(report.read_bytes())
        intake.require(reports[0] == reports[1], f"{outcome['id']}: report differs between fresh processes")
        observed = intake.decode(reports[0])
        expected = outcome["expected_report"]
        mismatched = [key for key in expected if key != "error" and observed.get(key) != expected[key]]
        mismatched += [f"error.{key}" for key in expected.get("error", {}) if observed["error"].get(key) != expected["error"][key]]
        intake.require(not mismatched, f"{outcome['id']}: report differs from its expectation in {mismatched}")
    return len(case["resource_outcomes"])


def verify(directory, renderer=None, viewers=False):
    case = intake.decode((directory / "case.json").read_bytes())
    validate(case, directory)
    summary = {"case_id": case["case_id"], "subject": f"sha256:{subject(case)}", "approved": case["approval"] is not None}
    if renderer is None:
        print(json.dumps(summary))
        return
    bundle = ROOT / case["intake"]["path"]
    (ROOT / "target").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="representative-case-") as temporary:
        native = Path(temporary) / "native"
        intake.capture(bundle, renderer, native, 900)
        artifacts = {}
        for variant in case["variants"]:
            for fmt in case["formats"]:
                artifact = (native / f"{variant}-{fmt}.{fmt}").read_bytes()
                report = intake.decode((native / f"{variant}-{fmt}-report.json").read_bytes())
                expected = intake.decode((directory / f"expected-report-{fmt}.json").read_bytes())
                intake.require({"schema_version": report["schema_version"], "diagnostics": report["diagnostics"]} == expected,
                               f"{variant}-{fmt}: report differs from its expectation")
                artifacts[(variant, fmt)] = (artifact, report)
        semantics(artifacts, case["semantic_expectations"])
        summary["native"] = "repeatable; reports, dimensions and semantics match"
        if case.get("resource_outcomes"):
            summary["resource_outcomes"] = resource_outcomes(case, bundle, renderer, Path(temporary))
        if viewers:
            collection = Path(temporary) / "viewers"
            evidence.collect(bundle, directory / "reference", native, collection)
            summary.update(judge(case, collection))
            summary["viewers"] = "every registered comparison passes its budget or approved exception"
    print(json.dumps(summary))


def record_approval(directory, login, date, url, expected):
    case = intake.decode((directory / "case.json").read_bytes())
    validate(case, directory)
    intake.require(case["approval"] is None, "case is already approved")
    intake.require(expected.removeprefix("sha256:") == subject(case), "approval subject mismatch")
    case["approval"] = {"approved_by": login, "approved_at": date, "approval_url": url, "approval_subject_sha256": f"sha256:{subject(case)}"}
    validate(case, directory)
    (directory / "case.json").write_bytes(intake.encode(case))


def verify_registry(renderer=None, viewers=False):
    registry = intake.decode(REGISTRY.read_bytes())
    intake.require(registry["schema"] == "excalidraw-render/representative-case-registry/v1", "invalid registry")
    for entry in registry["cases"]:
        directory = ROOT / entry["path"]
        case = intake.decode((directory / "case.json").read_bytes())
        intake.require(entry["case_id"] == case["case_id"] and case["approval"] is not None, f"{entry['case_id']}: unregistered or unapproved")
        verify(directory, renderer, viewers)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    m = sub.add_parser("materialize")
    m.add_argument("output", type=Path)
    m.add_argument("--intake", type=Path, required=True)
    m.add_argument("--reference", type=Path, required=True)
    m.add_argument("--evidence-record", type=Path, required=True)
    m.add_argument("--spec", type=Path, required=True, help="JSON: case_id, formats, semantic_expectations, expected_reports, exceptions")
    v = sub.add_parser("verify")
    v.add_argument("directory", type=Path)
    v.add_argument("--renderer", type=Path)
    v.add_argument("--viewers", action="store_true")
    s = sub.add_parser("subject")
    s.add_argument("directory", type=Path)
    a = sub.add_parser("record-approval")
    a.add_argument("directory", type=Path)
    for name in ("login", "date", "url", "subject"):
        a.add_argument(name)
    j = sub.add_parser("judge", help="apply a case's budgets and exceptions to an existing viewer collection")
    j.add_argument("directory", type=Path)
    j.add_argument("collection", type=Path)
    r = sub.add_parser("verify-registry")
    r.add_argument("--renderer", type=Path)
    r.add_argument("--viewers", action="store_true")
    args = parser.parse_args()
    if args.command == "materialize":
        spec = intake.decode(args.spec.read_bytes())
        materialize(args.output, args.intake, args.reference, spec["case_id"], spec["formats"],
                    spec["semantic_expectations"], spec["expected_reports"], spec["exceptions"], args.evidence_record,
                    spec.get("resource_outcomes", []))
    elif args.command == "verify":
        intake.require(args.renderer or not args.viewers, "--viewers requires --renderer")
        verify(args.directory, args.renderer, args.viewers)
    elif args.command == "subject":
        case = intake.decode((args.directory / "case.json").read_bytes())
        validate(case, args.directory)
        print(f"sha256:{subject(case)}")
    elif args.command == "judge":
        case = intake.decode((args.directory / "case.json").read_bytes())
        validate(case, args.directory)
        print(json.dumps({"case_id": case["case_id"], **judge(case, args.collection)}))
    elif args.command == "record-approval":
        record_approval(args.directory, args.login, args.date, args.url, args.subject)
    else:
        verify_registry(args.renderer, args.viewers)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, TypeError, AttributeError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
