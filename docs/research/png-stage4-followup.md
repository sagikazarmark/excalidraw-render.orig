# Stage 4 follow-up checkpoint — 2026-09-19

Current checkpoint (2026-09-20): [final approved metadata application complete;
both affected parity targets pass](png-stage4-final-metadata-application.md).
Protected PDF and release representative v7 approvals are recorded and valid.
Live locator/owner bindings are current, with 1,803 assigned/excluded and 71 gaps.
Native baseline/#178, global ownership approval/closure and the fixed-size CMS
allocation/resource gap remain open.

Earlier binding status: [reconciled and validated, authorized native canary
qualified](png-stage4-binding-reconciliation.md).

Earlier tooling checkpoint: [release qualification and strict representative
ownership adapter](png-stage4-release-owner-adapter.md). The v6 dev-profile
collection is historical and requires a fresh release-profile successor; final
representative approval remains pending. The global CMS resource gap remains
explicit.

Later bounded integration checkpoint:
[provenance checks, pending inspection, and scoped-exception preparation plan](png-stage4-integration-provenance.md).
The earlier adapter status below describes its own historical checkpoint.

The remaining sections describe the original 2026-09-19 checkpoint; the current
application audit above supersedes its stale-binding and protected-pin status.

Base `629930b`; work remains uncommitted. **Stage 4 is not complete or
approval-ready.** The 59-owner ledger is still a proposal with stale source
bindings. Neither the retired v1 subject nor the diagnostic v4 bundle is a
maintainer approval request.

## Verified progress

### ICC safety

- Common-header rejection and the fallible matrix/TRC parser pass the 25-test
  `png_stage4_contract` suite, including 55 common-header negatives and 64
  independently authored sampled-curve/description equivalences.
- The malformed-ICC owner now invokes the common-header regression; the gray
  owner invokes the sampled-curve equivalences. Their helper is source-bound:
  `png_icc_safety.rs` SHA-256
  `b3c9ee249137314ef30d4c8d45b7b5dd5e381a1f7de3d5e0138440b8a12fdf81`.
- Native CMS refusal checks and both WASM workers pass. The allocation worker
  exercises real per-channel curve allocations as well as parser/transform
  boundaries. The review worker includes the new header/curve cases and the
  previous Adam7, ICC extent, lower-depth tRNS and gray-LUT regressions.
- The projection suite's obsolete CMS-helper digest was corrected after that
  helper passed. The complete native projection proof then passed in 143.75 s.
  Current helper digest:
  `b7cd9b08bffa0ae6694e07f30c0456c77637f7e89adfc15d4db40f82e6f7c780`.

These results do not replace final allocation-path review, release-mode owner
qualification, or the remaining ownership-source reconciliation. In particular,
the common version guard currently accepts major versions 2 through 4; the
qualification vectors exercise accepted versions 2 and 4.

### Source-image projection regressions

The initial broader run exposed old analytical expectations for PNG interpolation
and PDF image matrices. Those were investigated before changing expectations.

`tools/reference-oracle/scripts/stage4-sampling-probe.mjs` independently creates
opaque and partial-alpha PNGs in the pinned browser, samples magnifications and
a fractional crop with both axes flipped, and compares two fresh browser runs.
It does not use native renderer pixels as its oracle.

Observed Chromium 149.0.7827.55 values:

| Input / mapping | Browser values |
| --- | --- |
| Opaque black/white, 3×, first row | `[0, 0, 79, 159, 255, 255]` |
| Opaque black/white, 4×, first row | `[0, 0, 31, 95, 159, 223, 255, 255]` |
| Flipped 20.5-square crop, sample (25,14) | `[239, 15, 255, 255]` |
| Same crop, sample (14,25) | `[0, 0, 255, 255]` |
| Same crop, sample (25,25) | `[15, 239, 0, 255]` |

This falsifies the hypothesis that four-bit interpolation applies only to
transparent images. The old opaque expectations used full-precision weights
and rounding. The crop checks are now exact assertions instead of approximate
channel inequalities.

PDF matrix expectations now account for the image unit square's top row after
the page's top-down transform. For example, an unflipped 40-square image uses
`40 0 0 -40 0 40 cm`; a source flipped in both axes uses
`-40 0 0 40 40 0 cm`. Existing crop/clip rectangles remain independently checked.
The dark-mode stream selector also accounts for `/Interpolate true`.

**All 57 tests selected by `--test fast image_` pass on Rust 1.97.1.**
The previously approved PDF backend artifact digest still differs and was not
refreshed: expected `b053159d…`, observed `4cb446a2…` in the initial broad run.
Its structural/viewer requalification is still required.

## Representative candidate: retained failures

Directory: `tests/conformance/candidates/stage4-frames-text-assets-v4/`.
The six viewer captures were collected twice in fresh processes; each pair's
raster bytes repeat. `viewer-evidence/collection.json` binds inputs, resources,
viewer/comparator profiles and producer files. The text-audit profile binds its
positive custom-font run and the ambient-font/invalid-format negative controls.

`judgments.json` records whole-image and typography/raster/nested-SVG crop
results, exact SSIM bits and changed-mask hashes. No budget or crop was enlarged,
and no tolerance exception was introduced.

| Comparison | Whole changed pixels / 24,000 | Whole SSIM | Typography SSIM | Result |
| --- | ---: | ---: | ---: | --- |
| Official/native Chromium SVG | 0 | 1 | 1 | Pass |
| Official/native pure-Rust SVG | 0 | 1 | 1 | Pass |
| Official Chromium SVG / PDFium | 118 | 0.9898474730435529 | 0.9413564483014669 | Fail |
| Official Chromium SVG / PDF.js | 16 | 0.998520923299333 | 0.9920016961872306 | Fail whole |
| Official canvas / direct PNG | 13 | 0.9986390204274159 | 0.9924904143867819 | Fail |

Both nontext crops are exact for all five comparisons. PDF.js's typography crop
passes that viewer's existing canonical mode; its whole-image SSIM does not.
No diagnosis of a non-defect typography cause has been established that could
justify an approved judgment. The failures must not be hidden by scene fitting.

`semantics.json` independently checks the raster octets, authored RGB/alpha
samples, actual PDF soft-mask/resource references, selectable text, both URI
actions and their rectangles, nested vector use, and embedded scene values.
The PDF graph mutation test rejects six changes: detached mask, RGB corruption,
mask corruption, swapped URI actions, detached image resource, and an extra
image object.

### Approval gate checkpoint

`xtask/src/conformance_candidate_gate.rs` replaces the obsolete v1 approval
path. It checks the evidence set, producer/profile chains, capture bindings,
repetition, fixed crop plan, recomputed comparisons and independent semantics.
Materialization refuses to overwrite an existing evidence bundle. The consumer
uses the repository's approval validator with issue identifier `85`.

The materializer returned:

```text
gates_passed: false
approval_subject_sha256: null
decision_bundle_sha256: 7533cc42b1babc94ead3566f34c38a82430fde2ef88f318e5219ac01cadbdd8c
```

The stored `decision-subject.json` is **diagnostic only**, binding this source
checkpoint. Both `candidate-approval-subject` and approval-required
`candidate-verify` were run and refused it with:

```text
candidate is NOT approval-ready; canonical gates failed: pdf-pdfium, pdf-pdfjs, png-canvas
```

The supplemental `tests/conformance/candidates/registry.json` declares three
execution keys and `verify-all` checks it. **This is still incomplete integration:**
the old fast representative replay uses the retired v1 manifest, and native
all-format replay/report verification must be connected to the approved adapter
before claiming equivalence to the normal corpus path. The original execution
registry has not been edited. The supplemental keys are declarations, not three
new eligible executions.

Five candidate unit tests pass: retired-subject refusal, immutable
materialization, missing/duplicate/stale/detached descriptors, repository path
containment, and the semantic mutation test. There is no fabricated positive
approval fixture in the candidate evidence.

## Reproduction and checks

For the final focused checks:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0
cargo test --locked -p excalidraw-render --test fast image_
cargo test --locked -p excalidraw-render --test fast png_stage4_contract
cargo test --locked -p excalidraw-render --lib cms_failure_contract
cargo test --locked -p xtask conformance_candidate -- --nocapture
cargo clippy --locked -p xtask --all-targets -- -D warnings
```

All pass. The malformed/gray owner invocations were rerun after wiring in their
new regressions. The projection proof also passed after its helper-pin update.
Initial native exploratory runs accidentally used the shell's Rust 1.98 toolchain;
those are not substituted for the final 1.97.1 checks listed above. Clippy on 1.98
also introduces unrelated comparator lints; the 1.97.1 invocation passes.

WASM workers, each passing:

```sh
HEADLESS=1 WASM_BINDGEN_TEST_TIMEOUT=180 \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render \
  --lib wasm_tests::png_stage4_review_regressions_in_wasm -- --exact
HEADLESS=1 WASM_BINDGEN_TEST_TIMEOUT=180 \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
cargo test --locked --target wasm32-unknown-unknown -p excalidraw-render \
  --lib wasm_tests::cms_allocation_failures_in_wasm -- --exact
```

Viewer collection:

```sh
node tools/reference-oracle/scripts/stage4-candidate-views.mjs collect \
  tests/conformance/candidates/stage4-frames-text-assets-v4
```

The sampling probe runs in the same pinned Playwright OCI image as
`run-stage4-representative-v4.sh`, with the repository mounted at `/workspace`,
network disabled, and command
`node /workspace/tools/reference-oracle/scripts/stage4-sampling-probe.mjs`.

The initial entire library run had 403 passes, seven ignored tests and the stale
projection-helper pin failure; that proof subsequently passed. The broad fast
run timed out after 600 seconds and exposed the image assertions addressed
above, the stale v1 candidate manifest and the approved PDF artifact digest.
**No full-workspace green result is claimed.**

All six frozen PNG oracle hashes were rechecked and match the preceding review:
`reference`, `gain-map-reference`, `stage4-reference`,
`stage4-composition-reference`, `stage4-qualified-reference`, and
`stage4-oracle-reference`. `git diff --check` passes.

## Remaining work

1. Diagnose and resolve representative typography fidelity without changing the
   fixed budgets or using easier authoring to hide a mismatch.
2. Complete approval-aware native replay/report integration and mutation coverage
   for the full adapter; independently review its producer-chain completeness.
3. Reconcile Stage 4 helper/owner bindings only after their final required checks.
   `verify-png-stage4.mjs` still contains the obsolete v1 subject and requires an
   explicit update; its old completion output is not valid for this checkpoint.
4. Requalify the PDF backend structural/viewer evidence affected by correct image
   orientation/interpolation, then address the existing inventory-bound licensing
   snapshot and #178 through the established parent review/approval process.
5. Finish renderer Clippy/format checks and the required remaining native/WASM
   checks, followed by the parent's independent full-workspace run. No commit or
   maintainer approval has been made.
