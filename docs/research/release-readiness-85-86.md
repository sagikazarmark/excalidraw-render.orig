# Release readiness research: #85 and #86

## Implementation follow-up

The first draft inventory/result boundary now lives in
`tests/conformance/coverage/` and `xtask/src/coverage.rs`. It derives 320 field/label
rows and 168 all-format execution keys, records eight unresolved milestones,
validates current-case/source bindings, and compares result bundles across two
targets, two process orders and two sessions. Dagger checks inventory consistency
and can stage regeneration. The checks reject malformed/stale/incomplete/divergent
result sets. This is a foundation for checklist steps 1/4/6, not their closure:
global approved ownership, the complete atomic risk inventory, canonical snapshots
and actual collectors are still pending. The detailed audit below records the
state at `fd52a13`, before this follow-up.

## Release checklist — audit conclusion

**Neither #85 nor #86 is ready to close.** Existing focused-case approvals,
packaged native/WASM execution, and the completed font-subset aggregate are
real reusable evidence. The missing release deliverables are global coverage
closure and a coverage-complete canonical snapshot/target-result pipeline.
This audit adds documentation only; it does not regenerate evidence, alter
approvals, change issue states, or claim a full release qualification run.

| Order | Remaining deliverable | Existing work to reuse | Completion test / owner |
| --- | --- | --- | --- |
| 1 | Authoritative behavior/risk inventory and global ownership ledger | Scene-surface inventory, case coverage labels, exact vectors and specialized tests | Unknown/uncovered rows and duplicate primary owners fail a negative verifier test. Approve the ledger digest. **#85** |
| 2 | Generalize corpus schemas and runners for representative cases, crops and all applicable format judgments | 37 approved registered cases, public-scene intake pool, specialized PDF/frame gates | Promote representative cross-feature cases; connect every layer/format owner and judgment subject. **#84 → #85** |
| 3 | Finish codec/nested-SVG and complete-render fuzz reach | Existing codec regressions, scene/font fuzz, all-format frame properties | Generated/mutated assets and requests exercise every admitted codec and SVG/PNG/PDF projection, metadata, limits and failures; confirmed failures replay permanently. **#82 → #83 → #85** |
| 4 | Canonical public-API snapshots and fresh-session/process-order runner | Package-consumer expectations and repeated-call tests | Every corpus variant has exact artifact and serialized-report expectations; separate forward/reversed or deterministic-shuffle processes agree. **#85**, feeding **#86** |
| 5 | Complete browser public-facade risk coverage | Both-target consumer, PDF risk/fallback matrices, font corpus | Coverage-complete native/WASM case results, including one checked all-format mixed smoke, risk fonts/assets, ordered diagnostics and failures. **#86** |
| 6 | Full-risk target report schema and direct aggregator | Font aggregate's exact target-set/identity checks and provenance/binding mechanism | Missing/duplicate cases, missing/extra targets, changed artifacts/reports/traces or mismatched run inputs fail. Preserve trace equality rather than adding historical per-cell trace pins. **#86** |
| 7 | Make complete risk execution and aggregation automatic Dagger checks | Existing `packaging`, `wasm` and conformance checks | `dagger check` executes the current complete risk set and aggregate; an old recorded pass cannot stand in for current execution. Tag/packaged-source repetition feeds **#95**. |
| 8 | Final corpus/ledger/snapshot approvals and clean-run evidence | Existing approval-subject commands and issue attestations | All #85/#86 criteria demonstrated and recorded; then proceed through dependency audits **#87**, distribution qualification **#88**, upgrade mechanics **#94**, protected-tag release **#95**. |

Steps 1 and the initial snapshot/result runner can begin using current cases;
they need not wait for every representative scene or fuzz expansion. Closure
requires the final inventory, not merely a first green tracer case. There is
no defensible numeric “percentage complete” until that inventory exists.

### Corrections to older release tickets

- #45 is complete: retain its baseline packaged execution rather than rebuild it.
- The supported set is exactly Linux x86-64 and browser WASM; five-target wording
  in older comments is historical (ADR-0006).
- The font-subset release evidence is already `aggregated` / `pass` at this
  audit's base. #86 still needs the **whole renderer** result set, not another
  font identity refresh.
- AVIF is excluded from revision 1; cover its rejection, not decoder admission.
- #95's performance/footprint acceptance text is stale against current spec
  §15 and `docs/roadmap.md:3-17`. Issues #91–#93 are closed; those protocols
  remain dormant and must not reappear as release gates without activation.
- #154's dark-mode nested-SVG PDF limitation is explicitly in the current spec
  (`docs/renderer-spec-v1.md:863-869`). Choosing to change it before revision 1
  becomes retained is a maintainer decision, separate from asserting #85/#86
  coverage of the currently documented behavior. First publication freezes the
  revision (`:3303-3312`).

## #85 — Corpus coverage, approvals, and canonical snapshots

**Finding: #85 remains incomplete, despite substantial implemented conformance infrastructure and currently valid registered-case approvals.** The remaining work is to close the five-layer corpus and its global ownership inventory, extend judgment bindings/enforcement to the complete corpus, and implement corpus-wide renderer-owned byte snapshots with fresh-session and fresh-process order verification. Finishing prerequisites #83 and #84 is necessary but does not implement those closure mechanisms automatically.

### Scope and evidence basis

- Researched 2026-09-11 at commit `fd52a130e6a513ecb74d58bee1d1f493744f4139`, initially clean working tree, in `/home/laborant/excalidraw-render.orig`.
- This section assesses **#85**; the #86 assessment follows below.
- Read `AGENTS.md`, `docs/agents/domain.md`, `CONTEXT.md`, the current specification, and relevant ADRs. The controlling requirements are `docs/renderer-spec-v1.md:2975-3053` (§14.2), `:3231-3264` (§14.4), and `:3318-3324` (release-blocking coverage versus dormant protocols).
- Source citations below are repository-relative `path:line` references at the researched commit. Canonical JSON artifacts commonly occupy one line; commands below identify the exact fields counted rather than treating truncated displays as complete evidence.
- Issue sources: [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85), [#82](https://github.com/sagikazarmark/excalidraw-render.orig/issues/82), [#83](https://github.com/sagikazarmark/excalidraw-render.orig/issues/83), [#84](https://github.com/sagikazarmark/excalidraw-render.orig/issues/84), [#34](https://github.com/sagikazarmark/excalidraw-render.orig/issues/34), [#35](https://github.com/sagikazarmark/excalidraw-render.orig/issues/35). Read their bodies/states/comments with `gh issue view <number> --json number,title,body,state,comments,url` (some calls also requested labels). #85/#82/#83/#84 were open; #34/#35 were closed. Implementation conclusions below come from code and evidence, not checkbox state.

### Acceptance-criteria assessment

| #85 criterion | Already implemented / verified | Exact remaining work | Status |
| --- | --- | --- | --- |
| All five corpus layers; every behavior, difference, exception, diagnostic, raster class and interaction has one owner or valid exclusion | Focused approved reference cases; exact vectors; bounded property tests; scene/font minimized-regression manifests; a separate representative font-size board. `verify-all` passes all 37 registered cases. | Give the five layers a complete manifested coverage/runner contract; promote representative cross-feature scenes under #84; build a closed authoritative inventory and approved global ledger; assign exactly one primary owner or evidence-backed exclusion to every inventory row. | **Incomplete** |
| Approval subjects bind all judgment-bearing bytes; CI rejects missing/stale/duplicate/dead/unapproved/uncovered records | Per-case canonical subjects bind manifests and referenced artifacts/reports/provenance; schemas, hashes, local coverage, variants and approvals checked offline. Dagger invokes these verifiers. | Bind the coverage-ledger digest, canonical renderer expectations and remaining judgment inputs; generalize case format/layer/crop support; enforce inventory completeness and cross-case ownership; account for specialized PDF cases/sidecars and all executable owners; add negative verifier cases proving each rejection. | **Partial** |
| Every corpus variant renders twice in fresh public-facade sessions; risk set also passes reversed/shuffled fresh-process order | Public render calls create new sessions; reference generation uses two browser processes; selected package-consumer artifacts/reports have pins and repeats; rectangle formats are reversed in one process. | Generate Linux release-mode public-API canonical artifacts and serialized reports for every corpus variant/format/target; replay twice; declare a coverage-complete risk set and execute forward versus reversed/shuffled order in distinct fresh processes. | **Partial** |

### 1. Measured inventory: what exists, and what the counts mean

| Inventory queried | Observed result | Meaning / limitation |
| --- | --- | --- |
| `tests/conformance/cases/registry.json:1` | **37 cases: 35 `scene`, 1 `typography`, 1 `frame`** | These are registered cases, not all tests or all directories. The two specialized entries omit the registry `layer` field. |
| `tests/conformance/cases/focused/*/manifest.json:1` | **37 manifests, all with non-null approval; 36 say `focused`, 1 frame manifest omits layer** | Approval presence was additionally verified against bytes with `verify-all`, not merely counted. |
| Those manifests' `.variants` | **49 scene/typography variants, 49 SVG and 49 PNG format records** | No PDF record in this manifest shape; this does **not** mean no PDF evidence or tests exist. |
| `focused/frame-export/manifest.json:1`, `.targets[].outputs` | **7 frame targets, 7 SVG and 7 PNG output pairs** | Separate shape, omitted from the 49-variant count. The frame manifest also has no `.coverage` field; its verifier hard-codes semantic coverage. |
| `.coverage[]` across the 37 manifests | **226 primary declarations, 118 distinct identities, 27 identities repeated across manifests** | These are declared coverage labels, not an exhaustive behavior inventory or percentage covered. `reference.svg` and `reference.direct-png` each have **36** primary owners. |
| Registry `.coverage[]` alone | **221 primary declarations, 115 distinct identities** | Different from manifest totals because specialized entries keep metadata elsewhere. Do not mix denominators. |
| Those manifests' `.crops` | **0 crop records** | Focused scene schema currently forbids nonempty crops; mixed representative typography/non-text crops still need support. |
| Focused scene JSON, literal element/payload census | **12 element kinds; 7 payload MIME types; 4 elements with explicit opacity != 100; 19 with explicit nonzero angle; 2 with nonempty groupIds** | A literal input census, including frame elements; not proof of every semantic path or admitted asset class. |
| `crates/excalidraw-render/tests/qualification/scene-fuzz-regressions.json:1` | **22 cases, 10 declared risk classes**, originating issue 81, broader issue 83 | Real replay corpus for admission/normalization/SVG; not whole-renderer fuzz completion. |
| `crates/excalidraw-render/tests/qualification/font-subset-fuzz-regressions.json:1` | **12 cases plus 1 public-facade scene**, broader issue 83 | Separate bounded font-subset scope. |

The directory `tests/conformance/cases/focused/` contains **38 directories**, including unregistered `pdf-isolated-fallback`. It is used by specialized PDF/size tests, so it is **not automatically a dead case**. `crates/excalidraw-render/tests/harness_pdf_viewers/backend_qualification.rs:54` consumes the specialized fallback evidence; `crates/excalidraw-render/tests/qualification/svg-font-subset-size-corpus.json:1` also lists it. A future dead-case detector must map these real owners, rather than equating “not registered” with “unused.”

The five layers are therefore unevenly integrated:

1. **Exact vectors exist:** e.g. Rough.js operation/fill vectors in `crates/excalidraw-render/src/render/tests.rs:2860-3194`, plus normalization, diagnostic and semantic-accounting checks in that module. They are not all connected to a global conformance ownership ledger.
2. **Focused reference scenes exist and are approved:** all 37 registered cases pass offline verification. `xtask/src/conformance.rs:139-221` handles general scene cases; `:224-300` handles the specialized frame case.
3. **Representative material exists, but #84's corpus is absent:** `tests/public-scenes/README.md:3-18` explicitly identifies its 15 public scenes as raw material, unmanifested/unsanitized/unapproved and outside gates. A separate `representative-mixed-font-board.excalidraw` is real test input, but its size-corpus test compares subset SVG against complete-chunk control, not official cross-feature SVG/PNG/PDF conformance (`crates/excalidraw-render/src/render/tests.rs:182-235`; size manifest `:1`). Neither satisfies #84's acceptance criteria.
4. **Adversarial/resource tests exist:** fixed parser boundaries (`crates/excalidraw-render/tests/limits_scene_admission.rs:1-52`), generated frame budget boundaries (`src/render/frame_batch_property.rs:51-73,157-180`), and nested-SVG depth/fallback-limit tests (`tests/fast/nested_svg_asset_render.rs:122,591`). They still need explicit manifested global ownership.
5. **Permanent regressions exist:** scene corpus checks schema, IDs, risk completeness, bounds, claimed minimization and replay outcomes (`src/scene_fuzz/corpus.rs:112-239`); font regressions have their own schema/risk set (`src/render/font_subset_fuzz.rs:8-27,45-89`). A `minimized: true` assertion checks metadata, not whether minimization was actually performed or whether every discovered failure was recorded.

### 2. Coverage ledger: the central missing mechanism

The normative ledger must cover every rendering-affecting upstream behavior, format-specific difference, compatibility exception, diagnostic row, decoder/profile/orientation/alpha/animation class and declared interaction. Every row needs exactly one primary owner or an approved proof of unreachability/no static-export effect; lack of evidence is not an exclusion (`docs/renderer-spec-v1.md:3000-3013`).

No global test-ownership ledger/enforcement was found. The existing mechanisms do narrower jobs:

- `tests/oracle/scene-surfaces/revision-1.json:1` has `coverage.status: "complete"`, `fieldDispositionCount: 202`, `unresolved: []`, an upstream type inventory and five official fallback recipes. This is a **field/value disposition and fallback-policy inventory**, with its own semantic completeness checks (`tests/oracle/scene-surfaces/README.md:3-14`). It does not assign every conformance behavior/diagnostic/codec interaction to an executable case and variant. Its “complete” is valid in that narrower domain.
- `xtask/src/conformance.rs:49-119` reads the registry and rejects duplicate **case IDs**. It does not reject duplicate coverage ownership across cases.
- `:762-790` requires each general scene's primary labels to match that same registry entry, rejecting duplicates **inside that one manifest**. An identity introduced consistently into both files is not checked against a separately closed global inventory.
- `xtask/src/main.rs:86-93` implements `verify-all` by iterating registered IDs. It does not enumerate every disk case/test owner or compare coverage against the full revision inventory.
- Concrete duplicate ownership already passes the verifier: registry `:28-44` makes rectangle primary for `reference.svg`/`reference.direct-png`; `:124-144` does the same for freedraw. The measured manifest total is 36 owners each. Several actual behavior labels also duplicate (`transform.rotation`: 3; `text.align`: 4; `frame.name`: 3).

**Required implementation:** establish the inventory and ownership model first; separate reusable feature/risk tags from one primary owner; connect all specialized layers/format expectations to it; record an approved ledger digest; make missing inventory, unknown IDs, duplicate owners, stale links, absent required formats/variants, orphaned/dead cases and unapproved exclusions fail CI. A zero uncovered count must be computed against that inventory, not asserted as a free-standing metadata flag.

### 3. Approval binding and verifier enforcement

**Implemented and verified:**

- `xtask/src/conformance.rs:1036-1066` enforces canonical manifest spelling and adjacent digest; `:1859-1923` rehashes file lengths/bytes and constrains descriptor paths; `:1926-1938` builds the canonical manifest-plus-file-descriptors subject after removing only `.approval`.
- General scene verification checks schema, current reference identities, assets, scene, variants, provenance, reports and approval (`:139-221`). Variants must match registry IDs/options/order/count and have exactly SVG and PNG (`:1400-1487`). Reports must match the declared ordered diagnostic JSON (`:1378-1397`).
- Approval has exactly four fields and binds the computed subject; identity/date/comment-URL syntax and the configured repository issue are checked (`:1825-1856`). This is an **offline scoped-link check**, not a GitHub API verification that the named account authored the comment or is the designated maintainer. Human approval remains the authority required by spec §14.4.
- `record_approval` refuses existing approval and a supplied subject mismatch (`:988-1025`). Reference generation refuses to replace approved evidence (`tools/reference-oracle/scripts/conformance-case.mjs:541-545`).
- Dagger's registered-case, SVG-viewer and PDF-viewer checks are wired (`dagger.dang:183-220`). The three offline verifiers passed in this research session.

**Incomplete scope and actionable gaps:**

- There is no ledger subject to approve yet. Case approval cannot bind missing inventory/ownership decisions.
- The general verifier forces `layer == "focused"` (`xtask/src/conformance.rs:178-180`), contributor MIT/Apache license metadata, and `sanitization.performed == false` (`:803-826`). Although the scene schema enumerates all five layers, this code rejects representative external cases; its crops schema has `maxItems: 0` (`tests/conformance/focused-scene-case-v1.schema.json:107-110,140-162`). #84 needs schema/generator/verifier and harness support as well as scene selection.
- General case `comparison.comparator_profiles` must be empty and its sole viewer scope is **reference evidence repeatability only** (`xtask/src/conformance.rs:940-963`). Do not present these manifests as already binding the full canonical viewer/comparator judgments required by §14.4.
- Specialized PDF expectation/exception files are not all inside the registered case subject: frame descriptor collection binds only scene/report/provenance and SVG/PNG outputs (`xtask/src/conformance.rs:303-330`); the typography collector likewise excludes its PDF sidecars (`:597-621`). The established PDF convention explicitly stores exceptions outside the case approval subject and pins their file digest in the consuming test (`tests/conformance/README.md:447-460`). For example, `harness_pdf_viewers/excalifont_text.rs:30-39,91-100` pins a PDF exception and structure expectation; the exception itself records issue 73 (`focused/excalifont-text/pdf-visual-exception.json:1`). **These are genuine issue-backed judgments with immutable test pins, not unapproved by definition; but a test pin is not the complete canonical approval subject #85 promises.** Consolidate or explicitly bind each judgment through its own approved subject, including viewer identity, measured ceilings and all structure expectations.
- Stale-exception enforcement is uneven: direct PNG explicitly requires an exception still to fail the canonical budget (`parity_registered_case_png.rs:14-19,135-146`); the Excalifont PDF gate only compares against its exception ceiling (`harness_pdf_viewers/excalifont_text.rs:69-88`). Extend the stale check wherever still absent; the frame PDF gate already has it per `tests/conformance/README.md:454-460`.

Preserve the **producer-only binding rule** (`tests/conformance/README.md:237-251`): bind judgment/evidence-producing inputs, not every consuming test or CLI dispatch file. ADR-0005 `:23-32,49-55` also limits comparator identity to its own source/dependency closure. #85 needs stronger judgment scope without reintroducing irrelevant reapproval churn.

### 4. Canonical snapshots: distinguish reference repeats, session repeats, and process order

The spec requires renderer-owned snapshots generated on pinned Linux with `--release --locked`, public API only, **every corpus variant twice through fresh sessions**, plus a risk set in another process with reversed/deterministically shuffled order (`docs/renderer-spec-v1.md:3253-3264`).

| Existing mechanism | What it actually proves | Missing for #85 |
| --- | --- | --- |
| Oracle `renderOnce` twice per variant and `assertRepeatable` (`tools/reference-oracle/scripts/conformance-case.mjs:275-288,533-537`) | Distinct Chromium PIDs; equal reference encoded-artifact/raw-raster hashes, dimensions and request sets. | This is reference-renderer generation, not renderer-owned public-facade snapshots. |
| Manifest `fresh_process_runs: 2` and verifier (`xtask/src/conformance.rs:792-801,1720-1786`) | Checked historical provenance declarations and matching recorded runs. | Offline verification does not execute browser or renderer processes. |
| Registered PNG/SVG parity harnesses | Each variant rendered once, exact dimensions/reports and geometric/raster parity. `common/registered_cases.rs:132-174`; PNG `:75-149`; SVG `parity_registered_case_svg.rs:299-319`. | No second render per variant, no whole-artifact canonical byte snapshot for this full set. Worker scheduling is not a controlled reversed-process order test. |
| Registered report comparison | Full ordered diagnostic data compared as JSON values (`tests/common/reports.rs:1-5,24-29`). | Does not pin serialized report byte spelling for all corpus variants. |
| Package-consumer rectangle | Checked SVG/PNG/PDF artifact bytes and serialized report, forward then reversed **format** order (`tests/package-consumer/tests/public_facade.rs:152-180,460-489`). | Same process, one scene; not a fresh-process reversed corpus risk set. |
| Package-consumer PDF risk set | Four PDF scene cases have artifact/report hash pins and two renders (`:70-95,398-425`); eight PDF fallback families have separate pins (`:96-145,430-457`). | Not a risk inventory for every backend/decoder/font/metadata/diagnostic path; fallback-family loop renders only once. |
| Other selected repeats | Excalifont SVG and noisy PNG repeated (`:185-214,358-393`); public direct/lazy frame equivalence (`:218-249`). | Useful existing coverage, still not corpus-wide snapshots or process-order proof. |
| Frame properties | SVG/PNG/PDF batch-vs-cold equality of bytes, reports, traces around semantic limits (`src/render/frame_batch_property.rs:51-153`). | Internal traced seam, generated subset, not canonical public-API snapshots. |

Fresh sessions themselves are implemented: every scene call constructs `RenderSession::new` (`src/render/mod.rs:99-123`), every frame call constructs `new_target` (`:126-157`). Reusing one immutable `Renderer` does not invalidate session freshness. The missing requirement is systematic execution/comparison coverage and the separate-process order driver.

**Remaining deliverable:** a machine-readable canonical renderer expectation set mapping corpus case + variant + export target + format + options to exact artifact and complete serialized report bytes/digests, with canonical Linux generation identity and approved bindings; a public-facade replay runner that refuses missing/extra variants; and a declared risk-selection map covering every category in §14.4. Compare forward process results against reversed/shuffled fresh-process results by semantic key, not output position. Expose these #85 artifacts/results for the separate #86 target work without treating existing font-only evidence as whole-corpus completion.

### 5. Prerequisites #82/#83/#84: current reach and remaining work

**#34/#35 are implemented prerequisites.** Both current qualification manifests pass their offline verifiers. Their issue histories carry approved subjects and later reapprovals; the latest inspected PDF round is [#35 comment 5609745623](https://github.com/sagikazarmark/excalidraw-render.orig/issues/35#issuecomment-5609745623). Qualifying the harness does not qualify every renderer scene: canonical SVG candidate gating is still rectangle-specific (`tests/harness_svg_viewers.rs:1-19`); PDF candidate gates cover named modules rather than all registered cases (`tests/harness_pdf_viewers/main.rs:1-19`).

**#82 is an indirect prerequisite through #83, not a direct blocker listed on #85.** Existing codec/sanitizer unit and facade tests are meaningful: APNG preservation/flattening (`tests/fast/static_png_asset_render.rs:75-115`), PNG EXIF orientation (`:647`), GIF/WebP extra-frame accounting (`static_gif_asset_render.rs:70`, `static_webp_asset_render.rs:61`), ICO selected-entry limits (`static_ico_asset_render.rs:57`) and recursive SVG limits (`nested_svg_asset_render.rs:122`). These do not amount to the required nightly generation/mutation across all codec profiles, orientations, alpha/animation modes, XML/CSS/references/data images and fallback paths. The registered `image-codecs` case declares one payload per family, crop and flips (`registry.json:879-1009`), not a complete accepted-profile/color-path matrix. Expand and characterize that matrix under §14.2; do not infer coverage from MIME family counts.

**#83 has a substantive foundation, but complete rendering is not fuzzed yet:**

- Scene fuzz has 64 fixed-seed PR inputs and defaults to 4,096 nightly (capped at 16,384), with explicit input/element/point/mutation bounds (`src/scene_fuzz/mod.rs:66-75,250-280`). It checks canonical preservation, normalization idempotence/order/diagnostic locations and all three formats' normalization (`:363-419`).
- Actual public rendering is **SVG only**, using a default request and two fresh `Renderer::new().render` calls (`:1125-1185`). Merely normalizing PNG/PDF does not reach their encoding, metadata, projection or atomic assembly.
- Structured file generation supplies one fixed PNG data URL, malformed PNG, an external URL or invalid/missing fields (`src/scene_fuzz/generator.rs:202,757-799`). It does not generate JPEG/GIF/WebP/BMP/ICO or nested-SVG payloads, much less the full #82 path matrix.
- Scene regression replay checks 10 risk classes and repeats every case; pinned admitted reports are only `(code, json_pointer)` records and pinned render outcomes only artifact-vs-failure/code (`src/scene_fuzz/corpus.rs:82-115,162-239`). Full render reports are compared between fresh calls inside the exerciser, but are not full canonical report snapshots in this regression manifest.
- Font fuzz has its own seven-risk set and bounded tests (`src/render/font_subset_fuzz.rs:8-27`); its facade paths likewise construct SVG requests (`:301,625`). Frame batch properties reach all formats and limit boundaries, but do not replace whole-renderer generated scene/request fuzzing.
- Nightly wiring exists for **font-subset-fuzz, frame-batch-property, scene-fuzz** (`dagger.dang:488-570`; `.github/workflows/svg-font-subset-fuzz.yml:3-25`). The schedule is code evidence, not proof of successful recent nightly executions. Add codec/SVG and complete-renderer operations to the canonical runner and retain execution evidence. Do not prescribe a particular fuzz engine: the issue requires reach and bounded outcomes, not `cargo-fuzz` specifically.

To finish #83: expand generated requests/inputs through SVG, PNG, PDF, metadata, diagnostics, compatibility/PDF fallback and semantic resource limits; assert deterministic complete outcomes, bounded failure/termination and no partial artifact exposure; minimize every confirmed failure into permanent public-facade replay with the appropriate expectations. #85 then assigns owners to these regressions and risk paths.

**#84 is still needed, but its older audit is stale as a feature inventory.** The [maintainer intake comment](https://github.com/sagikazarmark/excalidraw-render.orig/issues/84#issuecomment-5509477654) names scene, licensing/provenance, sanitization sign-off, evidence generation and approval. The [September 4 audit](https://github.com/sagikazarmark/excalidraw-render.orig/issues/84#issuecomment-5533968694) described 13 synthetic registered cases. Current source includes line/roundness/rotation/opacity/groups, Virgil/Lilita/Comic/other text families, emoji/missing-glyph, iframe and embedding cases (registry `:2439,2577,3089,6716,7287`; scene census above). Do not re-open those as absent. PDF viewer gates are wired to the harness tier rather than forgotten `#[ignore]` tests (`.config/nextest.toml:58-64`; PDF harness `main.rs:1-19`).

What remains is approved **representative cross-feature interaction coverage**, with all applicable variants/formats, exact dimensions/semantics/reports, whole and focused crops, visual/resource outcomes and repeated bytes. The pool already has licenses/provenance and a content review (`tests/public-scenes/README.md:73-109`), so build on that work; the immutable sanitized promotion and per-case approvals still need recording. Generalize the focused-only verifier and empty-crop assumptions before claiming the documented generation command can already accept these cases unchanged.

### 6. Scope corrections: active requirements versus dormant or excluded behavior

- **AVIF admission is not a revision-1 release task.** Current spec §8.2 explicitly excludes AVIF and requires `image.unsupported-format` (`docs/renderer-spec-v1.md:722-725`); later-revision qualification is `:1749-1753`. The rejection path has a facade test (`tests/fast/static_png_asset_render.rs:802`). Inventory/own that required rejection, rather than reviving the older #84 audit's AVIF promise.
- **`render.no-meaningful-artifact` is reserved and unreachable in revision 1** (`docs/renderer-spec-v1.md:2173-2174`). A global inventory can carry an approved unreachability exclusion with proof; do not manufacture a runtime emission solely to mark it covered. Other exclusions still need their own evidence and approval.
- **Dotted-fill official byte evidence cannot be demanded unchanged:** reference `DotFiller` uses unseeded randomness and the two-process generator rejects it. Existing JS-derived vectors and a facade fixture provide evidence (`tests/conformance/README.md:185-194`; `src/render/tests.rs:3087-3194`). This behavior still needs a valid owner; randomness is not proof that the rendering behavior is irrelevant/unreachable.
- **A visual class is not a corpus layer.** Frame tests use `MixedRepresentative` for some targets (`parity_frame_export.rs:61-65`); that does not create a licensed representative-layer case or the missing crop evidence.
- **Known defects, compatibility exceptions, reference authority divergences and rasterizer limitations are different records.** Current registered PNG/SVG known-defect lists are empty (`parity_registered_case_png.rs:42-49`; `parity_registered_case_svg.rs:90-95`). Existing divergence/limitation pins are not defects waiting for a fix (`CONTEXT.md:43-85`; `tests/conformance/README.md:111-183`). Bind/account for their judgments; do not treat every nonzero metric as a release defect.
- **Only Linux and browser WASM are supported** (ADR-0006 `:23-39,51-58`), superseding older ADR references to five targets. Dagger is canonical; Actions is an optional caller. This research does not assess #86's target aggregation.
- **Performance and canonical WASM-footprint protocols are dormant**, not #85 release blockers (`docs/roadmap.md:3-17`; spec `:3318-3324`). This does not make active conformance/font subset requirements optional.
- **Resource trace stability is an equality property, not a request to restore per-cell trace pins** (ADR-0002 `:7-25,30-39`). ADR-0007 `:40-58` establishes portable math/lint policy; its historical broad corpus measurement (`:84-110`) is useful precedent, not the systematic current all-variant snapshot mechanism §14.4 requires.

### 7. Actionable remaining #85 checklist, in dependency order

1. **Freeze the conformance inventory and ownership schema.** Derive rows from the scene-surface authority, spec differences/exceptions, diagnostic/error registry and accepted codec/profile/animation/interaction inventory. Identify runnable owner + case/variant/format/target or proof-backed exclusion; approve inventory/ledger digests. Reclassify duplicated `primary` metadata as tags/secondary evidence or choose one actual primary owner.
2. **Integrate all five layers.** Map existing exact vectors, boundary tests and minimized regressions before adding cases. Include specialized frame/PDF owners. Implement representative/external licensing/sanitization and crops in schemas, generators, verifiers and parity runners; complete #84 promotions and remaining #82/#83 corpus inputs. Empty-directory creation or adding layer tags alone is insufficient.
3. **Complete evidence and judgment scope.** For each inventory owner, declare and execute applicable formats/variants, exact semantics and complete ordered reports, whole/crop classes, visual and resource expectations. Bind all profiles, PDF sidecars, exceptions/divergence decisions and renderer canonical expectations through appropriate approved subjects; preserve existing valid approval evidence and producer-only policy.
4. **Make closure fail closed in CI.** Extend `verify-all` (or a dedicated mandatory closure verifier) to reject unknown/uncovered identities, missing/stale owners/variants/formats, cross-case duplicate primary ownership, dead/unowned cases, invalid/unapproved exclusions, missing/stale approval subjects and stale exceptions. Test those failures with targeted verifier negative cases. A passing per-case verifier is necessary but no longer sufficient.
5. **Implement canonical renderer snapshots and replay.** On pinned Linux `--release --locked`, generate via the public API every corpus variant's artifact and complete serialized report expectations; replay twice through fresh sessions. Include scene and frame export plus typed failure/report expectations where no artifact is appropriate.
6. **Implement the declared process-order risk gate.** Select risks through inventory coverage for every category in spec §14.4; run forward and reversed/deterministically shuffled order in separate fresh processes, compare exact outputs by semantic key, and wire it into the release-mode PR gate without path filters. Hand the resulting expectation/result contract to #86's owner for their separate target work.
7. **Obtain final approvals and run the completed closure gates.** Record per-case/ledger subjects in review comments; verify all bindings offline and perform isolated pre-release reference reproduction under §14.1. Close #85 only after criteria 1–3 are evidenced, rather than because #83/#84 have closed. Long generation/reproduction and fuzz runs are follow-up execution, not claimed results of this research.

### 8. Commands and observed verification

Executed in the repository:

```sh
git rev-parse HEAD
git status --short
cargo run --offline --locked --release -p xtask -- conformance verify-all
target/release/xtask conformance verify-svg-viewers
target/release/xtask conformance verify-pdf-viewers
```

Results: clean initial tree; commit above; **37 registered cases verified**; **canonical SVG viewer qualification verified**; **canonical PDF viewer and structural qualification verified**. The locked offline xtask build took approximately 51 seconds. These commands verify evidence, not renderer visual parity or fresh rendering. No long generation/test suites were run.

Reproducible inventory queries (executed equivalent expanded queries during research):

```sh
jq '{cases:(.cases|length), kinds:(.cases|group_by(.kind)|map({kind:.[0].kind,count:length}))}' tests/conformance/cases/registry.json
jq -s '{manifests:length, approved:([.[]|select(.approval!=null)]|length), variants:([.[].variants[]?]|length), formats:([.[].variants[]?.formats|keys[]]|group_by(.)|map({format:.[0],count:length})), crops:([.[].crops[]?]|length)}' tests/conformance/cases/focused/*/manifest.json
jq -s '[.[].coverage[]?|select(.role=="primary")|.identity]|{records:length,unique:(unique|length),duplicates:(group_by(.)|map(select(length>1)|{identity:.[0],count:length}))}' tests/conformance/cases/focused/*/manifest.json
jq '{targets:(.targets|length),formats:([.targets[].outputs|keys[]]|group_by(.)|map({format:.[0],count:length})),layer,coverage}' tests/conformance/cases/focused/frame-export/manifest.json
jq '{cases:(.cases|length),risks:([.cases[].risk]|unique),originatingIssue,broaderFuzzIssue}' crates/excalidraw-render/tests/qualification/scene-fuzz-regressions.json
jq '{cases:(.cases|length),public_facade_cases:(.publicFacadeCases|length),broaderFuzzIssue}' crates/excalidraw-render/tests/qualification/font-subset-fuzz-regressions.json
jq '{coverage:.coverage}' tests/oracle/scene-surfaces/revision-1.json
jq -s '{types:([.[].elements[]?|.type]|unique),mime_types:([.[].files[]?|.mimeType]|unique),opacity:([.[].elements[]?|select(.opacity!=null and .opacity!=100)]|length),rotation:([.[].elements[]?|select(.angle!=null and .angle!=0)]|length),groups:([.[].elements[]?|select((.groupIds//[]|length)>0)]|length)}' tests/conformance/cases/focused/*/scene.excalidraw
```

No exact remaining **number of behavior rows** can responsibly be supplied before the authoritative global inventory exists. The exact actionable deficit is the missing closure machinery and unowned/unproven categories above; the measured counts describe existing records, not an inferred coverage percentage.

## #86 — Full native/WASM determinism execution and aggregation

**Finding: target execution is implemented; full-risk coverage and direct
aggregation are not.** The successful font aggregate proves its declared scope,
not every rendered artifact/report/trace category. Sources are
[#86](https://github.com/sagikazarmark/excalidraw-render.orig/issues/86),
closed prerequisite [#45](https://github.com/sagikazarmark/excalidraw-render.orig/issues/45),
and spec §14.4/§15 (`docs/renderer-spec-v1.md:3253-3289`).

### Acceptance-criteria assessment

| #86 criterion | Evidence today | Remaining work | Status |
| --- | --- | --- | --- |
| Every supported target executes public-facade rendering rather than compiling only | `dagger.dang:55-64,448-453` invokes release-mode `public_facade` tests on native and pinned browser WASM. Consumer dependencies resolve the unpacked crates (`tests/package-consumer/Cargo.toml:7-21`). Clean-commit execution is attested on #101. | Retain this harness and extend its coverage through the #85 result inventory. | **Implemented** for the criterion's target-execution requirement |
| Central aggregation directly compares exact artifacts, reports and traces over the complete risk set | Font target reports bind commit, corpus, profile, approval, two aggregate digests and counts. Both supported targets agree on 977 font-byte cells and 1204 trace cells. | General renderer result records, complete risk inventory, artifact/report/trace equality, fresh-process order, negative completeness tests and automatic execution. | **Partial: font subset scope only** |
| Browser smoke covers all formats, representative fonts/assets, frames, ordered diagnostics, serialization, licenses and repeated calls | Ten packaged facade tests execute on both targets, with meaningful coverage below. Six additional WASM worker tests run font/scene qualification and regressions. | Coverage-complete representative fonts/assets and mixed all-format checked scene; canonical frame/error/report results and missing risk paths; use the complete result inventory to prove coverage. | **Partial** |

### What the existing browser/public-facade suite proves

Source: `tests/package-consumer/tests/public_facade.rs` (line references below).

| Existing test/input | Proven scope | Important boundary |
| --- | --- | --- |
| Approved rectangle (`:14-19,152-180,460-489`) | Exact SVG/PNG/PDF artifact bytes and serialized report; forward and reverse format calls | One scene and one process, not reversed/shuffled corpus processes |
| Excalifont typography (`:183-214`) | SVG font subset, whole-artifact hash, repeat bytes and notices | Does not exercise all font families, CJK/emoji/bidi/missing-glyph layout across all formats |
| Direct/lazy frames and errors (`:216-289`) | All-format frame API equivalence and low-element-limit typed failures | Native and WASM can independently agree with themselves yet differ from each other; these results have no shared cross-target canonical pins/records |
| License bundle and scene-surface diagnostic (`:292-326`) | Exact license digest; expected diagnostic count, code and pointer | Not a full serialized nontrivial ordered-report risk inventory |
| Compatibility fallback scene (`:22,30-64,328-354`) | SVG/PNG/PDF artifact and serialized-report hashes, generated fallback text | One fallback input; loop does not independently repeat each format |
| Noisy PNG (`:28,356-394`) | Predictive scanline filters/compression, exact artifact/report pins and repeated artifact bytes | PNG decoder/encoder path, not all six admitted raster decoders/profile classes |
| Four PDF risk entries (`:70-95,396-425`) | Selectable text, source PNG, native nested SVG and isolated fallback; exact artifact/report pins and two calls | PDF only; not full metadata, geometry/numeric and decoder inventory |
| Eight PDF fallback families (`:96-145,428-457`) | Filter-family-specific artifact/report pins | One render each; no general cross-target result export |

The consumer uses only the supported public API and is non-published. Existing
same-hash assertions on both targets **do prove cross-target equality for those
pinned outputs**; the missing central report is not a reason to discard that
proof. The gap is completeness and a machine-readable, directly compared result
set. `wasm_tests` (`crates/excalidraw-render/src/lib.rs:50-84`) separately exercises
font qualification, font fuzz, trace replay and scene fuzz in dedicated workers.
Those six tests call internal test seams; they do not replace the public-facade
consumer or all-format coverage.

Browser controls are present: manifest-pinned Chromium/driver/runner and timeout
in `dagger-evidence.dang:1-14` and `tools/ci/install-browser.sh:4-14`, DNS and
background-service restrictions in `tests/package-consumer/webdriver.json:3-12`,
empty font directory in `fontconfig.xml:3-5`. This audit inspected configuration;
it did not independently network-trace a run. The spec's **one checked scene**
combining rough geometry, bundled/fallback text, an image, frames/clipping and
ordered diagnostics in every format is not present in this consumer: these
features are divided among the inputs above (§15 `:3283-3289`).

### Why the current aggregate is not #86's completed aggregate

`dagger.dang:70-80` calls only `aggregate-svg-font-subset-targets`.
The report emitted by `xtask/src/svg_font_subset_targets.rs:93-125` has no
per-scene artifact or compatibility-report results. Its `AGREED_FIELDS`
(`:41-50`) compare the commit, font corpus/profile/viewer subject, generated
font-byte digest, resource-trace digest, and two counts. `:186-254` correctly
requires exactly the two supported targets and equal fields. The serialized
target report here must not be confused with the renderer's serialized
compatibility report: the latter is absent from this aggregate.

Useful mechanisms already exist and should be reused:

- Exactly one measurement must appear in the log (`:128-144`). The Dagger
  producer requires successful test exit (`tools/ci/qualification.sh:1-13`);
  an isolated materializer command merely parses a supplied log and is not
  itself proof that tests ran (`xtask/tests/svg_font_subset_targets.rs:77-155`
  deliberately uses synthetic logs to test serialization).
- Dagger evidence export depends on the packaged facade and native CLI, checks
  clean source, and carries the qualification log (`dagger.dang:413-428`). Those
  consumer successes are execution prerequisites, not per-case data in the JSON.
- Recorded aggregate validation checks provenance, corpus/profile/viewer and
  committed measurement bindings (`xtask/src/svg_font_subset_targets.rs:398-445`);
  recording verifies Git ancestry and copies exact bytes (`:511-541`).
- The committed font report (`tests/conformance/svg-font-subset-target-aggregate/revision-1.json:1-49`)
  names clean `3f06c91` and passes for both targets. This research ran
  `target/release/xtask conformance verify-svg-font-subset-release`, which printed
  `verified SVG font subset release evidence (aggregated)`. The earlier actual
  Dagger run is documented in `docs/font-identity-refresh.md:105-135` and
  [#101 attestation](https://github.com/sagikazarmark/excalidraw-render.orig/issues/101#issuecomment-5639444844);
  it was not repeated during this audit.

### Trace seam and automatic-gate gaps

The public consumer imports no trace API. Trace helpers are internal and gated
for tests (`crates/excalidraw-render/src/render/mod.rs:41-61`). The font trace
matrix compares cold/warm/cache/frame paths on each target
(`resource_trace_qualification.rs:10-14,27-46,90-151,188-222`). It emits no general
cross-target trace result inventory. Keep this useful equality proof, but add a
test-only full-risk trace collector/result contract; do not expose a new supported
trace API just to satisfy the test harness. Canonical artifact/report generation
must continue to use the public facade. ADR-0002 `:7-25,30-39` requires trace
equality, not restored historical per-cell trace constants.

`qualification` and `publicFacade` are automatic checks (`dagger.dang:379,406`);
`targetEvidence` is an explicit operation, without `@check` (`:70`). The generated
PR workflow runs the check groups (`.github/workflows/dagger.yaml:19-31`), while
the aggregate workflow is manual only (`packaged-public-facade.yml:3-4,14-19`).
Thus even font aggregation is not currently a fresh PR gate. Verifying an existing
recorded aggregate does not prove that today's candidate ran a complete risk
aggregate. Wire the new **full-risk execution plus aggregation** as a canonical
Dagger check; keep writing/binding release records explicit.

### #85/#86 handoff and closure test

Agree one case-result key and expected inventory before implementing two runners:
case, variant, export target, format, normalized request/options, compatibility
revision and bound input/profile identity. #85 owns corpus closure, approved
canonical artifact/report expectations, and the fresh-session/process-order
schedule. #86 consumes that exact inventory for both targets, includes applicable
typed failures and diagnostic/report bytes, joins test-only trace results, and
rejects missing/extra/duplicate records or mismatched provenance before comparing
values. No broad renderer categories may be inferred from a few passed tests.

Require a clean-run result bundle that (1) covers every declared risk category,
(2) matches canonical artifact/report expectations on both supported targets,
(3) agrees across process orders, and (4) centrally agrees on fresh traces as
well as artifacts/reports. Missing target, dropped case, changed order/result,
stale input digest and changed trace must each have meaningful negative tests.
Run it through Dagger on PR candidates and, with #95, from protected-tag packaged
sources. That closes #86 without redoing the completed font refresh.
