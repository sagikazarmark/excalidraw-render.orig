---
status: accepted; amended 2026-11-01 (Dagger retired)
---

# Run the gates from devenv and just on any host

ADR-0006's 2026-09-11 amendment made the Dagger module the canonical CI and
evidence runner. It gave every check the same sealed Linux container, but at a
cost the module's structure makes hard to avoid. A check runs inside a shared
build graph, so even `lint:clippy` first compiles every workspace test target,
the release CLI and the release test dependencies. A cold `lint:clippy` took
34m35s on an Apple-silicon laptop, where the module's `linux/amd64` pin runs
under emulation; `cargo clippy` with the same arguments in the devenv shell
took 44s from an empty target directory. Every GitHub Actions job starts a cold
engine. Around the module grew a second pin system: a tool manifest of archive
URLs and checksums, install scripts that write to `/opt` and `/usr/local/bin`,
and evidence bundles that bind the manifest's and `dagger.lock`'s digests.

The renderer does not need any of that to be reproducible. Its float math is
owned (ADR-0003, ADR-0007), and the guards that tie checks to
`x86_64-unknown-linux-gnu` protect where evidence was recorded, not what it
says. With the host guard on native candidate replay relaxed to the pinned Rust
version, `aarch64-apple-darwin` rebuilt the release CLI, rendered every
registered candidate twice in fresh processes, and matched the Linux-recorded
artifact and report bytes. The whole conformance suite, all 1632 fast-tier
tests and the render tier passed on the same host.

## Decision

The gates run from a justfile in the devenv shell. Each check is a recipe;
`just --list` is the inventory and `just ci` runs the per-change set. CI is a
caller that runs recipes in the devenv shell, one job per check. Dagger is no
longer the CI authority.

devenv pins the tools. `devenv.lock` fixes nixpkgs and nixpkgs-multiverse, so
every tool has one version on every host, and an exact version is selected
where a verdict depends on it: the Rust toolchain, qpdf 12.3.2 (the PDF
inspector refuses any other), and wasm-bindgen-cli at the `wasm-bindgen`
version in `Cargo.lock`. `tests/package-consumer/tool-manifest.json` and the
scripts that install from it retire.

Evidence records the identities of the tools that produced it, such as
`rustc -vV`, the wasm-bindgen runner and the browser version, instead of
digests of pin files. Verification compares artifact and report bytes against
approved expectations. It does not refuse a run because a pin file changed.
Records that already carry manifest or lock descriptors keep them as
historical provenance, which is how their verifiers treat them today.

Checks run on any host the devenv shell supports. Guards that only enforced
"recorded on Linux x86-64" go; guards on behavior stay, such as the exact
release profile of native replay and the pinned Rust version. The supported
targets of ADR-0006 are unchanged: the determinism promise and the target
evidence still name `x86_64-unknown-linux-gnu` and browser WASM. A run on
another host is a check, not target evidence. Making macOS a supported target
remains a spec amendment with its own qualification, which the replay above
suggests it would pass.

Browser WASM runs in a browser from the devenv shell over WebDriver, and the
run records the browser version. The renderer makes no browser API calls and
its tests run in a dedicated worker, so the browser build cannot change
artifact bytes. The supported target is `wasm32-unknown-unknown` in a browser,
not in one pinned Chromium build. The reference oracle and the viewer harnesses
keep their pinned images: there the browser draws the evidence.

The gates split by when they run:

- Every change: formatting, lints and the feature matrix, MSRV, the fast,
  render, feature and doc test tiers, the conformance verifiers, and the
  packaged consumers (CLI, public facade, no-CJK) on the host target.
- Release: qualification, native and WASM snapshot collection and replay,
  traces, full equality and target evidence. They are recipes too, and a
  release runs them on the supported targets.

Dagger keeps the reference-oracle and viewer evidence operations, which run
pinned Docker images, until they move. `input-boundaries` and
`dagger-gha:generate` checked the module itself and retire with its gates.

## Consequences

- Spec section 9 is amended: `devenv.lock` pins every CI executable, and the
  browser-WASM target is "in a browser". ADR-0006 gains a pointer here.
- A laptop runs the same per-change gates as CI, natively. Cargo's own
  fingerprints decide rebuilds, so a check compiles only what it reads.
- `docs/ci.md`, `docs/development.md` and the release runbook describe
  recipes instead of Dagger calls.
- New snapshot, trace and result bundles change schema to record tool
  identities. The native baseline is replayed, not regenerated: its expected
  bytes do not depend on the pin files.
- A tool update is a `devenv update` reviewed like any lockfile change. It no
  longer invalidates evidence by changing a digest, only by changing bytes.
- The sealed input views of the Dagger module go. A recipe can read any file
  in the checkout, and Cargo's change tracking replaces the compile view.

Amended 2026-11-01: the remaining Dagger operations move to recipes and the
module retires. devenv supplies PDFium build 7947 for the host beside qpdf, and
the PDF harness records the library that ran instead of pinning one digest. The
pure-Rust SVG viewer builds for the host. On macOS arm64 both reproduce every
retained raster byte for byte, as does the font subset corpus. The oracle and
viewer recipes run their digest-pinned images on the host's Docker, so they need
Docker; the Docker-free oracle path and the nested daemon for hosts without
Docker go with the module. Records that name the module's files keep them as
historical provenance.
