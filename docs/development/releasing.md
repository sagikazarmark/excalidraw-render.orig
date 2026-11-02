# Releasing

The [0.1.0 scope](../release-0.1.0.md) and
[known limitations](../reference/known-limitations.md) describe what a release
promises. This runbook is the procedure for every release. Until 0.1.0 is
out, also work through the [first-release checklist](first-release.md).

The workspace publishes five crates in lockstep, in this dependency order:
`excalidraw-render-fonts-xiaolai`, `excalidraw-render-fonts-xiaolai-extended`,
`excalidraw-render-fonts`, `excalidraw-render`, `excalidraw-render-cli`. The font
crates carry the Excalidraw release as build metadata
(`0.1.0+excalidraw.0.18.1`). crates.io ignores build metadata when comparing
versions, so a font refresh for a new Excalidraw release needs a new version
number, not only new metadata.

## Versioning rule for output bytes

A patch release never moves the golden record (`tests/conformance/golden`)
or a parity pin: it carries fixes whose artifact bytes are unchanged for every
registered export target, so an application that pinned the previous patch
sees the same bytes. A change that moves them, including a bump of a crate in
Dependabot's `output-bytes` group (Krilla, HarfRust, usvg, resvg, tiny-skia,
skera, the decoders, …), waits for the next minor release, and the minor
release restages everything it moved once: `just update-golden`, the
two-sided tolerance exceptions the parity harnesses report, and the
representative candidate through `restage.yaml`. The golden test classifies
the change; the review of its diff is the acceptance.

## 1. Prepare the candidate

1. Bump `version` in `[workspace.package]`, every font crate's `version`
   (keeping its `+excalidraw.X.Y.Z` metadata), and the `=X.Y.Z` requirements in
   `[workspace.dependencies]`. Run `cargo update --workspace`.
2. Commit, and make sure the tree is clean: release provenance must not come
   from an edited working tree.

## 2. Verify

In the devenv shell, on any supported host (the release checks need the
`release` profile: `devenv --profile release shell`). The `Release` workflow
runs `just release` from the release commit; the other three run locally or
come from the commit's CI run:

```sh
just ci
devenv --option languages.rust.version:string 1.92.0 shell -- just msrv
devenv --profile release shell -- just release
cargo publish --workspace --dry-run --locked
```

`just ci` is the per-change gate (lint, tests, conformance, packaging),
`just msrv` the MSRV check, and `just release` the release checks: the
release-tier verifiers and tests, native and experimental-WASM qualification,
the packaged consumers in WASM and the golden targets in WASM. The PDF and SVG
viewer harnesses run nightly and on demand (`just pdf-harness`,
`just svg-harness`); see [conformance operations](../../tests/conformance/README.md).
The publish dry run packages and compiles every crate against its packaged
dependencies.

## 3. Tag and publish

```sh
git tag -s vX.Y.Z -m "excalidraw-render X.Y.Z"
git push origin vX.Y.Z
cargo publish --workspace --locked
```

`cargo publish --workspace` uploads in dependency order and waits for each
crate to reach the index before publishing its dependents. If it stops part
way, publish the remaining crates in order with `cargo publish -p <crate>`.

Check that [docs.rs](https://docs.rs/crate/excalidraw-render/latest/builds)
built `excalidraw-render`.

## 4. Build the release archive

```sh
git switch --detach vX.Y.Z
git status --porcelain   # must print nothing
just release-archive X.Y.Z
```

The recipe builds whatever is checked out, so check out the tag in a clean tree
first; the archive build refuses a version argument that differs from the
workspace version. It builds the Linux x86-64 (glibc) CLI with the devenv toolchain into `dist/`,
producing `excalidraw-render-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
and its `.sha256` file. The archive holds the binary, README, and every license.

## 5. Create the GitHub release

```sh
gh release create vX.Y.Z dist/* --title "excalidraw-render X.Y.Z" --generate-notes
```

The generated notes list the merged pull requests since the previous tag; edit
them before publishing so they summarize the release, link repository files by
full URL and carry no placeholder text.

## 6. Smoke-test the published artifacts

```sh
cargo install --locked excalidraw-render-cli --version X.Y.Z
excalidraw-render tests/conformance/cases/focused/bound-text-containers/scene.excalidraw -o /tmp/smoke.svg
```

Also unpack the GitHub archive on Linux x86-64, verify its checksum, and run
`excalidraw-render --version`, which prints
`excalidraw-render X.Y.Z (Excalidraw 0.18.1)`. Then add the release commit,
crate checksums, and the release-check logs to the GitHub release notes
(`gh release edit`): the tag is already fixed.
