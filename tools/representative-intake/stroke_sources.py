"""Capture immutable primary-source identities for the PNG stroke diagnosis."""
import hashlib
import json
from pathlib import Path
import sys
import urllib.request

REVISION = "53348aa333da02b77c4b5797e2de722f5abde7d0"
FILES = ["src/core/SkAnalyticEdge.h", "src/core/SkAnalyticEdge.cpp", "src/core/SkScan_AntiPath.cpp"]


def main():
    rows = []
    for path in FILES:
        url = f"https://raw.githubusercontent.com/google/skia/{REVISION}/{path}"
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read()
        rows.append({"path":path,"url":url,"bytes":len(data),"sha256":hashlib.sha256(data).hexdigest()})
    record={"schema":"excalidraw-render/velox-stroke-primary-sources/v1","skia_revision":REVISION,
            "chromium_deps":"https://chromium.googlesource.com/chromium/src/+/149.0.7827.55/DEPS",
            "sources":rows,"approval":None}
    with Path(sys.argv[1]).open("x") as output: output.write(json.dumps(record,indent=2)+"\n")


if __name__=="__main__": main()
