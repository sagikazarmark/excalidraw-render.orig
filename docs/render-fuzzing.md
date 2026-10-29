# Complete-render fuzzing (#83)

The confirmed #30 seam is `SceneDocument::parse` → validated `RenderRequest` →
`Renderer::render`. `tests/render_fuzz` uses only that public facade. It adds
cross-stage generation to the parser/normalization (#81) and codec/sanitizer
(#82, [asset fuzzing](asset-fuzzing.md)) targets.

## Gates

- Ordinary PR tests run 36 fixed-seed requests and intact controls in all three
  formats. The nine families exercise shapes, rough fills/strokes, arrows and
  freedraw, multilingual text and font fallback, frames and relationships,
  nested SVG sanitization, PDF filter fallback, offline placeholders,
  normalization repairs, and mixed scenes containing those stages together.
- Requests vary background, padding, scale/downscale, light/dark mode, frame
  controls, all optional metadata switches, supplemental conflicts and PDF
  fallback policy. Thirteen limit profiles exercise admission, geometry, text,
  diagnostics, working memory, output, work, surfaces, local assets and a late
  output boundary one byte below the complete artifact. Controls explicitly
  require successful SVG/PNG/PDF projection, allowed and forbidden fallback,
  optional metadata omission and fatal output exhaustion.
- Every case is parsed/rendered again after other cases, in reversed case and
  format order with a fresh renderer. Artifact SHA-256, complete serialized
  reports, outcome kinds and complete failure descriptors must agree. The
  initial pass shares a renderer across independent request-scoped sessions.
- Successful SVG is parsed and nested SVGs inspected for active content and
  external image/CSS references; PNG chunks/CRCs and pixels are decoded; PDF streams,
  single-page structure, terminal carrier and xref address are checked.
  Metadata presence follows the request and omission report; carried summary
  counts agree with pre-metadata diagnostics; embedded scene values and their
  digest are checked independently through all three carriers.
- Only an explicit set of semantic resource/fallback failures is allowed.
  Mandatory-notice encoding failure under constrained budgets additionally
  requires successful rendering with restored budgets and a notice-bearing
  manifest. Unexpected encoder/internal failures and panics fail the test.
  The separate subpixel PNG control records the specified truncation failure
  (§8.1), rather than admitting arbitrary encoding errors into the population.

This is deterministic grammar/mutation fuzzing, not coverage-guided fuzzing or
proof over all inputs. Existing exact boundary, visual, allocation-refusal and
no-hidden-I/O architecture gates remain independent authorities. Fatal outcomes
are observed only through the facade's descriptor/report-only error variant;
the harness introduces no writer or resource callback. Browser WASM runs the
same smoke adapter through `just render-fuzz-wasm` (debug symbols are disabled
to bound wasm-bindgen's module-processing memory; test assertions remain on).
The nightly GitHub workflow runs `just nightly-render-fuzz` with 4,096 cases and
a 90-minute job timeout. It retains test-profile
overflow/debug assertions. Timeout, OOM and abort are failed jobs, never accepted
outcomes. Tracing flushes the complete reproducer before each render so a native
trap or hang has a last-input record.

Inputs are capped at 64 KiB, component scenes at five elements and mixed scenes
at 31 elements. Mutation is bounded and preserves the envelope on six of eight
strategies; others truncate JSON or append invalid UTF-8. Semantic ceilings cap
geometry at 16,384 segments, text at 512 scalars, nested SVG at 128 nodes/depth 2,
surfaces at 65,536 pixels, output at 8 MiB, logical memory at 256 MiB and total
work at 1,073,741,824 units (font subset admission needs the normal font budget).
These are logical ceilings, not allocator RSS claims. Only digests and reports
are retained between passes, not the full population's artifacts.

```sh
cargo test --locked -p excalidraw-render --test render_fuzz
just nightly-render-fuzz        # 4,096 cases; pass a count to change it
just render-fuzz-wasm
```

## Reproduce, minimize, retain

Save the complete JSON after `render-fuzz reproducer:` (or `render-fuzz` in trace
output), then replay it without the generator:

```sh
EXCALIDRAW_RENDER_FUZZ_REPLAY=/tmp/reproducer.json cargo test --locked \
  -p excalidraw-render --test render_fuzz \
  render_fuzz::replay_captured_complete_render -- --ignored --exact
```

Order-comparison failures print the complete ordered case array; the replay
entry point accepts that array and restores its shared-renderer history before
the reverse fresh-renderer comparison. Minimize the history as well as the
failing scene and retain it in the regression entry's optional `history` array.

The JSON contains exact scene bytes as base64, the request option bits, limit
profile and discovery identity. Late-output cases also capture `output_ceilings`
in SVG/PNG/PDF order so replay and minimization preserve the actual request;
permanent boundary regressions require those concrete ceilings.
`request()` defines the versioned harness bit
mapping; preserve it when promoting regressions. Reduce scene elements and
relationships, then fields, text, asset nodes and bytes while retaining the
**same** failure predicate and request. Use process timeouts when minimizing
hangs. Do not mistake parser rejection or a different resource crossing for the
original failure. `EXCALIDRAW_RENDER_FUZZ_TRACE=1` plus `--nocapture` prints and
flushes inputs before execution.

Every confirmed renderer defect must be minimized and added to
`tests/qualification/render-fuzz-regressions.json` with its discovery seed/index,
failure description, complete reproducer and exact ordered expectations:
`rejected` has a descriptor; `failure` has descriptor and report; `artifact` has
SHA-256 and report. Expectations are in SVG/PNG/PDF order (one entry for parse
rejection). The ordinary regression test enforces unique IDs, minimization and
replays each entry through the facade twice. Keep a focused red test for the
specific defect before fixing production. No confirmed renderer defects were
found in the initial 4,096-case run, so the manifest starts empty rather than
claiming authored controls as discoveries.
