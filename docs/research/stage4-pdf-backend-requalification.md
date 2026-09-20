# PDF backend requalification — blocked on asymmetric PDF.js colour control

Subsequent bounded diagnosis identified a **pre-existing producer defect**, not
a tiny-image viewer limitation: [image colour diagnosis and narrow pin proposal](stage4-image-colour-diagnosis.md).
The original failed controls below remain preserved; their colour gate was not
changed to pass.

**No approval subject was issued.** The protected project scene's artifact,
structural and repeated viewer evidence is collected, and the exact proposed
replacement structural hashes are retained. An additional independent source
control failed; it was not relaxed or replaced with a passing fixture.

Evidence directory:
`tests/conformance/candidates/stage4-pdf-backend-requalification-v1/`.
Auditable blocked record: `blocked-proposal.json`, SHA-256
`b94d47b1bb48e526700f05983e107b8b2781583312a8f1324da06e85baaea538`.
It has `approval: null` and `approval_subject_sha256: null`.

## Exact protected baseline reproduced

The old worktree registration was prunable. A new detached worktree was created
at `/tmp/opencode/stage4-pdf-baseline-requalification-629930b`, commit
`629930b4ce4e8d09d4f44a42c495278aab0ead15`. Its source working tree was clean
before and after building/rendering. The main working tree was not used to build
the old executable and no production source was modified.

Build: Rust/Cargo 1.97.1, `--release --locked`, debug0, optimization 3, LTO,
one codegen unit, panic abort, no incremental compilation or extra rustflags.
The old executable hashes to
`954bc7d116941db79adaa6177c55f392769532219c7a4babd607d63f8b64f190`.
Two fresh processes produced the exact protected PDF and identical reports; the
collector independently repeated that reproduction using the old executable.

| Artifact | SHA-256 |
| --- | --- |
| Exact protected old PDF | `b053159dc6ea0649e6c0fa5840196ccb81db6b41b7eb69d0181392851017ee43` |
| Current release PDF, repeated | `4cb446a2555072f98fe714ab2c2808e1ad0211b876e5c7ced5a95200261b7d06` |

The current CLI report matches the baseline report except its artifact descriptor.
Artifact bytes and full reports repeat for both implementations and are retained.

## Pinned tools and structural result

qpdf was absent and restored from its unchanged pinned release archive:
`44f2c53bf784c0143128d80d2b9946e9793962c5bb403b75c0024cb4d8e346b9`.
The verified archive's Unix library symlinks were restored exactly (Python ZIP
extraction initially stored them as text). qpdf reports 12.3.2. Its executable
hash is `bfc1708204fd1ae0c7b49e7cda737e35e89e509286974d1729366f1a58d697b3`;
the loaded libqpdf hash is
`8c3a493fc59b65ad1c332a08c4d0470bc03eb89faa9dd444c102206e659fcaa3`.
The existing approved qpdf/PDFium/PDF.js profile validation passed. The profile
files and harness implementations were not changed; their identities and actual
tool binaries are retained in the blocked record.

Both old and current PDFs were inspected twice with the unchanged pinned qpdf
harness. Repeated reports are identical. The old PDF passes the protected
expectation. The current PDF fails exactly these two old digest checks:

| Check | Protected value | Proposed exact value |
| --- | --- | --- |
| `operation_sha256` | `678bb667b05d6ec652516f202d4e9b17e7960fb0f52b8f030b93de62e201fb6e` | `ebf933b7a43e6d53730578e44562484e2aa04eb4cc0d47af7c876625e054de16` |
| `inventory_sha256` | `e33789145c274e9363302378e4d76b2bc3aed2643b1c456a758abf6389ad6b64` | `4a981dfce4603756ec561dc8afaf0a1d5d14d2e86146763635d27a76429baca8` |

`proposed-structure.json` changes only those two values. The current PDF passes
that **separate proposed** expectation. The protected expectation is untouched.

Exact document/page differences are six JSON leaf values:

1. Both document file IDs change with the output bytes.
2. RGB source image and attached mask `interpolate` change from false to true.
3. Operation 377, `cm`, changes from `[40,0,0,40,0,0]` to
   `[40,0,0,-40,0,40]` (two operands).

Normalizing only those fields makes every document and page field identical.
Thus all other geometry, text operations, embedded fonts/mappings, vector
operations, clips, forms, image/mask samples, annotations and metadata are
unchanged. The 602-operation count and every other protected structural
assertion pass. A focused mutation test rejects unrelated font, text, mask-data
and XMP changes; xtask Clippy also passed.

## Protected scene viewer and source-image ROI

Both old/current project PDFs were rendered twice by each unchanged pinned
viewer at 520×200. For **both PDFium and PDF.js**, old and current rasters are
byte-identical: zero changed pixels, SSIM 1. Every repeat is identical too.

The scene's source image independently decodes to one opaque black gray/alpha
texel `[0,255]`. At its authored 75% opacity over white, interior device ROI
`(375,56,30,29)` is exactly `[64,64,64,255]` in both viewers. Its logical image
rectangle is unchanged. **A constant 1×1 source cannot reveal vertical inversion.**

## Blocker: independent asymmetric source control

A separate diagnostic control uses a known opaque 2×2 PNG, enlarged to 40×40:
red/green on the top row, blue/yellow on the bottom. It does not replace or ease
the protected scene. Source coordinates, PNG bytes and all produced observations
are retained.

The old PDFium result correctly demonstrates the old unit-image inversion:
blue/yellow at the top and red/green at the bottom. The collector then stopped:

```text
pdfjs approved source corner mismatch at 5,5: [0, 0, 0, 255]
```

Further observation with the unchanged current PDF and the same PDF.js profile
also failed the colour oracle:

| Device sample | Source expectation | Old PDF.js | Current PDF.js diagnostic |
| --- | --- | --- | --- |
| (5,5) | red | black | white |
| (34,5) | green | black | black |
| (5,34) | blue | white | black |
| (34,34) | yellow | black | black |

The old PDF.js control repeated; the additional current diagnostic was a single
observation and is not claimed as completed repeated qualification. This is a
colour-handling discrepancy, not proof of its root cause. It cannot be hidden
by weakening the oracle or changing the approved viewer configuration. The
protected constant-black source conceals it, so protected-scene equality alone
does not establish general asymmetric image correctness.

The parent must review whether to investigate this separate control failure
before deciding the narrow protected-pin update. No protected test canary,
structural expectation or approval was changed, and no full workspace ran.

## Reproduction and next bounded work

The updated collector requires the old executable explicitly, reproduces its
exact approved PDF before trusting it, checks unrelated structure/report changes,
binds tools before/after collection, and fails on the asymmetric colour oracle:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_RELEASE_DEBUG=0
# Use a NEW output directory; preserve v1's failed observations.
cargo run --locked -p xtask -- conformance candidate-pdf-requalify \
  NEW_REQUALIFICATION_DIRECTORY \
  target/stage4-pdf-baseline-requalification/approved.pdf \
  /tmp/opencode/stage4-pdf-baseline-requalification-629930b/target/release/excalidraw-render
```

No final candidate capture was performed. All changes in this unit are tooling,
documentation, new evidence paths and restoration of the exact pinned tools.
