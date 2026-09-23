"""Representative case contracts at the command boundary."""
import json
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib

ROOT = Path(__file__).resolve().parents[2]
SLIDES = ROOT / "tests/conformance/cases/representative/excali-local-slides"
SIZE = 32
RASTERS = ("official-chromium", "official-rust", "official-png", "native-chromium", "native-rust", "native-pdfium", "native-pdfjs")


def run(*arguments):
    return subprocess.run([sys.executable, "-B", "tools/representative-intake/case.py", *map(str, arguments)],
                          cwd=ROOT, capture_output=True, text=True, timeout=600)


def region(changed, ssim):
    bits = struct.pack(">d", ssim).hex()
    return {"changed_pixels": changed, "changed_pixels_pass": changed == 0, "ssim_bits": bits, "ssim_pass": ssim >= 0.999}


def image(noisy):
    """A black square on white; `noisy` darkens every fourth pixel of the crop."""
    pixels = bytearray()
    for y in range(SIZE):
        for x in range(SIZE):
            value = 0 if 8 <= x < 24 and 8 <= y < 24 else 255
            if noisy and x < 16 and y < 16 and (x + y) % 4 == 0:
                value = 120
            pixels += bytes((value, value, value, 255))
    return zlib.compress(bytes(pixels))


@unittest.skipUnless(SLIDES.exists(), "requires the registered slides case")
class CaseTest(unittest.TestCase):
    def setUp(self):
        (ROOT / "target").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="case-test-")
        self.addCleanup(self.temporary.cleanup)
        self.case = json.loads((SLIDES / "case.json").read_text())

    def case_copy(self, exceptions):
        copy = Path(self.temporary.name) / "case"
        shutil.rmtree(copy, ignore_errors=True)
        shutil.copytree(SLIDES, copy)
        case = json.loads((copy / "case.json").read_text())
        case["exceptions"], case["approval"] = exceptions, None
        (copy / "case.json").write_text(json.dumps(case))
        return copy

    def collection(self, pdfium_avatar, noisy_reference):
        """Synthetic judgments over real rasters: the official Rust raster is
        noisy in the avatar crop when `noisy_reference`, widening the controls."""
        directory = Path(self.temporary.name) / "collection"
        shutil.rmtree(directory, ignore_errors=True)
        judgments = []
        for variant in self.case["variants"]:
            (directory / variant).mkdir(parents=True)
            regions = {"width": SIZE, "height": SIZE, "whole_class": "mixed-representative",
                       "crops": [{"id": "slide1-avatar", "class": "geometry/image", "x": 0, "y": 0, "width": 16, "height": 16}]}
            (directory / variant / "regions.json").write_text(json.dumps(regions))
            for name in RASTERS:
                (directory / variant / f"{name}.rgba.zlib").write_bytes(image(noisy_reference and name == "official-rust"))
            for comparison in self.case["comparisons"]:
                avatar = pdfium_avatar if comparison == "pdf-pdfium" else region(0, 1.0)
                judgments.append({"variant": variant, "comparison": comparison, "result": {
                    "whole": region(0, 1.0), "crops": [{"id": "slide1-avatar", "region": avatar}]}})
        (directory / "viewers.json").write_text(json.dumps({"judgments": judgments}))
        return directory

    def exception(self, variant, changed, ssim):
        return {"variant": variant, "comparison_id": "pdf-pdfium", "region": "slide1-avatar", "max_changed_pixels": changed,
                "minimum_ssim_bits": struct.pack(">d", ssim).hex(), "reason": "test", "approving_issue": "https://example.invalid",
                "viewer_profile": "canonical-pdf-pdfium-v1"}

    def test_offline_verification_binds_every_retained_byte_and_the_approval_subject(self):
        result = run("verify", SLIDES)
        self.assertEqual(result.returncode, 0, result.stderr)
        copy = self.case_copy(self.case["exceptions"])
        (copy / "expected-report-svg.json").write_text('{"schema_version":1,"diagnostics":[{}]}')
        self.assertIn("retained case files changed", run("verify", copy).stderr)
        copy = self.case_copy(self.case["exceptions"])
        case = json.loads((copy / "case.json").read_text())
        case["approval"] = {"approved_by": "someone", "approved_at": "2026-09-23",
                            "approval_url": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1",
                            "approval_subject_sha256": "sha256:" + "0" * 64}
        (copy / "case.json").write_text(json.dumps(case))
        self.assertIn("approval does not bind the exact case subject", run("verify", copy).stderr)

    def test_a_failing_region_within_its_identical_input_controls_passes_without_an_exception(self):
        judged = run("judge", self.case_copy([]), self.collection(region(3, 0.99), noisy_reference=True))
        self.assertEqual(judged.returncode, 0, judged.stderr)
        self.assertEqual(json.loads(judged.stdout)["within_controls"], 2)
        # An exception the controls already cover is dead.
        covered = [self.exception(v, 3, 0.99) for v in self.case["variants"]]
        self.assertIn("dead exceptions", run("judge", self.case_copy(covered), self.collection(region(3, 0.99), True)).stderr)

    def test_beyond_its_controls_a_region_needs_an_exact_ceiling_exception(self):
        exact = [self.exception(v, 1, 0.99) for v in self.case["variants"]]
        judged = run("judge", self.case_copy(exact), self.collection(region(1, 0.99), noisy_reference=False))
        self.assertEqual(judged.returncode, 0, judged.stderr)
        self.assertEqual(json.loads(judged.stdout)["exceptions_applied"], 2)
        for over in (region(2, 0.99), region(1, 0.989)):
            self.assertIn("comparisons fail", run("judge", self.case_copy(exact), self.collection(over, False)).stderr)
        self.assertIn("comparisons fail", run("judge", self.case_copy([]), self.collection(region(1, 0.99), False)).stderr)


if __name__ == "__main__":
    unittest.main()
