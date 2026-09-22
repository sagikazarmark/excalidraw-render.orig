"""Viewer-resource derivation contracts at the command boundary."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
VELOX = ROOT / "tests/conformance/proposals/velox84-evidence-v1/viewers"


def run(command, directory):
    return subprocess.run([sys.executable, "-B", "tools/representative-intake/viewer_assets.py", command, str(directory)],
                          cwd=ROOT, capture_output=True, text=True, timeout=900)


class ViewerAssetsTest(unittest.TestCase):
    def test_single_face_derivation_matches_retained_stage4_resources(self):
        for mode in ("light", "dark"):
            result = run("verify", VELOX / mode)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_changed_resources_and_unobserved_substitutes_are_rejected(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="intake-viewer-assets-") as temporary:
            directory = Path(temporary) / "light"
            shutil.copytree(VELOX / "light", directory)
            # Retained paths name the original directory; re-derive them here.
            for name in [p for p in directory.rglob("*") if p.name.endswith(("-assets.json", "viewer-assets-plan.json"))]:
                name.write_text(name.read_text().replace(str((VELOX / "light").relative_to(ROOT)), str(directory.relative_to(ROOT))))
            self.assertEqual(run("verify", directory).returncode, 0)
            (directory / "official-rust.svg").write_text((directory / "official-rust.svg").read_text() + " ")
            self.assertIn("derived viewer resource differs", run("verify", directory).stderr)

            svg = (directory / "official.svg").read_text()
            closing = svg.rindex("</svg>")
            (directory / "official.svg").write_text(svg[:closing] + '<text font-family="Helvetica, Segoe UI Emoji">x</text>' + svg[closing:])
            self.assertIn("official-font-resolution.json", run("verify", directory).stderr)
            forged = {"schema": "excalidraw-render/representative-font-resolution/v1", "approval": None,
                      "input": {"sha256": hashlib.sha256((directory / "official.svg").read_bytes()).hexdigest()},
                      "undeclared": [{"requested_family": "Helvetica", "agrees": False,
                                      "chromium": {"postScriptName": "LiberationSans"}, "fontconfig": {"postscript_name": "FreeSans"}}]}
            (directory / "official-font-resolution.json").write_text(json.dumps(forged))
            self.assertIn("Chromium and fontconfig disagree", run("verify", directory).stderr)


if __name__ == "__main__":
    unittest.main()
