# JPEG duplicate/late EXIF qualification (#82 / #85 / #189)

## Scope and independent evidence

Starting at `b0b6004`, this tranche adds **82 cells / 164 vectors**. Each cell
crosses baseline or progressive JPEG, little/big-endian TIFF, and an explicit
APP1/IFD precedence scenario; its two asymmetric patterns are a 33×25 ramp and
48×32 edges. Controls distinguish identity, 180°, 90° clockwise and 270°.

The author inserts complete APP1 segments over frozen RGB JPEG bytes, preserving
entropy data. Late locations are actual JPEG marker boundaries: before EOI or
before the second progressive SOS. Independent Rust reconstruction checks exact
payloads, placement and the closed matrix.

Official Excalidraw v0.18.1 SVG/direct-PNG exports were collected under pinned
Chromium 149.0.7827.55, twice in fresh processes with opposite order and fresh
contexts; networking is disabled by the pinned OCI runner. Scene image extents
use the browser's observed intrinsic dimensions to avoid accidental resampling
of the correctly oriented image. This is recorded input authoring, not a native
output-derived expectation. PDF sample color authority is the separately
rasterized official SVG.

Reference SHA-256:
`cd811bca81e24aaeb652c86d3e6dfd7b4aeb65d8050764da34b129530aac6ecf`.
The record binds the fixture author, source JPEGs, wrapper/build files, collector,
runner, reference profile/assets and comparator profile.

## Measured precedence and gates

The browser selects the **first EXIF APP1 before the first scan**, including one
with invalid orientation zero, absent orientation tag or bad TIFF magic. Within
that IFD it selects the first valid orientation: `[0,6]` uses 6, `[6,3]` uses 6,
`[3,6]` uses 3 and `[6,0]` uses 6. APP1 before EOI or between progressive scans
does not override early metadata; a late-only orientation is ignored.

Reference orientation must match exactly one control. An independent coordinate
permutation of untagged browser samples confirms each control rather than relying
only on the same tag parser. The owner selector itself executes reconstruction
and discrimination checks. Every vector has exact complete empty reports, fresh
artifact/report replay, exact SVG compressed octets, artifact/page/image extents
and opaque alpha. Passing vectors use unchanged canonical GeometryImage budgets
for PNG and bound PDF image samples. This is not whole-PDF viewer qualification.

**46 cells / 92 vectors pass:** four orientation controls, valid duplicate-tag
ordering, valid-tag-before-invalid, late-only and early/late conflict cases,
including the progressive between-scan scenarios.

**36 cells / 72 vectors fail**, tracked in
[#189](https://github.com/sagikazarmark/excalidraw-render.orig/issues/189):
the renderer's JPEG header path keeps the last APP1; its TIFF reader stops at
the first orientation tag even when invalid. Eight duplicate-block scenarios
and invalid-tag-before-valid therefore choose the wrong orientation.

Each failing PNG retains its exact changed-pixel count and SSIM bits, requiring
an actual canonical failure. The PDF retains transposed **image** dimensions
(`33×25 ↔ 25×33` or `48×32 ↔ 32×48`) while its page extent stays correct. Bound
image pixels match the independently declared wrong-orientation control and the
corresponding permutation of untagged PDF samples; intended-reference comparison
must return `DimensionMismatch`. These are structural defect checks, not approved
exceptions. No failure is assigned an owner and no tolerance is widened.

## Append-only ownership

The prior catalog is authenticated from
`tests/oracle/jpeg-exif/prior-catalog.json` (canonical digest verified against
the untouched `b0b6004` ledger before the normal extension).
The normal extension preserves all **1,992** earlier dispositions/criteria and
three exclusions, then adds 82 explicit obligations. Only 46 passing cells
receive the exact compiled test owner.

| Measure | Count |
| --- | ---: |
| Obligations | 2,074 |
| Primary owners | 1,958 |
| Approved exclusions | 3 |
| Assigned/excluded | **1,961** |
| Explicit gaps | **113** |

The 36 new gaps expose concrete precedence failures previously hidden inside the
coarse JPEG orientation facet. That facet remains open. The dependency graph,
168 inventory keys and approved V10 are unchanged. No production code changes.

## Reproduce

```sh
bash tools/reference-oracle/run-jpeg-exif.sh verify
cargo test --locked -p excalidraw-render --test fast jpeg_exif_contract::
node tools/reference-oracle/scripts/jpeg-exif-ownership.mjs verify
cargo run --locked -p xtask -- conformance verify-ownership
```

Use Rust 1.97.1 with incremental/debug profiles disabled. The ordinary Rust suite
uses frozen evidence without external runtime tools. #187 sampling, #178/native-baseline work,
broader TIFF/IFD-chain and MPF/gain-map qualification remain separate.

## Final verification

- Separate pinned-browser recollection reproduced all 164 vectors and the entire
  reference record byte-for-byte.
- Full workspace run: **1,235 passed, zero failures, 19 existing ignored tests**,
  including all 599 fast tests and registered-case parity gates.
- Standards review identified that the exact owner selector omitted the standalone
  canonical negative-control discrimination gate. It now calls the same assertion
  helper directly. All four affected tests, formatting and Clippy pass after this
  test-only strengthening; re-review confirmed closure. Spec review found no
  additional scoped issue. The long full suite preceded that local correction and
  was not redundantly repeated.
- Normal ownership verification, approved V10 validation and the append-only
  preservation audit passed. Only the 46 new owner-source bindings were refreshed.
