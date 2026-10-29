# Bounded GIF LZW, palette and extension qualification (#82 / #85)

Starting at `fafa996`, this tranche adds **56 bounded cells** without production,
dependency, earlier reference or approval changes.

## Independent inputs and expected output

The author implements longest-prefix LZW without importing a production GIF
encoder. A separately structured JS audit and an independent fixed-table Rust
prefix/suffix decoder check the actual packed stream against authored indices.
Expected RGBA comes directly from those indices and explicit RGB palettes.
Tests verify code-width transitions, clears, references, KwKwK events, dictionary
saturation, sub-block framing and byte-exact container reconstruction.

- **36 matrix cells / 72 vectors:** 2/16/256-color global palettes × GIF87a/89a ×
  growth-to-full/midstream-reset/KwKwK × 1/255-byte subblocks × two seeded patterns.
- **Eight extension cells/vectors:** before/after comments with ordinary/split
  subblocks, unknown application, NETSCAPE infinite/finite counts, and plain text
  before the opaque full-screen image.
- **Twelve authored malformed controls:** invalid code sizes, undefined codes,
  missing/early EOI, excess pixels, truncated or unterminated subblocks, missing
  trailer and GIF87a extension violations. These are not browser observations.

Binary patterns are 257×193 so the dictionary genuinely reaches 4096 entries;
16/256-color patterns are 129×97. All growth vectors continue at a full dictionary
without clearing. Reset vectors reach 12 bits and clear midstream. Dedicated
KwKwK vectors contain 129–1,310 code-equals-next events. Binary growth/reset
actually reads every width from 3 through 12; other palettes start at 5 or 9.

The primary format authority is the
[GIF89a specification](https://www.w3.org/Graphics/GIF/spec-gif89a.txt), including
Appendix F and the deferred-clear cover sheet. NETSCAPE is conventional extension
syntax. Its fixtures have one image, so no looping/timing semantics are claimed.
Plain text has no GCE and precedes an opaque full-screen image; no intermediate
text or font rasterization claim is made.

## Frozen reference and native gates

Official Excalidraw v0.18.1 / Chromium 149.0.7827.55 collection uses the unchanged
pinned profile/OCI with network disabled, two fresh processes in opposite order
and fresh contexts. Generation and separate read-only recollection match exactly.
All **80 vectors / 240 observed surfaces** equal authored dimensions and RGBA.

Reference digest:
`sha256:3c05784f1152a10abcc731638ed46a185c16871ecb050da56f5caba7de30f707`.
The record binds 18 author/collector/wrapper/profile inputs. Its size is
55,363,520 bytes because complete source, trace and three-surface evidence is
retained. No production decoder output serves as expected evidence.

Both exact owner selectors execute all binding, reconstruction and negative
helpers. Native SVG preserves exact source octets. PNG and bound PDF image/ICC/
soft-mask samples equal authored/browser pixels exactly, with independent
artifact/page/image dimensions and opaque RGBA8 checks. Full reports remain
empty for these static images; repeated fresh renders have identical bytes and
reports. Malformed controls require exact decode-failed recovery, dimensions,
format invariants and equality to missing-asset controls. Missing EOI and excess
pixels reject correctly. No renderer mismatch was found.

## Preserved ownership

The prior catalog is reconstructed append-only and authenticated against the
`fafa996` ledger before normal extension. All 2,262 previous dispositions and
criteria, three exclusions, 168 inventory keys and approved V12 remain intact.

| Measure | Count |
| --- | ---: |
| Obligations | 2,318 |
| Primary owners | 2,230 |
| Approved exclusions | 3 |
| Assigned/excluded | **2,233** |
| Explicit gaps | **85** |
| New owners | 56 |

This does not close GIF umbrella facets, arbitrary extension/GCE interactions,
animation timing or later-frame composition, or the full fuzz/resource matrix.
The global ledger remains unapproved. Deferred JPEG and #178 work remain separate.

## Reproduction

```sh
bash tools/reference-oracle/run-gif-lzw.sh verify
cargo test --locked -p excalidraw-render --test fast gif_lzw_contract::
node tools/reference-oracle/scripts/gif-lzw-ownership.mjs verify
cargo run --locked -p xtask -- conformance verify-ownership
```

Use Rust 1.97.1 with incremental/debug profiles disabled. Ordinary tests need no
browser or Node runtime. Reference generation is create-only; verification mounts
the repository read-only and builds the wrapper in temporary container storage.

## Final verification and reviews

- Generation and separate read-only browser verification produced the identical
  reference digest, with all 240 surfaces exactly equal to authored samples.
- Full workspace: **1,255 passed, zero failures, 19 existing ignored tests**,
  including 619 fast tests and both registered-case parity targets.
- Spec review found the frozen missing-EOI control also underfilled its declared
  image. Added a separate authored 4×1 pair that emits exactly four indices,
  differing only by EOI, with complete framing/trailer. Disabling the dependency's
  EOI check accepts those four indices; production rejects the EOI-less member.
  Both exact owners execute the paired SVG/PNG/PDF gates. The frozen compound
  control and reference bytes remain untouched.
- Both final owner tests pass together and individually after this strengthening;
  formatting and Clippy pass. Re-review confirmed the Spec finding closed.
  Standards review found no hard violations; typed statistics and aggregation
  helper extraction remain low-priority maintainability suggestions.
- Only the 56 new source bindings were refreshed. Preservation, ownership and
  approved V12 verification pass. The long full suite preceded the local test
  correction and was not redundantly rerun afterward.
