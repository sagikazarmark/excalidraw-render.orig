# 0.1.0 preparation record

This is a historical record. The release-preparation scripts, publication wrapper,
and Dagger release-artifact command described below have been removed. Releases
are now managed by the maintainer; these commands are not current instructions.

**Status: preparation verified; Dagger Cloud is green (23/23 checks).**
The release remains unpublished, pending the repository move and clean-tag run.

The maintainer requested **preparation only**, before moving the code to the
existing `sagikazarmark/excalidraw` repository. This work starts at `c5fec02`;
the working-tree runs below include uncommitted changes. No tag, repository
visibility change, registry publication, or GitHub release is part of this session.

## Decisions and implementation

- ADR-0016 defines the initially scoped release and version-scoped output
  contract, with experimental WASM. The original comprehensive conformance
  specification remains the design target; exhaustive closure is follow-up work.
- ADR-0017 makes approved representative outputs plus fresh replay authoritative.
  The existing V26 manifest and approval subject are unchanged. Source/build
  identities stay in the historical record; changed expectations still invalidate
  their approval. Generator source changes alone no longer force a successor.
- The release manifest's Dagger orchestration descriptors were refreshed
  mechanically. Its historical font aggregate remains attributed to its actual
  original commit; this update does not claim a new clean-commit aggregate.
- The unapproved ownership ledger's catalog digest was re-derived after adding
  the release-policy notice to the spec. Its 2,440 assignments/dispositions are
  unchanged: 2,424 assigned/excluded, 16 explicit gaps.
- Dagger is `v1.0.0-beta.14`, with a regenerated lockfile and thin Actions adapter.
  Dagger Cloud is authoritative while Actions is unavailable because of billing.

The priorities are also recorded on historical issues
[#30](https://github.com/sagikazarmark/excalidraw-render.orig/issues/30#issuecomment-5817230821),
[#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5817235266), and
[#86](https://github.com/sagikazarmark/excalidraw-render.orig/issues/86#issuecomment-5817235192).
None of those original completion criteria are claimed complete.

## Verification completed

- Pinned-Rust local `xtask` suite: 139 unit tests and 23 integration tests passed;
  the two additional historical-provenance regression tests subsequently passed.
- The CLI help regression passes for scene/frame `--help` and `-h`, including
  JSON report mode. The package rehearsal discovered and prompted this fix:
  help previously exited with usage status 2 instead of success.
- The public-API JPEG truncation characterization passes for SVG, PNG and PDF.
  It confirms #193 remains an exposed malformed-input correctness limitation,
  with repeatable output and no decode-failure diagnostic for the bounded fixture.
- The new experimental-WASM gate has adversarial unit tests for exact allowances,
  missing runs, stale fixes, unexpected variants, report changes, and trace drift.
- Clippy for the changed Rust tooling/CLI and `git diff --check` passed locally.
- All nine PDF viewer/structure gates passed on Dagger Cloud in **21m32s**
  ([trace](https://dagger.cloud/sagikazarmark/traces/6cce12dc10d97b0dc263bc7781ad7cd6)).
  Actual test execution took 175 seconds; the remainder included cold tool
  acquisition and release compilation.
- The SVG candidate passed both canonical viewers on Dagger Cloud in **20m54s**
  ([trace](https://dagger.cloud/sagikazarmark/traces/2d884197ca4372e2d43613646a00d985)).
  The viewer test itself took 47 seconds; cold setup and compilation dominated.
- Both review axes found no unresolved implementation findings after corrections;
  the later inventory-provenance change also passed an incremental review.

### Package/build/install rehearsal

`dagger --engine cloud api call release artifacts --output target/release-0.1.0`
passed in **10m36s**:
[Dagger trace](https://dagger.cloud/sagikazarmark/traces/71914ccf908af4b31b448ba84f06c3e7).

All five crates passed Cargo's package build verification and archive inspection.
Approximate compressed sizes: asset shards 8.0 MiB and 4.5 MiB, base catalog
911 KiB, library 1.4 MiB, CLI 58 KiB; each is below the default 10 MiB limit.
The unpacked CLI installed successfully against the unpacked sibling crates and
rendered SVG/PNG/PDF; help and frame listing passed. Exported SHA-256 checksums
were independently rechecked locally. The build used Rust 1.97.1 and glibc 2.41.

The exported bundle was a **working-tree rehearsal** (`publishable_source: false`).
The then-current publication guard correctly rejected it as final release evidence.

## Completed full CI

`DAGGER_X_RELEASE=v1.0.0-beta.14 dagger --engine cloud check --progress plain`
passed all **23 checks** in **41m53s**:
[verified Cloud trace](https://dagger.cloud/sagikazarmark/traces/edd0afb9ff9614e46df005b09d214aaf).
The Cloud trace was independently queried with `dagger trace` and reports
`PASSED` / `23 passed`.

- 1,420 fast-tier tests passed, with 13 intentionally skipped tests covered by
  their separate harness/qualification selections where applicable.
- All 21 render-tier tests passed; their actual execution took about 14 seconds.
- Native release qualification passed 474 tests (five ignored).
- All ten packaged-facade tests passed on both native and browser WASM.
- Native snapshot replay, experimental-WASM replay, joined artifact/report/trace
  comparison, catalog/evidence/ownership checks, formatting, clippy, doctests,
  and generated-workflow freshness all passed.

Full CI is still substantial. The common test compilation took about 21 minutes,
and the exhaustive unsupported field/value test took 1,098 seconds. The unit-only
development command in CONTRIBUTING avoids that integration sweep. The cache
change removes unnecessary serialization; deeper compile/test optimization is
follow-up work rather than a claim that this full suite is now a short loop.

Early Cloud runs were interrupted as fixes landed, and one acquisition failed
because Cloud had no spare engine capacity. Those interrupted summaries are not
counted as green CI. The completed run above is the authority. Subsequent edits
only record results and clarify development commands; no renderer, test, or
verification logic changed after that run's source snapshot.

## Packaged-consumer snapshot diagnosis

The first completed all-checks run after removing exclusive Cargo-cache mounts
finished in 43m01s: 21 checks passed, and the native/WASM packaged-facade checks
both exposed the same three stale artifact expectations. All other checks,
including native baseline replay and the scoped full-target comparison, passed.

The original ten-test packaged consumer is a fast red-capable reproduction
after compilation (about 0.05 seconds locally). Packaging the untouched starting
commit `c5fec02` reproduces the same three failures and actual hashes, ruling out
the release-preparation changes. Rendering through the workspace CLI produces
the same values despite the consumer's separate dependency lock. All reports and
the other artifact expectations remain unchanged.

Clean historical builds distinguish stale expectations from new renderer defects:

| Control | Historical expectation | Current expectation | Evidence |
| --- | --- | --- | --- |
| Noisy magnified PNG | `865cb135…23800` | `46501d85…dcea7` | `629930b` reproduces the old value; subsequent canvas four-bit bilinear sampling/truncation (`c9fc43d`) predates this preparation |
| Source-image PDF | `aaffc925…65e1b0f` | `121f3a72…a39ed4` | `629930b` reproduces the old value; `c9fc43d` explicitly corrected image orientation, interpolation, and ICCBased references |
| Turbulence PDF | `ad152568…5d528` | `0048833b…d4c1a` | The parent of `9712833` reproduces the old value; shared raster-closure planning changed it, and `629930b` already produces the current value |

Only those three companion test hashes were refreshed. The existing assertions
continue to require exact artifacts and reports on both targets; no new tolerance
or experimental-target exception was added. All ten tests then passed locally.
Historical Git-archive comparisons explicitly cleaned workspace build artifacts
between revisions, because restoring older source mtimes can otherwise reuse a
stale Cargo build. Diagnostic checkouts and comparison outputs are isolated under
`/tmp/opencode`, not part of the deliverable.

Updating the companion test's source links changes four metadata hashes in the
derived inventory, with every execution key and option unchanged. Approved native
and PDF-judgment records now retain that inventory descriptor as historical
provenance while still rechecking all current semantic keys, outputs, reports and
judgments. New, unapproved records must bind the current inventory. Regression
tests reject changed inputs and tampered approval-bound provenance. The original
approved baseline, PDF judgment record, and V26 evidence remain untouched.

## Native/WASM collection and scoped comparison

The fresh `full-results` collection completed in **40m55s** on Dagger Cloud
([trace](https://dagger.cloud/sagikazarmark/traces/55516242ed8c124d7209abf0cc7a5966)).
All 168 keys ran twice in forward and reverse processes on both targets. The
packaged/test-build join has zero disagreements. Reports and within-target
repeats agree; four PNG artifact pairs and three trace pairs differ across targets,
exactly the #178 case set. The strict comparison correctly still reports failure.

`tools/ci/release-results.py` passes both the joined collection and WASM snapshot
comparison with the exact measured pairs in
`tests/package-consumer/experimental-wasm.json`. This is the scoped experimental
0.1.0 gate, not a claim of strict equality. The legacy-stroke PNG measurements
use the **current approved native baseline** (21,988 bytes) and its fresh WASM
counterpart (21,987 bytes), rather than the outdated pair in the original issue
body. No native expectation was changed by this preparation.

## After the move

Release preparation and publication are [maintainer-managed](../release.md).
