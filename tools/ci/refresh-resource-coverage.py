"""Refresh derived consumer bindings after reviewed authority migration."""
import hashlib
import json
import subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def digest(path): return hashlib.sha256((ROOT/path).read_bytes()).hexdigest()
def read(path): return json.loads((ROOT/path).read_text())
def write(path,value): (ROOT/path).write_text(json.dumps(value,ensure_ascii=False,indent=2)+"\n")
requirements=read("tests/conformance/coverage/requirements.json")
for row in requirements["requirements"]:
    for evidence in row["evidence"]: evidence["source"]["sha256"]=digest(evidence["source"]["path"])
write("tests/conformance/coverage/requirements.json",requirements)
inventory=subprocess.check_output([str(ROOT/"target/debug/xtask"),"conformance","inventory"],cwd=ROOT)
(ROOT/"tests/conformance/coverage/inventory.json").write_bytes(inventory)
seed=json.loads(subprocess.check_output([str(ROOT/"target/debug/xtask"),"conformance","seed-ownership"],cwd=ROOT))
ledger=read("tests/conformance/coverage/ownership.json")
ledger["catalog_sha256"]=seed["catalog_sha256"]
for row in ledger["assignments"]:
    owner=row["disposition"].get("owner",{})
    for field in ["source","manifest"]:
        if field in owner: owner[field]["sha256"]=digest(owner[field]["path"])
write("tests/conformance/coverage/ownership.json",ledger)
