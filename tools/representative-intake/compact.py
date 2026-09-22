"""Compact retention of representative evidence (#84).

Large scenes produce tens of megabytes of rasters, fonts and derived SVGs per
collection. Everything is reproducible, so a compact record keeps only the JSON
records (judgments, receipts, declarations, regions, reports) plus a complete
digest inventory of every file the full collection held. `check` accepts a
fresh full collection only if every artifact, raster, judgment and derived
resource digest is identical; process receipts and the executable/tool identity
are the only fields allowed to differ. Nothing here grants approval.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys

import evidence
import intake

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "excalidraw-render/representative-compact-evidence/v1"
ROOTS = ("reference", "native", "viewers")
RECEIPTS = {"reference": "reference.json", "native": "capture.json", "viewers": "viewers.json"}
VOLATILE_RUN_FIELDS = ("browser_process_id", "container_namespace", "virtual_time_ticks_base")


def inventory(directory):
    return {str(p.relative_to(directory)): intake.descriptor(p.read_bytes()) for p in sorted(directory.rglob("*")) if p.is_file()}


def signature(kind, record):
    """Receipt content that must repeat exactly across fresh collections."""
    record = json.loads(json.dumps(record))
    if kind == "native":
        # Executable and tool digests identify the observer, not the observation.
        record.pop("renderer_binary"); record.pop("tool")
    elif kind == "viewers":
        record.pop("sources")
        record.pop("native")
        record["inputs"] = [b if not b["path"].endswith("/capture.json") else {"path": b["path"]} for b in record["inputs"]]
        for capture in record["captures"]:
            for run in capture["runs"]:
                for field in VOLATILE_RUN_FIELDS:
                    run.pop(field, None)
    return record


def retained(directory):
    return sorted(str(p.relative_to(directory)) for p in directory.rglob("*.json") if p.is_file())


def retain(bundle, roots, output):
    evidence.verify(bundle, roots["reference"], roots["native"], roots["viewers"])
    record = {"schema": SCHEMA, "status": "pending-review", "approval": None, "eligible_executions": [],
              "case_id": intake.decode((bundle / "plan.json").read_bytes())["case_id"],
              "bundle": evidence.relative(bundle), "roots": {k: evidence.relative(v) for k, v in roots.items()},
              "inventory": {k: inventory(v) for k, v in roots.items()},
              "retained": {k: retained(v) for k, v in roots.items()},
              "signatures": {k: signature(k, intake.decode((v / RECEIPTS[k]).read_bytes())) for k, v in roots.items()}}
    output.mkdir()
    for kind, directory in roots.items():
        for name in record["retained"][kind]:
            target = output / kind / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(directory / name, target)
    (output / "compact.json").write_bytes(intake.encode(record))
    verify(output)


def verify(output):
    """Offline integrity of the retained records; re-executes nothing."""
    record = intake.decode((output / "compact.json").read_bytes())
    intake.require(record["schema"] == SCHEMA and record["approval"] is None and record["eligible_executions"] == [], "compact evidence cannot grant approval")
    intake.verify(ROOT / record["bundle"])
    for kind in ROOTS:
        actual = sorted(str(p.relative_to(output / kind)) for p in (output / kind).rglob("*") if p.is_file())
        intake.require(actual == record["retained"][kind], f"{kind} retained inventory changed")
        for name in actual:
            intake.require(intake.descriptor((output / kind / name).read_bytes()) == record["inventory"][kind][name], f"retained {kind}/{name} changed")
        intake.require(signature(kind, intake.decode((output / kind / RECEIPTS[kind]).read_bytes())) == record["signatures"][kind], f"{kind} receipt signature changed")
    viewers = intake.decode((output / "viewers" / "viewers.json").read_bytes())
    intake.require(viewers["files"] == {k: v for k, v in record["inventory"]["viewers"].items() if k != "viewers.json"}, "viewer inventory detached from its record")
    bundle = ROOT / record["bundle"]
    intake.require([b for b in viewers["inputs"][:3]] == [evidence.bind(bundle / n) for n in ("manifest.json", "plan.json", "scene.excalidraw")], "evidence detached from intake bundle")
    for judgment in viewers["judgments"]:
        path = Path(judgment["report"]["path"]).relative_to(record["roots"]["viewers"])
        intake.require(intake.decode((output / "viewers" / path).read_bytes()) == judgment["result"], "judgment detached from retained report")
    current = [evidence.bind(ROOT / s["path"]) for s in viewers["sources"] if (ROOT / s["path"]).exists()]
    stale = sorted({s["path"] for s in viewers["sources"]} - {s["path"] for s in current if s in viewers["sources"]})
    print(json.dumps({"status": "verified-compact-observations", "approval": None, "stale_producers": stale}))


def check(output):
    """Accept a fresh full collection at the recorded roots only if it repeats every digest."""
    record = intake.decode((output / "compact.json").read_bytes())
    verify(output)
    roots = {k: ROOT / v for k, v in record["roots"].items()}
    evidence.verify(ROOT / record["bundle"], roots["reference"], roots["native"], roots["viewers"])
    for kind, directory in roots.items():
        fresh = inventory(directory)
        expected = record["inventory"][kind]
        volatile = {RECEIPTS[kind]}
        differing = sorted(n for n in set(fresh) | set(expected) if n not in volatile and fresh.get(n) != expected.get(n))
        intake.require(not differing, f"fresh {kind} collection differs: {differing[:10]}")
        intake.require(signature(kind, intake.decode((directory / RECEIPTS[kind]).read_bytes())) == record["signatures"][kind], f"fresh {kind} receipt differs")
    print(json.dumps({"status": "fresh-collection-repeats-compact-record", "approval": None}))


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    r = sub.add_parser("retain")
    r.add_argument("bundle", type=Path)
    for kind in ROOTS:
        r.add_argument(kind, type=Path)
    r.add_argument("output", type=Path)
    for name in ("verify", "check"):
        sub.add_parser(name).add_argument("output", type=Path)
    args = parser.parse_args()
    if args.command == "retain":
        retain(args.bundle, {k: getattr(args, k) for k in ROOTS}, args.output)
    else:
        (verify if args.command == "verify" else check)(args.output)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, TypeError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
