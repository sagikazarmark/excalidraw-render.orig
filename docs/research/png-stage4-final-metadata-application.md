# Stage 4 final approved metadata application — 2026-09-20

The user explicitly approved **“Approve updates and finish”** for exact batch
`sha256:3074ae5df1aa0c91743c68f4217f066370698f3d8970bac02dec5b1aacc00e84`.
The parent confirmed its 116 packet files and 261 live bindings before application.
This unit applied that reviewed metadata, resolved both affected parity targets,
and left production, reference images, bound adapters and v7 evidence unchanged.

## Genuine approvals

Duplicate searches found no existing matching batch/case decision comments.
Authenticated `sagikazarmark` recorded the user's decision on 2026-09-20:

| Scope | Actual approval comment |
| --- | --- |
| Exact batch and enumerated migration | [#85 / 5751047975](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5751047975) |
| embeddable-links | [#103 / 5751048156](https://github.com/sagikazarmark/excalidraw-render.orig/issues/103#issuecomment-5751048156) |
| text-line-endings withdrawal | [#110 / 5751048361](https://github.com/sagikazarmark/excalidraw-render.orig/issues/110#issuecomment-5751048361) |
| emoji-fallback | [#121 / 5751048564](https://github.com/sagikazarmark/excalidraw-render.orig/issues/121#issuecomment-5751048564) |
| bidi-text, bidi-alignment, text-alignment-vertical withdrawal | [#152 / 5751048754](https://github.com/sagikazarmark/excalidraw-render.orig/issues/152#issuecomment-5751048754) |

Comments explicitly record the user-granted scoped decision, not autonomous agent
approval. The normal `conformance record-approval` command recorded all six case
approvals with their actual registered issue URLs and verified the subjects.
The fetched API records and four-field approvals are retained in
`tests/conformance/coverage/stage4-six-case-application/`.

## Applied cases and exact subjects

All changes are `default/png/whole`. Scene, official artifact/raster, provenance,
options, expected reports, SVG geometry and unrelated manifest fields are
unchanged. Only reviewed exceptions, case approval metadata and digest sidecars
changed; the central case registry carries the same exact exception changes.

| Case (`focused.<name>.v1`) | Applied limit/action | Canonical case subject SHA-256 |
| --- | --- | --- |
| embeddable-links | 410 / `3fefe50f5a0d41c1` | `c2f3d3e8499d5558487847915a9fdd963765225694bb0b711f8be525f5ce1001` |
| emoji-fallback | 1184 / `3fec457e1caa29ce` | `95d5200586291ee3dd68854ce3a456e19f43707d884dc6cb7b604c91ec80ca45` |
| bidi-text | 2375 / `3fec7b0fe0040f08` | `9fc60fe6485a8af4d2b7754b23d7acb3dc95b3bbe30fe2365d6a078302f2184f` |
| bidi-alignment | 1903 / `3feac3b632cb3ae3` | `5f4c10af4dc0fa04ee268a16163fc4e66e0ac7aa2a13d0c23b7fbcaa0d0446a5` |
| text-line-endings | Withdrawn; 9 pixels / 0.9990016606868246 canonical pass | `2faeac908c14ee1c12e0357bcf2d29c26f107d0f92aa5e6571b41590ba66ddc1` |
| text-alignment-vertical | Withdrawn; 20 pixels / 0.9993888979077394 canonical pass | `f9f652a17ea64b4db5615c58f1d83eca7ce0e40b55c30b43a5c5cd6bc5adc972` |

The four replacement reasons describe the actual approved per-glyph device-phase
rule and retain the existing non-defect causes. The separate #168 SVG limitation
pin remains **1190 / `3fea564b9c97ab4f`**. Global budgets are unchanged.

## Exact migration and final counts

Only the unowned whole-PNG exception obligations for text-line-endings and
text-alignment-vertical were retired. All remaining obligation identities,
criteria, pointers, candidate selections, owner commands/rationales, test-source
bindings and all three approved exclusions were preserved.

The actual approved manifests—not pending previews—drove the normal inventory
and catalog derivation. The reviewed transition updates exactly **122 authority
bindings, 12 case-owner manifest bindings and 18 manifest-derived input digests**.
All 168 semantic execution identities/options/reference artifacts survive;
the other 150 full execution records are byte-for-byte equivalent as JSON.

| Measure | Final value |
| --- | ---: |
| Obligations / assignments | **1,874** |
| Primary owners | 1,800 |
| Approved exclusions | 3 |
| Assigned/excluded | **1,803** |
| Explicit gaps | **71** |
| Manifest tolerance-exception obligations | 22 |
| New Stage-4 owners | 60 |
| Original semantic execution identities | 168 |
| Supplemental approved v7 keys | 3 |

| Identity | Final SHA-256 |
| --- | --- |
| Catalog canonical digest | `6ff4b1b746aa45d07874e5735eececdada2775bb535d75960d651e0afe2cf92d` |
| `tests/conformance/coverage/inventory.json` | `5d4c81d5cc17b5153fe5b0a6784c0e2499a947c77eb32d600c6a65e6a7130faa` |
| `tests/conformance/coverage/ownership.json` | `e8880dd039d49c6a69bf08b82ab0b7d3727d14b2ae78db333193bd9a1bdc5a38` |
| Applied transition record | `cab91b2b67063c9795bf23dba08d467f917883be7665014b150fab99b3087892` |

The applied transition record is
`tests/conformance/coverage/stage4-six-case-application/applied-transition.json`.
It records every allowed before/after metadata binding. Historical reconciliation
and proposal records are preserved; the unbound preservation audit authenticates
this transition before using archived before bytes for historical checks.

## Checks: combined evidence, no redundant full run

The parent reported its completed full workspace run passed all other targets,
including **fast 572**, library, xtask and approved representative execution.
Only the two parity targets failed on the six stale exception records.

After this metadata application, the exact affected-target command passed:

```sh
export PATH=/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_RELEASE_DEBUG=0
cargo test --locked -p excalidraw-render \
  --test parity_registered_case_png --test parity_registered_case_svg -- --nocapture
```

- `parity_registered_case_png`: **10 passed**, including every registered scene's
  official direct-PNG comparison.
- `parity_registered_case_svg`: **8 passed**, including every registered scene's
  SVG geometry/raster comparison and exact limitation/compatibility pins.
- All six normal approved case verifications passed.
- Normal inventory and ownership verification passed at the final counts.
- Applied-state preservation and live owner/requirement source audits passed
  with no stale bindings.
- V7 approved consumer and registry each returned exactly three receipts, two
  fresh release processes per format. Strict representative-owner verification
  passed. Its approved canonical subject remains:
  `sha256:c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05`.
- Reviewed batch files and all non-metadata pre-existing files remained unchanged;
  `git diff --check` passed.

This combines the parent's completed workspace run with successful reruns of
the only affected targets. This unit did **not** rerun the full workspace.
Execution logs are at `target/stage4-six-case-application/`.

## Remaining blockers and handoff

The final Stage-4 metadata application is complete; the two reported test failures
are resolved. Work is ready for the parent's final inspection and commit.

Still explicit and outside these scoped approvals:

1. Native baseline refresh/reapproval and #178 native/WASM equality work. The
   baseline still binds an older inventory; its files/approval were not refreshed.
2. Global ownership-ledger approval and release closure, with 71 explicit gaps
   and the draft inventory's eight unresolved milestones.
3. The pre-existing fixed-size moxcms execution-table allocation/resource gap;
   this work does not establish end-to-end CMS OOM closure.

No commit was made, and no new production or adapter work was opened.
