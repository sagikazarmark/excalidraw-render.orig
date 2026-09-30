---
status: accepted
---

# Pin the canonical comparator to its own dependency closure

The canonical visual comparator profile,
`tests/conformance/comparator-profiles/canonical-visual-v1.json`, is the
identity of "the approved comparator". The SVG viewer qualification (#34), the
PDF harness qualification (#35), and the rectangle PDF renderer expectations
(#44) each bind its digest, so any change to the profile invalidates all three
maintainer approvals at once.

Until #118 the profile pinned, next to the comparator's own sources, the root
`Cargo.toml` and the whole workspace `Cargo.lock`. `excalidraw-render-compare`
has no normal dependencies and five dev-dependencies, so no renderer dependency
can change its build or its bytes, yet every dependency change anywhere in the
workspace moved the profile and forced a three-approval round whose complete
delta was one digest. That happened three times in five weeks (d26a4b1,
cdd02be, 6137b6b). Approvals that never carry a decision train everyone to
rubber-stamp, which is the opposite of what the binding is for.

The profile now pins the comparator's own closure instead of the workspace's.
`implementation.sources` lists only files under
`crates/excalidraw-render-compare/`, and the schema enforces the prefix. The
`lockfile` entry is replaced by `implementation.closure`, a digest-pinned
committed file, `canonical-visual-v1.closure.json`, holding the packages
reachable from `excalidraw-render-compare` in `Cargo.lock` — name, exact
version, source, and checksum — sorted by name and version. The two
root-manifest facts the comparator inherits, `edition = "2021"` and
`unsafe_code = "forbid"`, are stated in the profile and checked against both the
workspace and the crate manifest; that replaces the root `Cargo.toml` digest.
This supersedes the description in ADR-0004 of what the profile binds; the
round ADR-0004 describes was the last one the old binding forced.

## How the closure is derived and verified

The closure is derived from the lockfile rather than from `cargo tree` because
the lock is target- and feature-agnostic — the same bytes on every host — and it
is the only place exact checksums live. It is a superset of what any one
platform builds; that is the intended definition of *what could reach the
comparator's bytes*. Walking the lock resolves dependency specs in all three
forms Cargo writes (`name`, `name version`, `name version (source)`), so a
lockfile that drops a second version of a package and rewrites the specs of its
dependents does not move the closure. Measured against history, the closure is
byte-identical before and after all three rounds listed above, so none of them
would have needed an approval under this binding.

The comparator profile test recomputes the closure from the live lock and fails
naming exactly the packages that differ; an `#[ignore]`d test in the same file
materializes the committed copy, so verification and refresh share one
implementation. Both read `Cargo.lock` and the two manifests with hand-rolled
line readers on purpose: a `toml` or `cargo-lock` dev-dependency would enter the
very closure being pinned, so every upstream release of the reader would itself
move the profile.

## What still moves the profile

The schema `comparator-profile-v1.schema.json` is amended in place rather than
cut as v2. It is digest-bound by the profile, the profile moves regardless, and
the comparison protocol named by `canonical-visual-comparator.v1` is unchanged;
only what the identity pins is different. The amendment is itself a full
three-approval round. #118 recommended bundling it with the next unavoidable
one, and the maintainer chose to take it standalone so every later round is
free of the false positive.

Three triggers remain that do not change the comparator's bytes on the host
that runs it, and each is accepted as legitimate because it touches an identity
the comparator builds against. A package reachable only on another target
(`wasm-bindgen`, `windows-link`, `redox_syscall`, `wasip2`, and their
dependents, all via `jsonschema`) moves the closure when the renderer side
bumps it, because the lock is host-agnostic by design. Enabling extra features
on a shared `[workspace.dependencies]` entry the comparator also uses
(`jsonschema`, `png`, `serde_json`, `sha1`, `sha2`) can add packages to that
entry's lock dependencies and so to the closure. And the root package's own
version is a closure entry, so a `workspace.package.version` bump moves the
profile; that is a release event, when every artifact moves anyway, and it was
already true under the old binding through the root `Cargo.toml` digest.

## Amended 2026-09-30: the closure is recorded, not bound (#212)

Pinning the closure still made routine renderer bumps expensive. `serde_json`,
`sha1`, `sha2`, `png`, `jsonschema`, `wasm-bindgen` and `syn` all reach the
comparator, and seven closure refreshes followed (`comparator-closure-refresh-v1`
to `v7`: #206, #208 ×4, #211, #207). Two of them also migrated a vector source
that dependency's API change touched.
- **The cascade:** each one moved the profile digest, and live evidence binds
  that digest: 18 oracle references, both viewer qualifications with their
  sidecars, the rectangle PDF expectations, `xtask/src/pdf_viewers.rs`, and in
  turn the test sources, `coverage/pdf-judgments.json` and `coverage/
  ownership.json` that bind those. A refresh rebound about 50 files without
  changing one measurement.
- **The trust code:** each refresh also needed an archived preimage and a
  `COMPARATOR_REFRESHES` row, so that evidence measured under the old profile
  stayed valid.

**Decision.** The profile names only what can change a measurement: the
comparator's sources, the protocol, the limits, the vectors and the imported
fixtures. It no longer carries `implementation.closure`, and the schema drops
the field. `canonical-visual-v1.closure.json` stays committed and reviewed in
Git (ADR-0019), and the profile test still requires it to equal the
comparator's reachable subgraph of the live `Cargo.lock`, naming any package
that differs. No digest binds it, so a closure-only move changes that one file
and nothing else. Refreshing it is one command:

```sh
EXCALIDRAW_REFRESH_COMPARATOR_CLOSURE=1 cargo test --locked -p excalidraw-render-compare --test profile -- --ignored materialize
```

Without the variable, `materialize` only stages the closure under Cargo's
scratch directory, so a run of every `--ignored` test leaves the tree clean.

**Why this is enough.** A dependency move that changed what the comparator
computes would show up where measurements are defined:
- the adversarial Pixelmatch and SSIM vectors and the imported upstream
  fixtures, which the profile still binds by digest and the comparator's tests
  execute;
- the viewer qualifications and oracle references, which record measured
  results.

The closure digest added no evidence beyond those. It only restated
`Cargo.lock`, which Git already reviews. The three triggers listed above no
longer move the profile.

**Transition.** Removing the field moved the profile, and its schema digest,
one last time. `comparator-closure-refresh-v8` archives the preimages, and its
`COMPARATOR_REFRESHES` row migrates the schema digest the way earlier rows
migrated vector digests. That is the last row. Evidence bound to any archived
profile still validates through the same successor: after the reviewed digest
migrations, a superseded profile must equal the live one once
`implementation.closure` is removed. A profile move now means a change to the
comparator's sources, protocol, limits, vectors or fixtures, and that gets its
own review.
