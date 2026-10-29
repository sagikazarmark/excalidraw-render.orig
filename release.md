# Releasing

The [0.1.0 scope](docs/release-0.1.0.md) and
[known limitations](docs/known-limitations.md) describe what a release
promises; [release readiness](docs/release-readiness.md) tracks the open
items for the first release. This runbook is the procedure.

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
- The [known limitations](docs/known-limitations.md) link their tracking
  issues by full URL under the new repository name, so they resolve only after
  the rename. Check them once the repository is public.
- Private vulnerability reporting (Settings → Security) is on; confirm it
  survived the rename. The README points reporters at the Security tab, which
  offers no "Report a vulnerability" button until it is on.
- Publish the crates soon after the repository goes public: the five crate
  names are free on crates.io until then, and the public repository names them.
- The crates.io token needs the `publish-new` scope for the first publication,
  since none of the crates exist yet.

## 1. Prepare the candidate

1. Bump `version` in `[workspace.package]`, every font crate's `version`
   (keeping its `+excalidraw.X.Y.Z` metadata), and the `=X.Y.Z` requirements in
   `[workspace.dependencies]`. Run `cargo update --workspace`.
2. Commit, and make sure the tree is clean: release provenance must not come
   from an edited working tree.

## 2. Verify

On Linux x86-64, in the devenv shell:

```sh
just ci
devenv --option languages.rust.version:string 1.92.0 shell -- just msrv
just release
cargo publish --workspace --dry-run --locked
```

`just ci` is the per-change gate (lint, tests, evidence, packaging), `just msrv`
the MSRV check, and
`just release` the release checks (native and experimental-WASM qualification,
snapshot replay, traces and full equality). The SVG/PDF viewer harnesses run separately;
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

After the first publication, add any co-owners with
`cargo owner --add <login> <crate>` for each of the five crates, and check that
[docs.rs](https://docs.rs/crate/excalidraw-render/latest/builds) built
`excalidraw-render`.

## 4. Build the release archive

```sh
git switch --detach vX.Y.Z
git status --porcelain   # must print nothing
just release-archive X.Y.Z
```

The recipe builds whatever is checked out, so check out the tag in a clean tree
first; the archive build refuses a `--version` that differs from the workspace
version. It builds the Linux x86-64 (glibc) CLI with the devenv toolchain into `dist/`,
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
