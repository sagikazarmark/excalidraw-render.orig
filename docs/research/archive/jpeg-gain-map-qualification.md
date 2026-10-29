# JPEG MPF / Adobe XMP gain-map qualification (#82 / #85 / #190)

## Scope and independently authored structure

This tranche follows `b150028` without production/dependency/approval changes.
It adds **52 cells / 104 vectors** crossing baseline/progressive JPEG, both MPF
TIFF byte orders and two asymmetric RGB base patterns. Five valid signals cover
identity, nonidentity SDR-base and nonidentity HDR-base with ramp/edge gain maps.
Eight absent/malformed controls are kept separate from valid containers.

Unlike the earlier `MPF\0fixture-index` unit fixture, these vectors contain a real
MP Index IFD: version `0100`, two 16-byte MP entries, exact image sizes, primary
offset zero and TIFF-relative secondary offset. The 90-byte APP2 is first after
SOI, placing its TIFF base at file byte 10. Primary XMP has a Container Directory
with the exact secondary length; secondary XMP has Adobe hdrgm 1.0 min/max/gamma,
offset, capacity and base-rendition fields. The independent Rust structural gate
checks actual image boundaries, ranges, namespaces/values and full byte reconstruction.

Wire-format facts were checked against these primary sources at libultrahdr
commit `f3e36226914289cfefcf0d81a6471cf6266c79fb`:

- [multipictureformat.cpp](https://github.com/google/libultrahdr/blob/f3e36226914289cfefcf0d81a6471cf6266c79fb/lib/src/multipictureformat.cpp)
- [multipictureformat.h](https://github.com/google/libultrahdr/blob/f3e36226914289cfefcf0d81a6471cf6266c79fb/lib/include/ultrahdr/multipictureformat.h)
- [jpegrutils.cpp](https://github.com/google/libultrahdr/blob/f3e36226914289cfefcf0d81a6471cf6266c79fb/lib/src/jpegrutils.cpp)

This is independently authored fixture code, not copied upstream implementation.
It does not claim ISO 21496-1, Apple signaling, every MPF class or conformance to
every libultrahdr restriction: at the cited commit its XMP metadata decoder
explicitly rejects `BaseRenditionIsHDR=True` as well as its encoder using an SDR
base. The HDR-base vectors exercise Adobe XMP signaling interpreted by Chromium;
libultrahdr supplies wire-format vocabulary, not independent HDR-base validation.

## Reference collection and judgments

Official Excalidraw v0.18.1 SVG and direct canvas PNG were collected in pinned
Chromium 149.0.7827.55, two fresh processes in forward/reverse order, fresh context
per vector, networking disabled. Official SVG is independently rasterized for
the PDF sample-color authority. Source/producer/profile/comparator descriptors
are bound in the frozen record:

`sha256:af3c97e389ef230c57bbe9a4b3406dc1ef1d6a2ad37b0e8ad2e1f4fbe6568ce4`.

All vectors run in SVG/PNG/PDF twice. Gates check exact machine reports, byte
replay, canonical source data URLs, actual artifact/page/image dimensions,
opaque alpha, PNG RGBA8/metadata and bound PDF image/ICC/soft-mask samples.
Independent structural and negative-control checks execute inside both owner
selectors. Whole-PDF viewer qualification is not claimed.

**12 SDR cells / 24 vectors pass**: identity and two nonidentity SDR-base signals.
Browser output and native PNG/PDF artifacts match the interpreted base, while
SVG preserves both complete compressed images and metadata.

**16 framing-recovery cells / 32 vectors pass** their exact existing strict
contract: absent MPF, absent primary XMP, truncated secondary EOI and a third
image yield `image.decode-failed` in all formats. Recovery artifacts equal
independent missing-asset controls. Browser base fallback is characterized, not
claimed visually equivalent to strict native rejection.

**24 cells / 48 vectors remain gaps under
[#190](https://github.com/sagikazarmark/excalidraw-render.orig/issues/190):**

- Eight HDR-base cells (16 vectors): the browser applies the gain map for SDR
  projection, while native PNG/PDF remain identical to the unmodified base.
  The distinct ramp/edge maps and HDR/SDR base flags prove that signaling is not
  universally ignored by the reference. Exact per-format failure measurements
  are retained, not accepted tolerance ceilings.
- Sixteen malformed-admission cells (32 vectors): bad MPF offset, size/count and
  missing secondary are silently accepted. Base-image equality does not validate
  the index. These cases receive no owners pending a validation/recovery decision.

HDR measurements match across both scan modes, TIFF orders and PNG/PDF:

| Signal | Base pattern | Changed pixels | SSIM |
| --- | --- | ---: | ---: |
| hdr-ramp | ramp | 553 | 0.860099630813284 |
| hdr-edges | ramp | 648 | 0.8479795899866851 |
| hdr-ramp | edges | 1,214 | 0.9116739114307459 |
| hdr-edges | edges | 1,262 | 0.8737353963660468 |

The test records exact SSIM bits and requires canonical failure. No ceilings are
widened and no malformed acceptance is counted as safety qualification.

## Ownership and verification commands

The prior catalog is reconstructed append-only and authenticated against the
`b150028` ledger before extension. All **2,074 prior dispositions**, criteria,
three exclusions, 168 inventory keys and approved V11 survive unchanged.

| Measure | Count |
| --- | ---: |
| Obligations | 2,126 |
| Primary owners | 2,022 |
| Approved exclusions | 3 |
| Assigned/excluded | **2,025** |
| Explicit gaps | **101** |
| New owners | 28 |
| New #190 gaps | 24 |

```sh
bash tools/reference-oracle/run-jpeg-gain-map.sh verify
cargo test --locked -p excalidraw-render --test fast jpeg_gain_map_contract::
node tools/reference-oracle/scripts/jpeg-gain-map-ownership.mjs verify
cargo run --locked -p xtask -- conformance verify-ownership
```

Use Rust 1.97.1 and disabled incremental/debug profiles. Ordinary tests require
no external runtime. Global ledger approval, broader codec qualification,
#187 sampling and #178/native-baseline work remain open or deferred.

## Verification and review

- Separate two-process pinned-browser recollection reproduced the full reference
  record byte-for-byte.
- Full workspace: **1,244 passed, zero failures, 19 existing ignored tests**,
  including 608 fast tests and both registered-case parity targets.
- Standards review found that negative structural/output mutations and map
  discrimination were separate tests rather than executed by each exact owner
  selector. These were extracted into assertion helpers called by both owners.
  All seven final focused tests, formatting and Clippy passed afterward; the full
  suite preceded this local test-only strengthening and was not repeated.
- Spec review clarified the cited libultrahdr HDR-base restriction above. No
  production or evidence changes were required. The low-priority positional
  metadata-parameter naming concern was not expanded into unrelated refactoring.
