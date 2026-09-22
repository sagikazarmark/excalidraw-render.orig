"""Black-box contracts at the agreed intake command seam."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


TOOL = Path(__file__).with_name("intake.py")


class IntakeTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.scene = b'{"type":"excalidraw","version":2,"elements":[],"appState":{},"files":{}}\n'
        (self.source / "scene.excalidraw").write_bytes(self.scene)
        (self.source / "LICENSE").write_text("MIT fixture attribution\n")
        self.provenance = {
            "identity": "fixture",
            "upstream": {"repository": "https://github.com/example/board", "commit": "a" * 40,
                         "path": "scene.excalidraw", "permalink": "https://github.com/example/board/blob/" + "a" * 40 + "/scene.excalidraw"},
            "license": {"spdx": "MIT", "text": "LICENSE"},
            "scene": {"file": "scene.excalidraw", "bytes": len(self.scene),
                      "sha256": hashlib.sha256(self.scene).hexdigest()},
        }
        (self.source / "PROVENANCE.json").write_text(json.dumps(self.provenance))
        self.plan = {
            "schema": "excalidraw-render/representative-intake-plan/v1",
            "case_id": "representative.fixture.v1",
            "source_identity": "fixture",
            "purpose": "Test a complete intake, without granting approval.",
            "sanitization": {"operation": "reviewed-no-change", "review_notes": "Authored empty fixture."},
            "variants": [{"id": "light", "color_mode": "light", "include_background": True,
                          "padding": 10, "scale": 1, "metadata": True}],
            "formats": ["svg", "png", "pdf"],
            "applicability": {"frame_exports": "No frames", "assets": "No images", "sizing": "Scale one"},
            "regions": [{"id": "whole", "class": "mixed-representative", "element_ids": []}],
        }
        self.plan_path = self.root / "plan.json"
        self.plan_path.write_text(json.dumps(self.plan))
        self.bundle = self.root / "bundle"

    def run_tool(self, *arguments, success=True):
        result = subprocess.run([sys.executable, "-B", str(TOOL), *map(str, arguments)],
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        return result

    def prepare(self, success=True):
        return self.run_tool("prepare", self.source, self.plan_path, self.bundle, success=success)

    def test_prepared_intake_preserves_source_and_cannot_claim_approval(self):
        result = json.loads(self.prepare().stdout)
        self.assertEqual(result["status"], "pending-review")
        self.assertEqual((self.bundle / "scene.excalidraw").read_bytes(), self.scene)
        manifest = json.loads((self.bundle / "manifest.json").read_text())
        self.assertIsNone(manifest["approval"])
        self.assertEqual(manifest["eligible_executions"], [])
        self.assertEqual(len(manifest["proposed_executions"]), 3)
        self.run_tool("verify", self.bundle)

    def test_incomplete_or_unknown_plan_fields_are_rejected_before_writing(self):
        for change in ({"formats": ["svg", "png"]}, {"approval": {"approved_by": "invented"}},
                       {"variants": self.plan["variants"] * 2},
                       {"regions": [{"id": "whole", "class": "typography", "element_ids": ["absent"]}]}):
            with self.subTest(change=change):
                self.plan_path.write_text(json.dumps({**self.plan, **change}))
                self.prepare(success=False)
                self.assertFalse(self.bundle.exists())

    def test_scene_tampering_and_forged_approval_are_rejected(self):
        self.prepare()
        manifest_path = self.bundle / "manifest.json"
        original = manifest_path.read_bytes()
        manifest = json.loads(original)
        manifest["approval"] = {"approved_by": "invented"}
        manifest_path.write_text(json.dumps(manifest))
        self.run_tool("verify", self.bundle, success=False)
        manifest_path.write_bytes(original)
        (self.bundle / "scene.excalidraw").write_bytes(self.scene + b" ")
        self.run_tool("verify", self.bundle, success=False)

    def test_bad_provenance_does_not_create_output_or_overwrite_existing_bundle(self):
        self.provenance["scene"]["sha256"] = "0" * 64
        (self.source / "PROVENANCE.json").write_text(json.dumps(self.provenance))
        self.prepare(success=False)
        self.assertFalse(self.bundle.exists())
        self.provenance["scene"]["sha256"] = hashlib.sha256(self.scene).hexdigest()
        (self.source / "PROVENANCE.json").write_text(json.dumps(self.provenance))
        self.prepare()
        before = (self.bundle / "manifest.json").read_bytes()
        self.prepare(success=False)
        self.assertEqual((self.bundle / "manifest.json").read_bytes(), before)

    @unittest.skipUnless(os.environ.get("REPRESENTATIVE_RENDERER"), "set REPRESENTATIVE_RENDERER to exercise actual CLI capture")
    def test_capture_repeats_real_renderer_and_remains_unapproved(self):
        self.prepare()
        output = self.root / "capture"
        self.run_tool("capture", self.bundle, os.environ["REPRESENTATIVE_RENDERER"], output)
        capture = json.loads((output / "capture.json").read_text())
        self.assertEqual(capture["status"], "observed-unapproved")
        self.assertEqual(capture["eligible_executions"], [])
        self.assertEqual([r["format"] for r in capture["rows"]], ["svg", "png", "pdf"])
        self.assertTrue(all(r["fresh_process_runs"] == 2 for r in capture["rows"]))
        self.run_tool("verify-capture", self.bundle, output)
        capture_path = output / "capture.json"
        original = capture_path.read_bytes()
        del capture["renderer_binary"]
        capture_path.write_text(json.dumps(capture))
        self.run_tool("verify-capture", self.bundle, output, success=False)
        capture_path.write_bytes(original)
        report = output / "light-png-report.json"
        report_original = report.read_bytes()
        for mutation in ({"schema_version": True}, {"format": "svg"}, {"diagnostics": [{
                "code": "color.invalid", "severity": "warning", "format": "png",
                "effects": ["visual", "visual"], "element_id": None, "json_pointer": "",
                "message": "observed", "details": None}]}):
            with self.subTest(mutation=mutation):
                changed = json.dumps({**json.loads(report_original), **mutation}).encode()
                report.write_bytes(changed)
                resealed = json.loads(original)
                resealed["files"][report.name] = {"bytes": len(changed), "sha256": hashlib.sha256(changed).hexdigest()}
                capture_path.write_text(json.dumps(resealed))
                self.run_tool("verify-capture", self.bundle, output, success=False)
        capture_path.write_bytes(original)
        report.write_bytes(report_original)
        report.write_bytes(report.read_bytes() + b" ")
        self.run_tool("verify-capture", self.bundle, output, success=False)

    @unittest.skipUnless(os.environ.get("REPRESENTATIVE_RENDERER"), "real renderer required")
    def test_process_order_drift_failure_and_timeout_at_command_boundary(self):
        self.prepare()
        log = self.root / "invocations.jsonl"
        wrapper = self.root / "renderer"
        # A process-boundary observer delegates every render to the real CLI.
        # Faults happen only after real rendering; no renderer output is mocked.
        script = f'''#!{sys.executable}
import json, pathlib, subprocess, sys, time
args = sys.argv[1:]
log = pathlib.Path({str(log)!r})
prior = log.read_text().splitlines() if log.exists() else []
with log.open("a") as stream: stream.write(json.dumps(args) + "\\n")
result = subprocess.run([{os.environ["REPRESENTATIVE_RENDERER"]!r}, *args])
if result.returncode: sys.exit(result.returncode)
FAULT
'''
        faults = {
            "observe": "pass",
            "drift": 'if len(prior) >= 3:\n    with open(args[args.index("--report-output") + 1], "ab") as report: report.write(b" ")',
            "failure": "sys.exit(1)",
            "timeout": "time.sleep(5)",
        }
        for mode, fault in faults.items():
            with self.subTest(mode=mode):
                log.unlink(missing_ok=True)
                wrapper.write_text(script.replace("FAULT", fault))
                wrapper.chmod(0o755)
                output = self.root / mode
                arguments = ["capture", self.bundle, wrapper, output]
                if mode == "timeout":
                    arguments += ["--timeout", "0.5"]
                self.run_tool(*arguments, success=mode == "observe")
                if mode == "observe":
                    calls = [json.loads(line) for line in log.read_text().splitlines()]
                    self.assertEqual([args[args.index("--format") + 1] for args in calls],
                                     ["svg", "png", "pdf", "pdf", "png", "svg"])
                else:
                    self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
