# Stage-4 representative V15 and exception-owner tranche — pending approval

Two exact subjects for one #85 decision. Nothing here is approved or applied.

| Subject | SHA-256 |
| --- | --- |
| Representative V15 (`representative.stage4-frames-text-assets.v15`) | `d4a1cd3430d038c1e6d219de22e9f6ceb021eceb43d76af42a25887476b5e30e` |
| Exception-owner tranche (`exception-owner-tranche.json`) | `1c97f94181070295443759441d136c2a80279f34c9ea6886aa6710725938f733` |

## Why a successor

The approved V14 representative binds every `xtask/src` file, `Cargo.lock`,
`Cargo.toml` and the spec as producer sources. The spec edits for #154/#84
(a9d9e5e, 4713c2e, 3d5a151) and the #85 exception adapter (1a5cfae) staled
that binding, so `verify-ownership` could not pass. V15 is a
source-binding-only successor, collected by `collect.py` (the V14 recipe with a
fresh colour bridge, now `stage4-pdf-colorspace-bridge-v9`):

- Native SVG/PNG/PDF artifacts and all three reports are byte-identical to V14
  (`representative-equality.json`); so are all viewer rasters.
- Judgments and exception proposals equal V14's apart from hashes and the
  bridge record length. The three exception scopes and exact ceilings are
  unchanged: PDFium 118 px (two scopes), PDF.js 16 px.
- `candidate-verify` passes on the fresh release build.

## Exception-owner tranche

The 22 `exception:focused.*:png:whole` gaps are assigned to
`parity_registered_case_png::every_registered_scene_case_matches_its_official_direct_png`,
which holds each exception at its exact recorded ceilings and fails it once the
canonical budget passes. The adapter (xtask `exception_scope`) admits that owner
only for tolerance-exception obligations, and only when the exception is the
sole png/whole exception of an existing variant of a registered scene case with
no viewer profile. That test is the only owner such an obligation accepts.

## Dry run

With a placeholder approval, `apply.py` plus the tranche validated end to end
and was then reverted: the owning test passed, all xtask tests passed, and
`verify-ownership` reported 2418 assignments, 2343 assigned/excluded and
75 gaps (71 codec, 4 closure).

## Applying

After a genuine approval comment on #85:

    python3 tests/conformance/proposals/stage4-v15-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
