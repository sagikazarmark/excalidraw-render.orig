# Stage 4 bounded integration unit: provenance and pending inspection

Subsequent bounded unit: [scoped exception proposal tooling](png-stage4-scoped-proposals.md).

This unit changes only xtask, candidate tooling, and this audit. It does not
change production rendering/ICC code, v4 authoring, budgets, crops, corpus
approvals, or protected baselines. No viewer collection or full suite was run.

## Completed

- Collection verification requires the exact 19 viewer producer/dependency
  descriptors and all eight candidate-specific font-audit producer descriptors.
  Missing, duplicate, substituted and stale entries fail. Viewer/comparator
  profiles remain checked through their existing approved qualification paths.
- The collector binds sources, profiles, assets and inputs before rendering and
  checks them again afterwards; verify mode also checks the recorded source set.
- `verify_assets` reconstructs the SVG-derived resource plan, embedded WOFF2 and
  image bytes, and optional format-hint normalization from the original SVGs.
  It independently decodes WOFF2 to SFNT, checks primary-face coverage, reconstructs
  the pure-Rust SVG and complete resource declarations, and compares exact bytes.
  It does not trust matching stored descriptor hashes as proof of derivation.
  Verification is read-only, including when a derivative is missing or corrupt.
- `candidate-inspect DIRECTORY` checks provenance, authoring, semantic results
  and fresh native replay before returning an inspection result. Valid failing
  comparisons return `status: pending_judgment`; canonical success returns
  `ready_for_approval_check`. Both have `approval_status: not_evaluated`, a null
  approval subject, and no eligible execution keys. Stale evidence remains an
  error, not a pending judgment. Existing manifests must pass their binding checks.

The archived v4 collection is intentionally not rebound: its smaller producer
set and old source hashes are historical observations. End-to-end inspection of
fresh capture evidence remains part of the later recapture unit.

## Targeted verification

With Rust 1.97.1 and `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_TEST_DEBUG=0`,
`CARGO_PROFILE_DEV_DEBUG=0`:

```sh
cargo test --locked -p xtask --bin xtask provenance_ -- --nocapture
cargo clippy --locked -p xtask --bin xtask -- -D warnings
node --check tools/reference-oracle/scripts/stage4-viewer-assets.mjs
node --check tools/reference-oracle/scripts/stage4-candidate-views.mjs
git diff --check
```

All pass. The test filter selects six tests, including three new tests:

1. Removal of each of the 27 mandatory producer entries, plus duplicate, stale
   digest and substituted-path mutations in each producer set.
2. Mutation of normalized SVG, pure-Rust SVG, WOFF2, SFNT, an image, resource plan
   and font declaration; deletion of a derivative; byte snapshots prove failed
   verification neither repairs nor creates files.
3. Inspection classifies failed measurements as pending; even synthetic passing
   measurements cannot create approval or eligible executions; incomplete
   comparison sets are rejected.

## Next bounded unit: scoped exception proposals

The user authorized **preparation**, not approval. Preserve that distinction.
The existing `conformance::verify_exceptions` contract uses exact registry/
manifest equality and case approval to bind `variant`, `format`,
`viewer_profile`, `region`, `max_changed_pixels`, `minimum_ssim_bits`, `reason`
and `approving_issue`. Its present admitted scope is whole direct PNG only, so
the supplemental adapter must explicitly validate PDF viewer/region scopes;
simply passing PDF records to that verifier is not sufficient.

Required next work:

1. Bind the persisted repository diagnosis and its producer chain into the
   proposal, together with case/variant, viewer profile identity and digest,
   comparator identity, exact region coordinates and captured artifacts.
2. Recompute fresh per-scope measurements. Proposed ceilings must match the
   observed changed-pixel count and exact SSIM bits with no margin. Reject
   duplicate/unknown scopes, widened ceilings, stale diagnosis or producer
   bindings, and exceptions whose scope now passes the canonical gate.
3. Keep canonical and proposed-exception judgments separate. A human-approvable
   proposal bundle may contain pending scopes, but approved execution must
   require genuine case approval binding those exact scope records and files.
   No invented approval object belongs in corpus evidence.
4. Recheck which scopes actually need exceptions. Prior measurements indicate
   PDFium whole and typography, and PDF.js whole. PDF.js typography already
   passed its existing viewer mode, so it must retain a canonical judgment rather
   than receive an unnecessary exception. Recheck both PNG whole and crop after
   the typography fix; the reported focused-crop success alone is not a whole
   artifact qualification.

Historical reference measurements to verify, not silently adopt as new evidence:

| Scope | Changed pixels | SSIM |
| --- | ---: | ---: |
| PDFium whole | 118 | 0.9898474730435529 |
| PDFium typography | 118 | 0.9413564483014669 |
| PDF.js whole | 16 | 0.998520923299333 |
| PDF.js typography (canonical pass) | 16 | 0.9920016961872306 |

The reported causes are PDFium's whole-pixel glyph-origin Y quantization/gamma
and PDF.js outline positioning versus Chromium's quarter-pixel glyph origins.
The diagnosis agent will persist the proof; this audit does not substitute for
that evidence. PDF geometry and the global comparison budgets remain unchanged.
