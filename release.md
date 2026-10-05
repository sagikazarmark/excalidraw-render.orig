# Releasing

The [0.1.0 scope](docs/release-0.1.0.md) and
[known limitations](docs/known-limitations.md) describe what a release
promises. The [preparation record](docs/release-preparation.md) is historical;
this runbook is the current procedure.

The workspace publishes five crates in lockstep, in this dependency order:
`excalidraw-render-fonts-xiaolai`, `excalidraw-render-fonts-xiaolai-extended`,
`excalidraw-render-fonts`, `excalidraw-render`, `excalidraw-render-cli`. The font
crates carry the Excalidraw release as build metadata
(`0.1.0+excalidraw.0.18.1`). crates.io ignores build metadata when comparing
versions, so a font refresh for a new Excalidraw release needs a new version
number, not only new metadata.

## Before the first release

- Publish the full history at `github.com/sagikazarmark/excalidraw-render`,
  the `repository` in the root `Cargo.toml`. crates.io keeps it per published
  version and resolves README links against it, so the code must be there,
  with this layout, before `cargo publish`. Renaming the development
  repository (`excalidraw-render.orig`) and making it public keeps the issue
  numbers that commits, ADRs and docs cite; a new repository starts its own
  numbering, and those `#N` references then point at unrelated issues.
- The [known limitations](docs/known-limitations.md) cite no issue numbers.
  Open public issues for the follow-ups worth tracking there.

## 1. Prepare the candidate

1. Update `CHANGELOG.md`: replace "Unreleased" with the date and summarize the
   release.
2. Bump `version` in `[workspace.package]`, every font crate's `version`
   (keeping its `+excalidraw.X.Y.Z` metadata), and the `=X.Y.Z` requirements in
   `[workspace.dependencies]`. Run `cargo update --workspace`.
3. Commit, and make sure the tree is clean: release provenance must not come
   from an edited working tree.

## 2. Verify

```sh
dagger --engine cloud check
cargo publish --workspace --dry-run --locked
```

`dagger check` is the release gate (lint, MSRV, tests, evidence, packaging,
native and experimental WASM). The SVG/PDF viewer harnesses run separately;
see [conformance operations](tests/conformance/README.md). The publish dry run
packages and compiles every crate against its packaged dependencies.

## 3. Tag and publish

```sh
git tag -s vX.Y.Z -m "excalidraw-render X.Y.Z"
git push origin vX.Y.Z
cargo publish --workspace --locked
```

`cargo publish --workspace` uploads in dependency order and waits for each
crate to reach the index before publishing its dependents. If it stops part
way, publish the remaining crates in order with `cargo publish -p <crate>`.

## 4. Build the release archive

```sh
dagger --engine cloud call release-archive --version X.Y.Z export --path dist
```

This builds the Linux x86-64 (glibc) CLI with the pinned toolchain from the
tagged tree, producing `excalidraw-render-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
and its `.sha256` file. The archive holds the binary, README, and every license.

## 5. Create the GitHub release

```sh
gh release create vX.Y.Z dist/* --title "excalidraw-render X.Y.Z" \
  --notes-file <(awk '/^## /{p=($2=="X.Y.Z")} p' CHANGELOG.md | tail -n +2)
```

The notes are the body of the `## X.Y.Z — <date>` section, without its
heading, up to the next `## ` heading or the end of the file.

## 6. Smoke-test the published artifacts

```sh
cargo install --locked excalidraw-render-cli --version X.Y.Z
excalidraw-render tests/conformance/cases/focused/bound-text-containers/scene.excalidraw -o /tmp/smoke.svg
```

Also unpack the GitHub archive on Linux x86-64, verify its checksum, and run
`excalidraw-render --version`. Record the release commit, crate checksums, and
the Dagger trace in the changelog entry.
