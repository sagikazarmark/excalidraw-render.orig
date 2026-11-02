# Conformance verifiers and xtask

Scope: the eight commands `just conformance` runs (`justfile:94-102`), the
records they bind, and the `xtask` crate. All eight pass on macOS with
rustc 1.97.1 and node 24.21 from devenv.

## 0. Headline findings

1. **Only one renderer-output check exists in the gate, and it runs twice.**
   `verify-all` (through `conformance_candidate_gate::verify_registry`,
   `conformance_candidate_gate.rs:757`) and `verify-ownership` (through the
   representative owner, `coverage/ownership/representative.rs:189`) both call
   `conformance_candidate_native::replay` (`conformance_candidate_native.rs:349-409`),
   which runs `cargo build --release --locked -p excalidraw-render-cli` with
   LTO, one codegen unit and `panic=abort` (`:162-181`), demands `rustc 1.97.1`
   exactly (`:206`), renders one 200×120 scene with four elements
   (`candidates/stage4-frames-text-assets-v29/scene.json`) in three formats
   twice each, and requires byte identity with the committed `native.*`.
   Everything else in the gate is self-consistency or provenance checking of
   committed files.
2. **The 37 registered cases are verified three times per run**
   (`conformance::verify` from `verify-all` at `main.rs:190`, from
   `verify-inventory` at `coverage.rs:389-391`, and from `verify-ownership` at
   `coverage/ownership.rs:817-819`). The SVG and PDF viewer verifiers run
   three times (standalone and once inside each candidate-gate pass,
   `conformance_candidate_gate.rs:391-392`).
3. **The gate is not offline Rust**: the candidate gate shells out to
   `node tools/reference-oracle/scripts/stage4-viewer-assets.mjs <dir> verify`
   (`conformance_candidate.rs:33-39`).
4. **Cascade cost**: a string-token rename (`f347f6af`, zero glyphs moved)
   cascaded into 25 + 100 + 7 + 5 = 137 files across four commits, needed a
   Linux x86-64 and Docker restage, and the commit message says `main` was
   "Not yet green, pending the x86_64 Linux restage". The skera 0.9 bump
   (`efe4d414`, 8 real files) cascaded into 27 + 5 + 107 + 3 = 142 bookkeeping
   files. 132 of 845 commits (15.6%) touch `ownership.json`; 78 (9.2%) are
   titled rebind, refresh, restage or requalify.
5. **No evidence that `verify-inventory`, `verify-ownership` or the per-case
   half of `verify-all` ever caught a renderer regression.** The real drift
   that happened (#107: PDF structure expectations stale for weeks) was in
   `#[ignore]`d harness tests that nothing ran.

## 1. `verify-all`

Reads, per case (`conformance.rs:138-215`): registry, manifest and sidecar,
scene, provenance, per variant the official SVG/PNG/rgba pairs and expected
reports, the four case schemas, the reference profiles and scene surface with
sidecars; 447 files, about 69 MB (66 MB `.rgba`). The candidate gate adds the
V29 directory (52 files), the frozen V4 authoring (compared byte for byte,
`conformance_candidate_gate.rs:841-869`), the viewer profiles, the comparator
profile, two `docs/research/stage4-pdf-typography-*` files pinned by hash
(`conformance_candidate_exceptions.rs:13-14`), both viewer qualification trees,
`tools/pdf-harness/pdfium-runtime.json`, and through `producers()`
(`conformance_candidate_native.rs:61-90`) every file under the four renderer
and font crates minus tests (412 files, hashed twice).

Checks: canonical JSON and sidecar; exact key set and manifest/registry
agreement; schema validation; file descriptor digests; decoded official PNG
equals the rgba; SVG extent against raster dimensions; interior alpha;
provenance literals (container digest, node `v24.16.0`, Chromium
`149.0.7827.55`, browser args, two processes, `reference.invalid` URLs); the
profile, asset and surface digests against hard-coded constants
(`conformance.rs:14-18`). All self-consistency, schema or provenance.
Candidate gate: registry row shape; descriptors; frozen V4 bytes;
`judgment-plan.json` equals a literal; empty expected reports; recomputed
exception proposal; the five comparisons recomputed from stored rasters with
the live comparator; node asset verification; **the release build and render**;
collection receipts.

Unique catch: inconsistent hand edits of evidence files (the parity harnesses
read `manifest.json` and `scene.excalidraw` directly and never check digests).
It cannot fail because the renderer rendered the 37 cases wrongly; it never
renders them. The candidate half catches any byte change on one scene.

Requalification triggers: any output byte change on the V29 scene → a V30
through `just candidate-successor` (Linux, Docker, node, PDFium); rustc ≠
1.97.1; comparator profile or the two research files changing. Dependency
bumps that do not change bytes do not trigger it (ADR-0017/0019 made producer
digests historical).

Runtime: 8.6 s with the release CLI cached; the CI Conformance job took 8m39s,
almost all of it builds.

## 2. `verify-inventory`

Reads `coverage/inventory.json` (189 KB), `requirements.json`, the registry,
the scene surface (370 KB), the font catalog, every manifest, scene and
reference artifact, then all 37 cases again. `derive_inventory()`
(`coverage.rs:166-366`) rebuilds the inventory and requires byte equality: 320
coverage rows, 168 executions, each with
`inputs_sha256 = sha256(canonical{manifest, scene, profile, catalog, options, export})`
(`:286-287`). Pure self-consistency; zero renderer involvement.

Any change to `crates/excalidraw-fonts/catalog/revision-1.json` (which embeds
the subset profile digest) rewrites all 168 `inputs_sha256` (336 lines in
both `f347f6af` and `f12a5843`), which re-keys `pdf-judgments.json` (56 keys)
and the native baseline (168 keys, Linux-only restage). 0.56 s.

## 3. `verify-ownership`

Reads `ownership.json` (3.16 MB, 2440 assignments), `ownership-policy.json`
(378 KB), `png-profiles.json`, `docs/renderer-spec-v1.md`, the inventory
(re-derived), the registry and manifests, `pdf-judgments.json`, the native
baseline and 56 PDF report blobs, `layers.json`, the candidate registry and
V29 manifest, the representative cases and intake bundles, the test-group
manifest, and the Rust source of 197 distinct test functions. Then the 37
cases again, then the full candidate gate again.

Checks: `catalog_sha256` equals the derived catalog (inventory descriptor,
spec digest, policy, PNG profiles, surface, every manifest); one assignment
per obligation; policy `spec_anchor` text exists in the spec; each owning
`#[test] fn` resolves at the recorded module path with a canonical command
vector; case owner manifest digests; exception owners are the PNG harness
test; the `closure:pdf-judgments` record rebuilds and its 56 derived reports
equal the baseline blobs; the representative owner runs the full candidate
gate (duplicate); the three representative cases' file inventories.

Unique catch: deleting or ignoring an owning test without editing the ledger;
a tolerance exception without a ledger row. It cannot detect a test that
asserts nothing. Any spec edit rebinds `catalog_sha256` (31 of 107 spec
commits co-edit `ownership.json`). There is no reassign command
(`assign_gaps` refuses owned rows, `:975-977`); the top owner
`sdr_cicp_candidates_match_the_reference` owns 704 rows. 15.1 s.

## 4. `catalog verify`

Reads the font catalog and sidecar, incorporated and reviewed-decode records,
230 WOFF2 chunks (about 16 MB), the reference profiles, scene surface and
schema, compatibility artifacts, generated Rust, the corpus, five license
bundle copies, `Cargo.lock`, the oracle lockfile and three font `Cargo.toml`.

Checks: generated code equals `generate_*()` output (`:905-941`); the catalog
equals `expected_catalog()`; re-decodes all 230 chunks with `wuff 0.2.9` and
compares with `reviewed-decodes/revision-1.json` (`:1290-1340`); exactly one
`wuff` at 0.2.9 in the lock (`:1188-1205`); font crate versions carry
`+excalidraw.0.18.1`; corpus schema and structure (`:2446-2612`); hard-coded
digests for profile, corpus and schema (`:71-81`).

Unique catch: a forgotten `catalog materialize`; a `wuff` bump that changes
decode bytes (a real correctness risk for PDF embedding). 1.2 s.

## 5. `verify-svg-font-subset-release`

Reads `svg-font-subset-release-evidence/revision-1.json`,
`svg-font-subset-target-aggregate/revision-1.json`,
`svg-font-subset-cross-target-measurement/revision-1.json` and the live
descriptors they name. Checks the `pending_ci`/`aggregated` state pair, that
the aggregate's corpus, profile and measurement digests equal the live files
(`:391-433`), provenance URL shape, and that the retired Dagger descriptors
have the right shape (`:638-653`). Self-consistency and provenance. Any
corpus or profile change → `pending_ci` → needs a Linux and WASM CI run
(`538b72e7`, `2e2a537b`). 7 ms.

## 6. `verify-svg-font-subsets`

Reads `svg-font-subset-viewer-qualification/{revision-1.json,
chromium-report.json, pure-rust-report.json}` (2.4 MB), the corpus, schema,
subset profile and two viewer profiles. Checks canonical form, identity,
hard-coded digests (`:29-30, :281-285`), 780 cells → 662 executions present in
both reports, environment literals (Chromium 149, node v24.16.0, playwright
1.61.0, resvg 0.48.1, harfrust 0.12.0, rustc 1.97.1; `:480-508`), all seven
checks passing with two equal repeats. Any subsetter or profile change →
corpus digest → `just viewer-evidence font-subset generate` (Docker, Linux),
rewriting 2.4 MB even when "all 662 executions keep every raster byte for
byte" (`fb855f6d`, `c388d460`). 0.10 s.

## 7. `verify-svg-viewers`

Reads the rectangle SVG viewer qualification (11 files, 420 KB), two viewer
profiles, the comparator profile, `tools/svg-viewer-rust/Cargo.lock` (live
digest, `:97`). Checks canonical form, reference raster equals candidate raster,
repeat hashes, comparator report equals a literal, profile runtime literals
(resvg 0.48.1, rustc 1.97.1, …; `:126-163`). It verifies the viewer, not the
renderer. 16 ms.

## 8. `verify-pdf-viewers`

Reads the rectangle PDF viewer qualification (29 files, 2.9 MB),
`pdfium-runtime.json`, profiles, schemas, the Chromium reference raster.
Checks canonical form, comparator digest constant (`:20`), per-viewer report
literals, structure-report failing check ids, profile literals (PDFium
152.0.7947.0, PDF.js 6.1.200, qpdf 12.3.2), runtime receipts recomputed. No
renderer involvement; the "valid" fixture is a project-authored calibration
PDF. 0.10 s.

## 9. Cascade map

```
ROOTS                                     BOUND RECORDS (per-change gate in bold)
Cargo.lock: skera/HarfRust/read-fonts  ─┐
subset profile token                   ─┼─▶ svg-font-subset-profile.json ──▶ catalog/revision-1.json
                                         │        │                                   │
                                         │        ▼                                   ▼
                                         │   qualification-corpus.json      **inventory.json** 168×inputs_sha256
                                         │        │  xtask consts: catalog.rs:71,79,2508   │
                                         │        │  svg_font_subsets.rs:29,281            ├─▶ **ownership.json**.catalog_sha256
                                         │        ▼                                        ├─▶ **pdf-judgments.json** 56 keys
                                         │   **svg-font-subset-viewer-qualification/**     │        │
                                         │     (4 files, 2.4 MB, Docker Chromium, Linux)   ▼        ▼
                                         │        │                               native-baseline/ 168 keys + 187 blobs
                                         │        ▼                               (Linux x86-64 restage)
                                         │   **svg-font-subset-release-evidence** ──▶ pending_ci
                                         │        └─▶ target-aggregate (release-evidence.yaml, Linux+WASM)
                                         └─▶ renderer output bytes ──▶ **candidates/…-v{N+1}/** (52 files, Docker+node+PDFium, Linux)
                                                                       ──▶ candidates/registry.json
                                                                       ──▶ **ownership.json** representative row
                                                                       ──▶ xtask representative.rs test literal
docs/renderer-spec-v1.md (any edit) ──▶ **ownership.json**.catalog_sha256 (31/107 spec commits)
rustc version ──▶ conformance_candidate_native.rs:206, svg_viewers.rs:136, svg_font_subsets.rs:498,
                  devenv.nix, tools/svg-viewer-rust/rust-toolchain.toml, profiles, font-subset reports
tools/svg-viewer-rust/Cargo.lock ──▶ pure-rust profile ──▶ svg-viewer manifest, font-subset manifest + reports, xtask literals
comparator profile ──▶ pdf_viewers.rs:20, both viewer manifests, candidate collection (historical)
comparator deps ──▶ canonical-visual-v1.closure.json only (ADR-0005 amended)
tools/reference-oracle/**, tools/pdf-harness/pdfjs/** ──▶ nothing (historical_producer)
*.rs comments / test helpers ──▶ nothing (ADR-0017/0019)
```

Measured cascades:

| Root change | Commits | Files | Real code | Linux/Docker |
|---|---|---|---|---|
| Token rename `f347f6af` | 4 | 137 | ~6 | yes |
| skera 0.9 `efe4d414` | 5 | 150 | ~12 | yes |
| resvg 0.48 viewer `a547ddfc` | 1 | 31 | ~3 | yes |
| jsonschema 0.60.2 `3ed0e951` | 1 | 1 | 0 | no |

## 10. Ownership, inventory and closure in plain terms

- **168 keys**: 56 render targets (48 scene variants + 1 typography + 7 frame
  targets) × 3 formats. Each key's `inputs_sha256` includes the font catalog,
  so any catalog change re-keys all 168.
- **Test group** (ADR-0018): a JSON manifest listing test selectors so one
  ledger owner can name several tests. One exists.
- **Ownership ledger**: 2440 rows (118 case-label, 605 surface-field × format,
  122 diagnostic × format, 22 tolerance exceptions, 1543 codec cells, 4
  interactions, 4 closures, 22 advisory) each assigned to one owner: 2296 to
  197 test functions (one owns 704, the top eight own 1298), 119 to case
  manifests, 3 to a test group, 2 to records, 1 to the representative; 16 gaps.
- **Closure**: four meta-obligations; `verify-closure` is designed to fail
  until release.

`verify-inventory` guarantees the inventory is a deterministic function of
its inputs. `verify-ownership` guarantees every row has a disposition, every
named test exists as an unconditional `#[test]`, every case digest is current,
and the pdf-judgments record matches the baseline. Neither guarantees a test
asserts anything relevant (OWNERSHIP.md says so). Neither has caught a
renderer regression. The ledger is maintainable only by regeneration, and
regeneration of its dependents needs Linux and Docker.

## 11. Representative candidates

`candidates/` holds seven directories (265 files, 9.5 MB). Only V29 is
registered; V4 is the frozen authoring template; `stage4-pdf-colorspace-bridge-v20`
(576 KB) is referenced by nothing; `stage4-excalifont-canary` is read by no
verifier; `v2` and the bare directory are referenced only by retired `.mjs`
producers.

"Requalify the representative" is `just candidate-successor <dir>`
(`justfile:310-328`): Linux x86-64, Docker (Chromium, PDF.js), node, PDFium,
pre-built pure-Rust viewer; about 15–30 minutes plus a 52-file review. What it
verifies that the parity harnesses do not: Chromium rendering our SVG with a
CDP font audit; PDFium and PDF.js rendering our PDF against Chromium's raster
of the official SVG; the official canvas against our PNG; byte stability of
the release binary across two processes. The first three are frozen
measurements re-judged with the live comparator; only the last exercises
today's renderer, on one four-element scene.

## 12. xtask size (38,295 lines)

| Group | Lines | Inline tests | Role in the gate |
|---|---|---|---|
| per-case verifier `conformance.rs` | 2,604 | 302 | verifier (3×) |
| candidate gate, native replay, exceptions, semantics, assets | 3,556 | 958 | verifier (2×) and generation |
| inventory `coverage.rs` | 587 | 26 | verifier |
| ownership ledger (7 files) | 4,306 | 1,963 | verifier and 10 record commands |
| pdf-judgments | 1,689 | 412 | verifier and generator |
| five-layers (advisory) | 1,622 | 573 | mostly dead |
| viewer qualification verifiers (4) | 3,386 | 695 | verifiers and materialize |
| catalog and sfnt | 4,295 | 454 | verifier and import |
| snapshots, baseline, wasm, traces | 1,776 | 403 | release only |
| font-subset viewer runner | 533 | 0 | generation |
| representative intake (13 files) | 5,414 | 566 | generation |
| pdf-harness inspector | 2,038 | 14 | harness |
| demo | 793 | 50 | unrelated |
| cli and main | 1,131 | 205 | plumbing |
| `xtask/tests/*.rs` | 4,565 | — | tests |

Verifier machinery ≈14.5k lines, generation ≈12k, tests ≈11.2k.

## 13. Assessment

| Command | Verdict |
|---|---|
| `verify-all` per-case | Keep, once; drop the provenance literal checks |
| `verify-all` candidate gate | Release only, or replace with the golden snapshot |
| `verify-inventory` | Remove from the PR gate; keep as a release pre-step |
| `verify-ownership` | Release only, or reduce to "owning tests exist" |
| `catalog verify` | Keep |
| `verify-svg-font-subset-release` | Delete with the aggregate chain |
| `verify-svg-font-subsets` | Release or on demand |
| `verify-svg-viewers`, `verify-pdf-viewers` | Keep (cheap); fold literals into the profiles |

Pure-bookkeeping records (exist only to bind other records): `inventory.json`,
`ownership.json`, `ownership-policy.json`, `pdf-judgments.json`, `layers.json`,
`requirements.json`, `png-profiles.json`, the three font-subset release
records, every `.sha256` sidecar (about 60), `candidates/registry.json`, and
the recomputed layer of V29. About 4 MB and 80 files. Prunable under ADR-0022
now: `stage4-pdf-colorspace-bridge-v20`, `stage4-excalifont-canary`,
`stage4-frames-text-assets{,-v2}`, and most of `proposals/` (19.6 MB). The
evidence that protects the renderer is about 270 MB of the 330 MB tree:
`cases/`, the baseline blobs, `tests/oracle/` and the font corpus.
