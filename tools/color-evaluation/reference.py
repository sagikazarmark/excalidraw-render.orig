#!/usr/bin/env python3
"""Pinned-browser CSS swatches, independent of the Rust conversion candidates.

This is supplemental browser evidence, not an approved reference-renderer export.
"""
import argparse
import json
from pathlib import Path
import socket
import subprocess
import time
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--browser", type=Path, required=True)
parser.add_argument("--driver", type=Path, required=True)
parser.add_argument("--record", action="store_true")
args = parser.parse_args()
here = Path(__file__).resolve().parent
browser = args.browser.resolve(strict=True)
driver = args.driver.resolve(strict=True)
versions = [subprocess.check_output([str(p), "--version"], text=True).strip() for p in [browser, driver]]
assert all("149.0.7827.55" in v for v in versions), versions
observations = json.loads((here / "expected-native.json").read_text())
inputs = sorted({r["css"] for key in ["css", "artifacts"] for r in observations[key]})
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
base = f"http://127.0.0.1:{port}"


def request(method, path, body=None):
    req = urllib.request.Request(base + path, method=method, data=None if body is None else json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=90) as response:
        result = json.load(response)["value"]
    if isinstance(result, dict) and "error" in result:
        raise RuntimeError(result)
    return result


with subprocess.Popen([str(driver), f"--port={port}"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL) as process:
    try:
        for _ in range(100):
            try:
                request("GET", "/status")
                break
            except OSError:
                time.sleep(0.1)
        session = request("POST", "/session", {"capabilities": {"alwaysMatch": {"browserName": "chrome", "goog:chromeOptions": {"binary": str(browser), "args": ["--headless=new", "--no-sandbox", "--disable-dev-shm-usage", "--force-color-profile=srgb"]}}}})["sessionId"]
        prefix = f"/session/{session}"
        request("POST", prefix + "/timeouts", {"script": 90000})
        rows = request("POST", prefix + "/execute/async", {"args": [inputs], "script": """
const inputs = arguments[0], done = arguments[arguments.length-1];
(async () => {
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = 4;
  const ctx = canvas.getContext('2d', {colorSpace:'srgb', willReadFrequently:true});
  const pixel = () => Array.from(ctx.getImageData(2,2,1,1).data);
  const rows = [];
  for (const css of inputs) {
    const admitted = CSS.supports('color', css);
    if (!admitted) { rows.push({css, admitted, canvas:null, svg:null}); continue; }
    ctx.clearRect(0,0,4,4); ctx.fillStyle = css; ctx.fillRect(0,0,4,4);
    const direct = pixel();
    const root = document.createElementNS('http://www.w3.org/2000/svg','svg');
    root.setAttribute('width','4'); root.setAttribute('height','4');
    const rect = document.createElementNS(root.namespaceURI,'rect');
    rect.setAttribute('width','4'); rect.setAttribute('height','4');
    rect.setAttribute('fill',css); root.appendChild(rect);
    const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(root)], {type:'image/svg+xml'}));
    const img = new Image();
    try {
      await new Promise((resolve,reject) => {img.onload=resolve; img.onerror=reject; img.src=url;});
      ctx.clearRect(0,0,4,4); ctx.drawImage(img,0,0);
      rows.push({css, admitted, canvas:direct, svg:pixel()});
    } finally { URL.revokeObjectURL(url); }
  }
  done({rows});
})().catch(e => done({error:String(e)}));
"""})
        assert "error" not in rows, rows
        report = {"versions": versions, "color_space": "srgb", "rows": rows["rows"]}
        expected = here / "expected-reference.json"
        if args.record:
            expected.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        else:
            assert report == json.loads(expected.read_text()), "pinned browser observations changed"
        request("DELETE", prefix)
        print(f"Pinned Chromium canvas/SVG: {len(inputs)} CSS observations passed")
    finally:
        process.terminate()
        process.wait(timeout=10)
