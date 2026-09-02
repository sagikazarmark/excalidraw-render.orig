# Architecture Cleanup Plan

Status: reviewed twice (adversarial multi-agent critique + judge; second
multi-agent pass 2026-09-01, amendments applied), awaiting execution.
Date: 2026-09-01. Owner: Mark Sagi-Kazar.

Scope: right-size the machinery around the renderer while compatibility
revision 1 is still draft. The rendering architecture itself (SceneDocument →
normalization → relationships → ResolvedScene → per-format plans; frame
listing/extraction/rendering as separate API pieces) is sound and stays.

## Decisions

### D1 — Determinism promise scoped to artifact bytes + diagnostics

Keep resource limits and the semantic ledger. Convert the canonical resource
trace from pinned constants to a cross-target equality property:

- Remove the hard-coded trace digests in `resource_trace_qualification.rs`
  and the per-cell `canonicalResourceTraceSha256` fields in the qualification
  corpus and its schema. One atomic change-set: corpus regen + catalog reseal
  + release-evidence manifest refresh
  (`tests/conformance/svg-font-subset-release-evidence/revision-1.json` pins
  the corpus by path+sha256 and is re-verified in CI on every push) + xtask
  validator edits (`catalog.rs` presence checks, qualification cell struct)
  + schema edit + spec edit.
- Keep-list — only the sha pins go: the relative equality asserts
  (cold == warm == cached, batch == cold) and the absolute envelope-delta
  spot-checks (`working_bytes`, `total_work_units`) in the same tests stay.
- Each CI target computes trace digests fresh; the existing
  `aggregate-svg-font-subset-targets` step asserts equality across all five
  targets. In addition, `aggregate` writes the cross-target trace digest to a
  committed evidence file (restores commit-over-commit visibility: a trace
  change remains a reviewable diff, updated mechanically), and the
  qualification test gains a diagnostic mode dumping per-cell trace digests
  so an aggregate mismatch can be bisected. The wasm-vs-native nondeterminism
  canary (the class of check that caught issue #98) is preserved — per-cell
  artifact-byte pins (`generatedWoff2Sha256`) remain; the refactoring tax of
  absolute trace pins is not.

### D2 — Brotli fork retired in favor of upstream `portable-float` (PR #259)

Three stages. Note: `cargo package --locked --workspace` runs in CI on every
push, so a git dependency on brotli main breaks CI immediately — the swap
cannot merge to main before an upstream release.

- **A (now, branch only):** swap `ttf2woff2-deterministic` to
  `brotli = { git, rev = "fb3104d", default-features = false, features = ["std", "portable-float"] }`
  with the profile identity string UNCHANGED; run `catalog verify` +
  `cargo test --lib font_subset` on native + wasm to prove byte-identity.
  On the stage-A branch, also comment out the `cargo package` /
  packaged-facade workflow steps (safe — the branch never merges; they
  otherwise kill every target job before evidence materializes) so all five
  targets and the aggregate job run: a green 5-target aggregate at unchanged
  pins is the byte-identity proof (manual native + wasm alone leaves both
  darwin targets and windows-msvc unproven). Keep the release-evidence
  `pending_ci` state machine consistent with any branch-local evidence
  edits. Produce a scripted mechanical diff of the fork vs upstream 8.0.4.
  The branch never merges.
- **B (now):** request an upstream rust-brotli release containing PR #259.
- **C (on release):** pin `=8.0.x`, delete `crates/brotli-deterministic`,
  perform exactly ONE identity-string + full evidence refresh (the identity
  string feeds every subset font's name table, so it regenerates all
  font-subset evidence — never refresh twice), spec/profile edits in the
  same change-set. C runs after step 4 (D1) even if the upstream release
  lands first — the stage-A proof is banked, C is not urgent — and all other
  evidence-touching work freezes during C's change-set. At C, re-run the
  mechanical diff against the released tag (not just `fb3104d`), and compare
  decompressed sfnt tables (excluding the rewritten name table) pre- vs
  post-refresh, isolating the identity-string delta from any encoder delta.
  Fallback if no upstream release by 2026-12-01: zero-patch re-vendor of
  upstream at `fb3104d`. The fallback is not free — the identity token
  changes (to the form `excalidraw-render-brotli-0.2.0(upstream-fb3104d)`;
  the current token claims `upstream-8.0.4`, which a `fb3104d` snapshot is
  not) and it consumes the same single refresh.

## Execution order

**Step 0 — Record decisions.** ADR-0002 (D1) and ADR-0003 (D2) in `docs/adr/`.

**Step 1 — Independent quick wins (parallelizable).**
- 1a. Delete `lib.rs::__mvp` + `model.rs` + `render_svg.rs` + `backends.rs`;
  drop `anyhow`/`euclid`/`roughr` from the lib crate; fix stale crate docs
  (`lib.rs:1-4` still describes the proof-of-concept CLI). Done when the
  workspace builds and tests green with the three deps and all `__mvp`
  references gone.
- 1b. Small dead code: hand-rolled SHA-256 in `resource_accounting.rs` (use
  `sha2`); stop emitting the five unread generated field families
  (`states`, `value_rules`, `value_json`, `format_rules_json`, `when`) —
  xtask emitters + `scene_surface.rs` struct defs only, the frozen JSON
  manifest keeps the data; derive the shard count instead of asserting
  exactly 2 (`xtask/src/catalog.rs:388-393`). The emitter changes regenerate
  `scene_surface_revision_1.rs` and force a catalog reseal — include it in
  the same change-set. The neighboring `assets.len() != 230` pin
  (`catalog.rs:380`) stays: unlike the shard count, it is cross-checked by
  release evidence (`qualifiedSources: 230`).
- 1c. Wire `verify-svg-viewers` and `verify-pdf-viewers` into CI — the only
  unwired verify commands (`verify-svg-font-subsets`,
  `verify-svg-font-subset-release`, and `catalog verify` already run).
  Evidence generation stays manual. Once these gate CI, every evidence
  refresh (step 4, stage C) must update their pinned descriptor hashes in
  the same change-set.
- 1d. Create `docs/roadmap.md` and move spec §15–16 (performance protocol,
  WASM footprint) into it as aspirational. Move, do not delete — the
  footprint budget is the only pushback against 12.4 MiB of embedded fonts.

**Step 2 — Brotli track**: stages A/B/C above. Stages A/B run parallel to
everything; stage C is serialized behind step 4 per D2.

**Step 3 — Split `render/mod.rs`** along existing seams (`frame_target`,
`resolve`, `geometry`, `diagnostics`, `bound_text`, `font_demand`; relocate
inline tests; break `RenderSession::render` into phase methods). Pure code
motion — does not wait for D1, but steps 3→4→5 are strictly serial (all
three touch the same code). Land step 3 as verifiably pure motion:
`git diff --color-moved=dimmed-zebra` clean, identical test count before and
after. No in-flight frame work exists in this checkout as of 2026-09-01; if
any is open elsewhere (a PR), land or park it first.

**Step 4 — Implement D1** (trace pins → cross-target equality). Add a
batch-vs-standalone frame property test — property-based/randomized around
resource-limit boundaries, not a fixed scene list — including near-limit
scenes (`resource.*-exceeded` diagnostics are artifact-embedded, so charge
timing can change bytes). This test is the entire safety net for step 5's
replay deletion and must exist before step 5.

**Step 5 — Frame-target dedup + journal-replay simplification**, gated on
the step-4 property test (keep replay if lump-sum charging fails it). The
heavy selection logic is already shared; unify the ~40-line filter
application in both paths while preserving each path's error/diagnostic
strategy. Add the metadata `estimate`/`build` and
`manifest_json_len`/`manifest_json` agreement assertions in the same pass.
Conditionally contract-touching: if lump-sum charging changes artifact bytes
for near-limit scenes, that is a contract-visible change — pre-stage the
spec edit.

**Step 6 — `frames --list`**: dry-run mode honoring `--name-regex` /
`--max-frames`; `output`/`format` become `Option` with
`required_unless_present = "list"`; prints ordinal/kind/name/derived
basename via `--report-format human|json`; exit 0 on empty; new list-mode
report shape; spec grammar amendment in the same change-set. Extract a CLI
`frames.rs` module as part of this step (main.rs is ~2,950 lines).

## Dropped after review

- Approval-ceremony downsizing: `record-approval` is one automated command
  with format-only validation — per-use cost is near zero, and in an
  agent-driven repo the login field marks where the human signed off. Keep
  as-is.
- Interim stripping of the brotli fork (ffi/concat/bin/threading): wasted
  effort if the upstream release lands; resurrect only as the fallback.

## Not doing (deliberate)

- No "render any component" public API: the compatibility promise is only
  definable per reference-anchored export target; arbitrary subsets would
  arrive as a new target/revision reusing the internal filter seam.
- Backend element-enum unification: deferred; partly contract-driven
  (per-format fill semantics). Revisit after step 3.
- Keeping as-is: lossless JSON parser, resource limits, diagnostics
  registry, revision identity, `ResolvedScene` seam, asset shards,
  `skera-safe`, `excalidraw-render-compare`, `tools/svg-viewer-rust`,
  package-consumer tests.

## External gate

First crates.io publication is blocked on an upstream rust-brotli release
containing PR #259 (git deps cannot be published; `portable-float` does not
exist in released 8.0.4) OR the stage-C fallback re-vendor. The block is a
policy consequence of D2 (retiring the patched fork), not a technical one —
a vendored crate is publishable, and CI already packages the fork today. Pre-publish checklist:
run `cargo package` once without `--no-verify` (CI never build-verifies
packages); publish the interdependent `excalidraw-render-*` crates in
dependency order; check `.crate` tarball sizes against crates.io's 10 MiB
cap (`excalidraw-render-assets-data-000` carries ~8.0 MiB of incompressible
WOFF2). Every contract-touching step carries its spec edit in the same
change-set per the draft-revision upgrade policy.

## Deferred follow-ups (recorded 2026-09-02)

The unpinned xtask evidence modules (`conformance.rs`, `svg_viewers.rs`,
`catalog.rs`) now share one canonical-JSON serializer and digest/approval
helpers, with the ASCII-key guard applied uniformly. The pinned modules —
`pdf_viewers.rs` and `main.rs` (approval subject on issue #35),
`svg_font_subsets.rs` and `svg_font_subset_runner.rs` (issue #99), and
`svg_font_subset_targets.rs` (cross-checked against #99's subject) — keep
their duplicate copies deliberately: editing a pinned file invalidates its
human approval record, and spending re-approval ceremonies on provably
identical code is a bad trade. The approval record, not file duplication,
is the isolation boundary.

**Ride-along rule:** the next change that already forces a re-approval of
issue #35 or #99 must also fold in, for the files that change anyway:
deduplicating the shared helpers against `conformance.rs`, and porting the
ASCII-key guard into that family's `write_canonical`. Until then the guard
divergence is latent only — every committed manifest key is ASCII, and
writer-side keys are string literals.

`resolve_scene` was restructured 2026-09-02 (design-it-twice: minimal-interface
vs phase-pipeline vs deep-image-module, hybrid of the latter two): image
resolution is a deep module (`render/image.rs`, `ImageResolver` with an
`ImageDisposition` protocol owning the asset cache and image accumulators), and
the remaining phases are private session methods (`acquire_resolution_inputs`
ends both variation axes behind `ResolutionInputs`; `account_semantic_census`;
`assemble_resolved_scene` over `SceneChrome`). The orchestrator is ~40 lines
plus the element loop; byte/trace neutrality proven by the pinned suites.
