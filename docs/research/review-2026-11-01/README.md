# Testing strategy review, 2026-11-01

Review of the workspace at `c0928ea1` along one question: which parts of the
testing process protect the renderer against regressions and keep it
compatible with the pinned Excalidraw release, and which parts are machinery
whose cost is not matched by what it catches. The motivation was a month of
restage, requalify and re-pin commits after dependency bumps, and the sense
that the process had become brittle and undocumented.

Six read-only review agents each took one layer; this file is the synthesis,
and the per-layer reports carry the file references, counts and run IDs. The
numbers below were taken from the tree, `git` history and GitHub Actions run
data, and the load-bearing claims were spot-checked directly: 22 tolerance
exceptions across 20 of 37 cases; both committed provenance commits unreachable
from `main`; no branch protection on `main`; 132 of 845 commits touching
`ownership.json`.

| Report | Layer |
|---|---|
| [fast-tier.md](fast-tier.md) | The `ci-fast` tier: renderer unit and component tests, CLI, fonts, comparator, xtask tests |
| [render-tier.md](render-tier.md) | The `ci-render` tier: the two registry parity harnesses, frame export, the limits binaries |
| [conformance-verifiers.md](conformance-verifiers.md) | The eight `just conformance` commands, the evidence records they bind, the cascade map |
| [release-pipeline.md](release-pipeline.md) | `just release`: qualification, native baseline, WASM replay, traces, target evidence, the package consumer |
| [external-harnesses.md](external-harnesses.md) | The reference oracle, the PDF and SVG viewer harnesses, viewer qualification, Docker and host-tool pins |
| [fuzz-nightly-ci.md](fuzz-nightly-ci.md) | The fuzz and property families, the workflows, CI timings and the failure history |

## Diagnosis

The repository contains two systems under the name "testing".

**Tests of the renderer**: unit and component tests, the Rough.js and
number-format vectors, the codec oracle contracts, the parity harnesses
against official evidence, the determinism checks, the packaged-crate
consumer. These are mostly good and some are excellent.

**An evidence-provenance ledger**: inventory, ownership, closure, candidates,
aggregates, target evidence, viewer qualifications, trace equality, `.sha256`
sidecars and approval URLs. Its job is to prove that the first system was run
on a specific target at a specific commit, by binding digests of records to
digests of other records.

The ledger is roughly two thirds of the complexity (xtask is 38k lines, about
14.5k of them verifier machinery; `tests/conformance` is 126 MB;
`ownership.json` alone is 3.16 MB and 2440 rows). It is the source of nearly
all the churn, and it has caught no renderer regression in the history. It is
also not sound today:

- The provenance commits recorded in `native-baseline/revision-1.json`
  (`d4686803`) and `svg-font-subset-target-aggregate/revision-1.json`
  (`c5c4118c`) are not ancestors of `main`; PR #251 was rebased after the
  evidence was recorded. No verifier re-checks ancestry.
- The cross-target trace digest was stale on `main` for about two weeks
  (`f347f6af`'s own message), because the verifier compares it with an
  equally stale aggregate.
- The reference oracle, the only source of truth for what Excalidraw 0.18.1
  draws, was broken from 2026-10-14 to 2026-11-01: Dependabot moved its lock
  and bundle, and a second latent bug held a stale surface-digest constant. No
  CI job runs it.
- The PDF and SVG viewer harness gates were scheduled on 2026-09-05 to close
  #107 ("expectations drifted unnoticed") and unscheduled on 2026-10-13. The
  risk is open again.

**Platform dependence.** The CI failure history has no failure from
platform-dependent behavior, and ADR-0024 records that macOS arm64 reproduced
the Linux bytes exactly: all 1632 fast-tier tests, the render tier, the
candidate replay, the font corpus and the viewer rasters. What felt like
platform dependence is policy: `require-native` guards that enforce "recorded
on Linux x86-64", and `linux/amd64` Docker images that miss Excalidraw's
1000 ms font-subsetting deadline under ARM emulation. The renderer is portable;
the evidence process is not.

**Requalification.** 97 of the 749 commits since 2026-09-01 (13%) are restage,
requalify, re-pin or rebind commits. A string-token rename in the subset
profile (`f347f6af`, zero glyphs moved) cascaded into 137 files over four
commits, needed Linux and Docker, and left `main` red for 79 minutes. The
skera 0.9 bump touched about 150 bookkeeping files for about 12 real ones. The
proximate cause of the recent wave is that GitHub billing stopped every
workflow run from 2026-08-14 to 2026-10-12 (344 failed runs), so the
Linux-only evidence recipes could run nowhere while development continued on
macOS; the backlog surfaced as a cascade once CI returned.

## What protects the renderer today

In order of strength:

1. **Path geometry against the official SVG**
   (`parity_registered_case_svg.rs`, the `paths` dimension): exact string
   equality of every `<path d>` and the root extent on 48 variants. Any
   geometry drift, seed change, roundness error or bounds error fails with the
   offending path printed. Independent of any rasterizer; it has caught two
   defects raster comparison missed.
2. **Codec oracle contracts** (`tests/oracle/*` consumed by the `fast`
   binary): about ten thousand rows of Chromium 149 exports of official
   Excalidraw 0.18.1, compared under the comparator's budgets.
3. **Ported-algorithm vectors**: Rough.js 4.6.4, perfect-freehand 1.2.0 and
   ECMAScript number formatting, derived from the pinned upstream code.
4. **Normalization tests and the field-contract matrices**: authored encoding
   of the upstream restore semantics, judged as A/B equivalence rather than
   pins.
5. **The native baseline replay** of 168 keys and the **WASM replay**: the
   determinism promise of ADR-0002 and ADR-0006. The WASM replay found a real
   wasm32-only bug.
6. **PNG parity** on the 26 of 48 variants that pass the canonical budget.
7. `catalog verify` (reviewed decodes feed PDF font embedding), the packaging
   checks and `optional_cjk`.

What the suite is weak on: color changes (Pixelmatch flags a pixel only above
about 26 grey levels, and no textual `stroke`/`fill` comparison exists);
text-position regressions on the 22 exception-carried PNG variants, whose
one-sided ceilings leave a dead zone; anything PDF-visual (only the
unscheduled harness rasterizes a PDF); real-world and adversarial scenes (all
37 registered cases are `layer: "focused"`, although CONTEXT.md defines the
corpus as focused, real-world, adversarial and regression); and hostile input
(the fuzzers run with lowered limits, `scene_fuzz` never reaches the PNG or PDF
rasterizers, and every hostile-input bug in the history was found by hand).

## Component verdicts

| Component | What it checks | Value | Churn or cost | Verdict |
|---|---|---|---|---|
| Fast tier: behavioral and official-evidence tests (about 1100) | Renderer behavior against upstream-derived data | High | Low | Keep in the PR gate. Delete the 13 "digest of a committed file equals a literal" tautologies; de-duplicate the 230 umbrella re-invocations (about a third of the tier's wall clock, no added coverage); demote about 20 whole-artifact self-pins to a snapshot module. |
| xtask tests (292; four build an LTO release CLI with `rustc 1.97.1` to render one four-element scene) | The verifier machinery (about 98%) | Low | A 16-minute CI job compiling 52 binaries for 10 tests | Shrink with the machinery; move the node, qpdf and release-CLI tests to an opt-in profile. |
| `parity_registered_case_svg`: paths, extent, report | Exact equality against the official SVG | Highest | None | Keep. Add exact textual dimensions for `stroke`, `fill`, `opacity` and `<text>` positions. |
| `parity_registered_case_svg`: raster | Both SVGs through the workspace resvg | Medium (self-judged) | `pins_are_measured_by_the_locked_rasterizer` fails on every resvg bump (3 bumps, 0 measurement moves) | Keep; pin a harness-only rasterizer or drop the lock mirror. |
| `parity_registered_case_png` | Our PNG against the official canvas raster | High where clean (26/48) | 22/48 variants exception-carried, every one on the SSIM 0.999 floor; 15 pass the pixel budget; ADR-0014's own controls show the reference disagreeing with itself at SSIM 0.92 | Keep; derive the SSIM floor from identical-input controls (the `official-svg.rgba`/`official-png.rgba` pairs are committed and unread by this tier); two-sided bounded exceptions; a `known-divergence` group. |
| `parity_frame_export` | Seven frame targets | Medium | `rasterize_svg` uses an empty fontdb, so frame 7's text is vacuous on the SVG side | Keep, fix. |
| `limits_scene_admission` | 64 MiB and million-entry ceilings | Medium-high | None | Keep. |
| `limits_scene_scaling` | 20 s and 60 s wall-clock ceilings in debug on shared runners | Medium | The only timing-flake risk (2.6–5× margin) | Keep; make the ceiling relative or run in release. |
| `verify-all`, per-case half | Manifest, registry, file digests, schema, provenance literals | Medium | Runs three times per CI run | Keep once; drop the provenance-literal checks. |
| `verify-all`, candidate gate | The only renderer-output check in `just conformance`: one scene, byte identity, LTO build | Low-medium | Runs twice; forces a Docker and Linux successor on every intentional byte change | Release only, or replace with the golden snapshot. |
| `verify-inventory` | `inventory.json` is a function of its inputs | None for the renderer | Any font-catalog change re-keys all 168 keys | Remove from the PR gate. |
| `verify-ownership` (4.3k lines, 3.16 MB ledger) | Named tests exist; catalog digest including the spec digest; a duplicate candidate replay | None observed; one test owns 704 of 2440 rows | 132 of 845 commits | Release only, or retire the ledger. |
| `catalog verify` | Codegen freshness, 230 reviewed decodes, version metadata | High | 1.2 s | Keep. |
| `verify-svg-font-subset-release` and the aggregate records | "CI has re-run the aggregate" state machine | None beyond the per-cell pins the corpus test asserts | 37 commits; red after every subsetter touch until a Linux and WASM run | Delete. |
| `verify-svg-font-subsets` | 2.4 MB of Chromium output is self-consistent | Low | Regenerated 17 times, twice in three days with zero raster change | Release or on demand; re-key by glyph set. |
| `verify-svg-viewers`, `verify-pdf-viewers` | Viewer qualification records | Low-medium | Version literals duplicated between xtask and the profiles; PDF.js profile says 6.1.200, live is 6.2.108 | Keep (cheap); fold literals into the profiles. |
| Native baseline replay (168 keys through the packaged facade) | The primary byte-determinism gate | High | 17 s of work behind four verifier passes, a provenance schema, a Linux-only guard ADR-0024 calls unnecessary, and a `restage.yaml` round trip to update | Keep the property; replace the mechanism with a golden-bytes test updatable on any host. |
| WASM snapshot replay and `experimental-wasm.json` | Native equals WASM except four recorded PNG keys | High | 33 s plus Firefox | Keep nightly and release. |
| Trace pipeline (`stage-traces`, `trace-logs`, `full-equality`) | Test-only ledger equality across targets | None beyond artifact bytes, by ADR-0002's own statement | About 15 of 47 release minutes; edits tracked sources in a worktree | Delete. |
| Qualification corpus test and frame-batch property | 780 WOFF2 pins; batch equals standalone | High | The report is scraped from a test log | Keep as a release gate through a nextest profile. |
| Package consumer: packaging, `check-packaged-cli.sh`, `check-no-cjk.sh`, `optional_cjk.rs`, behavioral facade tests | Published crates build, size limits, features gate, native and WASM | High | Low | Keep in the PR gate. |
| Package consumer: about 22 byte pins | The renderer's own digests | Duplicates the baseline | Re-pinned 14 times in eight weeks, wrong once | Fold into the golden snapshot. |
| Reference oracle | What Excalidraw 0.18.1 exports | Essential | Never in CI; broken 2.5 weeks unnoticed; no batch mode | Keep; add a scheduled drift job; build the bundle once per run. |
| `harness_pdf_viewers` | The only external check on Krilla output | High | Docker for PDF.js; unscheduled since 2026-10-13 | Schedule nightly. |
| `harness_svg_viewers` | The rectangle in Chromium and the pinned resvg | Low-medium | Docker | Nightly with the PDF harness. |
| SVG and PDF viewer qualification | Judges render a control | Medium, stable | Small | On demand. |
| Representative candidate pipeline | Frozen viewer measurements of one scene | Medium; the live part is the byte pin | Linux, Docker, node, PDFium, about 30 minutes and a 52-file review per round | On demand; decouple from the PR gate; prune unread candidates. |
| Fuzz smokes (five families, fixed-seed xorshift, about 19 s) | Determinism and typed-error regression | Medium | Low | Keep in the PR gate. |
| Nightly fuzz expansions | The same fixed seed, 4096 cases, release profile | Low; no recorded discovery; the one real finding (2026-08-05) was lost | Five jobs of setup and compile for 1–4 minutes of fuzzing | One job, a seed from the run id, an issue on failure, a coverage-guided target. |
| `tools/ci/release-results.py` | The #178 allow-list in Python over xtask's JSON | Negative | Two languages for one check | Delete; an xtask flag. |
| CI layout (16 jobs, about 103 runner-minutes for 17 minutes of wall clock) | — | — | Every job pulls the browser closure for `cargo fmt`; `Test (join)` serves a branch protection that does not exist | Collapse to about six jobs; slim the devenv closure for CI. |

## Target state

**PR gate (`just ci`)**: `lint` (fmt, clippy, features, deny) and `msrv`;
`build` (one nextest archive); `test` shards from the archive; `conformance`
reduced to the per-case verifier run once, `catalog verify` and the viewer
record checks; `packaging` without the byte pins. The golden snapshot runs in
the render tier, so the 168-key determinism check becomes a per-change gate.

**Golden snapshot**: one test over the 168 inventory keys through the public
API, blobs content-addressed as today, updated with one environment variable
on any host. ADR-0024 established that macOS reproduces the Linux bytes; the
first macOS update is diffed against the Linux-recorded blobs before the
`require-native` guard goes. `verify-pdf-judgments` keeps reading the same
blobs.

**Release (`just release`)**: the corpus test and frame-batch property on
native and WASM, the WASM snapshot replay, `packaging` and `wasm-packaging`.
Logs are uploaded; the run URL goes in the release notes, not in the tree.

**Nightly**: one fuzz job, the WASM job, the PDF and SVG viewer harnesses.
**Weekly**: the reference oracle drift check.

**Deleted**: the trace pipeline; the font-subset target-evidence chain;
`verify-inventory` and `verify-ownership` from the PR gate;
`release-results.py`; the digest tautologies; unread candidates.

**Policy**: byte-moving dependency groups merge only between minor releases
and are restaged once.

## Upstream tracking

Moving from 0.18.1 to a newer Excalidraw is the purpose of the pinned design,
and the design helps in principle: everything derives from one closure, and
the oracle's two-process stability check with the path-exact comparison would
classify every upstream change. In practice the profile digest appears in 684
files and about 13 hard-coded source constants, `revision-1` is a path and a
type name rather than a parameter, no command regenerates all cases, and spec
§3 requires a second revision to coexist with a retained first one, for which
nothing is parameterised. The minimum before a bump is feasible: one recipe
that regenerates every case in one container session, constants read from the
profile sidecar, and a scheduled drift job so the oracle is known to work
before it is needed.

## Order of work

1. Schedule the viewer harnesses nightly and the oracle drift check weekly.
2. Collapse the CI jobs and slim the devenv closure for CI.
3. Trim `just conformance`; delete the trace pipeline and the font-subset
   release chain.
4. The golden snapshot replacing the baseline machinery, the candidate replay
   and the consumer pins.
5. Recalibrate the PNG budgets from identical-input controls; two-sided
   exceptions; the `known-divergence` group.
6. Fast-tier cleanup; the frame-export fontdb; the relative scaling ceiling.
7. The dependency-bump policy; the oracle batch recipe.

## Outcome

The seven steps landed on the branch `review/testing-strategy` the same day
(ADR-0025 records the decision). Rendering all 168 export targets on macOS
arm64 reproduced every Linux-recorded blob, which settled the golden-record
question; measuring the identical-input controls for every variant did not
support deriving the PNG SSIM floor from them, which is recorded in
[render-tier.md §9](render-tier.md) and left with ADR-0014, while the two-sided
exceptions it did motivate found eleven stale records.

Recommended and deferred, planned as tickets in
[follow-up-plan.md](follow-up-plan.md); in rough order of value:

- Exact textual dimensions in the SVG harness for `stroke`, `fill`,
  `stroke-width`, `opacity` and `<text>` positions (render-tier 3).
- A harness-only rasterizer pinned apart from the renderer's `resvg`
  (render-tier 4), or dropping `pins_are_measured_by_the_locked_rasterizer`.
- Dropping the provenance-literal checks from `verify-all`, folding the
  viewer version literals into the profiles, and resolving the PDF.js
  6.1.200/6.2.108 identity mismatch (conformance-verifiers §13,
  external-harnesses §9).
- Pruning the unread candidate directories under ADR-0022.
- Re-keying the font-subset viewer qualification by glyph set, a
  `--supersede` flag for the generators, and `revision-N` as one
  configuration value (external-harnesses §9).
- A `fuzz/` crate with cargo-fuzz targets, one nightly family under the
  default limits, and promoting the 2026-08-05 finding if it reproduces
  (fuzz-nightly-ci §10); the comparator closure test and the pdfjs lockfile
  digest.
- Demoting the remaining whole-artifact self-pins in the fast tier, folding
  the four one-test binaries into `fast`, and a characterization marker for
  known-defect pins (fast-tier 2, 5, 6).
- The node and qpdf xtask tests to an opt-in profile, and Node out of the
  base devenv closure.
