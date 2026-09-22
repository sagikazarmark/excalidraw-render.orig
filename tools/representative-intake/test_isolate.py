"""The agreed command seam checks diagnostic derivation rather than renderer internals."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "tests/conformance/intake/velox-design-v1/scene.excalidraw"


class IsolationTest(unittest.TestCase):
    def command(self, *args, success=True):
        result = subprocess.run([sys.executable,"-B","tools/representative-intake/isolate.py",*map(str,args)],
                                cwd=ROOT,capture_output=True,text=True,timeout=30)
        self.assertEqual(result.returncode==0,success,result.stderr)

    def test_controls_preserve_original_geometry_and_binding_values(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "target",prefix="isolate-test-") as temporary:
            output = Path(temporary) / "controls"
            self.command("prepare",output)
            self.command("verify-inputs",output)
            source = json.loads(SOURCE.read_bytes())
            geometry = json.loads((output / "geometry/scene.excalidraw").read_bytes())
            self.assertEqual(len(source["elements"]),len(geometry["elements"]))
            for before,after in zip(source["elements"],geometry["elements"]):
                for field in before.keys()-{"strokeColor","backgroundColor"}:
                    self.assertEqual(before[field],after[field],field)
            minimal = json.loads((output / "minimal-rectangle/scene.excalidraw").read_bytes())
            self.assertEqual(len(minimal["elements"]),2)
            self.assertEqual(minimal["elements"][0]["id"],"bounds-anchor")
            self.assertEqual(minimal["elements"][0]["strokeColor"],"transparent")
            self.assertIsNone(minimal["elements"][1]["boundElements"])
            geometry["elements"][0]["x"] += 1
            (output / "geometry/scene.excalidraw").write_text(json.dumps(geometry))
            self.command("verify-inputs",output,success=False)

    def test_completed_collection_is_not_overwritten(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "target",prefix="isolate-test-") as temporary:
            output=Path(temporary) / "controls"
            self.command("prepare",output)
            marker=output / "isolation.json"
            marker.write_text("existing diagnostic record\n")
            self.command("collect",output,"--renderer",ROOT / "target/release/excalidraw-render",success=False)
            self.assertEqual(marker.read_text(),"existing diagnostic record\n")


if __name__=="__main__": unittest.main()
