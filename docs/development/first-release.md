# First release (0.1.0) checklist

One-time steps and open items for the first publication, on top of the
[release runbook](releasing.md). Delete this file, and the pointer to it in the
runbook, once 0.1.0 is out; move anything still open into issues first.

## One-time steps

- Publish the full history at `github.com/sagikazarmark/excalidraw-render`,
  the `repository` in the root `Cargo.toml`, before `cargo publish`. crates.io
  keeps the repository URL per published version and resolves README links
  against it, so the code must be there, with this layout. Rename the
  development repository (`excalidraw-render.orig`) and make it public rather
  than creating a new one: that keeps the issue numbers that commits, ADRs and
  docs cite, where a new repository would start its own numbering and every
  `#N` reference would point at an unrelated issue.
- Once the repository is public, check the issue links in the
  [known limitations](../reference/known-limitations.md): they use full URLs
  under the new name, so they resolve only after the rename.
- Confirm private vulnerability reporting (Settings → Security) survived the
  rename. The README points reporters at the Security tab, which offers no
  "Report a vulnerability" button until it is on.
- Publish the crates soon after the repository goes public: the five crate
  names are free on crates.io until then, and the public repository names them.
- The crates.io token needs the `publish-new` scope, since none of the crates
  exist yet.
- After publishing, add any co-owners with `cargo owner --add <login> <crate>`
  for each of the five crates.
- `cargo login` with that token before `cargo release --execute`, and set up
  git tag signing (`user.signingkey`), since the release tag is signed.
- Turn on GitHub Pages with "GitHub Actions" as the source (Settings → Pages).
  That creates the `github-pages` environment, which by default deploys only
  from the default branch; add a tag rule for `v*` to its deployment branches
  and tags (Settings → Environments → github-pages), or the deploy job of the
  release fails. Run `pages.yml` once from `main` to check the deployment.

## Open items

What the readiness review (2026-10-18, revised by the 2026-11-01 testing
review) left open. Delete each line once it is done or tracked as an issue.

Hostile and real-world input:

- [ ] A `fuzz/` crate outside the workspace members with cargo-fuzz targets
      `parse_render_{svg,png,pdf}` (`SceneDocument::parse` → render under the
      default limits), seeded from `tests/public-scenes`, the registered scenes
      and the regression corpora, and a `just fuzz <target> <minutes>` recipe.
      Run one session of a few hours and triage what it finds; keep it out of
      CI.
- [ ] One run over an uncommitted corpus of a few hundred public
      `.excalidraw` files, weighted toward older Excalidraw versions, with
      every rejection triaged.
- [ ] Check whether `cargo install excalidraw-render-cli` uses the workspace
      `[profile.release]` (`panic = "abort"`, LTO); a member's packaged
      manifest probably drops it, so the installed CLI may differ from the
      release archives, which dist builds with `[profile.dist]`
      (`[profile.release]` unchanged).

Platforms:

- [ ] A `windows-latest` and `macos-latest` CI job: `cargo build --locked -p
      excalidraw-render-cli`, the CLI tests and one smoke render, with rustup
      and the pinned toolchain (devenv does not run on Windows).
- [ ] State the Windows status in the README.

Compatibility revisions:

- [ ] Record in spec § 3 whether a reference-profile bump after publication
      adds revision 2 beside revision 1 (a second catalog, case set and golden
      record) or supersedes it while 0.x retains nothing.

After the first release: `cargo-semver-checks` against the published
baseline; an umbrella error type and consistent option builders; splitting the
CLI's `main.rs`; the `&'static str` and `String` public fields of
`DiagnosticDetails` (0.2).
