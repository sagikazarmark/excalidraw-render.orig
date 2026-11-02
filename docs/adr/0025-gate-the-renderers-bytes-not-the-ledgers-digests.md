---
status: accepted
---

# Gate the renderer's bytes, not the ledger's digests

The 2026-11-01 testing strategy review (`docs/research/review-2026-11-01/`,
in the tree at `1ab9342a`) found two systems under the name "testing". The tests of the renderer (unit
and component tests, the Rough.js and number-format vectors, the codec oracle
contracts, the parity harnesses against official evidence, the determinism
checks, the packaged consumer) were mostly good and some excellent. Around
them had grown an evidence-provenance ledger: inventory, ownership, closure,
candidates, aggregates, target evidence, viewer qualifications, trace
equality, `.sha256` sidecars and approval URLs, whose job was to prove that
the first system had run on a specific target at a specific commit by binding
digests of records to digests of other records.

The ledger was about two thirds of the complexity (xtask 38k lines, about
14.5k of them verifier machinery; `tests/conformance` 126 MB; `ownership.json`
3.16 MB) and the source of nearly all the churn: 97 of the 749 commits since
2026-09-01 restaged, requalified, re-pinned or rebound evidence; a string
token rename in the subset profile cascaded into 137 files, needed Linux
x86-64 and Docker, and left `main` red for 79 minutes. It had caught no
renderer regression in the history, and it was not sound: the provenance
commits in two committed records were unreachable from `main`, a cross-target
trace digest was stale for two weeks, the reference oracle was broken for 2.5
weeks with no job running it, and the viewer harness gates scheduled on
2026-09-05 to close #107 had been unscheduled since 2026-10-13.

What felt like platform dependence was policy: the CI failure history had no
platform-dependent failure, and rendering all 168 export targets on macOS
arm64 reproduced every Linux-recorded byte, as ADR-0024 predicted.
`require-native` guards enforced "recorded on Linux x86-64", and `linux/amd64`
Docker images missed Excalidraw's font-subsetting deadline under ARM
emulation; the renderer itself was portable.

## Decision

**The per-change gate judges the renderer's output; records about records
are the release tier or gone.**

- The golden record (`tests/conformance/golden/native.json` with its blobs)
  holds the artifact and report bytes of every registered export target. The
  render-tier test `golden_native` derives the keys from the registry, renders
  each twice (forward and reverse, a fresh renderer each time) and holds them
  to the record on every change; `just update-golden` rewrites it on any
  host, and the review is the diff. The same test under wasm32 in Firefox
  (`just golden-wasm`) holds the browser build to the same record with the
  four recorded #178 PNG differences, which must all appear exactly as
  recorded. This replaces the native baseline, its packaged-facade collector,
  the WASM staging, the baseline provenance checker and the restage job, and
  the package consumer's own byte pins, which now reproduce the rectangle
  from the golden record.
- `just conformance` verifies the registered cases, the sealed catalog and
  the viewer qualification records. The representative candidate gate, the
  inventory, the ownership ledger and the font-subset viewer qualification
  are `just conformance-release`: they bind evidence only the release checks
  consume and re-key on every font-catalog change. The four xtask tests that
  build the release CLI are the release tier too (`just test-release`).
- The trace pipeline retired. ADR-0002 already said the trace proves nothing
  the artifact and report equality does not; it cost two further release
  builds and a source-editing worktree stage.
- The font-subset target-evidence chain retired: the log-scraped report, its
  aggregate, the recorded aggregate with a run URL, the cross-target
  measurement and the `pending_ci`/`aggregated` manifest. Both targets assert
  every corpus cell's WOFF2 digest against one committed pin, which is the
  cross-target agreement.
- Tolerance exceptions record the observed measurement exactly and the
  harness fails when it moves in either direction, as the SVG pins already
  did; eleven of twenty-two had improved beneath their one-sided ceilings
  unrecorded. The clean and the excepted PNG variants are judged by separate
  tests.
- The viewer harnesses run nightly and the reference oracle weekly, on the
  runner's Docker, and a scheduled failure opens an issue. The nightly fuzz
  expansions run in one job with a seed from the run id.
- CI runs six gates from a lean devenv base; the browser stack is the
  `browser` profile and the release tooling the `maintainer` profile.
- `require-native` remains only where the host is the point: the native
  release archive and the representative candidate, whose pinned images are
  `linux/amd64`.
- Byte-moving dependency bumps (Dependabot's `output-bytes` group) merge
  only between minor releases, and a release restages once.

## Consequences

- Artifact byte stability is a per-change gate (about three seconds for 336
  renders) rather than a release step behind a Linux-only workflow, and its
  update is one command on a laptop.
- `just conformance` runs in about a second. `just ci` has no step that
  shells out to node, builds the release CLI or needs Linux.
- A dependency bump that moves output is adopted by `just update-golden`, the
  parity harnesses' reported re-recordings, and, when the candidate's bytes
  move, a successor through `restage.yaml`; the inventory, pdf-judgments and
  ownership records follow with `conformance inventory`,
  `conformance pdf-judgments-generate` and the new
  `conformance rebind-ownership-catalog`, which does what thirty-one commits
  did by hand.
- The browser-WASM identity check is in-crate and in-process rather than
  through the packaged facade in two browser launches. The packaged aspect is
  `just wasm-packaging`; the separate-process pair had caught nothing in
  nineteen baseline revisions.
- The identical-input controls the review proposed deriving the PNG SSIM floor
  from were measured and do not support it (the data is in the review's
  render-tier report); the canonical budgets are unchanged and the question
  stays with ADR-0014.
- Records that no check reads any more (the baseline directory, the three
  font-subset release records, the trace note) are gone under ADR-0022; Git
  history keeps their bytes.
- This supersedes the "native evidence needs x86_64-unknown-linux-gnu" guard
  of ADR-0006 for the snapshot and qualification recipes, the target-evidence
  aggregate it describes, and the trace-equality property ADR-0002 held as a
  cross-target check. Their statements about artifact bytes stand.
