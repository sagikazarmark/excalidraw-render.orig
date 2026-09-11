# ADR-0003: single font identity refresh

Base commit: `16d1fe4` (the Dagger aggregate verification record).
Status: generated, verified on both targets, and approved by the maintainer;
awaiting a clean-commit packaged-consumer/aggregate run. Publication remains blocked.

## Changes

The revision-1 subset profile now identifies the actual upstream implementations:
`skera-0.7.0`, `ttf2woff2-0.13.3`, and `brotli-9.0.0`. The dependencies and
encoder flags themselves were already updated by the fork-retirement commits.
The profile's other settings and table rules are unchanged. Spec §8.3.3 names
the same versions and requires portable-float. The temporary crate-root WASM
`println!` shim is removed; the qualification measurement and optional per-cell
trace now use `console_log!` at their WASM call sites.

The corpus, packaged corpus copy, catalog/incorporated profile and cross-target
measurement have been regenerated. All 977 successful cells change exactly
their WOFF2 digest, semantic subset identity, and transformation-notice digest.
Their coverage/scalars, source identities, applicability and failure cases are
unchanged. Three valid fuzz-regression cases share a new WOFF2 pin; the focused
Excalifont SVG byte pin changes as well. Its PNG and PDF byte pins still pass.

## Measured results

| Input / measurement | Refreshed SHA-256 |
| --- | --- |
| Profile semantic digest | `522b5b9a6f1ae71af1ce72ddda9db2c6aca6607fc7567b1362ec449c2339abd0` |
| Profile JSON | `083073315198f4cc4d67cd2f3a3fa7b65c6ce651dfb52fed61f5090c932b92c7` |
| Qualification corpus | `3d034ec2f9850d3e3cd19d0fc0c6305405240a151e7700eaa0ba2d43a43d3f8a` |
| Generated font bytes (977 cells) | `6fb430ede9291c241081002ae6fd9c29d439fd878cbccc20f632016d196f459b` |
| Resource traces (1204 cells) | `79ea93cdb58f7f348a14df21358be0066360fde67aa3b0cbbd7c6556bccf90fa` |

Native release qualification passed all 358 tests (5 ignored). Chromium
149.0.7827.55 WASM qualification passed all 6 selected tests, with the exact
same measurement above. The resource-trace digest is unchanged from before
the refresh. Both canonical viewers independently pass all 857 executions:
zero changed pixels, SSIM 1, all seven semantic checks pass, and both
fresh-process repeats match for each role. After matching executions by source
and class coverage, all raster digests and pure-Rust semantic digests are
unchanged. Chromium semantic digests change with the profile-derived font
family identity; source-versus-subset comparisons still pass exactly.

Before/after viewer inputs were generated through Dagger and matched by their
`coverage` records (rather than their intentionally changed semantic IDs).
Every one of the 857 decompressed subset sfnts has an identical table set and
identical table bytes except `name`, after zeroing `head.checkSumAdjustment`.
Every source sfnt is byte-identical. This isolates the profile/name change
from the encoder: no outline, mapping, metric, hinting, or layout table changed.
The earlier retirement commits and ADR-0003 retain the upstream mechanical
diff and byte-identity proof; this change does not replace the encoder again.

## Execution fixes found during the round

- The Dagger harness acquisition uses the kernel.org Ubuntu HTTPS mirror and
  fails on incomplete apt indexes. archive.ubuntu.com downloads stalled before
  rendering; package signature verification remains enabled.
- Concurrent fresh Chromium processes produced `Page.captureScreenshot`
  errors or hangs on varying cases in both direct and nested Dagger containers.
  One isolated case passed repeatedly. Sequential roles and cases completed
  the entire corpus, both directly and through the pinned harness. The driver
  now runs one browser at a time, with a 60-second per-case deadline. It retains
  all 857 cases, both roles, both repeats, fresh processes, browser arguments,
  virtual time, font audits, dimensions, and comparison rules. This scheduling
  change is included in the #99 approval subject; no retry masks a failed case.
- `dagger api call wasm qualification-log` (or `packaging qualification-log`)
  exports a passing test log before approval allows target-report creation.
  Ordinary qualification checks still require report materialization.

The full pinned `evidence viewers --kind font-subset --mode generate` completed
in about 36 minutes. Export only its evidence directory to avoid transferring
compiler output:

```sh
dagger api call evidence viewers --kind font-subset --mode generate \
  directory --path tests/conformance/svg-font-subset-viewer-qualification \
  export --path target/font-refresh-viewer-evidence
```

Generation requires retiring the old manifested qualification for this explicit
refresh. The previous manifest remains available at the base commit. Subsequent
verification uses `reproduce`, not another identity refresh.

## Recorded approvals

### #99 — full dual-viewer qualification

Subject: `sha256:6721df453ac6edb3225d8e6f49f92ebb440dfb84ad7cd28eb54a1992cb06a1df`

Includes the refreshed corpus/profile and reports, qualification logging and
digest constants, verifier constants, and sequential/deadline Chromium driver.
The ordinary viewer profiles and pure-Rust driver are unchanged. Explicitly
approved on 2026-09-11 and recorded in
[#99](https://github.com/sagikazarmark/excalidraw-render.orig/issues/99#issuecomment-5638706817).

### #120 — reviewed decodes

Subject: `sha256:9193b0c743eb4b5c31bf695e7a7e572bcb1c61a47b887d6cb4bb0858d4290bdd`

All 230 reviewed decoded fonts are unchanged. The sole non-approval field change
is `qualificationCorpusSha256`, so the catalog generator correctly cleared the
previous approval. Explicitly approved on 2026-09-11 and recorded in
[#120](https://github.com/sagikazarmark/excalidraw-render.orig/issues/120#issuecomment-5638706797).

## Remaining completion gates

Both approvals were recorded with the corresponding xtask commands through
Dagger, and the release manifest now binds the approved viewer record.
Commit the coordinated
refresh, run packaged consumers and the supported-target aggregate with Dagger,
attest that clean-commit run, and bind the aggregate to release evidence.

Lint, registered conformance verification, ordinary SVG/PDF viewer verification,
the render test tier, and doctests pass. The fast tier initially found one
expected SVG byte-pin change (updated and independently retested); its other
eight failures were approval-dependent xtask tests. These gates are rerun after
approval; package export requires the coordinated package changes to be committed.
