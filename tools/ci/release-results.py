#!/usr/bin/env python3
"""0.1.0 experimental-WASM gate over freshly validated xtask collections.

The caller must first run report-wasm-snapshots or join-trace-results. This
module never generates expectations or converts a failing run into evidence.
Strict equality remains available through the original xtask verifiers.
"""

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NATIVE = "x86_64-unknown-linux-gnu"
WASM = "wasm32-unknown-unknown"


def load(path):
    return json.loads(Path(path).read_text())


def key(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def validate(runs, executions, pins, traces):
    """Check every key/repeat/order; allow only exact recorded cross-target pairs."""
    expected = [key(e["key"]) for e in executions]
    require(expected and len(set(expected)) == len(expected), "invalid inventory keys")
    targets = {NATIVE, WASM}
    orders = {"forward", "reverse"}
    seen = set()
    processes = set()
    per_target = {}
    for run in runs:
        pair = (run["target"], run["order"])
        require(pair[0] in targets and pair[1] in orders and pair not in seen,
                "unexpected or duplicate target/order")
        seen.add(pair)
        process = run["process_id"]
        require(isinstance(process, str) and process and process not in processes,
                "missing or reused process identity")
        processes.add(process)
        wanted = expected if pair[1] == "forward" else expected[::-1]
        require([key(r["key"]) for r in run["results"]] == wanted,
                "missing, extra, stale, duplicate, or reordered keys")
        for result in run["results"]:
            samples = result["samples"]
            require(len(samples) == 2 and samples[0] == samples[1],
                    f"within-session drift: {result['key']}")
            sample = samples[0]
            fields = {"outcome", "report", "trace"} if traces else {"outcome", "report"}
            require(set(sample) == fields, "unexpected sample fields")
            records = [sample["report"]]
            if traces:
                records.append(sample["trace"])
            outcome = sample["outcome"]
            if outcome["kind"] == "artifact":
                require(set(outcome) == {"kind", "bytes"}, "invalid artifact outcome")
                records.append(outcome["bytes"])
            else:
                require(outcome["kind"] == "failure" and outcome.get("code"),
                        "invalid failure outcome")
                # Snapshot failures include an exact error-descriptor blob.
                if "descriptor" in outcome:
                    records.append(outcome["descriptor"])
            for record in records:
                digest = record["sha256"]
                require(set(record) == {"sha256", "byte_length"}
                        and isinstance(digest, str) and len(digest) == 64
                        and all(c in "0123456789abcdef" for c in digest)
                        and type(record["byte_length"]) is int and record["byte_length"] > 0,
                        "invalid byte descriptor")
            identity = (pair[0], key(result["key"]))
            previous = per_target.setdefault(identity, sample)
            require(previous == sample, f"within-target process-order drift: {result['key']}")
    require(seen == {(t, o) for t in targets for o in orders}, "missing target/order")
    require(pins["schema"] == "excalidraw-render/experimental-wasm-differences/v1"
            and pins["issue"] == 178, "invalid difference record")
    allowed = {p["case"]: p for p in pins["cases"]}
    require(len(allowed) == len(pins["cases"]), "duplicate difference record")
    used = set()
    for execution in executions:
        k = execution["key"]
        a, b = (per_target[(t, key(k))] for t in (NATIVE, WASM))
        require(a["report"] == b["report"], f"cross-target report drift: {k}")
        pin = allowed.get(k["case"])
        if pin is None or k["variant"] != "default" or k["format"] != "png" or k["export"] != {"kind": "scene"}:
            require(a == b, f"unexpected cross-target difference: {k}")
            continue
        require(a["outcome"] == {"kind": "artifact", "bytes": pin["native"]}
                and b["outcome"] == {"kind": "artifact", "bytes": pin["wasm"]}
                and a["outcome"] != b["outcome"],
                f"#178 measurement changed or became stale: {k}")
        if traces:
            if pin.get("traces") is None:
                require(a["trace"] == b["trace"], f"unrecorded trace difference: {k}")
            else:
                require(a["trace"] == pin["traces"]["native"]
                        and b["trace"] == pin["traces"]["wasm"],
                        f"#178 trace measurement changed: {k}")
        used.add(k["case"])
    require(used == set(allowed), "unused/stale #178 difference records")
    return len(used)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=["snapshots", "joined"])
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    inventory = load(ROOT / "tests/conformance/coverage/inventory.json")
    pins = load(ROOT / "tests/package-consumer/experimental-wasm.json")
    if args.kind == "snapshots":
        report = load(args.directory / "replay.json")
        baseline = load(ROOT / "tests/conformance/coverage/native-baseline/revision-1.json")
        runs = [dict(r, target=WASM) for r in report["runs"]]
        # Native baseline is already verified by report-wasm-snapshots. Both
        # synthetic views below refer to expectations, not new native execution.
        results = [{"key": e["key"], "samples": [e["sample"], e["sample"]]}
                   for e in baseline["expectations"]]
        for order in ("forward", "reverse"):
            runs.append({"target": NATIVE, "order": order,
                         "process_id": f"baseline-{order}",
                         "results": results if order == "forward" else results[::-1]})
    else:
        comparison = load(args.directory / "comparison.json")
        require(not comparison["public_trace_disagreements"],
                "packaged/test-build disagreement")
        result = load(args.directory / "results.json")
        require(result["schema"] == "excalidraw-render/trace-collection/v1"
                and result["scope"] == "joined", "expected freshly joined collection")
        runs = result["runs"]
    count = validate(runs, inventory["executions"], pins, args.kind == "joined")
    print(f"0.1.0 experimental-WASM gate passed; {count} exact #178 differences remain. "
          "This is not strict cross-target equality.")


if __name__ == "__main__":
    main()
