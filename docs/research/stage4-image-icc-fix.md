# Authorized PDF image ICC binding fix and requalification proposals

Current status (2026-09-20): the scoped protected backend proposal below was
[approved and applied](stage4-pdf-approved-transition.md). The original evidence
and approval subject are preserved. Representative and baseline approval remain
separate; the sections below retain the production-fix qualification history.

## Production scope and red/green proof

The sole production change is in `src/render/pdf.rs`: each source RGB image
XObject now writes `/ColorSpace [/ICCBased <ICC_PROFILE_OBJECT> 0 R]` instead of
the page-local `/CS1` alias. Page and form `/CS1` resources and content operators
remain unchanged. The mask remains DeviceGray. No typography, PNG, ICC decoding,
geometry, interpolation, budget or profile implementation changed in this unit.

The public-facade regression `tests/pdf_image_colorspace.rs` failed before the
fix with **“image must carry a self-contained colour-space value, not /CS1”**.
After the fix it passes for 2×2 and 40×40 sources with opaque and partial alpha,
with no preceding colour operator. It follows the image's ICC reference, checks
the profile's three-channel/DeviceRGB declaration, and checks the actual catalog
OutputIntent refers to that same object.

The binding-aware PNG/PDF sample oracle and candidate semantic validator now
require the direct ICCBased value, preserve the legal page resource binding,
and follow the catalog's actual OutputIntent. Mutation checks reject old aliases,
wrong ICC references and detached output intents in addition to the existing
wrong RGB/mask/resource/link mutations. No existing field was relaxed.

## Pinned viewer controls

The release facade viewer regression passed all eight source/viewer combinations:
2×2 and 40×40, opaque and partial-alpha, PDFium and PDF.js. Each viewer ran twice;
rasters repeat and all four unprimed source corners match the independent colour/
alpha oracle. Native PDFs repeat as well. Evidence is retained at
`tests/conformance/candidates/stage4-image-icc-controls-v1/`.

Original failed 2×2 controls remain untouched. The new protected-PDF collector
reproduces old PDF.js's documented gray failure explicitly as a negative control;
both current viewers must pass the actual source-colour oracle. It does not
relabel the old failure as a source-colour pass.

## Protected backend proposal and subsequent scoped approval

Directory: `tests/conformance/candidates/stage4-pdf-backend-requalification-v2/`.
Approval-subject digest:

```text
sha256:c15e6bab141ac8080bac099932e5561bfcbee4c000f4957fec71e87d5e573389
```

| Identity | Value |
| --- | --- |
| Protected original native PDF | `b053159dc6ea0649e6c0fa5840196ccb81db6b41b7eb69d0181392851017ee43` |
| Proposed corrected native PDF | `50180ecf1822d7d573ea78cc6375cd3f83af8ef2748422b7e9b3d52a1b7ba6dc` |
| Proposed operation hash | `ebf933b7a43e6d53730578e44562484e2aa04eb4cc0d47af7c876625e054de16` |
| Proposed inventory hash | `a993befecf6da05bbf204d240bc3242b045a9eefd7460348811e1dfe3e83159b` |

The clean-base producer again reproduces the original artifact twice. Current
release bytes/reports repeat. The original protected structural expectation
passes old output and fails current output only at the two expected digest
checks; the separate proposed expectation passes current output. Normalizing
only image orientation, image/mask interpolation, the direct image colour-space
reference and derived file IDs makes all remaining document/page fields equal.
All text, fonts, mappings, geometry, clipping and samples remain unchanged.

Old/current protected-scene rasters are byte-identical in both pinned viewers
(zero changed pixels, SSIM 1). The constant-black source ROI remains correct.
Fixed unprimed colour controls pass both viewers. At initial collection,
`approval.json` was null and protected pins were unchanged. The subsequent
user-granted scoped approval and exact applied pin transition are recorded in
`applied-transition.json`; see the [application audit](stage4-pdf-approved-transition.md).
The subject verification command replays current native output twice and still
returns the original approved subject.

## Explicit typography provenance bridge

Directory: `tests/conformance/candidates/stage4-pdf-colorspace-bridge-v1/`.

| Identity | Value |
| --- | --- |
| Diagnosed v4/v6 PDF | `95c399f0eaaee5bd0b2ae13a50a1a68171c0d5abaeab0b71bdf8816abda5817b` |
| Fixed PDF | `310ca74edc3ea7e69fc549739c45e05d90d7522704be1d94bb9f553084658265` |
| Bridge file hash | `9d97c210dc63a0a5ffda4431b9bd7887243f53dde0befa528aaa37384ecab286` |
| Canonical bridge-record hash | `58a455a47e26dc67ec63fe0789dcb32d3bb2525316515664599f660381d2bbae` |

Fresh release PDF/report pairs repeat. The bridge compares qpdf's **raw, not
decoded, stream bytes** and every object dictionary. Only image object `18 0 R`
changes from its resource alias to the existing ICCBased array; derived trailer
IDs/xref offsets may change. Every font program, text coordinate, content stream,
image/mask sample stream, image flag and other dictionary is byte-identical.
Any unrelated stream/object/profile-reference change is rejected.

Two fresh captures per viewer are byte-identical to the original diagnosed
rasters. Canonical typography comparisons were recomputed:

| Viewer | Whole changed pixels / SSIM bits | Typography changed pixels / SSIM bits |
| --- | --- | --- |
| PDFium | 118 / `3fefacd49b980278` | 118 / `3fee1f978eeaac28` |
| PDF.js | 16 / `3feff3e226be27cf` | 16 / `3fefbe7a57566bff` (canonical pass) |

The original diagnosis/proof files remain unchanged. Exception preparation
accepts a changed `native.pdf` only through this validated bridge and binds the
**actual fixed PDF plus bridge descriptor** into the proposal/approval subject.
All other diagnosis inputs must still match exactly. Tests reject a missing,
stale or unrelated bridge; they also prove the original canonical PNG and other
unexcepted gates remain active. No tolerance limit changed.

## Tested binding refresh

After the focused checks, the explicit follow-up record
`tests/conformance/coverage/stage4-image-icc-binding-reconciliation.json`
(SHA-256 `eede9f5e414bfb42563c9d7d4e835254f54e9c507f3181f2ec4af0932f694409`)
refreshes the unchanged `fuzz.codecs` locator's source identity and 236 source
bindings across the three affected owner modules. Owner selectors, commands,
criteria, dispositions, all 168 execution records and all three approved
exclusions remain unchanged. The PNG sample oracle helper pin is now
`44a5b9b4fba24704191b32f53008467605e6afa67f3fad02a3937ea53aba64c5`.

Current catalog digest:
`944fb1bb349def48ebff5bd570c34ee492aa54893842ba7d5a8b8a5c43c099fd`.
Inventory/ownership validation and the chained preservation audit pass, with no
stale live locator or owner source. Totals remain 1,876 assignments, 1,802
assigned/excluded and 74 gaps; the representative interaction remains a gap.

## Checks and next review

Passed: the red/green facade regression; both pinned-viewer colour controls;
four PNG profile/oracle tests; 25 Stage-4 PNG tests; 29 static-PNG tests; five JPEG
tests; ten dark-mode/frame/image tests; candidate semantic mutations; bounded
bridge mutations; six scoped-proposal/approval tests; real release-owner positive
integration using an isolated verifier fixture; xtask Clippy and diff checks.
No full workspace ran.

Parent review commands (pinned Rust 1.97.1/debug0):

```sh
cargo test --locked --release -p excalidraw-render --test pdf_image_colorspace -- --include-ignored --nocapture
cargo run --locked -p xtask -- conformance pdf-requalification-subject \
  tests/conformance/candidates/stage4-pdf-backend-requalification-v2
cargo run --locked -p xtask -- conformance verify-inventory
cargo run --locked -p xtask -- conformance verify-ownership
node tools/reference-oracle/scripts/verify-png-stage4.mjs
```

Parent review approved the protected-pin/structural proposal, and the user's
scoped decision was recorded and applied in the subsequent bounded unit. A final
representative v7/v8 capture must use the frozen producer/consumer bindings;
the evidence here covers the narrowly scoped PDF bridge and backend proposal,
not a final all-format candidate. Work remains uncommitted. Existing baseline
approval staleness and the fixed-size moxcms resource/OOM gap remain explicit.
