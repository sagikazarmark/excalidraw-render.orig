# Release pipeline

Scope: `just release` and its recipes, `release-evidence.yaml`,
`restage.yaml`, the `nightly.yaml` WASM job, `tools/ci/*`, and
`tests/package-consumer`. Timings are from GitHub Actions logs.

## 1. Pipeline map

`release: (qualification native) (qualification wasm) packaging wasm-packaging full-equality`
(`justfile:137`), `full-equality: (trace-logs native) (trace-logs wasm) snapshot-replay wasm-snapshot-replay xtask`
(`:200`). As executed in run 41077154714 (2026-10-30, `Target evidence` step
46m57s):

| # | Step | Runs | Target | Wall | Real test time | Compared against |
|---|---|---|---|---|---|---|
| 0 | `xtask` | `cargo build -p xtask` | any | 3m52s | – | – |
| 1 | `qualification native` | `EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1 cargo test --release -p excalidraw-render --lib`, then `materialize-svg-font-subset-target` | Linux x86-64 only | 8m43s | 497 tests, 220 s | corpus per-cell pins (`font-subset-corpus.json`, 671 KB) |
| 2 | `qualification wasm` | same under `--target wasm32-unknown-unknown wasm_tests` in headless Firefox | wasm32, any host | 10m45s | 19 tests, 358 s | same pins |
| 3 | `packaging` | `cargo package`, `check-packaged-cli.sh`, `public_facade` + `optional_cjk`, `check-no-cjk.sh` | host | 4m07s | <1 s | pins in `public_facade.rs:28-155`, `expected/rectangle.*` |
| 4 | `wasm-packaging` | the consumer tests in Firefox | wasm32 | 3m27s | <2 s | same pins |
| 5 | `trace-logs native` | `git stash create` + worktree; `stage-traces`; two `cargo test --release --lib trace_snapshot_*` in `target/trace-build` | Linux only | 9m23s | 2 × 5 s | cross-target only |
| 6 | `trace-logs wasm` | same for wasm32 | wasm32 | 5m47s | 2 × 10 s | – |
| 7 | `snapshot-replay` | `verify-inventory`; `native_snapshots -- collect` (168 keys × 2 sessions × 2 processes); `verify-native-snapshots`; `verify-native-baseline`; `replay-native-snapshots` | Linux only (`native_snapshots.rs:108`) | **17 s** | 10 s | `coverage/native-baseline/` |
| 8 | `wasm-snapshot-replay` | `stage-wasm-snapshots`; two Firefox runs; `report-wasm-snapshots`; `release-results.py snapshots` | wasm32 | **33 s** | 2 × 9 s | baseline + `experimental-wasm.json` |
| 9 | `full-equality` tail | `aggregate-traces`; `join-trace-results`; `release-results.py joined` | any | ~1 s | – | `experimental-wasm.json` traces |
| 10 | `target-evidence` only | `aggregate-svg-font-subset-targets` | any | <1 s | – | `svg-font-subset-cross-target-measurement/` |

About 43 of 47 minutes is compilation across four separate build graphs.
Assertion time is about ten minutes, six of them the WASM lib tests, about one
minute everything snapshot or trace related. `verify-native-baseline` runs
four times per release. `release-evidence.yaml` is dispatch-only with two runs
ever; `restage.yaml` has four runs, two failed. The nightly WASM job
(`nightly.yaml:50-86`) runs qualification, packaged consumers, snapshot replay
and the fuzz smoke in about 17 minutes; it does not run traces or native
replay.

## 2. Native baseline

`tests/conformance/coverage/native-baseline/`: `revision-1.json` (77 KB),
`revision-1.sha256`, `blobs/` with 187 files (3.18 MB): 166 distinct artifacts
and 21 distinct serialized reports for 168 expectations (56 SVG, 56 PNG, 56
PDF). `provenance` records commit, rustc, consumer lock and config digests,
collector digest, four package archive digests (`baseline.rs:21-37`).

**`verify-native-baseline` is record consistency only; it does not render.**
`baseline.rs:237-243` → `load()` → `validate_baseline()` (`:95-195`): schema,
digest form, keys equal the current inventory keys, every blob exists and
rehashes, no extra blobs, reports well formed, cross-checked with
`pdf-judgments.json`. Provenance descriptors are checked by form only.
`docs/ci.md:185-186` says it "rerenders the current package"; that is
inaccurate. The render is `collect-native-snapshots` (`justfile:157-164`) and
the byte comparison is `replay-native-snapshots` → `compare()`
(`baseline.rs:245-281`).

**`just snapshot-replay` as a whole is a real regression test** (packaged
facade → 672 renders → digest equality against committed expectations, plus
within-session and cross-process equality). `verify-native-baseline` alone is
bookkeeping.

Procedure to update when output intentionally changes (as in `c56fe5ed` and
`c388d460`): push the change; trigger `restage.yaml` with `baseline=true`
(Linux x86-64 runner, about eight minutes; `stage-native-baseline` hard-fails
elsewhere, `justfile:371`, `native_snapshots.rs:108`); download the artifact;
replace the directory wholesale (189 files); run `verify-native-baseline`,
`verify-pdf-judgments`, `verify-ownership`; commit; set the font-subset
release manifest back to `pending_ci`; delete the aggregate; run
`release-evidence.yaml` (35–50 minutes); download; record the aggregate.

Nineteen commits touched the baseline in seven weeks. Per
`coverage/README.md:234-263`, at least four changed zero artifact bytes
("refreshed input bindings and provenance"); ADR-0019/0024 made those
unnecessary, so the current rate is lower: three content restages in five
weeks.

**Integrity gap**: `provenance.commit` in the baseline is `d4686803…` and in
the aggregate `c5c4118c…`. Neither is an ancestor of `main`; PR #251 was
rebased after the evidence was recorded. `stage_baseline` and
`record_aggregate` check ancestry at write time (`baseline.rs:206`,
`svg_font_subset_targets.rs:514`) but the verifiers never re-check it.

## 3. WASM snapshot replay

For all 168 keys, in Firefox, two fresh sessions per key in each of two browser
launches (forward and reverse), the artifact and report digests equal the
native baseline (`wasm.rs:152-178` → `baseline::compare_results`), except the
four PNG keys in `experimental-wasm.json` whose native and WASM digests must
match the recorded pair (`release-results.py:92-99`).

Real value, demonstrated: the first replay found the ICO `usize::BITS`
underflow on wasm32 that made three keys fail only in the browser
(`docs/research/wasm-snapshot-mismatches.md:12-18`). ADR-0006 names browser
WASM a supported target and ADR-0002 scopes determinism to artifact bytes, so
"same bytes on both targets" is the promised property. The replay is 33 s; the
expense is the Firefox and geckodriver dependency and the generated-harness
plumbing (`stage-wasm-snapshots` concatenates `native_snapshots.rs` with
`wasm-snapshots.inc` into a gitignored test file, `wasm.rs:63-78`). Firefox
and geckodriver are pinned in devenv; the Firefox version is recorded, not
asserted (ADR-0024). Not run on PRs; nightly and release only.

## 4. Traces and full equality

A trace is the serialized semantic resource ledger
(`RenderSession::new_with_trace`, `test_support.rs:28-37`), 370 KB–1.8 MB per
key. `trace-logs` plus `full-equality` prove that for 168 keys × 2 samples × 2
orders × 2 targets the trace digests agree except the three #178 PNG keys, and
that the test build's artifact digests equal the packaged facade's. It proved
something once: 30 cross-target differences from `sort_unstable_by_key` ties,
fixed by a total order (`docs/research/trace-collection.md:54-80`); that fix
changed no artifact bytes, reports or limit outcomes.

The user-visible consequence of the ledger is the outcome, which the snapshot
replay already captures. ADR-0002 says the trace is held stable "while proving
nothing the equality asserts do not already prove". Gating releases on the
intermediate event sequence of a test-only seam is over-engineering; it is at
most a nightly canary for accidental nondeterminism.

Fragility: `stage_traces` (`traces.rs:25-49`) appends an `include!` to a
tracked source and writes two untracked files; the recipe protects the
checkout with `git stash create` and a detached worktree (`justfile:189-192`),
but `git stash create` captures tracked modifications only, a dirty tree makes
the worktree `HEAD` a stash commit while `aggregate()` binds the real `HEAD`
("invalid trace completion binding"), and the instrumented copy needs its own
full release build (15 minutes of the 47 for 30 s of assertions).

## 5. Qualification run

`build.rs:10-19` turns `EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1` into
`cfg(release_qualification)`, which removes `#[ignore]` from the corpus test
(`font_subset_qualification.rs:1682-1696`) and the frame-batch property
(`frame_batch_property.rs:667-672`). The corpus test checks 231 sources × 9
classes, 780 successful cells, asserting per-cell WOFF2 digests and replaying
traces, then prints one `SVG_FONT_SUBSET_TARGET_MEASUREMENT={json}` line.

`read_measurement` (`svg_font_subset_targets.rs:124-141`) reads the whole log,
requires exactly one line with the prefix, parses JSON. It depends on
`-- --nocapture` (`qualification.sh:14`) and on `pipefail`; the report's
`"status": "pass"` is hard-coded (`:119`). It works, but it is a log scrape
with a schema where a test writing a JSON file would do.

`aggregate()` (`:182-223`) requires two reports agreeing on seven fields, then
`verify_cross_target_measurement` (`:543-555`) compares with the committed
measurement and rewrites it on disagreement. `record_aggregate` (`:508-529`)
copies the aggregate and flips the manifest `pending_ci` → `aggregated`.
`verify-svg-font-subset-release` in the per-change gate accepts either state
but requires the aggregate's corpus and profile digests to match the live
files. Any corpus or profile change → `pending_ci` and delete the aggregate
(`fb855f6d`, `c388d460`), then re-record after a Linux run (`2e2a537b`,
`538b72e7`). Two full cycles in four days. What the aggregate adds beyond the
test itself: nothing on generated bytes (both targets assert the same per-cell
pins); only `resourceTracesSha256`. The manifest still carries and form-checks
descriptors of the retired Dagger runner.

Churn: 37 commits on the two record directories (2026-08-01 → 10-30, about
2.8 per week); 7 on the cross-target measurement. `f347f6af` notes the
committed trace digest "had been stale since before HarfRust 0.14": wrong on
`main` for two weeks with no gate noticing.

## 6. Package consumer

`tests/package-consumer/` is an isolated workspace depending on
`../../target/package/excalidraw-render-0.1.0` with `[patch.crates-io]` for the
font crates and `cjk` as its own feature.

- `public_facade.rs` (512 lines, 9 tests): forward and reverse equality
  against `expected/rectangle.*`; Excalifont SVG digest; frames direct versus
  lazy; frame errors; `license_bundle()` sha; unknown-property diagnostic;
  fallback scene digests; noisy-raster PNG digest; four PDF risk-set digests;
  eight PDF fallback-family digests. About 22 SHA-256 pins of the renderer's
  own output. `d9fd9734` ("pins taken in f12a5843 were measured against a
  stale package build") shows the pin is whatever the renderer printed.
- `optional_cjk.rs` (4 tests): behavioral feature-gating checks; real.
- `examples/native_snapshots.rs` (462 lines): the 168-key collector.
- `check-packaged-cli.sh`: five archives exist, each under 9.5 MB (Xiaolai
  10 MB), `cargo check` of the packaged CLI. Real.
- `check-no-cjk.sh`: `cargo tree` must not list the Xiaolai crates; runs
  `optional_cjk` without `cjk`. Real.

`git log -- tests/package-consumer/tests/public_facade.rs`: 31 commits, about
14 since 2026-09-04 are digest re-pins (about 1.75 per week). The real value is
"the five archives build and the facade, CLI and feature flags work on native
and wasm"; the byte pins duplicate the baseline (the text-scene digest is
literally the same blob as baseline key `focused.excalifont-text.v1` SVG).

## 7. Requalification cascade

### skera 0.9 / HarfRust 0.15 (PR #251)

| Commit | Time | Host | Files | What |
|---|---|---|---|---|
| `efe4d414` | 10-30 13:41 | any | 8 | dependency bump |
| `d411ac99` | 13:44 | any | 5 | RTL cluster fix; spec; ownership rebind |
| `f12a5843` | 15:03 | any | 27 | profile token; corpus, catalog, inventory (338 lines), pdf-judgments, ownership, cross-target measurement, fuzz regressions, three in-crate pins, two consumer pins. "Not yet green, pending the x86_64 Linux restage" |
| restage run 41068668866 | 15:03 | Linux runner | – | 31m55s: baseline 5m56s, candidate 9m52s, viewer qualification 29m38s |
| `c388d460` | 15:52 | artifacts | 107 | baseline replaced, V29 added (53 files), registry and owner moved, four viewer files, manifest → pending, aggregate deleted |
| `c2068b32` | 15:56 | any | V28 dir | delete superseded candidate |
| `d9fd9734` | 16:03 | any | 1 | correct the two consumer pins |
| release-evidence 41077154714 | 16:03Z | Linux runner | – | 49m22s |
| `538b72e7` | 16:56 | any | 3 | record aggregate |
| merge `776bd8c8` | 10-31 13:44 | – | – | green |

About 3h15m of work; merged about 24 hours after start; the provenance
commits in both records are now unreachable from `main`.

### Token rename (direct on `main`)

| Commit | Time | Host | Files | What |
|---|---|---|---|---|
| `f347f6af` | 10-27 21:16 | any | 25 | token; corpus; catalog; inventory; judgments; ownership; measurement (fixing a two-week-stale digest); fuzz regressions; a pin |
| merge `da99eba6` | 21:50 | – | – | **CI red**: Conformance "stale native svg artifact", Packaging (consumer pins), Test |
| restage 40581590801 | 21:50Z | Linux | – | baseline job failed (provenance checker broken since the 10-04 crate rename); viewer 34m39s and candidate 9m13s succeeded |
| `d905b604` | 22:19 | any | 1 | fix `provenance.rs` |
| `c56fe5ed` | 22:19 | artifacts | ~100 | baseline, V28, registry and owner |
| `fb855f6d` | 23:01 | any | 7 | viewer qualification, manifest → pending, aggregate deleted, two consumer pins → green |
| release-evidence 40592579157 | 22:09Z | Linux | – | 34m45s |
| `2e2a537b` | 23:48 | any | 5 | record aggregate |
| `290c3dde`, `83960b50` | 23:59, 00:06 | Linux | – | second restage failed on the SVG viewer check, fixed, reproduced |

About 2h50m; `main` red for about 1h19m across three jobs; two tooling fixes
mid-cascade; one wasted restage run.

## 8. What users need versus what is gated

Users need (a) output that matches the official renderer visually, (b)
deterministic output for a version, (c) no panic or hang on hostile input,
(d) a crate that builds on stable with and without features, (e) WASM.

| Component | a | b | c | d | e |
|---|---|---|---|---|---|
| `just conformance` with the representative replay | yes | 1 scene | – | – | – |
| viewer qualifications | yes | – | – | – | – |
| `qualification native`: corpus, frame batch | partial | yes | – | – | – |
| `qualification wasm` | – | yes | – | – | yes |
| `packaging`, `check-*.sh` | – | – | – | yes | – |
| behavioral facade tests | – | – | – | yes | yes |
| facade byte pins | – | duplicate | – | – | – |
| `snapshot-replay` | – | **yes** | – | – | – |
| `verify-native-baseline` alone | – | – | – | – | – |
| `wasm-snapshot-replay` | – | yes | yes (ICO) | – | **yes** |
| trace pipeline | – | indirect | – | – | – |
| font-subset target evidence chain | – | – | – | – | – |
| nightly fuzz, `limits_*` | – | – | yes | – | – |
| `features`, `test-features`, `msrv`, `deny` | – | – | – | yes | – |

Components serving none of the five directly: `verify-native-baseline` as a
step, the entire trace pipeline, the font-subset target-evidence layer and its
three records and state machine, the provenance fields validated by form.
Together about 2,300 of the 4,687 lines of release-pipeline code, about 15 of
the 47 CI minutes, and 44 record-only commits. Notably (a) and (c), the two
things users would complain about first, are not in `just release` at all.

## 9. Assessment

| Component | Disposition |
|---|---|
| `qualification native` (corpus, frame batch) | Keep as release gate through a nextest `release` profile |
| `qualification wasm` | Keep; already nightly |
| `packaging`, `check-*.sh`, `optional_cjk`, behavioral facade tests | Keep (per change) |
| facade byte pins | Fold into the golden snapshot; keep the `expected/rectangle.*` round trip as a smoke |
| `snapshot-replay` collect and compare | Keep the property, replace the mechanism |
| `verify-native-baseline` | Delete as a step |
| `stage-native-baseline`, `restage.yaml` baseline job | Delete; replaced by an update variable on any host |
| `wasm-snapshot-replay`, `experimental-wasm.json` | Keep nightly and release, simplified |
| trace pipeline | Delete or nightly-informational |
| `materialize-svg-font-subset-target`, `aggregate-…`, `record-…`, `verify-svg-font-subset-release`, the three records | Delete |
| `release-evidence.yaml` | Keep as `just release` plus log upload; stop recording its URL into the tree |
| `restage.yaml` viewer and candidate jobs | Keep (Docker-bound) |

**Minimal release gate**: `just ci`; then on native a nextest `release`
profile running the corpus test, the frame-batch property and a golden
snapshot test over the 168 keys through the public API; the same three under
wasm32 in Firefox with the #178 allow-list as a constant in the test;
`packaging` and `wasm-packaging`. Nightly as today plus fuzz.

**Can the baseline become a golden-bytes test?** Yes. ADR-0024 reports
`aarch64-apple-darwin` matched Linux bytes on every registered candidate, the
corpus and the viewer rasters; the Linux-only restriction protects "where
evidence was recorded", which ADR-0024 calls a non-goal. The blobs are already
content-addressed and small. An update is one environment variable on a
laptop; review is `git diff --stat` of the blob directory, which is what
`c388d460` shows reviewers today. Residual risk: macOS identity was observed on
one scene and the corpus, not on all 168 keys; the first macOS update should
be diffed against the current Linux-recorded blobs before the guard goes.
