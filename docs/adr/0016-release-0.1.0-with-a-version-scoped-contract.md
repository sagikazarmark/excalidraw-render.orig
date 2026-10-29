---
status: accepted; approval tooling superseded by ADR-0019, Dagger authority by ADR-0024
---

# Release 0.1.0 with a version-scoped contract

The first release prioritizes the tested Rust library and Linux CLI over complete
revision-1 conformance closure. The maintainer accepted a release-first scope:
deterministic artifacts and reports for a pinned release and supported environment,
reviewed output changes between 0.x releases, and experimental browser WASM.
Publishing 0.1.0 does not freeze revision 1 or require retaining historical
rendering implementations in later packages; consumers needing historical output
pin the renderer and its dependency lockfile.

This supersedes ADR-0002/0006 only for the 0.x public compatibility and target
guarantees, and spec sections 3, 14–16 where they require first-publication
immutability, exhaustive closure, or unqualified native/WASM equality before an
initial release. Rendering rules and existing evidence judgments remain intact.
The exact scope and release gates are in [the release policy](../release-0.1.0.md).

Known WASM differences remain executed and narrowly tracked, not hidden by
skipping cases or increasing visual tolerances. Strict equality remains available
as a qualification goal. Evidence is accepted through Git/PR review and
reproducible tests (ADR-0019), with representative provenance separated from
expectations (ADR-0017). The gates run from the justfile in the devenv shell
(ADR-0024).
