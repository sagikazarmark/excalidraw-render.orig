"""Evidence command contracts; observations never turn into approvals."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import shutil
import sys

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(os.environ.get("REPRESENTATIVE_EVIDENCE_TESTS"), "requires collected proposal and Rust comparator")
class EvidenceIntegrityTest(unittest.TestCase):
    def test_omitted_sources_and_forged_receipts_are_rejected(self):
        proposal = ROOT / "tests/conformance/proposals/velox84-evidence-v1"
        source = proposal / "viewers"
        with tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="intake-integrity-") as temporary:
            output = Path(temporary) / "viewers"
            shutil.copytree(source, output)
            original = (output / "viewers.json").read_text().replace(str(source.relative_to(ROOT)), str(output.relative_to(ROOT)))
            for mutation, message in [("sources", "incomplete producer inventory"), ("receipt", "raw_capture"), ("report", "comparison report detached")]:
                record = json.loads(original)
                if mutation == "sources": record["sources"] = []
                elif mutation == "receipt": record["captures"][0]["runs"] = [{}, {}]
                else: record["judgments"][0]["report"]["sha256"] = "0" * 64
                (output / "viewers.json").write_text(json.dumps(record))
                result = subprocess.run([sys.executable,"-B","tools/representative-intake/evidence.py",
                    "tests/conformance/intake/velox-design-v1",str(proposal / "reference"),str(proposal / "native"),str(output),"--verify"],
                    cwd=ROOT,capture_output=True,text=True,timeout=60)
                self.assertNotEqual(result.returncode,0)
                self.assertIn(message,result.stderr)


@unittest.skipUnless(os.environ.get("REPRESENTATIVE_COMPARE"), "build the intake comparator")
class ComparisonTest(unittest.TestCase):
    def test_exact_and_changed_rasters_use_real_canonical_budgets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "a.rgba").write_bytes(bytes([255, 255, 255, 255]) * 1024)
            (root / "b.rgba").write_bytes(bytes([0, 0, 0, 255]) * 1024)
            (root / "regions.json").write_text(json.dumps({"width":32,"height":32,
                "whole_class":"mixed-representative","crops":[{"id":"text","class":"typography","x":0,"y":0,"width":16,"height":16}]}))
            for candidate, expected in (("a.rgba", True), ("b.rgba", False)):
                result = subprocess.run([os.environ["REPRESENTATIVE_COMPARE"], str(root / "a.rgba"),
                    str(root / candidate), "canonical", str(root / "regions.json"), str(root / "result.json")],
                    capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr)
                report = json.loads((root / "result.json").read_text())
                self.assertEqual(report["passed"], expected)
                self.assertEqual(report["whole"]["changed_pixels"], 0 if expected else 1024)
                self.assertEqual(report["crops"][0]["region"]["changed_pixels"], 0 if expected else 256)


@unittest.skipUnless(os.environ.get("REPRESENTATIVE_BROWSER_TESTS"), "requires pinned Docker browser")
class ViewerTest(unittest.TestCase):
    def test_fractional_clip_preserves_scale_and_rejects_undeclared_entities(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="intake-viewer-test-") as temporary:
            directory = Path(temporary)
            for name, body, success in [
                ("plain", '<rect width="40" height="100" fill="red"/>', True),
                ("newline", '<desc>source&#10;text</desc><rect width="40" height="100" fill="red"/>', True),
                ("entity", '<desc>&#97;</desc>', False),
                ("external", '<image href="https://external.invalid/image.png" width="2" height="2"/>', False),
            ]:
                source, output = directory / f"{name}.svg", directory / f"{name}.rgba"
                source.write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="100.75" height="100.25">{body}</svg>')
                command = ["bash", "tools/representative-intake/svg-viewer.sh", "render",
                           str(source.relative_to(ROOT)), str(output.relative_to(ROOT))]
                result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=60)
                self.assertEqual(result.returncode == 0, success, result.stderr[-2000:])
                if success:
                    summary = json.loads(result.stdout)
                    self.assertEqual((summary["width"], summary["height"]), (100, 100))
                    pixels = output.read_bytes()
                    self.assertEqual(pixels[(50*100+38)*4:(50*100+39)*4], bytes([255,0,0,255]))
                    self.assertEqual(pixels[(50*100+42)*4:(50*100+43)*4], bytes([0,0,0,0]))


if __name__ == "__main__":
    unittest.main()
