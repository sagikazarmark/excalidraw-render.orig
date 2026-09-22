"""Compact evidence retains records only and rejects any detached byte."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
VARMQ = ROOT / "tests/conformance/proposals/varmq84-evidence-v1"


def verify(directory):
    return subprocess.run([sys.executable, "-B", "tools/representative-intake/compact.py", "verify", str(directory)],
                          cwd=ROOT, capture_output=True, text=True, timeout=120)


class CompactTest(unittest.TestCase):
    def test_retained_records_verify_offline(self):
        result = verify(VARMQ)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIsNone(json.loads(result.stdout)["approval"])

    def test_changed_or_extra_retained_bytes_are_rejected(self):
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=ROOT / "target", prefix="intake-compact-") as temporary:
            for mutate, message in [
                (lambda d: (d / "viewers/light/regions.json").write_text("{}"), "retained viewers/light/regions.json changed"),
                (lambda d: (d / "viewers/light/native.png").write_bytes(b"x"), "viewers retained inventory changed"),
                (lambda d: (d / "viewers/light/svg-chromium-comparison.json").write_text(
                    (d / "viewers/light/svg-chromium-comparison.json").read_text().replace('"passed": true', '"passed": false')), "changed"),
            ]:
                directory = Path(temporary) / "copy"
                shutil.rmtree(directory, ignore_errors=True)
                shutil.copytree(VARMQ, directory)
                mutate(directory)
                result = verify(directory)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)


if __name__ == "__main__":
    unittest.main()
