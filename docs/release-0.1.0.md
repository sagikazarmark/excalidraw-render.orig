# 0.1.0 release policy

Status: accepted scope; [release preparation verified](release-preparation.md). No publication is
implied by this document. The code is published at
[`sagikazarmark/excalidraw-render`](https://github.com/sagikazarmark/excalidraw-render)
before the crates are.

This policy implements [ADR-0016](adr/0016-release-0.1.0-with-a-version-scoped-contract.md).
It takes precedence over the original revision-1 specification's publication,
historical-retention, exhaustive-coverage, and cross-target-equality requirements
for 0.1.0. The specification remains the rendering design and conformance target;
this release does not claim to have completed every requirement in it.

## Public scope

- Rust library and native CLI for whole-scene and frame SVG, PNG, and vector-first PDF.
- Excalidraw 0.18.1 compatibility target, with explicit diagnostics and
  [known limitations](known-limitations.md).
- Release-qualified native target: Linux x86-64 GNU, using the pinned Rust
  toolchain/build environment. Other native platforms are unqualified.
- Browser-hosted `wasm32-unknown-unknown` remains tested but experimental.
  Exact byte identity with native output is not a 0.1.0 guarantee. No browser
  bindings, npm package, or raw WASM binary are part of this release.
- Deterministic artifact/report output for the same renderer release, input,
  options, supported environment and dependency versions. The repository
  `Cargo.lock` and build inputs are pinned; the library's dependency
  requirements are semver ranges, so applications pin exact bytes with their
  own lockfile.
- Future 0.x releases may intentionally change output and diagnostics after
  review and release-note documentation. Revision 1 remains an evolving profile;
  package publication does not make it a retained historical implementation.

## Release gates

1. `dagger --engine cloud check` passes on the candidate tree. Dagger success
   means green CI; Actions status is not a release prerequisite.
2. SVG/PDF viewer gates pass using the pinned harnesses.
3. Packaged library and CLI build and execute; dependency order, archive sizes,
   bundled assets, and licenses are verified. Publication is rehearsed before
   credentials are used for an actual release.
4. All unexpected regressions are resolved. Experimental-WASM differences are
   limited to the explicitly tracked #178 cases; reports and repeatability stay
   exact. A changed known difference requires review rather than automatic acceptance.
5. Installation instructions, platform scope, limitations, and release notes
   describe the delivered behavior.
6. The final release is built from an identified clean tag. Publication uses the
   tested packages; installed public artifacts receive a final smoke test.

Current approval-bearing evidence remains checked. Representative producer
identities are historical provenance under [ADR-0017](adr/0017-separate-representative-expectations-from-producer-provenance.md);
fresh renderer replay must still match the approved outputs exactly. This changes
no historical approval record or reference output. Exported run
provenance must distinguish an edited working tree from a clean tagged candidate.

## Follow-up work

The issues below live in the development repository's tracker. The release
renames that repository to `excalidraw-render` and makes it public (see
[the runbook](../release.md)), so these numbers, and the `#N` references in
commits, ADRs and docs, keep pointing at the same issues.

| Work | 0.1.0 disposition |
| --- | --- |
| #85 full corpus/ownership closure | Follow-up; existing consistency and regression checks remain |
| #86 full strict cross-target qualification | Follow-up; execute native and experimental WASM now |
| #178 tiny-skia-path cross-target rounding | Known experimental-WASM limitation, exactly tracked |
| #94 upgrade automation | Follow-up |
| #196 extra representative frame variants | Follow-up |
| #68 AVIF interpretation | Unsupported in 0.1.0 |
| #187 JPEG sampling/CMYK, #190 HDR gain maps | Known codec limitations |
| #194 fractional stroke coverage, #197 unusual WebP ICC/alpha | Known rendering differences |
| #193 strict JPEG entropy handling | Known malformed-input correctness limitation; public-API characterization test retains exposure across all formats |

Build/install failures, missing required license/assets, unexpected output drift,
and demonstrated panics or unbounded processing in accepted inputs remain
release blockers. Global coverage closure and an approval-system redesign do not.
