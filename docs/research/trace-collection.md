# Full-inventory test-only trace collection

## Commands

```sh
just trace-logs x86_64-unknown-linux-gnu    # native test-only traces
just trace-logs wasm32-unknown-unknown      # browser test-only traces
just full-equality                          # traces joined to the packaged runs, gated
```

`just full-equality` collects both targets' traces, the native and WASM
snapshot replays, then runs `conformance join-trace-results` and
`conformance verify-joined-results` over `target/release-evidence/results`.
`join-trace-results` materializes diagnostics even when equality fails. Only
`verify-joined-results` gates completion: it rereads raw logs, validates inputs,
joins packaged results, and rejects either trace/public disagreement or cross-target
artifact/report/trace differences other than the exactly recorded #178 set. The
exported `results.json` uses `excalidraw-render/trace-collection/v1`,
deliberately not the standalone `coverage-results/v1`; the generic
`verify-results` cannot bypass join validation.

## Test seam

The trace module is generated inside the existing `cfg(test)`-only
`render::test_support` module in a staged source snapshot (a Git worktree under
`target/trace-source`). No public trace API, production feature, or checked-in
modification to the session is introduced. The generated test reuses the native
collector's request mapper and calls `render_with_trace` /
`render_frame_with_trace` on the same 168 inventory keys. Each key runs twice
in separate native processes / browser worker invocations for forward and
reverse order. Trace output includes digest and length plus the corresponding
test-build artifact/report measurement.

Each completion line binds target, order and input digest. Input binds the
inventory, scenes, renderer source bytes, collector/template/stager/runner,
tool and consumer identities and Git commit. Invalid descriptors,
missing/duplicate/reordered keys or completion, and wrong target/input fail
before comparison. The compiler/test commands are release-mode and locked.
These are runner-produced measurements, not independently authenticated logs:
the evidence provenance is the recipe invocation with its pinned inputs.

The public join validates the native candidate and browser input/commit,
requires exact keys/order, and records any test-build/public differences before
attaching traces to the actual packaged outcomes. Package archives and browser
package/tool logs remain in the exported component directories. Trace collection
does not make a synthetic trace value stand in for missing data.

`comparison.json` keeps separate `trace_differences`, `artifact_report_differences`,
and `public_trace_disagreements` arrays. The first two compare against the first
native sample and may contain repeated records for sessions/orders; count unique
keys when assessing affected coverage. Collecting a report successfully is not a
passing equality verdict.

## Release-order diagnosis and fix

The first collection found 30 trace keys differing across targets. For the 27
that were not PNG-length differences, the first divergent events were releases
during `format_planning`, not different acquisitions or charges. Examples:

- Bound text: native released `resolved_text_layout_and_glyph_geometry` where
  WASM released the same owner's `relationship_edge` first.
- Image codecs: native released `/elements/0/fileId`'s relationship edge where
  WASM released `/elements/0`'s `resolved_image` first.
- Frame 2: the two targets released `/elements/16/link` and
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

After the fix, 165 of 168 trace keys are equal. The three remaining are PNG
for freedraw-loop-fills, legacy-stroke-sharpness and dashed/zigzag-line fillers:
their output lengths differ under #178, so `artifact_retention.output_bytes`
and its work charge necessarily differ. No special-case suppression was added.
The equal-length rotated-shape-bounds PNG still differs in pixels but not trace.
The public join has zero disagreements, with four artifact keys and three
trace keys unequal across targets, all within the recorded #178 set.
