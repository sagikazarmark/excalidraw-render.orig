# Final #187 plan: color only, exact stable HEAD graph

The user chose **finish color only on the stable graph; defer sampling** after
minimal constant-neutral inputs demonstrated missing-row failures in RC2 despite
the original 76-vector pass. No final source freeze or current-C capture has been
authorized. Do not run the commands below until the parent explicitly freezes.

## Decision and preserved history

- Keep the parent's CMYK/YCCK raw-component integer conversion/truncated K fix.
- Restore Cargo.toml, root Cargo.lock and consumer Cargo.lock exactly to `4e5b624`.
  These bytes were checked: root lock `27717cb869ee8b99a6a401f0c14cb4953c3e19813a69033c9121d8beca513cd2`;
  consumer lock `897026587dec22941b32ecef3d55d51988f1813c006ab7dd355810fc2f39b5c5`.
- No lock proposal is applied. No historical-lock waiver is needed: the live root
  lock once again matches the frozen JPEG reference's source descriptor.
- Existing OLD HEAD 168-key capture remains valid. RC exploratory captures,
  graph/lock proposals, collector-at-capture, prior lock-transition JSON and logs
  remain unchanged. `retired-rc-plan/` archives the superseded pending scripts,
  plan, readiness and tiny-neutral source; `preserved-evidence.json` authenticates
  839 original evidence files. They are historical, not final qualifications.
- The tiny-neutral browser script now corrects `FicY` to the encoder's actual
  `FhcY` bytes before decoding. The original literal is retained visibly and in
  the archive. This edit is not claimed as a new browser run. Parent owns the
  minimized Rust known-failure control; no parent test is edited here.

## Exact cells and counts

The proposed new owners are **CMYK two cells plus YCCK thirteen cells = 15**.
All 17 existing qualified owner selections retain selectors/commands and refresh
only their final test-source hash. V8's representative binding becomes V9 only
after future genuine approval. All 38 criteria, other dispositions, three
exclusions, catalog and 168 full inventory keys remain unchanged.

Exactly six baseline sampling cells stay unowned under #187:

```text
codec:jpeg:color-sampling:ycbcr-4x2-baseline
codec:jpeg:color-sampling:ycbcr-1x4-baseline
codec:jpeg:color-sampling:ycbcr-2x4-baseline
codec:jpeg:color-sampling:ycck-4x2-baseline
codec:jpeg:color-sampling:ycck-1x4-baseline
codec:jpeg:color-sampling:ycck-2x4-baseline
```

The complete corpus is still 76 vectors: **64 canonical-pass vectors and 12
known-failure vectors**, with **24 exact PNG/PDF defect pins**. These pins are
known failures, never exception ceilings or evidence granting ownership.
Parent must confirm exact post-color-only metrics, especially YCCK corrupted
samples whose values can move when color conversion changes. The minimized
constant-neutral control remains an explicit source-bound known failure under
#187. No sampling success is claimed.

Final ledger totals: **1912 assignments / 1832 owners / 3 exclusions / 1835
assigned-excluded / 77 gaps**. Global approval remains null.

## Nested evidence gates

Keep the ten fixed inputs and reuse OLD captures. Final color-only results must:

- Pass the RGB/CMYK/progressive-YCCK and PNG/GIF independent sample/color gates.
- Keep the two nested noninterleaved controls explicitly canonical-failing:
  `jpeg-ycbcr-4x2-baseline-edges` and `jpeg-ycck-2x4-baseline-edges`. Require six
  reviewed exact sample/PDFium/PDF.js metric pins in the parent qualification
  record. YCbCr must retain its OLD metric exactly. Changed bad YCCK metrics must
  be explicitly reviewed; the checker does not presume old or RC metrics.
- Preserve the pre-existing asymmetric-alpha PDFium finding exactly at 24 pixels
  / `3febf9303648d4c8`, with OLD/final raster equality. It grants no exception and
  does not block unrelated color work when unchanged. PDF sample/mask and PDF.js
  gates must still pass.
- Preserve non-JPEG output/report/viewer equality and diagnostics. The filter
  control establishes unchanged bounded fallback behavior, not a blur oracle.

## Ready commands after explicit freeze

The parent first finishes production/test formatting/review and reports the
freeze. Supply the actual message in an external JSON:

```json
{"final_current_capture_authorized":true,"parent_freeze_message":"ACTUAL PARENT FREEZE MESSAGE"}
```

```sh
P=tests/conformance/proposals/jpeg187-requalification-v1
python3 "$P/qualify.py" freeze /tmp/opencode/jpeg187-parent-freeze.json
python3 "$P/qualify.py" stage-final /tmp/opencode/jpeg187-color-final-stage
python3 "$P/collect.py" final /tmp/opencode/jpeg187-color-final-stage "$P/final-freeze.json"
python3 "$P/nested.py" final /tmp/opencode/jpeg187-color-final-stage final-current "$P/final-freeze.json"
python3 "$P/nested.py" qualify final-current
python3 "$P/nested.py" compare final-current
python3 "$P/qualify.py" representative
python3 "$P/qualify.py" protected
python3 "$P/qualify.py" migration
```

All operations pin Rust 1.97.1 and memory-saving profiles. The final collector
requires all three Cargo files to equal archived HEAD bytes; neither RC lock
proposal is accepted. V9/bridge v3 pipeline and protected-PDF equality remain
strict. No new tolerance, golden refresh or approved-evidence rewrite is allowed.

## Parent source-bound qualification record

Before sealing, `final-checks/production-qualification.json` must include:

- Frozen `jpeg_reference_sha256` (`9b5ab60a…` full digest),
  `frozen_input_reconstruction_passed: true`, `reconstructed_vectors: 76`.
- `qualified_vectors: 64`, `known_failure_vectors: 12`, sorted
  `known_sampling_cells` equal to the six full IDs above.
- `sampling_defect_pins`: exactly 24 `{id,format,canonical_pass:false,
  changed_pixels,ssim_bits}` records, twelve source vector IDs × PNG/PDF.
- `nested_sampling_defect_pins`: exactly six analogous records using control `id`
  and `target` (`bound-pdf-samples.rgba`, `pdfium-0.rgba`, `pdfjs-0.rgba`).
- `neutral_failure_control`: `{followup_issue:187,canonical_pass:false,
  test_source:DESCRIPTOR,measurements:DESCRIPTOR}` for the minimized known failure.
- Exact final `test_source`, complete V9 `producer_sources`, passed commands/log
  descriptors in `checks`, and `diagnosed_inventory_changes` covering every actual
  changed key from OLD versus final 168 output comparison.

The checker rejects falsely passing known-failure pins, missing/duplicated cells,
old 76-pass claims, omitted neutral failure evidence and unexplained output drift.
The new packet and V9 subjects remain pending until independent review and actual
user approval. The user strategy decision is not approval of nonexistent subjects.

## Authorization/application boundary

The pending checker never writes the live ledger/registry. Optimized Python is
rejected before dispatch. Applied verification requires persisted actual
production approval/comment records, both exact subjects, author/date/URL/ID
agreement, a fresh matching GitHub response and corresponding V9 approval.
Parent retains atomic transaction ownership and strict-v2 rollback/retry behavior.

Baseline/#178, six sampling gaps/#187, global closure and fixed-size CMS resource
limitations remain explicit. No final capture, broad suite, approval or commit
is performed in this preparation update.
