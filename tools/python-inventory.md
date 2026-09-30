# Remaining first-party Python

Working-tree inventory, **2026-09-25: two Python files**, both owned by the
CI/release maintainer:

| File | Purpose |
| --- | --- |
| `tools/ci/release-results.py` | Experimental-WASM gate over validated snapshot/joined collections; root Dagger calls `snapshots` and `joined`. |
| `tools/ci/test_release_results.py` | Adversarial tests for the release gate's exact allowances, execution coverage, reports and trace drift. |

The count includes existing untracked first-party files, excludes deleted index
entries, dependencies and build/cache trees. Python under the reference oracle's
`node_modules/` is third-party. The archived
`tests/conformance/proposals/jpeg187-requalification-v1/nested/collector-at-capture.py.txt`
is historical source text, not an executable module. No first-party extensionless
Python entry point remains.

## Completed replacements and retirements

- Representative intake, case replay, comparison, viewers, evidence and retention
  run in Rust xtask. See [the workflow](representative-intake/README.md) and
  [Dagger check](representative-intake/ci/README.md).
- PDF fixtures/inspection and PDFium rendering use Rust; WebP input encoding uses
  the native libwebp helper. See [PDF harness](pdf-harness/README.md) and
  [reference oracle](reference-oracle/README.md).
- The last WebP Python audit moved to
  `node tools/pdf-harness/diagnostics/webp191-verify-boundary.mjs`, also run by
  `cargo test --locked -p xtask --test webp_boundary`. It preserves both record
  pins, input/output/profile checks, 144 viewer comparisons, and 672 negative
  buffer/semantic mutations.
- Eight BMP/color/kurbo/JPEG experiment runners retired; the probe crates
  followed on 2026-09-30. Their conclusions remain in `docs/research`, and the
  sources are in Git history.
  JPEG sampling and malformed-entropy behavior remain ordinary renderer tests.
- Completed proposal/approval transactions and frozen Python producers retired;
  retained evidence is checked by current Rust consumers.

See [historical tooling](historical-tooling.md) for earlier audit snapshots.
[CONTRIBUTING.md](../CONTRIBUTING.md) is the current test and expectation-update
guide; historical command transcripts are not maintenance instructions.
