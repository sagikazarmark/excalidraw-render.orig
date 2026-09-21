# #190 MPF-only final capture checkpoint — awaiting parent suite report

V12 (`representative.stage4-frames-text-assets.v12`) is materialized, approval null:

```text
sha256:f6c3a1fb16d0c1ce7d96b0ccd23ea2a4a95635f34ca56de3e669f28f9e81d9bf
```

Draft capture checkpoint (554 files), **not final production approval request**:

```text
sha256:315f42ca4ddb62ff4546dc3eed9f600ca351a638587e20a9bb6a54aeefd2814c
```

## Reviewed narrow scope

The exact parent freeze/selector map is in `parent-freeze-authorization.json`.
It settles a bounded primary two-image MP Index with optional UID/TotalFrames,
nonoverlapping relocated entries and representative primary attributes. Secondary
Index IFDs are rejected; the narrow Attribute version/individual-number IFD is
admitted; duplicate secondary MPF is rejected. Source-authored positive/adversarial
controls qualify these boundaries without adding a new browser-reference claim.

The 16 proposed malformed-index criteria explicitly change canonical-admission
wording to strict `image.decode-failed` recovery and independently validated index
inconsistency/browser base-fallback characterization. They do not claim all MPF
profiles are admitted or strict rejection equals browser pixels. All other
criteria and eight HDR-gap meanings are preserved. This policy transition still
requires actual exact-packet approval; the live policy remains unchanged.

## Completed results

- Fresh OLD `80670ae` and final packaged captures verify all 168 full keys, two
  sessions per key in fresh forward/reverse processes. **Every artifact/failure
  and full report record equals OLD exactly.** No unexplained change.
- Focused reruns: **46 JPEG tests** including **12 gain-map tests**, **10 library
  JPEG-codec tests**, and frozen JPEG input reconstruction pass with actual logs.
- All 104 applicable gain-map gates pass their declared contract:
  **24 canonical SDR + 64 strict recovery + 16 HDR known-failure vectors**.
  Original malformed framing 32 plus new malformed index 32 form the 64 recovery
  vectors. All **32 HDR failure pins** remain exactly as before; this is not HDR
  qualification or a tolerance exception.
- ICC160 and EXIF164 regression gates remain passing. #187 color matrix remains
  64 canonical + 12 known failures / 24 exact sampling pins.
- V12 native artifacts/full reports and six viewer-pair rasters equal approved
  V11. Repeats, resource derivation, font audit, existing three PDF scopes,
  effective comparison and semantic gates pass.
- Fresh strict bridge v6 and protected PDF artifact/full-report/qpdf/repeated
  PDFium/PDF.js equality pass. No approved pin/reference/profile changed.
- Pending exact migration: 16 new strict-recovery owners, 28 source refreshes,
  one V11→V12 representative and the explicit 16-criterion policy/catalog change.
  Proposed totals: **2126 assignments / 2041 assigned-excluded / 85 gaps**.

All 5,535 frozen source/tool/evidence files remain unchanged. Final sources:

| File | SHA-256 |
| --- | --- |
| `src/render/asset/jpeg.rs` | `1aa78e8ae12e830600d36ecd65acd6bd099be31e1aaa5570ce25fadc477e0f1d` |
| `src/render/asset/tests.rs` | `0e1a4e327766ff48ba403cf7c3e3ca16832a0e2987ef8b6ebd9086fe2ca4e6ef` |
| `tests/fast/jpeg_gain_map_contract.rs` | `e69f4e59d6e93d7f185554c8af07b72ca8e3adaad105d3a0f8f291df8b205b08` |

All paths above are under `crates/excalidraw-render/`. The changed legacy phony-MPF
unit fixture is included in the source-bound producer set; it now asserts strict
rejection rather than claiming a signature-only index is valid.

Current release CLI SHA-256:
`491bad705c366f5080ea98fe2f97b90304e203154a7bf578b485cfee132131aa`.
Actual Rust 1.97.1 release build/compiler/profile identity is retained in
`capture-summary.json` and V12 native capture. All Cargo inputs equal stable HEAD.

## Pending state and remaining handoff

V12 approved execution correctly rejects null approval. Current V11 and live
ownership checks reject stale source bindings; raw error logs are retained and
not waived. The live policy is still OLD, so no unapproved criterion migration is
hidden behind a new catalog hash. Strong actual approval and successful 109-test/
six-command receipt verification remain mandatory for the eventual **four-file**
policy/ledger/registry/V12-approval transaction and rollback protocol.

`parent-qualification.json` binds actual focused results and all final producer
descriptors. Parent format/Clippy/review statements are clearly marked session
attestations. The parallel full workspace result is awaited before final sealing;
no full suite was rerun by this collector. No collector fix or post-freeze code
edit was needed during this capture.

HDR reduced-IDCT diagnosis stays external/deferred: no decoder or HDR projection
implementation changed. Eight #190 HDR gaps, six #187 sampling gaps, baseline/#178,
global closure and CMS resource limits remain explicit. No approval, live policy/
ledger mutation, baseline refresh or commit occurred. This README is excluded
explanatory metadata.
