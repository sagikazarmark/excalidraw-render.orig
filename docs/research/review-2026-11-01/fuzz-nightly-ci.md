# Fuzz and property tests, nightly, CI wiring

Repository `sagikazarmark/excalidraw-render.orig`, default branch `main`,
**not protected** (`gh api .../branches/main/protection` → 404).

## 1. Fuzz and property inventory

All five families are hand-rolled fixed-seed xorshift generators; there is no
proptest, `arbitrary`, cargo-fuzz, libFuzzer or AFL, and no `fuzz/` directory.
`docs/asset-fuzzing.md:38` and `docs/render-fuzzing.md:39` say so.

| Family | File | Seed | PR smoke | Nightly | Profile | Properties | Panic capture | Corpus |
|---|---|---|---|---|---|---|---|---|
| scene_fuzz | `src/scene_fuzz/mod.rs` | `0xbb67_ae85_84ca_a73b` | 64 cases, 8 strategies | 4096 (`justfile:344`) | release | typed `scene.*` rejection with pointer; canonical fixpoint; normalization deterministic for three formats; render determinism **SVG only** (`mod.rs:1130-1134`) | `catch_unwind` | 22 authored cases |
| font_subset_fuzz | `src/render/font_subset_fuzz.rs` | `0x6a09_e667_f3bc_c908` | 64, 9 routes | 4096 | release | determinism; bounded sizes; typed stage only | `catch_unwind` | 12 cases + 51 layout cases |
| frame_batch_property | `src/render/frame_batch_property.rs` | `0x2545_f491_4f6c_dd1d` | **0 generated** in the PR gate (release-qualification only); 6 hand-authored scenes | 512 | release | batch equals standalone across an N−1/N/N+1 ladder | none | none |
| asset_fuzz | `tests/asset_fuzz/` | `0x3c6e_f372_fe94_f82b` | 64 + intact seeds | 4096 | release | two fresh renders equal; output within limit; PNG decodes; allowed codes only | `catch_unwind` | none |
| render_fuzz | `tests/render_fuzz/` | `0xa54f_f53a_5f1d_36f1` | 36 + 9×13 controls, 8 fixed mutations | 4096 | **test** (the only one with debug asserts) | forward then reverse replay equal; artifact inspection; 13 limit profiles | `catch_unwind` | 0 cases |

Every family uses one compile-time seed; replay is by printed reproducer, not
seed and index; no `*_FUZZ_SEED` variable exists (`release-readiness.md:95-97`
lists adding one).

**Has fuzz caught a bug?** The 22 and 12 regression entries are authored
coverage per required risk, not discoveries. The one documented finding is the
51 layout cases (#210), a differential comparison of two walkers. One real
nightly catch exists in the run history and is unrecorded: nightly
`30974954216` (2026-08-05) failed at `font_subset_fuzz.rs:318` for seed
`6a09e667f3bcc908` case 3482 on `d31399a5`, every night until billing stopped
runs on Aug 14; no commit cites it. The hostile-input bugs (`822c97dc`
strokeWidth 9.9e8 → tiny-skia panic and 2.4 GB; `391fecba`, `58d4d469`,
`d0e53ccc`, `371e84ef`, `9ea81419`) were found by hand;
`tests/fast/extreme_value_render.rs:3-4` says the fuzzers "run with lowered
limits, so they do not reach these magnitudes". Structurally, scene_fuzz
renders only SVG, the extreme tokens jump to 1e300 (caught by the coordinate
ceiling), and render_fuzz's `mutate` never touches `strokeWidth`.

PR-gate cost: about 19 s for all five smokes. Nightly: 44–219 s of fuzzing per
job behind 3–5 minutes of compile.

## 2. Panic and hang protection

`Cargo.toml:124-125` forbids `unsafe_code`; release `panic = "abort"`; the
test profile keeps unwind and debug asserts. `deny.toml`: advisories, yanked
denied, license allow-list, wildcards denied. Hostile-input tests beyond fuzz:
`extreme_value_render.rs` (7), `nested_svg_asset_render.rs`,
`limits_scene_admission.rs`, `limits_scene_scaling.rs`, the scene_fuzz nesting
test. Wall-clock assertions: `limits_scene_scaling.rs` (20 s, 20 s, 60 s;
observed 3.4–4.1 s and 22.6 s), `cluster_text_pdf.rs:68-73` 10 s,
`png_paint_mask_work.rs:168-170` 10 s, `src/render/text.rs:2060-2069` 5 s. No
flake observed. nextest sets no `slow-timeout`; a hang runs to the 45-minute
job timeout.

## 3. `ci.yaml` wiring

Every step runs under `devenv shell -- bash -e {0}`; every job checks out,
runs `setup-devenv` with the `rust-projects` Cachix cache, and (except fmt and
deny) `rust-cache` with `save-if: main`. Sixteen jobs on `ubuntu-latest`:

| Job | Recipe | Timeout | Needs |
|---|---|---|---|
| Format | `just fmt` | 15 | – |
| Clippy | `just clippy` | 30 | – |
| Dependencies | `just deny` | 15 | – |
| Clippy (features) | `just features` | 45 | – |
| Rust 1.92 | `just msrv` under the 1.92.0 toolchain | 30 | – |
| Test (build) | `just test-tools` + `just test-archive`, upload | 45 | – |
| Test (shard 1–4/4) | `just test-shard N/4` without the release-cli group | 45 | build |
| Test (release CLI) | `just test-release-cli`, not from the archive | 45 | – |
| Test (join) | loop over results; "the one check branch protection requires" (none exists) | 5 | all |
| Test (doc) | `just test-doc` | 60 | – |
| Test (features) | `check-format-features.sh` | 45 | – |
| Conformance | `just conformance` | 45 | – |
| Packaging | `just packaging` | 45 | – |

Durations (`gh run view --json jobs`):

| Job | 41329345570 | 41316987450 | 41264546534 |
|---|---|---|---|
| Test (release CLI) | 16.4 m | 9.9 m | 8.9 m |
| Test (features) | 12.9 m | 8.6 m | 9.1 m |
| Test (build) | 9.9 m | 5.4 m | 7.2 m |
| Conformance | 8.7 m | 5.2 m | 5.4 m |
| Test (doc) | 7.8 m | 2.4 m | 2.7 m |
| shards | 5.5–6.7 m | 5.2–6.5 m | 5.7–7.2 m |
| Packaging | 6.0 m | 5.9 m | 6.2 m |
| Clippy (features) | 5.1 m | 3.4 m | 3.7 m |
| Rust 1.92 / Clippy / Dependencies / Format | 3.8 / 3.5 / 2.2 / 2.1 m | | |
| **Wall clock** | **16.8 m** | 14.9 m | 14.6 m |
| **Runner-minutes** | **≈103** | ≈71 | ≈78 |

The critical path alternates between `Test (release CLI)` alone and build →
shard → join. PR runs are slower (18–28 m) because `save-if: main` never
saves rust-cache on branches. `Test (release CLI)` spent 3m56s building the
LTO CLI and 9m15s compiling all 52 test binaries to run ten tests in 69 s.
"Set up devenv" is 85–112 s in every job (490 nar downloads; the Format job
pulls the Firefox/Chromium/Node/clang closure for `cargo fmt`), about 27
runner-minutes per run.

## 4. Failure history

From 2026-08-01 to 2026-10-14 there were 400 runs and 344 failures: from
Aug 14 to Oct 12 every non-Dependabot run failed at start with "The job was
not started because recent account payments have failed or your spending
limit needs to be increased". CI did not run for about six weeks; gates ran
locally on an Apple-silicon laptop where the Dagger `linux/amd64` pin ran
under emulation (ADR-0024: a cold `lint:clippy` 34m35s). Linux-only evidence
could not be produced in that period; that is the backlog that surfaced as
requalifications after Oct 13.

`ci.yaml` failures, 2026-10-13 → 11-01 (17 runs; 10 of the last 40 = 25%):

| Class | Count | Evidence |
|---|---|---|
| (a) genuine renderer regression | **0** | — |
| (b) evidence or pin staleness | **7** | `41202588404`, `38942131464`, `38497472999`: comparator closure differs from `Cargo.lock` on Dependabot bumps; "exactly one wuff at 0.2.8"; `39997918989`: `font_subset_fuzz.rs:614` stage changed by ttf2woff2 0.14; `38473357759`, `38467607690`: stale pdfjs lockfile digest and eight GIF digests; `40581566218`: "stale native svg artifact", consumer pins, "recorded aggregate was produced for a different font subset profile", "56 derived PDF reports differ" |
| (c) infrastructure | **4** | Nix substituter `HTTP 416` while fetching Firefox's closure (onnxruntime, pipewire, systemd) for jobs that never open a browser (`41253073787`, `40651673920`, nightly `38846032678`); "No space left on device" (`41322669299`, `41249020621`) |
| (d) platform-dependent behavior | **0** | the only recorded platform deltas are the four #178 WASM PNG keys |
| (e) flaky timing | **0** | |
| (f) dependency major bump breaks compile | **4** | read-fonts 0.46 non-exhaustive match; `harfrust::GlyphId::to_u16` removed |
| (g) CI migration WIP | **1** | |

Of 749 commits since 2026-09-01, 97 (13%) are restage, requalify, re-pin or
rebind commits. `release-readiness.md:197-201`: "verification code (about 140k
lines) is larger than the renderer (about 78k). Any change that moves evidence
means restaging on x86-64 Linux."

## 5. `nightly.yaml`

Cron `17 3 * * *`; five expansion jobs plus `Browser WASM`; 90-minute
timeouts. Last 19 runs: 18 successes, one Nix 416. Wall clock 10–24 minutes;
each expansion is about 100 s devenv, 3–5 minutes compile, 44–219 s fuzzing.
Pre-devenv the nightly failed Aug 5–13 on the real finding above and Aug 14 →
Oct 12 on billing; no commit references a nightly failure. There is no
notification, issue-on-failure or Slack step. The PDF and SVG harness gates
that `f35a8e0c` scheduled nightly are no longer scheduled anywhere.

## 6. `restage.yaml` and `release-evidence.yaml`

`release-evidence.yaml`: one job, 180-minute timeout, `just target-evidence`,
upload; two runs, both green (34.8 m and 49.4 m). `restage.yaml`: inputs
`baseline`, `viewer`, `candidate`, `viewer-checks`; four runs, two failed on
bugs in the restage path itself (`40581590801`: provenance check assumed
`crates/<package>` paths since the Oct 4 rename; `40597598451`: `no such
command: +1.97.1`, a rustup call in a devenv shell). Reliable since. The Oct
27 profile rename cost three restage runs, one release-evidence run, five
commits and about 1.5 hours of runner time.

## 7. devenv

`devenv.nix` pins Rust 1.97.1 (overridable to 1.92.0 for MSRV), the wasm32
target, qpdf 12.3.2, PDFium build 7947 per host (four hashes), wasm-bindgen-cli
at the lock version, Firefox and geckodriver unpinned, Chromium Linux-only,
nextest from rolling nixpkgs (0.9.146; floor 0.9.133), Node unpinned, plus
cargo-audit, cargo-deny, cargo-dist, cargo-release, cargo-watch, wasm-pack,
lld, python3. Local closure on macOS: 3.7 GiB, 265 paths (Firefox 935 MiB,
clang 756 MiB, cctools 877 MiB).

`just ci` calls no `require-native` recipe, so it runs fully on macOS
(ADR-0024). Docker is needed only by `pdf-harness`, `svg-harness`,
`viewer-evidence`, `reference-profile`, `reference-case`,
`candidate-successor`. `require-native` gates `qualification native`,
`snapshot-replay`, `trace-logs native`, `full-equality`, `target-evidence`,
`release-archive`, `stage-native-baseline`, `candidate-successor`: everything
that produces committed evidence. Cachix is effective (nothing builds from
source); the cost is closure breadth.

## 8. `tools/ci` Python

`release-results.py` (145 lines) re-validates the WASM snapshot and joined
trace runs and allows only the four recorded #178 differences from
`experimental-wasm.json`; called from `wasm-snapshot-replay` and
`full-equality`. `test_release_results.py`: eight unittest cases as
`just test-tools`. No stated rationale for Python; it duplicates xtask's JSON
knowledge in a second language and exists only to apply a tolerance an xtask
flag could carry.

## 9. `analysis-scorecard.yaml`

OpenSSF Scorecard on push, branch-protection events and a weekly cron;
harmless, about 0.8 minutes, not a source of pain.

## 10. Assessment

**(a) Fuzz.** The PR smoke is cheap and belongs where it is, but it is
deterministic replay rather than search: one fixed seed per family, lowered
limits, scene_fuzz never reaches the rasterizers, frame_batch's generated
property runs in no PR job, three nightlies run without overflow checks, and
the one real nightly finding was lost. Every hostile-input bug was found by
hand.

**(b) Layout.** The conformance verifiers run three times per CI run
(Conformance job, xtask integration tests in the shards, xtask unit tests
re-deriving the baseline); the workspace compiles from scratch in nine jobs;
`Test (join)` serves a protection that does not exist; sixteen devenv setups
pull a browser closure for linters; `Test (release CLI)` could run from the
archive or disappear.

**(c) Cause of the pain.** Not platform-dependent tests: zero CI failures were
platform behavior and macOS bytes are identical. The chain was the billing
outage; an evidence model that pins a `Cargo.lock` subgraph, a
`package-lock.json` digest, "exactly one wuff at 0.2.8" and a profile token so
every Dependabot group tripped CI (7 of 17 failures) and every byte-moving
bump needs a Linux restage; the Dagger runner at 40× under emulation; and two
first-run bugs in the restage workflows written to unblock it.

**(d) Recommendations.**

1. Collapse to about six PR jobs: `lint` (fmt, clippy, features, deny),
   `msrv`, `build` (archive including the release CLI), `test` shards from the
   archive including the release-cli group, `features+doc`,
   `conformance+packaging`.
2. Slim the devenv closure for CI: no Firefox, Chromium, Node, cargo-dist,
   cargo-release or cargo-watch outside the nightly WASM job.
3. Delete `release-results.py` and `test-tools`; `Test (join)` or add the
   protection; the comparator closure test; the pdfjs lockfile digest.
4. Nightly: one fuzz job, a seed from `run_id`, at least one family under
   default limits in the `test` profile, the PDF and SVG harnesses back, an
   issue on failure; promote the Aug 5 finding if it reproduces.
5. A `fuzz/` crate with two or three cargo-fuzz targets seeded from the
   regression corpora and `tests/public-scenes`.
6. Byte-moving dependency groups only between minor releases.
