# 0.1.0 Release Readiness Plan

Status: in progress. Date: 2026-10-18, updated 2026-10-27. Owner: Mark Sagi-Kazar.

This plan records what stands between `main` and the first
release, and what to do about each item. The [release policy](release-0.1.0.md)
defines the gates and the [runbook](../release.md) the procedure; this
document does not change either.

The code review behind it was static: it read source, git history and CI
runs, and compiled nothing.

## Verdict

As code, `main` is a solid 0.1.0. It reads every element type Excalidraw 0.18.1
saves and checks its output against the real exporter with explicit tolerances.
The approved baseline has not moved since 2026-09-28. The workspace forbids
`unsafe`, and the renderer's own panic sites are invariants. Resource limits are
checked before decoding. The public Rust API is guarded against growth with
`#[non_exhaustive]`, has no public struct fields, and exposes no third-party
types except serde.

It is not yet proven against hostile or real-world input, and dependency bumps
still move output bytes. The release process gates are also open. The items
below close both.

## Release process gates

These are open regardless of code quality.

1. **Closed 2026-10-27.** `just release` ran on Linux x86-64 under devenv for
   the first time since #234, through `release-evidence.yaml`
   ([run 40592579157](https://github.com/sagikazarmark/excalidraw-render.orig/actions/runs/40592579157)
   on `fb855f6d`): native qualification 496 passed, 5 ignored; WASM 19
   passed; snapshot replay and full equality passed; the aggregate is
   recorded. The restage run before it found that the baseline stager's
   provenance check had been broken since the 2026-10-04 crate rename
   (`d905b604`).
2. **Closed 2026-10-27.** `just viewer-evidence svg reproduce` and
   `pdf reproduce` run the scripts against the host's Docker, and
   `restage.yaml` runs both on demand (`viewer-checks`). Both reproduced the
   committed evidence byte for byte on Linux x86-64 at `290c3dde`
   ([run 40598881895](https://github.com/sagikazarmark/excalidraw-render.orig/actions/runs/40598881895)),
   after HarfRust 0.14, the read-fonts parser and the token refresh.
3. **The repository is not renamed.** The remote is still
   `excalidraw-render.orig`, and `sagikazarmark/excalidraw-render` does not
   exist. The `repository` field in `Cargo.toml` and the known-limitations
   links already point at the new name. `.orig` is public, so
   the five crate names are visible while unclaimed: rename and publish close
   together.
4. **Closed 2026-10-27.** The profile token names wuff 0.2.9, skera 0.8.0 and
   ttf2woff2 0.14.1 (`f347f6af`; #246 was folded in because it bumped the
   named encoder). The corpus, catalog and inventory were regenerated with
   it, and the native baseline, viewer qualification, representative
   candidate (V28) and release evidence from the restage workflow's
   artifacts (`d905b604`..`2e2a537b`). Chromium and resvg
   rendered all 662 qualification executions pixel-identically to the
   retired set, and V28's PDF and PNG equal V27's: the rename moved
   identifiers, not glyphs.

Already in place: CI passes on `main` (all ten jobs, including MSRV), the
nightly passes, private vulnerability reporting is on, and all five crate names
are free on crates.io. Every open issue is either a follow-up listed in the
release policy or an enhancement (#233, #199); #213 is untriaged but looks
like follow-up work too.

## Code findings and recommendations

### 1. Hostile input can still reach dependency panics

The October 4–12 fixes show the pattern: extreme but finite values reaching
tiny-skia, Krilla or resvg.

- `822c97dc`: a huge `strokeWidth` panicked tiny-skia, and 40,000 emoji held
  2–5 GB of memory.
- `391fecba`: a gradient in an embedded SVG grew PDF memory without bound and
  panicked Krilla.
- `58d4d469`, `d0e53ccc`, `371e84ef`: quadratic work and nested-SVG stack depth.
- `9ea81419`: hatch fills buffered hundreds of MB per element.

None of the commits say a fuzzer found them. Every harness uses one hard-coded
seed (`scene_fuzz/mod.rs:66`, `tests/asset_fuzz/mod.rs:14`,
`tests/render_fuzz/generator.rs:6`, `render/font_subset_fuzz.rs:8`), so each
night replays the same cases. They also run under tiny limits (64 KiB inputs,
31 elements, 65,536-pixel surfaces) that rule out the values behind those bugs.

Before 0.1.0:

- [ ] Add a `*_FUZZ_SEED` variable beside the existing `*_FUZZ_CASES` ones. The
      nightly derives the seed from the run number and prints it, and PR runs
      keep the fixed seed. Failure labels already carry `seed=…`.
- [ ] Add a nightly fuzz family that runs under the default limits.
- [ ] Run one cargo-fuzz session of a few hours on `SceneDocument::parse` →
      render (SVG, PNG and PDF) under the default limits, seeded from the
      regression corpus and the conformance scenes. Commit the target, but
      leave it out of CI for now.
- [x] Document in the README that hosts rendering untrusted
      files should use a separate process with a timeout and a memory limit.
      `catch_unwind` does not help against out-of-memory, or when the host builds
      with `panic = "abort"`.
- [ ] Check whether `cargo install excalidraw-render-cli` uses the workspace
      `[profile.release]` (`panic = "abort"`, LTO). A member's packaged manifest
      probably drops it, so the installed CLI may differ from the release
      archive. Output bytes are unaffected.

### 2. Little real-world file exposure

Only three real-world scenes are gated. In the 15-scene public pool
(`tests/public-scenes`), which no check reads, two scenes were rejected for a
missing `files` key until `605d5116` (2026-10-07).

Before 0.1.0:

- [ ] Gate the public pool with a smoke test. For each scene, render all three
      formats under the default limits and assert no error or panic. Render each
      twice and check the outputs match, and pin the set of diagnostic codes.
      Skip visual approval, which keeps it outside the §14.2 sanitize-and-approve
      process.
- [ ] Run once over a larger uncommitted corpus of a few hundred public
      `.excalidraw` files, weighted toward ones older Excalidraw versions wrote,
      and triage every rejection.

After: promote interesting scenes to conformance cases over time.

### 3. Dependency bumps still move output

Krilla, HarfRust, usvg and read-fonts are all pre-1.0, and their recent bumps
moved output. The README already tells users to pin the version and keep
their lockfile when exact output bytes matter.

- [ ] Add a versioning rule to `release.md`: a patch release never moves the
      approved baseline. A bump that moves it waits for the next minor release.
      The conformance gate already detects any movement, so it classifies the
      change.
- [ ] Merge Dependabot's byte-affecting groups (Krilla, HarfRust, usvg, resvg,
      read-fonts, zune-jpeg, …) only between minor releases.

### 4. Windows and macOS are unverified

All CI runs on `ubuntu-latest`, and Windows has never been compiled. It
probably works: the published crates are pure Rust, and the CLI has non-Unix
fallbacks (`crates/excalidraw-render-cli/src/main.rs:1167-1178,1218-1230`).

Before 0.1.0:

- [ ] Add a `windows-latest` and `macos-latest` job that runs
      `cargo build --locked -p excalidraw-render-cli`, the CLI tests and one
      smoke render. devenv does not run on Windows, so the job uses rustup with
      the pinned toolchain. Leave out byte identity and release qualification.
- [ ] State the Windows status in the README.

After: add macOS and Windows binaries to the release archive if users ask.

### 5. The JSON report format is the real contract

The CLI reports embed the library's derived `Serialize` output
(`CompatibilityDiagnostic`, `OutputFormat`, `CompatibilityRevision`;
`crates/excalidraw-render-cli/src/main.rs:433-445`, `frames.rs:245-257`). A
library field rename therefore silently changes CLI `schema_version: 1`. The
render, frame-list and frame-batch reports all say schema 1, and only the
frame reports carry a `mode` field.

Rule: when unsure, leave it out. A later addition breaks nobody, and a removal
does.

Before 0.1.0:

- [ ] Give every report shape a discriminator: `"command": "render" |
      "frames.list" | "frames.export"`, or the existing `mode` applied
      consistently.
- [ ] Add golden tests that pin the serialized form of a render report, a
      partial frame batch, an error, and one diagnostic of each
      `DiagnosticDetails` variant.
- [ ] Drop `compatibility_revision` and `reference_profile_digest` from the JSON
      reports, and keep `renderer_version` and `excalidraw_release`. Add them
      back when revision 2 exists.
- [ ] In the Rust API, make `CompatibilityRevision`,
      `ReferenceProfileIdentity`, `SvgFontSubsetStage`, `FiniteValue::new` and
      `FrameBatchError::preparation_subject` `pub(crate)` or `#[doc(hidden)]`.
- [ ] Remove the hidden `--compatibility-revision` and `--list-revisions` CLI
      flags (`main.rs:195, 241`), after checking whether xtask or the
      conformance tooling uses them.
- [ ] Add the missing trait impls: `PartialEq`/`Eq`/`Hash` on `FrameRef`,
      `PartialEq` on `RenderRequest` and `FrameRenderRequest`, and `Default` on
      `Sizing`.

Leave for 0.2: the `&'static str` and `String` public fields in
`DiagnosticDetails` (`compatibility.rs:382, 394, 425, 440, 494`), whose
retyping means moving to accessors.

### 6. Changes are expensive

The verification code (about 140k lines across tests, xtask and tools) is larger
than the renderer (about 78k). Any change that moves evidence means restaging on
x86-64 Linux. This does not block the release, but it slows patch releases.

- [x] Add a `workflow_dispatch` "restage" workflow that runs the Linux-only steps
      (baseline restage, `just release`, viewer checks) and uploads the results
      as an artifact or a commit. It also closes gates 1, 2 and 4 above.
      Done 2026-10-27: `restage.yaml` stages the baseline, regenerates the
      font-subset viewer qualification, collects a representative successor
      and reproduces the SVG/PDF viewer qualifications, as artifacts;
      `release-evidence.yaml` already ran `just release`. Gates 1, 2 and 4
      are closed by it. The superseded V6 and V27 candidates were pruned once
      their last readers moved to V28 or carried their fixture themselves
      (ADR-0022).
- [ ] Combined with rule 3, refresh evidence once per minor release instead of
      piecemeal.
- Continue pruning in the direction of ADR-0022: when a digest pin forces a
      refresh, ask whether any check needs it.

## Deferred

- [x] cargo-deny (advisories and licenses) in CI: `just deny`, `deny.toml`.
- [ ] `cargo-semver-checks` after the first publication, once a baseline exists.
- [ ] An umbrella error type, consistent option builders (`builder()` versus
      `Default` plus `with_*`), and splitting the CLI's 3,000-line `main.rs`.

## Order

1. The restage workflow (finding 6). It unblocks every Linux-only step.
2. The JSON report and API trim, plus the trait impls (finding 5).
3. The fuzz seed variable and the default-limits family, then the public-pool
   smoke gate (findings 1 and 2).
4. The Windows and macOS job, and cargo-deny (finding 4, deferred).
5. A cargo-fuzz session and the large-corpus run, in parallel with the subset
   profile refresh (gate 4).
6. The versioning rule in `release.md` (finding 3), then the release gates and
   the runbook.
