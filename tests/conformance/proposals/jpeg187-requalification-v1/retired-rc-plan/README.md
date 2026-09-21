# #187 bounded capture infrastructure — awaiting final producer freeze

Current selection/readiness: [FINAL-PLAN.md](FINAL-PLAN.md). The parent selected
the **minimal** consumer lock (`500a9547…`); the aligned alternative below is
preserved exploratory history and is not the final capture input. Bridge v3 is
versioned but uncollected. Explicit parent freeze is still required.

This directory contains unapproved diagnostic/comparison evidence. It is **not**
a baseline update or final approval packet, and no approval subject is proposed
until the parent freezes final production/tests/tooling and final C is captured.
No production, workspace Cargo files, live ledger/registry, xtask or approved
evidence was edited by this collector work.

## Completed old capture

Detached worktree: `/tmp/opencode/jpeg187-old-4e5b624`, exactly commit `4e5b624`.
All tracked bytes were recorded before packaging and checked afterward.

`old/snapshots/` contains real packaged-public-facade outputs for all **168 full
inventory keys**: two fresh renderer sessions in each of two forward/reverse
worker processes (672 measurements), complete artifact/failure/report blobs and
all four packaged crate archives. The existing collector's two request-mapping
tests and normal `conformance verify-native-snapshots` passed. Source status is
recorded honestly; no baseline approval is inferred from this comparison capture.

`collect.py verify` independently checks input identities, package archives,
worker/manifest agreement, process/session repeatability, key completeness,
blob lengths/digests and the complete public report shape. Eight isolated negative
tests cover mutated blobs/packages/inputs, omitted keys, reused processes,
optimized startup, and refusal of incomplete or unauthorized final freezes.

## Package graph and proposed consumer locks

The source-bound exploratory current tree is `/tmp/opencode/jpeg187-current-probe`.
It is a copy of the parent's current bytes at staging time, not final C. Both old
and exploratory current trees successfully ran:

```sh
cargo package --locked --workspace --exclude xtask \
  --exclude excalidraw-render-compare --no-verify --allow-dirty
```

**No prerelease nested-version conflict prevented packaging.** Current unpacked
renderer metadata resolves direct zune-jpeg `0.5.16-rc2` / zune-core `0.5.3`,
image `0.25.8`, its moxcms `0.7.11`, and image's separate zune-jpeg `0.4.21` /
zune-core `0.4.12`. Image's GIF edge uses `0.13.3`; `0.14.2` loses its color_quant
feature edge. Production moxcms remains `0.9.0`.

Two isolated proposals are preserved (neither applied to the repository):

1. `proposed/package-consumer-Cargo.lock` and `package-consumer-lock.diff`: minimum
   RC/image resolution. SHA-256
   `500a95474dd043905056db92324f3f1cc8d8e9b908aa7de8d6e9d8b0d39d63ca`.
   This retains pre-existing consumer/workspace drift in bytemuck/proc-macro2/quote.
2. **Recommended for graph-equivalent comparison:**
   `proposed/package-consumer-aligned.Cargo.lock` and
   `package-consumer-aligned.diff`, SHA-256
   `eb6759841b138d329aefe50bb3da5cf73fb3ce5a0072dc2baebfe8f50d70dfc0`.
   In addition to the selected JPEG graph, it
   restores bytemuck `1.25.1`, proc-macro2 `1.0.106`, quote `1.0.46` to match the
   workspace's existing selections. The complete renderer normal/build dependency
   package-version set then matches the workspace exactly. Both feature inventories
   are recorded; workspace dev feature unification is not falsely called identical.
   The aligned packaged collector builds and its two tests pass with `--locked`.

The three extra alignment changes remove old consumer drift, not an RC resolver
requirement. Their exact diff needs parent review before adopting the aligned lock.
They do not change the root lock. The proposal is technically ready to apply once
the parent accepts that scope and rechecks it against final frozen root graph.
No broad dependency update was used.

## Nested SVG/PDF controls

`nested.py` authors ten controls outside production:

- Raw-RGB JPEG, CMYK, progressive YCCK and noninterleaved baseline YCbCr 4×2 /
  YCCK 2×4, taken from the frozen independent JPEG vectors.
- Independently authored RGB8, asymmetric RGBA8 and RGB16 PNG.
- Independently authored four-color palette GIF.
- Partial-alpha PNG within an explicit Gaussian-blur fallback closure.

For HEAD and exploratory current, all controls capture SVG/PNG/PDF twice in fresh
CLI processes, exact full CLI reports, qpdf decoded object graphs, and two fresh
PDFium/PDF.js rasters each. The qualified path follows page content `Do` → Form
resources/content `Do` → actual image and attached mask. It checks image sample
extents, media geometry, bit depth, actual catalog OutputIntent ICC profile,
alpha plane association and exact expected diagnostics. The filtered control
retains the same `pdf.raster-fallback` report in both producers.

The independent comparator reuses the unchanged production comparison crate.
For JPEG, the sample/color oracle is the frozen browser SVG raster. PNG/GIF
controls use their independently authored RGBA samples. Reports include canonical
pass/fail, changed-pixel counts and exact SSIM bits; no tolerances are added.

### Observed results (exploratory, must be rechecked after final freeze)

- RGB JPEG and all PNG/GIF/filter artifacts/reports/viewer rasters are byte-identical
  between HEAD and exploratory graph.
- Four-component/sampling JPEG SVG bytes remain exact. Corrected PNG/PDF output
  changes, while diagnostics remain empty; CLI artifact hashes/lengths correctly
  change in full reports.
- All five exploratory JPEG controls pass canonical comparison for bound PDF
  samples and **both viewers**. Old CMYK/YCCK ramps and both noninterleaved controls
  reproduce the known failures; new noninterleaved controls have zero canonical
  changed pixels.
- RGB8, RGB16 and palette GIF samples/viewers are exact against independent input.
- **Preserved pre-existing failure:** asymmetric RGBA8 PDFium differs from the
  independent alpha oracle by 24 pixels, SSIM bits `3febf9303648d4c8`, under both
  HEAD and exploratory current. PDF sample/mask bytes and PDF.js are exact, and
  old/new PDFium bytes are identical. This is not a new-graph regression or a
  claimed passing viewer qualification. No production fix is made here.
- The filtered alpha control proves old/new equality and bounded fallback
  semantics, not an independently authored blur oracle.

The observed nested JPEG samples show the renderer's nested-resource normalization
also changes with the direct decoder; do not attribute their correction solely
to image's old JPEG decoder. The downstream svg2pdf image branch is exercised by
the resulting actual Form image XObjects. No claim is made that the direct
four-component JPEG reaches image::load unchanged.

Evidence: `nested/{old,exploratory}/collection.json`, all per-case captures/models,
`nested/comparison.exploratory.json`, and `nested/qualification.exploratory.json`.
The exact collector version used for those captures is archived and authenticated
as `nested/collector-at-capture.py.txt`; later driver additions provide final-label
qualification without relabeling the exploratory collection. `file-inventory.json`
lists the entire current unapproved packet, including raw evidence and logs.
`old/build-and-package-proof.json` and the corresponding exploratory-current file
bind the actual Cargo release profile/executable and every authored archive payload
to its exact staged source bytes.

## Commands and next final capture

```sh
P=tests/conformance/proposals/jpeg187-requalification-v1
python3 "$P/collect.py" verify "$P/old/snapshots"
PYTHONDONTWRITEBYTECODE=1 python3 "$P/test_collect.py"
```

Generation commands already completed; they refuse overwriting evidence:
`collect.py old`, `stage-current`, `resolve-consumer`, `align-consumer`;
`nested.py author`, `old`, `exploratory`, `compare`, `qualify`.

Final **168-key C collection has deliberately not run**. After parent freeze:

1. Stage a fresh isolated final source tree with exact parent-frozen bytes and
   the reviewed aligned consumer lock. Bind all current production sources,
   inventory, registry, helper, new tooling and dependency graph. A freeze JSON
   must explicitly contain `final_current_capture_authorized: true` and complete
   `producer_files` path/hash mappings; the collector rejects missing required
   bindings and parent/stage drift.
2. Run the existing package collector once for final C:

```sh
python3 "$P/collect.py" final /tmp/opencode/FINAL_STAGE /tmp/opencode/FINAL_FREEZE.json
python3 "$P/collect.py" compare "$P/old/snapshots" "$P/final-current/snapshots"
python3 "$P/nested.py" final /tmp/opencode/FINAL_STAGE final-current /tmp/opencode/FINAL_FREEZE.json
python3 "$P/nested.py" qualify final-current
python3 "$P/nested.py" compare final-current
```

3. Compare all 168 **full keys**, artifact/failure blobs and complete public
   reports. Itemize any changed keys; unexpected changes block requalification.
   Keep captures unapproved and do not call baseline staging/approval commands.
4. Reuse old nested captures; remeasure the final nested raster/sample results
   against the same frozen oracles. The label-specific `qualify` and `compare`
   commands write fresh final result files without overwriting exploratory history.
5. Parent owns final V9/bridge-v3 and ledger approval subject. These outputs feed
   that pending packet; this work neither edits its adapters nor assigns owners.

Original baseline/#178 approval remains deferred. This infrastructure does not
grant end-to-end CMS OOM, WASM equality or global release closure.
