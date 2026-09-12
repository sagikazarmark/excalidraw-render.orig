# Full-inventory test-only trace collection

Implemented after `dde716c`, independently of upstream-blocked #178.

## Commands

```sh
# Trace-only measurements and diagnostic comparison, including failures:
dagger api call traces --output target/trace-results
# Packaged public artifact/report measurements joined to test-only traces:
dagger api call full-results --output target/full-results
# Required fail-closed gate (does not waive #178):
dagger check excalidraw-render:full-equality
```

On small engines the components can be collected sequentially:

```sh
dagger api call traces --output target/trace-results
dagger api call packaging snapshots --output target/trace-results/native-snapshots
dagger api call wasm browser-snapshot-logs --output target/trace-results/wasm-snapshots
cargo run --locked -p xtask -- conformance join-trace-results target/trace-results
cargo run --locked -p xtask -- conformance verify-joined-results target/trace-results
```

`join-trace-results` materializes diagnostics even when equality fails. Only
`verify-joined-results` gates completion: it rereads raw logs, validates inputs,
joins packaged results, and rejects either trace/public disagreement or cross-target
artifact/report/trace differences. The exported `results.json` uses
`excalidraw-render/trace-collection/v1`, deliberately not the standalone draft
`coverage-results/v1`; the generic `verify-results` cannot bypass join validation.

## Test seam

The trace module is generated inside the existing `cfg(test)`-only
`render::test_support` module in Dagger's source snapshot. No public trace API,
production feature, or checked-in modification to the session is introduced.
The generated test reuses the approved native collector's request mapper and
calls `render_with_trace` / `render_frame_with_trace` on the same 168 inventory
keys. Each key runs twice in separate native processes / browser worker
invocations for forward and reverse order. Trace output includes digest and
length plus the corresponding test-build artifact/report measurement.

Each completion line binds target, order and input digest. Input binds the
inventory, scenes, renderer source bytes, collector/template/stager/runner,
tool and consumer identities, Dagger lock and Git commit. Invalid descriptors,
missing/duplicate/reordered keys or completion, and wrong target/input fail
before comparison. The compiler/test commands are release-mode and locked.
These are runner-produced measurements, not independently authenticated logs:
normal evidence provenance is the Dagger invocation with its pinned inputs.

The public join validates the native candidate and browser input/commit,
requires exact keys/order, and records any test-build/public differences before
attaching traces to the actual packaged outcomes. Package archives and browser
package/tool logs remain in the exported component directories. Trace collection
does not make a synthetic trace value stand in for missing data.

## Initial execution findings

All 168 keys executed twice in both orders on both supported targets: **1,344
traced renders**. All four public collectors' outcome/report measurements match
their test-build counterparts: **zero join disagreements**. The combined result
still fails, honestly:

The final end-to-end `full-results` working-tree run completed in 24m39s,
exporting approximately 24 MB under `target/full-results`. A temporary
`cargo package --allow-dirty` override was needed for the uncommitted relocation
of the earlier ICO regression test module (Clippy requires tests after other
items); that override was removed afterwards. This is not a clean-commit release
attestation. `verify-joined-results target/full-results` was rerun on the exported
data and rejected the combined differences as intended. Lint and all workspace
test tiers/doctests pass.

- Four public PNG byte discrepancies remain the upstream-blocked #178 scope.
- **30 unique trace keys differ** across targets (120 individual sample/order
  differences from the first native sample). No root cause is asserted yet.
- Trace differences cover bound-text-containers, font-families-virgil-lilita-comic,
  frame-clip-mask-sharing, image-codecs, text-alignment-vertical, frame-export
  ordinals 2/3/4/7 in all formats, plus PNG for freedraw-loop-fills,
  legacy-stroke-sharpness and rough-fillers-dashed-zigzag-line.
- The equal-length rotated-shape-bounds PNG difference from #178 does not change
  its trace digest. Some other trace differences may follow output lengths, but
  that is not a diagnosis of the remaining 27 keys.

`comparison.json` keeps separate `trace_differences`, `artifact_report_differences`,
and `public_trace_disagreements` arrays. The first two compare against the first
native sample and may contain repeated records for sessions/orders; count unique
keys when assessing affected coverage. Collecting a report successfully is not a
passing equality verdict. Global ownership, representative corpus and fuzz closure
remain pending under #85/#86.

## Release-order diagnosis and fix

The first divergent events for the 27 non-PNG-length keys are releases during
`format_planning`, not different acquisitions or charges. Examples:

- Bound text: native releases `resolved_text_layout_and_glyph_geometry` where
  WASM releases the same owner's `relationship_edge` first.
- Image codecs: native releases `/elements/0/fileId`'s relationship edge where
  WASM releases `/elements/0`'s `resolved_image` first.
- Frame 2: the two targets release `/elements/16/link` and
  `/elements/16/frameId` in opposite order.

Both `resolution.rs` and `accounting/fallbacks.rs` sorted the semantic working
records using `sort_unstable_by_key(source_order)`. Several records legitimately
share a source owner. An unstable sort makes no promise about these ties, and
the native and WASM layouts/implementations selected different orders.

`SemanticWorkingRecord::release_order` now defines a total semantic order:
source order, pointer, event, element ID, local owner, then byte count. Both
release-record sorts use it. All trace-visible fields participate, so any remaining
ties describe indistinguishable releases. The sort remains allocation-free;
no stable-sort scratch allocation or new resource charge is needed. Releasing
the same bytes in a deterministic order preserves the endpoint, peak, and work
charges while fixing the intermediate trace sequence. A native/WASM-enabled
regression checks equal-owner records in opposite insertion orders and verifies
the intended tie breaks.

Full post-fix execution (`target/trace-fixed/`) has **165/168 equal trace keys**:
all 27 release-order mismatches disappeared. The only three remaining are PNG
for freedraw-loop-fills, legacy-stroke-sharpness and dashed/zigzag-line fillers.
Their output lengths differ under #178, so `artifact_retention.output_bytes`
and its work charge necessarily differ. No special-case suppression was added.
The equal-length rotated-shape-bounds PNG still differs in pixels but not trace.

The post-fix public join has **zero disagreements**, with four artifact keys
and three trace keys unequal across targets. Native packaged replay still matches
all 168 approved artifact/report expectations. Release qualification passes
360 native tests (5 ignored), and the font measurement remains exactly
`6fb430ed…f459b` / `79ea93cd…f90fa` for 977 byte cells and 1204 traces. Lint,
all conformance verifiers, workspace test tiers/doctests and generated-workflow
freshness pass. Diagnostic trace printing and the temporary dirty-packaging
override used for working-tree verification were removed. No native baseline
or font evidence pins changed.

Disk check before diagnosis found 18 GB available; cleaning only the completed
external raster probe's Cargo output reclaimed approximately 934 MiB. Probe
source and measurement evidence were retained. The final verification resumed
with 51 GB available following environment cleanup.

The independent accounting issue is resolved. Remaining equality failures wait
on the upstream math fix in #178; global ownership/corpus closure under #85/#86
remains separate work.
