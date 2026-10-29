# skera: published malformed-name-record panic fix

Checked **2026-09-10** against the crates.io API, downloaded crate archives, and `googlefonts/fontations` via `gh`. Scope: upstream release evidence and upgrade-facing API/dependency changes; the local `=0.5.1` pin and adapter guard are supplied context.

## Findings

**The latest non-yanked stable (non-prerelease) version is `skera 0.7.0`, published 2026-09-08 at 19:45:47.714191 UTC. It contains sagikazarmark's fix. The first published version containing the fix is `0.6.0`.** crates.io reports `max_stable_version = "0.7.0"` and `yanked = false` for both releases. [Registry metadata][registry]

| Version | crates.io publication (UTC) | Published `src/name.rs` | Archive VCS commit / release-tag target |
| --- | --- | --- | --- |
| `0.5.1` | 2026-07-23 22:07:28.829190 | Unchecked `.unwrap()`; fix absent | `734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830` |
| `0.6.0` | 2026-08-12 20:56:25.433651 | Checked error return; fix present | `74915340ab076dd6edf11c5ddfd285d20459fc78` |
| `0.7.0` | 2026-09-08 19:45:47.714191 | Checked error return; fix present | `59fcc3b4833c3e5032309bf1b75e50c5cff8fda2` |

Release inclusion was verified directly in the [0.5.1][crate051], [0.6.0][crate060], and [0.7.0][crate070] registry archives, including `.cargo_vcs_info.json`; each archive's SHA-256 matched the registry checksum. Each archive's `src/name.rs` and `src/lib.rs` matched its `skera-vVERSION` tag target byte-for-byte. GitHub's comparisons also show the fix's merge commit is an ancestor of the [0.6.0 publication commit][ancestry060] and [0.7.0 publication commit][ancestry070] (`behind_by = 0`, merge base equals the fix merge). This establishes published inclusion independently of `main`.

## Exact patch and behavior

- **PR [#1996][pr]**, “[skera] name: don't panic on out-of-range name record count,” by **Márk Sági-Kazár (`sagikazarmark`)**.
- Patch commit **[`aeede194891632850e0abdd0fe297319166bd0b5`][patch]**, Git author name `Mark Sagi-Kazar`, authored **2026-08-01 20:38:13 UTC**.
- Merged by **Qunxin Liu (`qxliu76`)** on **2026-08-05 16:16:33 UTC**, merge commit **[`9a16122b4eb49b1104a4f56610a64019647797c8`][merge]**. The merge commit's author is Qunxin Liu; the patch author is sagikazarmark. [PR metadata][pr-api]

The PR explains that parsing checks the `name` table's six-byte minimum header, but does not validate that its declared record count fits the table. In `serialize_name_records`, the patch replaces:

```rust
let name_records_bytes = data.get(name.name_record_byte_range()).unwrap();
```

with:

```rust
let name_records_bytes = data
    .get(name.name_record_byte_range())
    .ok_or(SubsetError::SubsetTableError(Name::TAG))?;
```

This checks the same byte range as the supplied local adapter guard. The only changed file is `skera/src/name.rs` (86 additions, one deletion): the checked return plus a malformed-count helper and two regression tests. Both tests overwrite a font's name-record count with `0xFFFF`; one checks the table-level error, and the other checks whole-font behavior. [Exact patch][patch]

**Important semantic distinction:** the upstream fix prevents this panic, but does not promise a top-level error. The whole-font regression calls `subset_font(...).unwrap()` and asserts that the output has **no `name` table**. In published `0.7.0`, `subset()` treats this table-subsetting error as “table subsetted to empty” and returns success. Thus the upstream behavior differs from an adapter that immediately returns `SubsetTableError(name)` to its caller. [Published name code/tests][name070], [published error handling][lib070]

## Upgrade-facing API and dependencies from `0.5.1`

The published-source diff shows **no signature changes to `Plan::new`, `subset_font`, `SubsetFlags`, or the `SubsetError` variants**. `Plan::new` still takes nine arguments; `subset_font(&FontRef, &Plan)` still returns `Result<Vec<u8>, SubsetError>`. Internal GSUB glyph mapping changes from a hash map to a vector, and glyph closure switches from `loca.get_glyf(...).ok().flatten()` to `loca.get(...).and_then(|g| g.into_glyph())`. These are implementation/dependency-API adaptations, not new arguments for callers. [0.5.1 library][lib051], [0.7.0 library][lib070]

| Dependency requirement | skera `0.5.1` | skera `0.6.0` | skera `0.7.0` |
| --- | --- | --- | --- |
| `skrifa` | `^0.45.1` | `^0.46.0` | `^0.47.0` |
| `write-fonts` | `^0.51.0` | `^0.52.0` | `^0.53.0` |
| `read-fonts`, through `write-fonts` / `skrifa` | `^0.42.0` / `^0.42.1` | `^0.43.0` / `^0.43.0` | `^0.44.0` / `^0.44.0` |

Sources: skera dependency APIs for [0.5.1][deps051], [0.6.0][deps060], [0.7.0][deps070]; dependency APIs for write-fonts [0.51.0][wf051], [0.52.0][wf052], [0.53.0][wf053] and skrifa [0.45.1][sf045], [0.46.0][sf046], [0.47.0][sf047].

The public signatures use `write_fonts::read` types such as `FontRef` and `IntSet`, so crossing these pre-1.0 dependency minor versions matters for callers exchanging those types: identical-looking types from separately resolved `read-fonts` versions are distinct Rust types. Other direct dependency requirements and feature settings are unchanged: `skrifa` uses `std` with defaults disabled; `write-fonts` uses `read` with defaults disabled; skera defaults remain empty and `cli = ["clap"]`. The edition remains Rust 2021, and the registry's `rust_version` is null for all three releases (no declared skera MSRV, not evidence of an unchanged effective toolchain minimum). [Library imports/signatures][lib070], [dependency APIs][deps070], [registry metadata][registry]

## Local retirement and verification

Implemented on 2026-09-10 against starting commit
`ee95007c11111011ce9eb7a4ef419fb9e270c839`:

- Pin skera 0.7.0 with defaults disabled and write-fonts 0.53.0; retire
  `crates/skera-safe` and its package-consumer patches. Both lockfiles move
  skrifa to 0.47.0 and font-types to 0.12.5.
- **Additional API change found during compilation:** read-fonts 0.44.0
  removed traversal despite retaining an empty `experimental_traverse` feature.
  The bounded layout validator therefore retains read-fonts 0.42.1 and parses
  the same sfnt bytes separately. Subsetting uses read-fonts 0.44.0 through
  write-fonts. [Published read-fonts library][rf044-lib],
  [feature manifest][rf044-manifest], local `render/font_subset.rs`.
- Upstream's dropped malformed name table still produces the renderer's typed
  `Sfnt` error in `rewrite_names`; the existing fuzz regression and malformed
  layout validation tests pass.
- Preserve the frozen profile token until ADR-0003's single refresh, now
  explicitly extended to skera as well as ttf2woff2 and brotli.

Verification used **rustc 1.97.1** (explicit `RUSTC` path is needed because the
development shell otherwise supplies a different rustc to Cargo),
wasm-bindgen-test-runner 0.2.126 and local Chromium 151.0.7922.173.

| Check | Result |
| --- | --- |
| Native release qualification, `--features release-qualification --lib` | 357 passed, 5 intentionally ignored |
| WASM font qualification, regression/smoke, resource matrix | All four passed |
| Generated font bytes | All 977 pins unchanged; aggregate `bb4e7dcdc4793db511aa7d75e77d54bd6b739477cc6e064153f9609e02fa9283` on both targets |
| Resource traces | 1204 cells; `79ea93cdb58f7f348a14df21358be0066360fde67aa3b0cbbd7c6556bccf90fa` on both targets |
| Regenerated complete viewer inputs | SHA-256 `554673d290a4da7b115ce7d92748e18a7784a31c55bcbc97411fb1a21da5b310`, identical to the approved input manifest |
| Workspace `ci-render` | 21 passed |
| Workspace `ci-fast` before binding renewal | 827 passed; sole failure was the stale #99 source digest |
| Binding renewal verification | All 9 affected viewer/target evidence tests passed; viewer and release evidence verifiers passed |
| Doc tests | Passed (no doc tests declared) |
| Package archives and packaged CLI | Passed |
| Packaged public facade | 10/10 passed on Linux and 10/10 on WASM |
| Catalog, 37 registered cases, canonical SVG/PDF viewer evidence | Passed |
| Formatting and clippy | Passed |

The complete WASM run passed five tests and failed
`scene_fuzz_regression_corpus_in_wasm`: `cascadia-replacement-ligature` orders
the missing-timestamp diagnostic before the two missing-glyph diagnostics,
where the corpus expects it after them. The identical test and failure were
reproduced at the starting commit with skera 0.5.1, rustc 1.97.1 and the same
browser. It predates this upgrade. Logs are under
`target/svg-font-subset-target-evidence/skera-0.7.0-{native,wasm}.log`; the
temporary baseline worktree was removed after comparison.

### Follow-up: cross-target diagnostic ordering

The scene corpus failure also reproduces with checksum-verified Chromium and
ChromeDriver **149.0.7827.55**, Rust 1.97.1 and wasm-bindgen-test-runner 0.2.126.
It is a renderer defect, independent of Chromium-151 drift and the skera upgrade.
`SourceDiagnosticPosition::as_u64` packed the `usize::MAX` missing-member
sentinel as `0xffffffff00000000` on wasm32, but saturated it to `u64::MAX`
natively. Missing-glyph diagnostics use `u64::MAX`, so the missing timestamp
sorted before them only on WASM. Explicitly widening the missing-member sentinel
to `u64::MAX` restores the native ordering without changing the corpus pins.

The reduced public-facade regression
`missing_timestamp_sorts_after_render_phase_missing_glyph` uses one Cascadia
character and a missing timestamp; it failed on pinned 149 before the fix and
passes afterward. The original scene corpus and fixed-seed smoke pass natively
and on Chromium 149 and 151. Local WASM runs used
`CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16` after
the default fat-LTO build exceeded compilation timeouts; native checks used the
unmodified release profile.

With the pinned tools installed, the original reproduction command is:

```sh
PATH="$BROWSER/chrome-linux64:$PATH" \
CHROMEDRIVER="$BROWSER/chromedriver-linux64/chromedriver" \
WASM_BINDGEN_TEST_WEBDRIVER_JSON="$PWD/tests/package-consumer/webdriver.json" \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
cargo test --locked --release -p excalidraw-render --lib \
  --target wasm32-unknown-unknown wasm_tests::scene_fuzz_regression_corpus_in_wasm \
  -- --nocapture
```

The only changed viewer qualification subject field is the `font_subset.rs`
source digest, now `9682171dc186d421ec271c64e607c417b1b51470acd619fd5e4c4183e4a280f9`.
The maintainer approved renewal in the session; it is recorded on
[#99](https://github.com/sagikazarmark/excalidraw-render.orig/issues/99#issuecomment-5621836548)
with subject `sha256:f4e1eba24a6155a147730910384e025b7c07bbabf88bfbeebbcd61417596d804`.
Existing canonical viewer reports, corpus and profiles are unchanged; viewers
were not rerun. Release evidence retains its pending-CI aggregate state.

[rf044-lib]: https://github.com/googlefonts/fontations/blob/59fcc3b4833c3e5032309bf1b75e50c5cff8fda2/read-fonts/src/lib.rs
[rf044-manifest]: https://github.com/googlefonts/fontations/blob/59fcc3b4833c3e5032309bf1b75e50c5cff8fda2/read-fonts/Cargo.toml
[registry]: https://crates.io/api/v1/crates/skera
[crate051]: https://crates.io/api/v1/crates/skera/0.5.1/download
[crate060]: https://crates.io/api/v1/crates/skera/0.6.0/download
[crate070]: https://crates.io/api/v1/crates/skera/0.7.0/download
[pr]: https://github.com/googlefonts/fontations/pull/1996
[pr-api]: https://api.github.com/repos/googlefonts/fontations/pulls/1996
[patch]: https://github.com/googlefonts/fontations/commit/aeede194891632850e0abdd0fe297319166bd0b5
[merge]: https://github.com/googlefonts/fontations/commit/9a16122b4eb49b1104a4f56610a64019647797c8
[ancestry060]: https://api.github.com/repos/googlefonts/fontations/compare/9a16122b4eb49b1104a4f56610a64019647797c8...74915340ab076dd6edf11c5ddfd285d20459fc78
[ancestry070]: https://api.github.com/repos/googlefonts/fontations/compare/9a16122b4eb49b1104a4f56610a64019647797c8...59fcc3b4833c3e5032309bf1b75e50c5cff8fda2
[name070]: https://github.com/googlefonts/fontations/blob/59fcc3b4833c3e5032309bf1b75e50c5cff8fda2/skera/src/name.rs#L75-L216
[lib051]: https://github.com/googlefonts/fontations/blob/734d3e01b1b5ad2e8e5a6fc3226ba8dbe4861830/skera/src/lib.rs
[lib070]: https://github.com/googlefonts/fontations/blob/59fcc3b4833c3e5032309bf1b75e50c5cff8fda2/skera/src/lib.rs
[deps051]: https://crates.io/api/v1/crates/skera/0.5.1/dependencies
[deps060]: https://crates.io/api/v1/crates/skera/0.6.0/dependencies
[deps070]: https://crates.io/api/v1/crates/skera/0.7.0/dependencies
[wf051]: https://crates.io/api/v1/crates/write-fonts/0.51.0/dependencies
[wf052]: https://crates.io/api/v1/crates/write-fonts/0.52.0/dependencies
[wf053]: https://crates.io/api/v1/crates/write-fonts/0.53.0/dependencies
[sf045]: https://crates.io/api/v1/crates/skrifa/0.45.1/dependencies
[sf046]: https://crates.io/api/v1/crates/skrifa/0.46.0/dependencies
[sf047]: https://crates.io/api/v1/crates/skrifa/0.47.0/dependencies
