"""Representative case contracts at the command boundary."""
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SLIDES = ROOT / "tests/conformance/cases/representative/excali-local-slides"


def run(*arguments):
    return subprocess.run([sys.executable, "-B", "tools/representative-intake/case.py", *map(str, arguments)],
                          cwd=ROOT, capture_output=True, text=True, timeout=300)


def region(changed, ssim):
    bits = struct.pack(">d", ssim).hex()
    return {"changed_pixels": changed, "changed_pixels_pass": changed == 0, "ssim_bits": bits, "ssim_pass": ssim >= 0.999}


@unittest.skipUnless(SLIDES.exists(), "requires the registered slides case")
class CaseTest(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="case-test-")
        self.addCleanup(self.temporary.cleanup)
        self.case = json.loads((SLIDES / "case.json").read_text())

    def collection(self, pdfium_avatar):
        judgments = []
        for variant in self.case["variants"]:
            for comparison in self.case["comparisons"]:
                avatar = pdfium_avatar if comparison == "pdf-pdfium" else region(0, 1.0)
                judgments.append({"variant": variant, "comparison": comparison, "result": {
                    "whole": region(0, 1.0), "crops": [{"id": "slide1-avatar", "region": avatar}]}})
        directory = Path(self.temporary.name) / "collection"
        directory.mkdir(exist_ok=True)
        (directory / "viewers.json").write_text(json.dumps({"judgments": judgments}))
        return directory

    def test_offline_verification_binds_every_retained_byte_and_the_approval_subject(self):
        result = run("verify", SLIDES)
        self.assertEqual(result.returncode, 0, result.stderr)
        copy = Path(self.temporary.name) / "case"
        shutil.copytree(SLIDES, copy)
        (copy / "expected-report-svg.json").write_text('{"schema_version":1,"diagnostics":[{}]}')
        self.assertIn("retained case files changed", run("verify", copy).stderr)
        shutil.rmtree(copy)
        shutil.copytree(SLIDES, copy)
        case = json.loads((copy / "case.json").read_text())
        case["approval"] = {"approved_by": "someone", "approved_at": "2026-09-23",
                            "approval_url": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1",
                            "approval_subject_sha256": "sha256:" + "0" * 64}
        (copy / "case.json").write_text(json.dumps(case))
        self.assertIn("approval does not bind the exact case subject", run("verify", copy).stderr)

    def test_failing_regions_need_a_matching_exact_ceiling_exception(self):
        ceiling = {e["variant"]: struct.unpack(">d", bytes.fromhex(e["minimum_ssim_bits"]))[0] for e in self.case["exceptions"]}
        within = min(ceiling.values())
        # Each variant's avatar sits exactly on its own ceiling.
        judgments = self.collection(region(0, 0.0))
        record = json.loads((judgments / "viewers.json").read_text())
        for judgment in record["judgments"]:
            if judgment["comparison"] == "pdf-pdfium":
                judgment["result"]["crops"][0]["region"] = region(0, ceiling[judgment["variant"]])
        (judgments / "viewers.json").write_text(json.dumps(record))
        result = run("judge", SLIDES, judgments)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["exceptions_applied"], 2)
        self.assertIn("comparisons fail", run("judge", SLIDES, self.collection(region(0, within - 1e-6))).stderr)
        self.assertIn("comparisons fail", run("judge", SLIDES, self.collection(region(1, 1.0))).stderr)
        self.assertIn("dead exceptions", run("judge", SLIDES, self.collection(region(0, 1.0))).stderr)


if __name__ == "__main__":
    unittest.main()
