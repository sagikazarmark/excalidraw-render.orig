# Representative corpus #84 — gap audit at afbe40f

## Conclusion

**#84 is partially implemented and remains open. The representative corpus is no longer absent.** There is **one currently registered, approved project-authored representative scene**, with **one request variant and three executions (SVG, PNG, PDF)**, real crop judgments, bound reference evidence, independent semantics, and fresh-process native artifact/report replay. Its supplemental registry lives under `tests/conformance/candidates/`, not `tests/conformance/cases/representative/`.

The remaining primary gap is **representative breadth and an extensible intake/execution contract**: the approved scene has four elements, the adapter accepts only that frozen scene family, no external pool scene has been promoted, and the representative executions do not participate in the shared native/WASM snapshot/trace inventory. Case approval already exists; global closure and approval do not.

**Recommended next slice:** enable a second, independently identified representative case and promote `velox-design` as an immutable licensed/sanitized relationship-and-typography board, with all-format whole/crop judgments and exact reports/repeats. Reuse the Stage-4 evidence mechanisms; retain its existing owner and judgments. This is a bounded expansion toward #84, not a claim that Velox alone covers frames/assets/nested SVG/modes.

## Scope and evidence quality

- Audited on 2026-09-22 at **`afbe40f81854dc90988e7786145b713a438b29f2`**. Repository-relative `path:line` citations refer to this HEAD. Canonical JSON files occupying one line are cited as `:1`; structured queries read their complete contents.
- Read `AGENTS.md`, `CONTEXT.md`, `docs/agents/domain.md`, ADR-0002, ADR-0005, ADR-0006 and ADR-0011 before the implementation audit. The controlling requirements are `docs/renderer-spec-v1.md:3048-3086,3123-3126,3304-3337`.
- Read [#84](https://github.com/sagikazarmark/excalidraw-render.orig/issues/84) with **`gh issue view 84 --comments`**, then its JSON body/state/labels/comments. It is open and labelled `ready-for-agent`. Read the current approval comment directly with the GitHub API, as described below.
- **Verified this session:** HEAD/status, structured file censuses, stored representative approval-subject/manifest agreement, SHA-256/length checks for 251 distinct bound files, current ownership binding, and the live approval comment's identity/scope/digest. Also directly detected the separate native baseline's stale inventory hash.
- **Inspected, not executed this session:** Rust validators, renderer replay, visual comparators, tests and Dagger wiring. Stored pass records are historical evidence, not fresh rendering results. No full suite, renderer build, Docker/viewer run, generator, or tracker write was performed. In particular, `candidate-verify`/`verify-representative-owner` can build the release CLI (`xtask/src/conformance_candidate_native.rs:107-179,289-303`), so they were not used as supposedly non-mutating offline checks.
- Initial unrelated untracked work was `docs/research/jpeg187-sampling-diagnosis.md`, `docs/research/jpeg187-sampling-hardening.md`, and `tools/jpeg187-diagnosis/`. It was preserved; this audit creates only this file.

## 1. Current inventory: count registrations, not historical revisions

| Scope counted | Current result | Interpretation and source |
| --- | ---: | --- |
| Main registry | 37 cases: 35 scene, 1 typography, 1 frame | `tests/conformance/cases/registry.json:1-16`; no representative registration here. The specialized entries omit the registry layer. |
| Main registered manifests | 37 non-null approvals; 49 scene/typography variants; 7 frame targets | Each scene/typography variant has SVG and PNG evidence. The frame targets separately have SVG/PNG outputs. Approval **presence**, not a new `verify-all` pass, was counted. |
| Main manifest crops | 0 | This schema still forbids nonempty crops: `tests/conformance/focused-scene-case-v1.schema.json:107-110`. |
| Shared execution inventory | 168 keys, **0 representative keys** | `(49 + 7) × 3 formats`; `tests/conformance/coverage/inventory.json:1`. These PDF execution keys must not be confused with complete PDF reference judgments. |
| Supplemental representative registry | **1 case, 1 `default` variant, 3 format executions** | `representative.stage4-frames-text-assets.v14`, `tests/conformance/candidates/registry.json:1-33`. Thus 38 currently registered cases across the two registries, but not 171 keys in one unified runner. |
| Current representative input | 4 live elements; 2 assets | One frame, one rotated Excalifont text, one PNG image, one nested-SVG image. `tests/conformance/candidates/stage4-frames-text-assets-v14/scene.json:4-154`. |
| Representative visual scopes | 5 comparison rows, each whole image + 3 crops | SVG Chromium/pure Rust, PDF PDFium/PDF.js, direct PNG/canvas: **20 region judgments**. Crops: typography, raster, nested SVG. `xtask/src/conformance_candidate.rs:193-321`; current `judgments.json:1`. |
| Representative tolerance scopes | 3 approved PDF scopes | PDFium whole + typography; PDF.js whole. `tests/conformance/candidates/registry.json:34-109`. |
| Historical Stage-4 manifests on disk | 11 manifests, 8 with approval objects | v1, v4, v6, v7–v14. The ten v4-and-later manifests bind the **same scene hash**. These are not eleven representative boards or eleven current variants. `xtask/src/conformance_candidate_gate.rs:702-728` freezes v4 authoring for successors. |
| External intake pool | 15 scenes, **0 current representative promotions** | Pool is explicitly raw, unsanitized/unapproved intake; `tests/public-scenes/README.md:3-18,22-38`. Current representative is project-authored. |
| Global ownership proposal | 2,418 assignments: 2,318 owned, 3 excluded, **97 explicit gaps**; global approval null | Counted `.assignments[].disposition.status` in `tests/conformance/coverage/ownership.json`; exactly one representative owner. This is neither a #84-specific gap count nor a coverage percentage. |

The current machine-readable ledger supersedes the historical V7/V8 and 1,874/1,912-row narrative checkpoints in `tests/conformance/coverage/OWNERSHIP.md:10-61`. The live representative binding is at `tests/conformance/coverage/ownership.json:48817-48844`; its rationale still recounts older revisions, while the binding selects V14.

## 2. What the approved representative actually establishes

### Approval and immutable evidence

The V14 manifest records `sagikazarmark`, 2026-09-21, and subject:

```text
sha256:0b9f6a7d6eb1fd176d412855aecdb63b309ed260ae4d552054dbef794deb74ba
```

Source: `tests/conformance/candidates/stage4-frames-text-assets-v14/manifest.json:1`. The live [#85 approval comment](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5768106992) is by that account and explicitly says `decision: approve`, `scope: pdfium192-successor`, with the same representative subject. Its production subject is separate. This is actual scoped approval, not approval inferred from an agent's research prose or a `passed` field.

The case manifest binds **51 file descriptors and 31 adapter producer-source descriptors**, profiles, request arguments, native capture, semantic expectations, judgment plan and exact exception proposal. The approval bundle excludes only `.approval` and includes the manifest and file descriptors (`xtask/src/conformance_candidate_gate.rs:445-460,487-565,654-671`). Verification re-derives judgments/semantics, checks source bindings and stored subject, requires approval before replay, and returns three bound execution receipts. This is substantially beyond the earlier focused-only approval boundary.

The session's read-only check:

1. Parsed `approval-subject.json`, proved structural equality to `{schema, manifest_without_approval, files}` and hashed its stored bytes excluding the terminal LF; the digest matched the approval and owner binding.
2. Rehashed manifest files, adapter producers, native-capture producers, reference-provenance sources, and the owner's registry/manifest descriptors: **251 unique files, zero hash/length errors**.
3. Retrieved the exact GitHub comment above and compared its explicit approved digest.

This is a **limited independent integrity check**, not the complete recursive xtask verifier, canonical-serialization proof, visual recomputation, or fresh execution.

### Request, dimensions, semantics and reports

- Fixed request: compatibility revision 1, zero padding, transparent background, clipping enabled, frame names/outlines disabled, embedded scene/provenance/diagnostic summary enabled. Reference export is light mode, scale 1. Reference dimensions are **200 × 120** (`xtask/src/conformance_candidate_native.rs:22-33`; V14 `provenance.json:37-57`).
- Whole/crop raster comparisons enforce exact dimensions; PNG must decode as RGBA8. PDF page/CropBox is **150 × 90 points** (`xtask/src/conformance_candidate.rs:169-185,296-319`; `xtask/src/conformance_candidate_semantics.rs:169-191`).
- Independent SVG checks follow actual links/image references, require selectable caption text, unchanged source PNG octets, and an active frame clip. PDF checks follow page/resources/content/annotations, require RGB and attached alpha, nested vector form, font/text, output intent, correct clipped/rotated link rectangles, and the source scene attachment (`xtask/src/conformance_candidate_semantics.rs:172-395,397-475`; V14 `semantics.json:1`).
- Independently authored diagnostic expectations are exactly empty for all three formats. Native replay validates the entire CLI JSON report envelope including artifact identity, and exact serialized report/artifact bytes against the capture (`xtask/src/conformance_candidate_gate.rs:407-412`; `xtask/src/conformance_candidate_native.rs:218-232,289-353`). Empty reports are meaningful evidence, but not mixed diagnostic-order coverage.
- Replay builds pinned Linux `--release --locked`, uses two new CLI processes **per format**, and rejects any byte drift. The CLI calls the public `Renderer::render` API (`xtask/src/conformance_candidate_native.rs:149-216,289-339`; `crates/excalidraw-render-cli/src/main.rs:638-676`). It is not a reversed-order multi-case process experiment or a browser-WASM execution.

### Visual budgets and exceptions

Stored V14 canonical results pass SVG Chromium, SVG pure Rust and direct PNG; both PDF whole-image comparisons need the approved typography-related scopes. All five **effective** rows pass with those scopes (`tests/conformance/candidates/stage4-frames-text-assets-v14/judgments.json:1`). The whole-image class is **geometry/image**, not mixed-representative; the classification of a visual region is distinct from its corpus layer.

The three exceptions are bound into the approval subject, have diagnosed viewer/text-rasterization causes and exact ceilings, and fail if measurements change **or the exception becomes unnecessary** (`tests/conformance/candidates/registry.json:34-109`; `xtask/src/conformance_candidate_exceptions.rs:136-168,198-215`). Do not label these approved non-defect judgments as renderer defects or demand another blanket approval for the unchanged V14 case.

### Coverage ownership and CI are real

`interaction:frames-text-assets` is assigned to the approved representative through the strict supplemental adapter. It validates the exact registry, manifest, subject, release consumer, formats and three receipts; it cannot own an unrelated obligation (`xtask/src/coverage/ownership/representative.rs:77-202`; ledger `:48817-48844`). The other three current interaction rows have test owners for dark-mode nested-SVG PDF behavior, diagnostic order, and frame-batch limits; their existence does not widen the representative's own variants.

Dagger's `coverageOwnership` check invokes this adapter through `verify-ownership` (`dagger.dang:233-237`; `xtask/src/coverage/ownership.rs:494-534`). The workspace fast tier also includes the real approval-aware representative registry consumer (`dagger.dang:324-365`; `.config/nextest.toml:47-51`; `xtask/tests/representative_candidate.rs:6-48`). Negative contracts cover unknown/broadened owners, unapproved cases, dev-profile captures, stale/redirected sources and approval mutations (`xtask/tests/representative_ownership.rs:18-87`; `xtask/src/coverage/ownership/representative.rs:364-476`; `xtask/src/conformance_candidate_gate.rs:772-976`). **These are inspected CI/test implementations, not a claim that current CI ran successfully.**

## 3. Remaining gaps against #84

| Requirement / gap | Concrete evidence at HEAD | Next work and authority |
| --- | --- | --- |
| Real-world cross-feature breadth | The only registered representative has frame/text/two images, frame membership and safe links. It has no arrows/bindings, bound text, groups, rough scene geometry, multiple fonts/fallback, or nonempty diagnostics; see V14 `scene.json:4-154`. | **Agent:** promote an actual board and add complementary interactions. **Human:** judge whether the resulting boards/variants are sufficiently representative; spec does not prescribe a numeric scene quota. Project-authored scenes are valid under §14.2, so external origin is not mandatory for every case. |
| Reusable external intake | Main verifier forces `layer == focused`, contributor MIT/Apache metadata and `sanitization.performed == false`; main crops are empty-only (`xtask/src/conformance.rs:178-180,803-826`; focused schema `:107-110`). Supplemental adapter requires exactly one Stage-4 case and frozen v4 bytes (`conformance_candidate_gate.rs:614-688,702-728`). | **Agent:** introduce an extensible representative manifest/runner path with source license/notice files, upstream identity, original/sanitized hashes, sanitization record and evidence descriptors. **Human:** accept the sanitization/licensing/provenance and final evidence. Existing pool review is reusable; do not claim it is a promotion sign-off. |
| Applicable variants and targets | Representative registry has only `default`; request is light, scale 1, transparent, names/outlines off, whole-scene export (`candidates/registry.json:17-20`; V14 `provenance.json:37-57`). There is no applicability matrix explaining scene/frame, mode, sizing or metadata selections. | **Agent:** declare and execute the applicable matrix with explicit reasons for omissions. Add frame and mode interactions where useful. **Human:** approve that matrix and classes/crops. Do not demand every Cartesian product, or infer completeness from one default key. |
| Representative notice/metadata semantics | Approved bytes bind actual metadata, and PDF independently checks the attachment. The semantic result has SVG/PDF sections only; it does not independently parse all format notice sets or PNG metadata/embedded-scene semantics (`conformance_candidate_semantics.rs:362-475`; V14 `semantics.json:1`). | **Agent:** add independent case-level notice/carrier expectations, especially SVG subset notices versus complete-font PDF notices and raster PNG behavior. Existing all-format notice tests are reusable (`crates/excalidraw-render/tests/fast/artifact_metadata.rs:279-329`), not missing implementation. |
| Representative resource behavior | Native representative consumer exercises successful rendering at defaults; fixed options/capture receipts contain no resource-boundary or trace contract (`conformance_candidate_native.rs:22-33,289-339`). Separate frame-batch limit tests exist and are owned (`crates/excalidraw-render/tests/fast/stage4_interaction_contract.rs:114-186`). | **Agent:** bind meaningful success/limit/failure expectations to selected real boards, including exact reports and atomic outcomes; connect fresh trace equality where required. Reuse boundary tests; do not claim the four-element success proves many-child-frame/resource scaling. Do not add historical per-cell trace pins (ADR-0002 `:7-25,30-39`). |
| Supported-target/process-order coverage | Shared inventory has 168 main-registry keys and no representative key; canonical baseline also has 168 expectations and no representative. Supplemental replay is native-only, fixed format order. | **Agent, #85/#86 handoff:** join representatives to the shared request/key/collector contract, native/WASM replay and fresh-order/trace equality. Existing Dagger `fullEquality` exists (`dagger.dang:90-121`); expand its inputs rather than inventing another target runner. **Human:** approve new/changed canonical expectations. |
| Global coverage closure | Ledger approval remains null; 97 gaps include `closure:all-five-layers`, `closure:atomic-profile-inventory`, `closure:pdf-judgments`, `closure:process-target-equality` (`coverage/ownership.json:5,1778-1807`). | **#85, partly fed by #84:** preserve/extend explicit dispositions and complete reviewed ownership before global approval. Validator separates consistency from closure (`xtask/src/coverage/ownership.rs:637-665`); ADR-0011 `:7-21` prohibits hiding new inventory work or silently replacing decisions. |

### Related concrete blocker: the shared baseline is stale

The baseline's recorded inventory digest is:

```text
1a53f503eefd4379670dfd11a8448cf0da7edfeef3ff858a9cc9c5e5e9b906ef
```

The current `tests/conformance/coverage/inventory.json` digest measured this session is:

```text
5d4c81d5cc17b5153fe5b0a6784c0e2499a947c77eb32d600c6a65e6a7130faa
```

Source: baseline `tests/conformance/coverage/native-baseline/revision-1.json:1`; validator requires this exact current descriptor (`xtask/src/coverage/snapshots/baseline.rs:93-107`). Thus the recorded approval is not sufficient for a current baseline validation pass. This is a **measured binding mismatch**, not an executed replay failure and not evidence that artifact bytes changed. Coordinate clean collection/review under [#85](https://github.com/sagikazarmark/excalidraw-render.orig/issues/85) / [#86](https://github.com/sagikazarmark/excalidraw-render.orig/issues/86); do not edit the approved hash in place to conceal it. It need not block authoring the next representative intake slice.

## 4. Historical statements to retire

- The [September 2 intake comment](https://github.com/sagikazarmark/excalidraw-render.orig/issues/84#issuecomment-5509477654) remains useful for human inputs, but its missing-layer/generator description is historical. A supplemental approved representative path now exists. Conversely, its generic `conformance generate <case-id>` instruction is not already a general external-scene intake facility: both current paths remain constrained as above.
- The [September 4 audit](https://github.com/sagikazarmark/excalidraw-render.orig/issues/84#issuecomment-5533968694) counted 13 synthetic cases. Current main registry has 37, including frame-mask, legacy-line-height, font-family, transform/opacity/group, cartoonist and iframe cases (`tests/conformance/cases/registry.json:1477,1635,3078,6694,6899,7147`). Do not recreate these as supposedly absent focused features.
- PDF evidence and executing harness gates exist, including the representative's two PDF viewers; ignored harness tests are deliberately selected by `.config/nextest.toml:58-67`. AVIF is explicitly rejected in revision 1 (`docs/renderer-spec-v1.md:723`), not an unfinished required decoder admission.
- `docs/research/release-readiness-85-86.md:65-100,128-155,174-176` expressly audits an older commit. Its claims of no representative corpus, absent general snapshot machinery and absent global ownership must now be read historically. The updated facts are the supplemental V14 case, live ownership proposal, implemented collectors/equality gate, and approved-but-now-stale shared baseline. Its general closure concerns remain relevant; its original successful commands were **not repeated here**.
- The supported set is Linux x86-64 and pinned browser WASM, with Dagger canonical (ADR-0006 `:23-39,51-58`). This audit does not restore five-target or dormant performance/footprint release requirements.

## 5. Recommended next actionable slice

### Promote Velox through a general representative contract

**Why this board:** it adds a real architectural diagram and relationship/typography density absent from Stage-4, without external asset complexity in the first intake implementation. Its local provenance already records the upstream pinned commit, MIT license and original digest (`tests/public-scenes/velox-design/PROVENANCE.json:1-42`). The full scene census this session found **53 live elements: 19 rectangles, 11 arrows, 23 text elements; 20 bound-text elements, 11 arrows with endpoint bindings, 30 non-null roundness records, font family 8, no files**. These are input counts, not a rendering pass. The pool's prior content review is documented (`tests/public-scenes/README.md:73-109`).

Completion contract for this slice:

1. **Agent — intake and applicability:** stage one immutable sanitized Velox scene with original/sanitized hashes, upstream permalink, preserved MIT attribution/license, recorded review/transforms, and a small explicit variant matrix. Start with whole-scene light and dark requests in SVG/PNG/PDF; identify exact dimensions/reports, whole visual class, typography/arrow-binding crops and expected notice differences. Propose which variants are applicable before generating expensive evidence. Do not invent frames/assets in an external board and still describe it as the unchanged upstream board.
2. **Agent — extensible registration and gates:** admit a second independent ID instead of another `stage4-frames-text-assets-vN` successor. Generalize identity, dimensions, semantic expectations, crops, reports and format/target requests while keeping completeness and approval checks. Add focused negative tests for missing format/variant/crop/report/provenance, stale scene/hash/subject, unapproved intake and duplicate ownership. The exact-one-case checks are at `conformance_candidate_gate.rs:622-638` and `coverage/ownership/representative.rs:77-101`; frozen-authoring and fixed-geometry assumptions need deliberate treatment, not simply deleting those checks.
3. **Agent — generate and qualify:** use pinned official SVG/direct-canvas evidence, both SVG and PDF viewers, explicit semantics/metadata/notices, meaningful default/resource outcomes and exact repeated artifact/report bytes. Connect the new case's execution keys to the shared collector contract. Diagnose out-of-budget regions rather than automatically creating tolerances. This work requires generation/execution outside this read-only audit.
4. **Human — exact review:** sign off sanitization/provenance, representativeness/applicable variants, classes/crops, reference evidence and any diagnosed tolerance exceptions; approve the exact resulting subject. Review the coverage selection and any changed canonical baseline subject under #85. Existing V14 approval is not authority for a new board or broadened limits.
5. **Agent — record authorized approvals and run gates:** only after that decision, record the approved subjects and verify the main/supplemental registry, owners, new parity/semantic/repeat gates and appropriate target checks. Source changes to the current candidate adapter can invalidate V14's broad producer bindings (`conformance_candidate_gate.rs:506-523,731-744`), so plan that review explicitly. Preserve comparator-specific binding discipline (ADR-0005 `:23-32,49-55`).

This slice closes **external intake + dense relationship/font interaction**, not all of #84. Next select a complementary frame/asset board such as `excali-local-slides` (8 frames, 16 image elements; pool README `:30`) and address applicable frame exports/resource outcomes. The current approved Stage-4 case remains useful nested-SVG/alpha/metadata evidence. The maintainer must ultimately judge that the combined corpus establishes the issue's real-world interaction promise; neither a single new board nor 97 global gaps supplies a numeric completion criterion.

## Reproducible read-only census commands

```sh
git rev-parse HEAD
git status --short
gh issue view 84 --comments
gh issue view 84 --json number,title,body,url,state,labels,comments
gh api repos/sagikazarmark/excalidraw-render.orig/issues/comments/5768106992
jq '{cases:(.cases|length),kinds:(.cases|group_by(.kind)|map({kind:.[0].kind,count:length}))}' tests/conformance/cases/registry.json
jq -s '{manifests:length,approved:([.[]|select(.approval!=null)]|length),variants:([.[].variants[]?]|length),frame_targets:([.[].targets[]?]|length),crops:([.[].crops[]?]|length)}' tests/conformance/cases/focused/*/manifest.json
jq '{cases:(.cases|length),executions:[.cases[].execution_keys[]]}' tests/conformance/candidates/registry.json
jq '{executions:(.executions|length),representatives:[.executions[]|select(.key.case|startswith("representative."))]}' tests/conformance/coverage/inventory.json
jq '{approval,total:(.assignments|length),statuses:(.assignments|group_by(.disposition.status)|map({status:.[0].disposition.status,count:length}))}' tests/conformance/coverage/ownership.json
```

Equivalent expanded queries and a `python3 -B -c` in-memory JSON/hash check produced the observations above. The Python check read bytes only and wrote no cache, staging files or evidence. Source inspection and these checks justify the audit findings; they do not substitute for the final slice's conformance run or human judgments.
