# Representative diagnostic retirement

Audited 2026-09-25 against the current working tree. The completed #84 diagnostic
workflows below are retired under the authorized historical-tooling cleanup.
Their source remains available in Git history. Commands naming them in the
README, research notes and proposal narratives are historical reproduction
recipes, not supported current commands.

## Retired

| Files | Completed diagnostic purpose |
| --- | --- |
| `diagnose.py`, `probe.py` | Expanded typography-crop comparison and raw PNG difference localization. |
| `isolate.py`, `test_isolate.py` | Velox paint-isolation derivation, collection and self-verification. The two tests checked preserved geometry/bindings, tamper rejection and refusal to overwrite a completed collection; they retire with that diagnostic workflow. |
| `stroke_probe.py`, `stroke_sources.py` | Fixed isolated-edge pixel sampling and capture of pinned Skia primary-source identities. |
| `qualify_edges.py` | The rejected smooth-rectangle correction's independent browser matrix. |
| `dark_filter_order.py` | Dark-theme filter/compositing-order attribution. |
| `pdf_thin_lines.py`, `pdf_subpixel_attribution.py` | Independent viewer thin-stroke controls and PDF/SVG sketch-fill attribution. |

This removes nine diagnostic scripts and one diagnostic-only test module.
Together they contain 755 lines.
`stroke_probe.py` was the only importer of `probe.py`; no retained Python module
imports these diagnostics. CI's unchanged `test_*.py` discovery previously
included the two isolation tests. Removing that module intentionally retires
those checks rather than weakening a production-rendering assertion.

## Retained live dependencies

`ci/dagger.dang` still builds the renderer and standalone `compare` crate, runs
Python unittest discovery, verifies and captures the Velox intake, verifies that
capture, and runs `case.py verify-registry --renderer ...`.

Its Python import closure is:

```text
case.py -> controls.py, evidence.py, intake.py
controls.py -> evidence.py, intake.py
evidence.py -> intake.py, viewer_assets.py
viewer_assets.py -> intake.py
intake.py -> standard library
```

All five scripts and `test_case.py`, `test_intake.py`, `test_evidence.py` and
`test_viewer_assets.py` are retained. In particular, `controls.py` participates
in case judgments, and viewer-assets tests actually re-derive retained resources.
The standalone `compare` crate supplies both the CI comparison test and Python
evidence/control recomputation. Viewer-fonts, browser adapters, reference
adapters and their source/lock/profile dependencies remain live.

Rust's `xtask/src/representative/capture.rs::verify_capture` also invokes the
retained `intake.py` for legacy v1 captures. The Rust representative retention,
case and viewer-assets tests consume retained proposal/case artifacts, not the
retired diagnostic producers. `png_edge_regression.rs` includes the archived
minimal-rectangle scene directly; that scene and the Rust reproducer remain.

## Source hashes and reference audit

Repository-wide executable-source searches (Rust, Node, Python, shell, CI and
Dagger) found no remaining caller or exact-hash reader requiring any retired
script. Searches included their full paths, basenames, current SHA-256 values
and the diagnostic packet paths.

The retired files' hashes occur in historical successor `freeze.json` maps,
whose collectors have already retired. The additional non-freeze source binding
is `velox84-png-isolation-v1/controls/isolation.json`'s descriptor for `isolate.py`;
its source-verification consumer was `isolate.py` itself. No current Rust or Node
consumer reads that producer descriptor. These records and all diagnostic
artifacts remain historical evidence; their recorded hashes are not rewritten.

Current Rust evidence verification validates producer inventory membership and
descriptor shape, while still comparing reference-profile hashes to actual
bytes (`xtask/src/representative/evidence.rs::verify_sources` and
`validate_reference`). Node's `reference.mjs` hashes its explicit reference
producer/profile list before and after collection. Neither inventory includes
the retired Python diagnostics. Python `evidence.py` likewise has an explicit
producer list containing the retained intake/evidence/viewer-assets adapters
and standalone comparator, not these diagnostics.

## Verification

- The unchanged Python discovery command ran with `REPRESENTATIVE_RENDERER`
  pointing to the existing debug renderer and `REPRESENTATIVE_COMPARE` pointing
  to the existing standalone release comparator. Before retirement: 20 tests,
  17 passed, three skipped. After retirement: 18 tests, 15 passed, the same three
  skipped (release-CLI case test, opt-in browser test and opt-in evidence test).
- `intake.py verify`, fresh Velox `capture` into
  `/tmp/opencode/representative-diagnostic-retirement-capture`, and
  `verify-capture` passed.
- `case.py verify-registry --renderer target/debug/excalidraw-render` passed for
  slides, Velox and varmq, including varmq's two resource outcomes. The initial
  120-second shell limit was insufficient; the complete retry passed with a
  600-second limit.
- The existing Rust `target/debug/xtask` verified all six legacy compact records
  exercised by `representative_retention.rs`: varmq V1/V2/V3, slides V1/V2 and
  Velox V2. This was a command-level check, not a fresh Rust test build.
- Post-retirement executable-source searches found no retired-script references.
  Before/after SHA-256 checks confirmed identical bytes for the five core Python
  scripts, four retained test modules and `ci/dagger.dang`. Scoped
  `git diff --check` passed.

These checks used existing local binaries; the Dagger container/build pipeline
and opt-in browser/full-evidence checks were not run.
