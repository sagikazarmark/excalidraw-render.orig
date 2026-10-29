---
status: accepted; amended 2026-09-11 (Dagger) and 2026-10-13 (ADR-0024 supersedes the Dagger amendment)
---

# Narrow supported targets to Linux and browser WASM

Compatibility revision 1 promised five supported targets — four native
(`x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`,
`x86_64-pc-windows-msvc`) and `wasm32-unknown-unknown` in the pinned Chromium
harness — each executing the renderer and deterministic-output tests, with a
central aggregation comparing their exact artifact bytes and resource-trace
measurements. That promise was only ever exercised by GitHub Actions, and the
five-target aggregate never assembled there: the wasm leg dropped its
measurement line until `6e5f9c4`, and the macOS and Windows legs' reports were
read off individual jobs, never aggregated.

GitHub Actions is now unavailable to this repository, and no macOS or Windows
hardware is. Keeping three targets in the contract that nothing can execute
would leave the release evidence permanently `pending` or force it to carry
open-ended exclusions for most of the target set. Neither is a promise anyone
can rely on.

Decision, 2026-09-06: supported targets are exactly `x86_64-unknown-linux-gnu`
and `wasm32-unknown-unknown` in the pinned Chromium harness. Both executed the
complete SVG font subset qualification at `6e5f9c4` with the pinned toolchain,
browser, and runner, and agreed byte-for-byte on the generated-byte and
resource-trace measurements committed in
`tests/conformance/svg-font-subset-cross-target-measurement/revision-1.json`.

Consequences:

- Spec §10 lists the two targets; §14 and §15 speak of "every supported
  target" and one native archive. The determinism promise of ADR-0002 is
  scoped to these two targets. Where ADR-0002, ADR-0003, and ADR-0004 say
  "five targets", they describe the contract as it stood when they were
  written.
- The target-evidence tooling requires exactly the two supported targets: the
  report and aggregate schemas move to v3 with the narrowed `target` set, and
  `aggregate-svg-font-subset-targets` rejects a report for any other target.
  The `packaged-public-facade` workflow's native matrix is one runner.
- While GitHub Actions is unavailable, the legs run off-CI with the
  workflow's exact commands and pinned tool identities, and the maintainer's
  attestation comment on the originating issue — environment, commands,
  reports with digests — is the aggregate's provenance in place of a run URL.
  `record-svg-font-subset-target-aggregate` accepts either.
- Re-adding a target is a spec amendment plus evidence: the target must
  execute the qualification with the pinned tool identities and agree on the
  committed measurement before it enters the supported set. Nothing in the
  renderer changes to support or drop a target; only what is promised does.

Amended 2026-09-11: Dagger is the canonical CI and evidence runner for both
supported targets. GitHub Actions is an optional caller, not the authority for
the commands or their results. The maintainer attestation path above is valid
for Dagger runs regardless of Actions availability; record the tested commit,
lock, command, reports and digests, plus a trace URL when available. The existing
`pending_ci` manifest state means pending target aggregation on any runner.
See `docs/ci.md` for the canonical operation inventory. This changes orchestration,
not the supported target set or ADR-0003's serialized evidence-refresh policy.

Amended 2026-10-13: ADR-0024 replaces Dagger with recipes in the devenv shell
as the runner, retires the tool manifest for `devenv.lock`, and lets the
browser-WASM target run in any browser. The supported target set is unchanged.
