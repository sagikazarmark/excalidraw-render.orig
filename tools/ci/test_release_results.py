"""Adversarial checks for the experimental target allowance, not renderer tests."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("release_results", Path(__file__).with_name("release-results.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class ReleaseResults(unittest.TestCase):
    def setUp(self):
        self.key = {"case": "fixture", "variant": "default", "format": "png",
                    "export": {"kind": "scene"}, "inputs_sha256": "d" * 64}
        native = {"sha256": "a" * 64, "byte_length": 3}
        wasm = {"sha256": "b" * 64, "byte_length": 4}
        report = {"sha256": "c" * 64, "byte_length": 2}
        self.pins = {"schema": "excalidraw-render/experimental-wasm-differences/v1",
                     "issue": 178, "cases": [{"case": "fixture", "native": native, "wasm": wasm}]}
        self.executions = [{"key": self.key}]
        self.runs = []
        for target, output in ((gate.NATIVE, native), (gate.WASM, wasm)):
            for order in ("forward", "reverse"):
                sample = {"outcome": {"kind": "artifact", "bytes": output}, "report": report}
                self.runs.append(copy.deepcopy({"target": target, "order": order,
                    "process_id": f"{target}-{order}", "results": [{"key": self.key, "samples": [sample, sample]}]}))

    def check(self):
        return gate.validate(self.runs, self.executions, self.pins, False)

    def test_exact_known_pair_passes(self):
        self.assertEqual(self.check(), 1)

    def test_missing_target_fails(self):
        self.runs.pop()
        with self.assertRaises(ValueError):
            self.check()

    def test_changed_known_output_fails(self):
        for sample in self.runs[-1]["results"][0]["samples"]:
            sample["outcome"]["bytes"]["sha256"] = "e" * 64
        with self.assertRaises(ValueError):
            self.check()

    def test_report_change_is_not_allowed(self):
        for run in self.runs[2:]:
            for sample in run["results"][0]["samples"]:
                sample["report"]["sha256"] = "e" * 64
        with self.assertRaises(ValueError):
            self.check()

    def test_unlisted_variant_is_not_allowed(self):
        self.key["variant"] = "dark"
        for run in self.runs:
            run["results"][0]["key"]["variant"] = "dark"
        with self.assertRaises(ValueError):
            self.check()

    def test_fixed_difference_is_stale(self):
        for run in self.runs[2:]:
            for sample in run["results"][0]["samples"]:
                sample["outcome"]["bytes"] = self.pins["cases"][0]["native"]
        with self.assertRaises(ValueError):
            self.check()

    def test_unknown_trace_difference_fails(self):
        for run in self.runs:
            for sample in run["results"][0]["samples"]:
                sample["trace"] = {"sha256": ("e" if run["target"] == gate.NATIVE else "f") * 64, "byte_length": 20}
        with self.assertRaises(ValueError):
            gate.validate(self.runs, self.executions, self.pins, True)

    def test_exact_trace_pair_does_not_allow_further_trace_drift(self):
        trace_pair = {"native": {"sha256": "e" * 64, "byte_length": 20},
                      "wasm": {"sha256": "f" * 64, "byte_length": 21}}
        self.pins["cases"][0]["traces"] = trace_pair
        for run in self.runs:
            target = "native" if run["target"] == gate.NATIVE else "wasm"
            for sample in run["results"][0]["samples"]:
                sample["trace"] = dict(trace_pair[target])
        self.assertEqual(gate.validate(self.runs, self.executions, self.pins, True), 1)
        for run in self.runs[2:]:
            for sample in run["results"][0]["samples"]:
                sample["trace"] = {"sha256": "a" * 64, "byte_length": 21}
        with self.assertRaises(ValueError):
            gate.validate(self.runs, self.executions, self.pins, True)


if __name__ == "__main__":
    unittest.main()
