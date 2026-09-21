# JPEG ICC qualification (#82 / #85 / #188)

## Bounded matrix and provenance

This tranche follows `ff103a5` without changing production, dependencies, existing
reference bytes or approval records. It adds **80 cells / 160 vectors**:

- RGB matrix/TRC and gray TRC ICC v2/v4, gamma 1 and 2.2.
- Baseline/progressive JPEG; asymmetric ramp and edge patterns per cell.
- Single APP2 or three parts split inside the header/table, in forward/reverse order.
- Bad profile signature, truncation and out-of-bounds tag offset.
- Missing/duplicate/zero-index/inconsistent-count multipart sequences.
- Opposite profile/source color models, explicitly unqualified.

`jpeg-icc-fixtures.mjs` authors profile and segment bytes independently of the
production parsers/CMS. It uses frozen JPEG sources from the preceding color
matrix. The Rust gate independently checks the closed cell set, profile hashes,
APP2 sequences/assembly and exact reconstructed JPEG source. No production output
provides expected pixels.

The reference renderer is official Excalidraw v0.18.1 under pinned Chromium
149.0.7827.55 and OCI digest
`111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9`.
Two fresh browser processes use opposite vector order and fresh contexts, with
network disabled. Official SVG and direct PNG export repeat byte-for-byte;
the SVG is independently rasterized for the PDF sample-color authority.

Reference SHA-256:
`7ffd57eaa8ddfe4a7d311c5a55c57b84a001463864ce6bd235613faab85e76d5`.
The record binds the author, collector/runner, wrapper/build sources, package
lock, reference profile/assets and canonical comparator profile.

## Gates and outcomes

All 160 vectors execute in SVG/PNG/PDF twice with fresh renderer instances.
Complete machine reports, deterministic bytes, source preservation, actual
SVG/PNG/PDF extents, opaque alpha and bound PDF image/ICC/soft-mask samples are
checked independently of visual budgets. PNG output is untagged RGBA8. Recovery
artifacts equal independent missing-asset controls. Human messages must be
nonempty but their wording is non-normative.

**36 color cells / 72 vectors pass** the unchanged GeometryImage budget against
both direct PNG and official-SVG-authoritative PDF samples: all RGB profiles and
gamma-2.2 gray profiles. This is sample-color qualification, not a whole-PDF
viewer claim. Gamma-1 tagged references are explicitly distinguished from the
untagged control, proving the ICC is not silently ignored. All valid multipart
layouts yield identical browser pixels and native PNG/PDF artifacts.

**28 recovery cells / 56 vectors pass exact strict recovery contracts**:
malformed ICC produces `image.unsupported-format` for PNG/PDF while SVG preserves
the source; malformed APP2 sequences produce `image.decode-failed` in all formats.
The browser admits these malformed controls. Their ownership is limited to the
existing strict recovery behavior, not visual parity or a newly granted exception.

**16 cells / 32 vectors remain gaps under
[#188](https://github.com/sagikazarmark/excalidraw-render.orig/issues/188)**:

- Twelve gray gamma-1 cells (24 vectors) fail canonical SSIM in PNG/PDF. Their
  48 exact known-defect pins require zero Pixelmatch changed pixels and SSIM bits
  `3fefeeb8ab8d6588` for ramps / `3feff7526521e20c` for edges. Both scan modes,
  profile versions and all layouts reproduce the same measurements. Pins reject
  movement in either direction and grant no tolerance exception.
- Four opposite-model cells (eight vectors) characterize current reports/source
  preservation without claiming color correctness. Gray JPEG with RGB profile is
  accepted; RGB JPEG with gray profile currently yields decode failure in PNG/PDF.
  Browser interpretation and the appropriate report classification need diagnosis.

## Ownership migration

The normal authenticated append-only extension uses the prior catalog at
`tests/conformance/proposals/jpeg187-requalification-v1/pending/catalog.json`.
All **1,912 prior dispositions**, criteria, three exclusions, 168 inventory keys,
V9 approval and dependency pins survive. Only new ICC cells receive owners/gaps.

| Measure | Final value |
| --- | ---: |
| Obligations | 1,992 |
| Primary owners | 1,896 |
| Approved exclusions | 3 |
| Assigned/excluded | 1,899 |
| Explicit gaps | 93 |
| New owners | 64 |
| New #188 gaps | 16 |

The increased gap count exposes concrete requirements previously hidden in
coarse JPEG facets. Global ledger approval and all broad JPEG facets remain open.

## Reproduction

```sh
bash tools/reference-oracle/run-jpeg-icc.sh verify
cargo test --locked -p excalidraw-render --test fast jpeg_icc_contract::
node tools/reference-oracle/scripts/jpeg-icc-ownership.mjs verify
cargo run --locked -p xtask -- conformance verify-ownership
```

Use Rust 1.97.1, with incremental and debug profiles disabled in this workspace.
Ordinary tests do not need a browser, network or container. #187 sampling,
#178 equality and native-baseline reapproval remain deferred/separate.

## Final verification

- Independent browser recollection reproduced the complete reference record
  byte-for-byte at the same digest.
- Full workspace command passed **1,226 tests, zero failures, 19 existing ignored
  tests**, including 590 fast tests and both registered-case parity targets.
- Standards review identified two gaps in the test gates: recovery artifacts
  needed independent extent/format checks, and exact owner selectors needed to
  execute their declared reconstruction/precedence/equivalence checks themselves.
  Both were corrected with shared assertion helpers and negative extent/PNG
  layout probes. Re-review confirmed both findings closed. Spec review found
  no additional blocker.
- All **six final ICC tests** pass after the test-only strengthening. Formatting,
  Clippy with warnings denied, ownership/inventory verification and V9 approval
  validation pass. Only the 64 new test-owner source hashes were refreshed;
  the preservation audit confirms every prior disposition remains identical.
- The full suite preceded the final strengthening; the affected tests were rerun
  afterward rather than redundantly repeating the long workspace suite.
