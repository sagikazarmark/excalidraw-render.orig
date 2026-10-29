# Metadata failure diagnostic ownership (#85)

Base: `97128331a18e250af876c3e0c7361a691dfb8441`.

## Proposed owners recorded

`metadata_failure_contract::metadata_failures_have_exact_recovery_in_every_format`
is a root library test that calls the public `Renderer` scene and frame methods.
It exercises all seven registered metadata failure codes in SVG, PNG and PDF,
including the shared scene-wrapper and format-specific scene-carrier producers
of `metadata.scene-embedding-failed`.
PDF's format-carrier compression site is separately exercised under the scene
compression code. All **21 diagnostic/format obligations** now bind this exact
library-test selector and its source digest. The global ledger reports **843
owned entries**; with the subsequently approved three generic-fallback exclusions,
current totals are **846 assigned/excluded and 82 gaps**. All 122 current
diagnostic/format entries have a proposed owner or approved exclusion. Global
approval remains pending.

These are **operation-failure contracts**, not claims that valid scene input can
make an otherwise deterministic serializer fail. The specification requires
recovery when a metadata operation returns a non-resource failure. Private
test-only refusals exercise that protocol at the real operation call sites;
expected diagnostics are authored independently of the production registry.

## Operation seam and production corrections

`render::metadata::operation` classifies failures by their actual serialization,
compression or carrier site. Provenance, diagnostic-summary and scene
serialization now retain their own registered identities instead of being
unrepresented or folded into manifest failure. A containing manifest operation
preserves an already-classified component error. Physical allocation refusal
remains `resource.memory-unavailable`; in particular, PDF XMP preparation no
longer relabels every error as optional manifest embedding failure.

Test builds install scoped thread-local refusals during metadata selection.
Each refusal occurs after the real operation and before its result is accepted,
requiring recovery to discard candidate metadata rather than exposing partial
bytes. The state is restored on unwind and isolated across threads. Selection
contexts can target one frame, or a complete manifest containing a prepared
scene. They model deterministic failures of the selected operation, rather than
one-off final-emission divergence. Production builds contain neither the fault
state nor its contexts, and the request/public renderer interface is unchanged.

The combined-failure regression initially observed summary failure before
provenance failure because final sorting used lexical code order for root
diagnostics. Metadata selection now records its semantic component position so
failure and resource-omission events retain provenance, summary, scene order.

Manifest storage reserves its exact estimated length, including escaped frame
names, before writing. This removes an implicit string-growth path for long
frame names while retaining the existing artifact bytes and logical estimates.

## Evidence

The aggregate executes the following independent checks:

- Full ordered reports, exact code/severity/format/effects/component/operation/
  recovery/owner/pointer, and nonempty messages for each operation and format.
- Byte-identical artifacts against independently rendered requests that omit
  precisely the failed component; healthy controls and fresh sessions verify
  repeatability and preserve the other requested components.
- Exact manifest structure, canonical-source digest and the frozen pre-metadata
  diagnostic summary, which excludes subsequent metadata warnings.
- Real mandatory notices: embedded font notices in SVG/PDF, and the official
  error-image recipe notice in PNG. Component failures preserve notices; whole
  manifest failures are fatal and publish no artifact.
- Early and scene-bearing manifest failures, including discarding previously
  accepted provenance/summary and a prepared scene payload.
- Physical allocation refusals, including required-notice and late-manifest
  cases, remain fatal resource failures without an optional-metadata warning.
- Disabled operations and generation-limit preemption do not execute the fault
  site. Later components continue after recoverable failures.
- Multiple failure ordering and the exact retained report prefix when the
  required omission diagnostic itself exhausts `max_diagnostics`.
- Per-frame isolation, direct/prepared batch replay, an unaffected second frame,
  and escaped long target names after component recovery.
- Scoped fault cleanup after panic and isolation from a concurrent render.

## Executable ownership adapter

Integration tests cannot use a library's private `cfg(test)` seams. The explicit
`binary: lib` test adapter admits a root `src/<module>.rs` registered exactly as
`#[cfg(test)] mod <module>;` in the renderer's default `src/lib.rs`. It retains
the built-in, unconditional, non-ignored test-function checks of the fast adapter
and binds the exact command:

```sh
cargo test --locked --release -p excalidraw-render --lib MODULE::FUNCTION -- --exact
```

The verifier parses the Cargo manifests with pinned `toml` tooling. It requires
explicit workspace membership without exclusions and rejects disabled,
redirected, procedural-macro or custom-harness library targets. `toml` is an
xtask-only dependency, not a renderer dependency. Syntax tests and a live-source
ledger test cover guard/selector/digest rejection. The fast adapter's existing
restrictions remain in force.

Commands:

```sh
target/debug/xtask conformance record-library-test-owner MODULE FUNCTION RATIONALE ID...
target/debug/xtask conformance refresh-library-test-owner-source MODULE
```

## Verification

Using Rust 1.97.1 explicitly and `CARGO_INCREMENTAL=0`:

- `cargo test --locked --workspace --no-fail-fast`: **1,084 passed, 18 ignored**.
  The fast tier passed all 513 tests in 894.50 seconds. Ignored tests require
  separate harnesses or explicit qualification/materialization runs.
- The **10 metadata contract tests** and **13 ownership-adapter tests** passed
  again after replacing positional test-oracle flags with named fields. That
  readability cleanup followed the full workspace run.
- Workspace/all-target Clippy with `-D warnings`, library rustdoc with
  `RUSTDOCFLAGS="-D warnings"`, and production renderer compilation for
  `wasm32-unknown-unknown` passed.
- Catalog verification, all **37** registered conformance cases, draft inventory
  verification, ownership verification and `git diff --check` passed.
- Standards review found no hard breach; its test-oracle readability concern was
  addressed. Spec review found no implementation defect; its pending-assignment
  finding was resolved by recording and verifying all 21 bindings.

The three generic known-element fallback exclusions have explicit digest-bound
[#85 approval](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5700122166).
The eight release milestones and #178 target-equality blocker remain separately
tracked. This work claims no new cross-target execution equality or global ledger
approval.
