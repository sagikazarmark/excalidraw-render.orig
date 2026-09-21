# Bounded GIF first-frame reference evidence

`reference.json` contains independently authored GIF bytes and RGBA expectations,
plus actual official Excalidraw v0.18.1 / Chromium 149 observations. It is an
immutable collection, not an approval or a production qualification claim.

```sh
bash tools/reference-oracle/run-gif-first-frame.sh generate # creates with wx; refuses replacement
bash tools/reference-oracle/run-gif-first-frame.sh verify   # recollects; repository mounted read-only
```

Both commands build in an ephemeral container tmpfs and verify the existing
revision-1 profile without updating it. The digest-pinned linux/amd64 OCI image
runs with network disabled. Every valid vector runs in a fresh context in each
of two fresh Chromium processes, forward then reverse. Complete observations
must be identical. All author, collector, wrapper, reused scene helper, profile
helper, canonicalization helper, lock, and profile inputs are source-bound.

## Matrix and eligibility

- **120 cells, 240 valid collected vectors:** 3 palette modes (global only,
  local only, both with local override) × 2 rectangles × 2 transparency states
  × 2 interlace states × 5 animation/disposal groups × 2 patterns per cell.
- Logical screen **9×7**; first image full-screen or **5×5 at (2,1)**.
- Static disposal 0; two-frame animation with first disposal **0, 1, 2, 3**.
  The second frame is full-screen, opaque, and visibly different. Disposal is
  tested as first-frame metadata, not as a later-frame compositing promise.
- Four explicit RGB entries per active table. Background index 3 differs from
  transparent index 0. Expected canvas outside the image rectangle is zero RGBA,
  including opaque subrectangles; it is not painted with the GIF background RGB.
- Interlace rows are packed in the four GIF passes by the author. Expectations
  are expanded from display-order indices independently of wire packing.
- **16 uncollected controls:** 12 malformed and 4 resource controls. Each has
  bytes, mutation intent, and intended strict/recovery classification. Those
  classifications require parent policy binding; they are not browser results.
  Reserved disposal 4 is only a malformed control, never a valid cell.

Total: **256 vectors, 136 cell identifiers** (120 matrix + 16 control cells).
GIF87a, dictionary growth, larger palettes, looping extensions, short-delay
animations, arbitrary capture offsets, and later-frame compositing are outside
this bounded collection.

## Animation capture proof

The initial GCE delay is **65535 centiseconds (655.35 seconds)**. Both browser and
host monotonic clocks enforce a **30-second** bound from before the first GIF
load through all captures. Every context is new. The profile, timers, and browser
animation preferences are unmodified. Each animated vector has independent
first/later RGBA hashes, and each observed surface records matches against both.
This establishes frame-zero capture for these fixtures within the bound; it
does not freeze exported SVG animation for arbitrary later viewing.

## Record layout and independent comparison

`cases` preserves GIF bytes/hashes, palettes, frame coordinates, display-order
indices, wire row order, offsets, authored first/later RGBA, the scene document,
and exact official PNG/SVG plus browser SVG-derived PNG. `reference.samples`
contains intrinsic GIF, official PNG, and SVG-viewer dimensions and RGBA.
`comparison` gives per-surface changed-pixel counts and first/later matches.
Top-level `differences` retains every authored/reference discrepancy. Collection
does not change authored expectations or discard mismatching vectors.

Generation is create-only; verification performs no repository writes. After
collection, preserve the source hashes. Corrections require a separately named
history/artifact and an explicit explanation, rather than editing the bound
author and regenerating this record. The parent task owns production behavior,
Rust tests, resource policy, ledger entries, and approval decisions.

## Collected result

Generation and a separate read-only verification both passed under the pinned
profile. All **240/240** valid vectors were admitted. All **720** observed
surfaces matched authored dimensions and first-frame RGBA exactly; **zero**
authored/reference differences were retained. For all **192** animated vectors,
none of the three observed surfaces matched the distinct authored later frame.
The remaining **48** valid vectors are static. Controls remain uncollected.

`reference.json` SHA-256:
`732e65695419a9f30d105b2d54cd3da1a392b9f7d2e51c89905abd116e825ae8`

The record binds 18 source/profile files. Its source-bound author, collector,
and wrapper have not been edited after generation.
