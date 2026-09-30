# Architecture review

Read-only review of the workspace at `a1be429`. No build, test or clippy was
run; observations come from source, manifests, the lockfile, docs and `git
log`. Line numbers refer to that commit.

---

## 1. Crate & module boundaries

### Workspace decomposition

| Crate | Role | Deps (workspace-internal) |
|---|---|---|
| `excalidraw-render` | core library, the only supported API | `assets-base`, `skera` (adapter), `ttf2woff2` (fork) |
| `excalidraw-render-cli` | native binary | `excalidraw-render` only |
| `excalidraw-render-assets-base` | sealed catalog + profile constants + face tables | `assets-data-000`, `assets-data-001` |
| `excalidraw-render-assets-data-00{0,1}` | 8 MiB `include_bytes!` shards | none |
| `skera-safe` | 25-line panic guard over `skera 0.5.1` | `write-fonts` |
| `ttf2woff2-deterministic` | vendored upstream 0.13.2 pinned to `brotli 9.0.0 portable-float` | none |
| `excalidraw-render-compare` | test-only comparator (dev-deps only, `publish = false`) | none |
| `xtask` | importer/verifier/evidence tooling | `assets-base` |

**Assessment: sensible and acyclic.** Dependencies point downward: CLI → lib →
{assets-base → data shards, skera-safe, ttf2woff2}. `xtask` depends only on
`assets-base` (to reuse `AssetRecord`/CJK tables), not on the renderer, so the
tooling cannot accidentally test the renderer through a back door. The
comparator is deliberately decoupled (ADR-0005) and reachable from the lib only
as a dev-dep.

The shard split is a crates.io size workaround, documented in ADR-0003's table
(data-000 at 7.97 MiB is 80% of the 10 MiB cap). `assets-base/src/lib.rs:401-402`
`include!`s two generated tables; the CJK predicate, subset profile and
face-order tables live here too, which makes `assets-base` slightly more than
"asset bytes" — it is really "revision-1 sealed constants". That is fine, but
its name undersells it.

### Public facade of `excalidraw-render`

`src/lib.rs:42-63` re-exports ~60 items from 6 modules. The entry points are
narrow and deep:

- `SceneDocument::parse(&[u8])` (`scene.rs:100`) and `SceneDocument::frames()` (`scene.rs:135`)
- `Renderer::{render, render_frame, render_frames}` (`compatibility.rs:148-173`)
- `RenderRequest::builder(...)` / `FrameRenderRequest::builder(...)` with validation in `build()` (`request.rs:411, 495`)

Everything else is contract vocabulary (`ErrorCode`, `DiagnosticCode`,
`ErrorDetails`, `Recovery`, `Measurement`, …) that the serialized JSON contract
(§11) requires to be public. No backend trait, no plan types, no
`ResolvedScene`, no `svg_fonts`/`font_subset` leak — matching spec §9 ("There
is no backend trait, public font option, or public subsetting API").
`SceneDocument`'s fields are private; all typed accessors are `pub(crate)`
(`scene.rs:139-360`). `CompatibilityDiagnostic`/`RenderError` have `pub(crate)`
fields with public getters only.

Two facade nits:
- `Renderer` is `Clone + Debug` and carries `&'static RevisionCatalog`
  (`compatibility.rs:95-97`); it is effectively a ZST token. `Renderer::new()`
  being `const fn` while `for_revision` is not is an asymmetry callers will notice.
- `license_bundle()` is duplicated on both `excalidraw-render` (`lib.rs:93`)
  and `assets-base` (`lib.rs:755`), reading two different `LICENSE-BUNDLE.txt`
  copies that `catalog verify` checks are byte-identical
  (`xtask/src/catalog.rs:767-773`). Correct, but a copy-sync gate rather than a
  single source.

### What the CLI depends on beyond the facade

`crates/excalidraw-render-cli/Cargo.toml:16-22`: `anyhow`, `clap`, `regex`,
`serde_json`, `unicode-normalization`, `unicase`.

- `anyhow` is **declared but never used** in `src/` or `tests/`.
- `regex`, `unicode-normalization`, `unicase` implement the §7 *frame output
  basename* contract (`frames.rs:742-790`: NFC, case-folded collision keys,
  reserved device names, 120-byte truncation with SHA-256 suffix).
- The CLI carries its own **hand-rolled SHA-256** (`main.rs:1842-1989`, ~150
  lines) although `sha2` is already a workspace dependency. Cleanup-plan item
  1b removed the same hand-rolled copy from `resource_accounting.rs` but not
  this one.
- `main.rs` is 1,989 lines and `frames.rs` 1,198 lines. `frames.rs:1` is
  `use crate::*;`, so the "module" is a file split, not an interface.

The CLI is thin over the *rendering* facade but not thin overall:
staging/atomic publication, report serialization (hand-written JSON writer
`main.rs:1602-1830`), basename derivation and bounded I/O are ~2,000 lines of
contract logic that library consumers cannot reuse. Whether basename derivation
belongs in the library is a spec question (§7 places it in the CLI contract),
but it is the one piece of *deterministic naming* behaviour that is only tested
through the binary.

---

## 2. Module size & cohesion inside `crates/excalidraw-render/src`

Total: ~51.6k lines under `src/`, of which ~6k is generated and ~7.5k is
`#[cfg(test)]` code living in `src/render/` (`tests.rs` 2,058;
`font_subset_qualification.rs` 1,469; `font_subset_fuzz.rs` 676;
`frame_batch_property.rs` 321; `resource_trace_qualification.rs` 229).

A structural fact that frames everything below: `render/mod.rs:66-69` does
`use self::{bound_text::*, diagnostics::*, font_demand::*, frame_target::*,
geometry::*, resolve::*, ...}` and 11 of the submodules open with `use
super::*;` (`bound_text`, `diagnostics`, `encode`, `font_demand`,
`frame_target`, `geometry`, `image`, `resolve`, and the test modules). The
`render/` directory is therefore **one flat namespace of ~25k lines spread
across files**. Commit `122a1e6` ("split the render module along its existing
seams") and `3406f41` ("split per-format encoding out") were file moves; they
did not introduce interfaces. The exceptions with real boundaries are `rough`,
`text`, `svg_fonts`, `font_subset`, `asset`, `pdf`, `png`, `svg`, `metadata`,
`notices`, `freedraw`.

### `render/mod.rs` — 4,787 lines — accretion

| Lines | What | Verdict |
|---|---|---|
| 85-114 | `FormatEncodeError`, `MetadataFailure` | shared error vocabulary; fine here |
| 116-196 | `render`, `render_frame`, `render_frame_scoped` entry points | fine |
| 204-210 | `SharedFontSubsetSession`, `SvgTreeCache` | fine |
| **212-590** | `PreparedFrameRender` + `PreparedOriginalJsonEvent/NormalizationEvent/RelationshipEvent` + replay | **seam → `render/prepared.rs`** (frame-batch journal; only `frame.rs` and `render_frame_scoped` use it) |
| 611-790 | `#[cfg(test)]` trace helpers | move to `render/tests.rs` or a `test_support.rs` |
| 793-968 | `ResolvedRelationships`, `ResolutionInputs`, `SceneChrome`, `RenderSession`, `RenderResourceAccount`, pending-diagnostic structs, `OptionalMetadataComponent`, `ArtifactMeasurement` | session core |
| 977-1216 | `RenderSession::{new, new_target, new_with_trace, observe_original_json_event, observe_normalization_event, observe_relationship_event}` | journal *ingestion* — belongs with 212-590 |
| 1216-1243 | `render()` orchestrator (27 lines) | good |
| 1247-1304 | `admit_session` | good |
| **1309-2218** | `resolve_scene` + phases `acquire_resolution_inputs` (1747), `account_semantic_census` (1911), `assemble_resolved_scene` (2115) | **seam → `render/resolution.rs`**; the cleanup plan's 2026-09-02 note says the orchestrator is "~40 lines plus the element loop" — the element loop is still ~430 lines inline (1309-1745) |
| **2309-2606** | `select_artifact_metadata` (300 lines) | **seam → `render/metadata.rs`** as a session extension `impl`, next to `ArtifactMetadata` it drives |
| **2608-3211** | `account_fallback_semantics` (600 lines, one function) | **seam → `render/accounting/fallbacks.rs`**; three distinct sub-responsibilities: frame-clip geometry charge (2618-2653), text work/working formulas (2654-2720), image/fallback rest |
| **3212-3404** | `preflight_pdf_raster_fallbacks`, `pdf_raster_surface_failure`, `required_raster_surface_failure`, `pdf_raster_fallback_failure` | **seam → `render/pdf.rs` (session `impl` block) or `render/encode.rs`**; only PDF uses them |
| **3405-4124** | `account_embeddable_label` (3405), `preflight_official_text` (3597), `retain_fallback_font_diagnostics` (3809), `omit_failed_resource_fallback` (3870), `preflight_fallback_text` (3921), `account_compatibility_omissions` (4063), `charge_local_omission` (4109) | **seam → `render/accounting/text.rs`** (~720 lines of text-only semantic accounting) |
| 4125-4772 | `acquire_working`, `charge_work`, `retain_diagnostic*`, `flush_*`, `scope_frame_diagnostics`, `take_report`, `admit_output`, `release_*`, `transact`, `validate_{svg,pdf}_dimensions`, `*_failure` constructors | session core — meter + diagnostic sink + error funnel (~650 lines); this is the cohesive "deep module" kernel and could be the whole of `mod.rs` |

Of the 3,800-line `impl RenderSession` (977-4772), only ~1,000 lines are
session-kernel concerns. The rest is per-stage semantic accounting that happens
to need `&mut self` for the meter. Recommendation: keep `RenderSession` +
meter/diagnostics/failures in `mod.rs`; move the phase bodies into sibling
files as additional `impl RenderSession` blocks (the pattern `encode.rs:11`
already uses), and replace `use super::*` with explicit imports so each file
states what it touches.

### `normalization.rs` — 4,474 lines — accretion with one cohesive extractable algorithm

| Lines | What | Verdict |
|---|---|---|
| 16-260 | 14 `Normalized*` types + `NormalizationOutcome` + `NormalizationEvent` | types module |
| 271-556 | `ElementDiagnosticContext` (285 lines) | diagnostic plumbing |
| 605-1132 | `normalize_scene_impl` — **one 530-line function** | accretion |
| 1148-1240 | `replay_normalization_events` | journal |
| 1241-1630 | working-byte *planning* (`planned_kind_working_bytes`, `kind_working_bytes`, `element_fields`, `restored_line_height`) | resource-model concerns interleaved with normalization |
| 1631-2307 | `normalize_element_kind` — **one 680-line function** | accretion |
| 2308-2820 | file records, crop, pressures, scale, frame rendering, binding, arrowheads | per-field normalizers, cohesive |
| **2821-3206** | fractional-index validation/repair (`is_valid_fractional_key`, `repair_fractional_indices`, `generate_n_fractional_keys`, `fractional_midpoint`, `increment_/decrement_fractional_integer`) | **~385-line self-contained algorithm with zero scene dependencies → `normalization/fractional_index.rs`**, independently unit-testable |
| 3207-3492 | generated IDs / rough seed / diagnostic helpers | fine |
| 3493-4474 | tests (981 lines) | move to `tests/normalization.rs` (which already exists, 609 lines) |

The interleaving of *resource-cost planning* (1241-1630) with *value
normalization* is the real cohesion problem: a reader cannot tell whether a
field's default is contract behaviour or accounting behaviour without reading
both. The two big functions are `match`-over-element-kind bodies that would
shrink if each `Normalized*Kind` had its own `fn normalize(...)`.

### `render/pdf.rs` — 3,721 lines — two modules in one file

| Lines | What |
|---|---|
| 21-40 | frozen `SRGB_TRC` (good: avoids `powf` at runtime, see §7) |
| 44-180 | `PdfPlan`, `PdfElement` (6 variants), `PdfRasterClosure`, `PdfGlyphMapping`, `PdfSvgChunk` |
| 200-350 | `PdfPlan::new` (lowering from `ResolvedScene`) |
| **363-938** | `PdfPlan::encode` — 575 lines, single function writing the object graph |
| 1034-1290 | font objects, glyph mappings, opacity ExtGStates |
| **1419-2047** | `encode_content` — ~630 lines, single function |
| 2195-2452 | clip-polygon geometry (`clip_convex_polygon`, `polygon_half_plane`, `line_intersection`) — target-neutral, belongs in `geometry.rs` |
| 2453-2519 | ICC profile builder |
| 2520-3157 | content-stream writers + `format_pdf_number`/`expand_exponent` |
| **3158-3477** | raster-closure & filter-occurrence analysis over `usvg::Tree` — this is *planning*, called from `preflight_pdf_raster_fallbacks` in `mod.rs:3212` |
| **3478-3720** | `svg2pdf` chunk renumbering/canonicalization (`renumber_svg_chunks`, `canonicalize_pdf_chunk`, `canonicalize_pdf_text`) — an adapter over a third-party writer's byte output |

Two clear seams: `pdf/nested_svg.rs` (3158-3720: the bounded svg2pdf adapter,
~560 lines, which the header comment at 42-43 already describes as "confined")
and `pdf/content.rs` (the content-stream writers). `encode` and
`encode_content` are too long to review for object-numbering invariants;
`ICC_PROFILE_OBJECT`/`OUTPUT_INTENT_OBJECT`/`FIRST_EXT_GSTATE_OBJECT` constants
at 15-17 and `checked_object(first, index, stride)` at 3702 imply a
hand-maintained object layout that would benefit from an allocator type.

### `render/png.rs` — 3,473 lines — three modules in one file

| Lines | What |
|---|---|
| 15-146 | `PngPlan`, `PngElement`, clip/mask types |
| 147-485 | frame-clip projection and coverage-area estimators |
| **695-1334** | `PngPlan::rasterize` — 640 lines, single function |
| 1334-1627 | per-index area queries the session uses for accounting |
| **1694-2061** | **the owned PNG encoder**: `PngIdatStream`, `compress_scanlines`, `ScanlineFilter`, `select_scanline_filter`, `paeth_predictor` — ~370 lines, zero dependency on the plan → `png/encoder.rs` (ADR-0004 describes this as an owned, independently tunable component; it should be a module) |
| 2062-2722 | tiny-skia path building, `paint_rough_drawing`, `PaintMasks` (2323-2557) |
| 2557-2560 | Font Awesome placeholder path constants (also cited in §10 license text) |
| 2723-3006 | conservative coverage-area estimators (accounting math, not painting) |
| 3075-3473 | tests |

`png/encoder.rs`, `png/coverage.rs` (accounting estimators), `png/paint.rs`
(tiny-skia lowering) are natural. The estimators exist because the ledger must
be a pure function of the scene (§13.2), not of rasterization — a good reason
to keep them *out* of the painter's file.

### `render/asset.rs` — 2,936 lines — accretion by codec

`bmp.rs` and `ico.rs` are already submodules (`asset.rs:27-28`), but PNG
(2243-2584, ~340 lines), JPEG (1848-2050 + 2585-2760, ~370), GIF (1516-1847,
~330), WebP (1183-1515, ~330) are inline. Also inline: the data-URL/base64
payload layer (2101-2242), the color-management layer
(`prepare_color_normalization`, `cicp_color_profile`, `png_color_profile`,
`LINEAR_TO_SRGB` — 320-342, 2759-2916), and the `Cache` (349-871, ~520 lines)
which is the actual deep module (semantic identity, occurrence dedup, charge
sharing). Seams: `asset/{png,jpeg,gif,webp,payload,color}.rs`, leaving
`asset.rs` as `MediaKind` + `Cache` + dispatch (~800 lines). Each codec already
has a `validate_*`/`decode_*`/`*_animation_validation_work` triple, so the
interface is already implicit.

### `render/rough.rs` — 2,141 lines — cohesive

A direct port of Rough.js 4.6.4 (spec §10) with its own `Random` (2105-2122),
`ecmascript_round` (2071), and the geometry primitives
`Point`/`Path`/`PathOperation`/`Drawing` (120-330). One responsibility
("reproduce Rough.js bit-for-bit"), one hidden state (the PRNG), deterministic
order. The only mismatch: `push_number`/`fixed_two_decimal_units` (336-404) is
*serialization*, not geometry, and is used by both `svg.rs:1517` and
`pdf.rs:2584`. It belongs in a `number_format.rs` beside
`freedraw::ecmascript_number` (`freedraw.rs:524`) and `svg::push_svg_number`
(`svg.rs:1628`) so all three formatting policies are reviewable together.
Also, `Path`/`Point` being defined inside the Rough port means `geometry.rs`,
`pdf.rs`, `png.rs` import their core geometry type from a *port of a JS
library* — inverted ownership.

### `scene.rs` — 1,973 lines — three responsibilities

- 1-90: fixed parser envelope limits (§13.1)
- 91-700: `SceneDocument` and the typed `*Source<'_>` accessors
- **834-1430: a bounded, lossless JSON parser** (`Parser`, ~600 lines; numbers kept as `Box<str>` tokens, `JsonValue` at 814)
- 1604-1765: original-JSON visitor for accounting
- 1766-1867: canonical JSON writer (§4.2)

The parser + `JsonValue` + canonical writer are a self-contained `json.rs`
(~900 lines) with no scene knowledge; `scene.rs` would keep envelope
validation and accessors.

### `render/svg.rs` — 1,697 lines

`SvgPlan::write` (608-1389) is a 780-line function. Otherwise cohesive.

### `render/resolve.rs` — 1,720 lines

`ResolvedScene::elements` (289-1116) is an 830-line function that resolves
*every* element kind, bound text, frame names, and bounds in one body. It is
the heart of stage 4 and would be the natural place for a per-kind `resolve_*`
split mirroring `PdfElement`/`PngElement`/`SvgElement`.

---

## 3. Pipeline shape

```
bytes ──SceneDocument::parse (scene.rs:100)──▶ SceneDocument { _root: Arc<JsonValue> }   [stage 1: preserve+parse]
   │   Parser (scene.rs:834), validate_envelope (1431)
   ▼
RenderSession::new (mod.rs:978) ─ admit_session (1247): scene_source_octets + original-JSON traversal charges
   ▼
acquire_resolution_inputs (mod.rs:1747)
   ├─ scene_surface::classify (scene_surface.rs:106) ─▶ SurfaceClassification { diagnostics, fallbacks }  [policy from generated table]
   ├─ normalization::normalize_scene_with_assets (normalization.rs:582) ─▶ NormalizationOutcome (normalization.rs:204)  [stage 2]
   ├─ relationships::resolve (relationships.rs:138) ─▶ RelationshipOutcome / ResolvedRelationshipGraph (relationships.rs:66,97)
   └─ FrameSelection::compute (frame_target.rs:241) ─▶ ResolutionInputs (mod.rs:812)  [frame scoping]
   ▼
account_semantic_census (mod.rs:1911) ── image::ImageResolver (image.rs:37) + asset::Cache (asset.rs:349)   [stage 3]
   ▼
element loop (mod.rs:1309-1745) ── geometry::rough_drawing (geometry.rs:157) → rough::* ; text::OfficialFont / layout_with_fallbacks (text.rs:762)
   ▼
assemble_resolved_scene (mod.rs:2115) ── ResolvedScene::elements (resolve.rs:289) ─▶ ResolvedScene (resolve.rs:5)   [stage 4: immutable semantic graph]
   ▼
account_fallback_semantics (mod.rs:2608)   [text/clip/fallback ledger charges over the resolved graph]
   ▼
encode (mod.rs:2219) ─▶ encode_{svg,png,pdf} (encode.rs:13,200,614)
   ├─ SvgPlan::new (svg.rs:433) / PngPlan::new (png.rs:486) / PdfPlan::new (pdf.rs:200)   [stage 5: typed plans]
   ├─ SVG only: font_demand::svg_font_demand → svg_fonts::build → font_subset::derive
   ├─ select_artifact_metadata (mod.rs:2309) ── metadata::ArtifactMetadata (metadata.rs:26), notices::MandatoryNotices
   └─ plan.measure()/encode_exact() | compress()+assemble | encode()   [stage 6: bytes]
   ▼
take_report (mod.rs:4348) ─▶ RenderOutcome { Artifact, CompatibilityReport, profile } (outcome.rs:310)
```

### Where the IRs are well-typed

- `SceneDocument` → `NormalizationOutcome` → `RelationshipOutcome` →
  `ResolvedScene` → `{Svg,Png,Pdf}Plan` → `Vec<u8>` are all concrete owned
  structs. `normalize_scene` (`normalization.rs:558`) and
  `relationships::resolve` are callable without a session in tests.
  `ResolvedScene` is reachable only through `RenderSession`.
- The spec's "no backend-native object in the shared graph" holds:
  `ResolvedScene` contains `rough::Drawing`, `text::ResolvedText`,
  `Arc<ResolvedImageAsset>`, no `tiny_skia`/`pdf_writer`/`usvg` types. The
  `usvg::Tree` cache is explicitly kept on the session (`mod.rs:205-210`).
- Backends do not touch `SceneDocument` (only `metadata.rs:72` does,
  legitimately, for the canonicalized-original embedding).

### Where the stage boundary is blurry

1. **Accounting is not a stage; it is a cross-cutting observer that can abort
   any stage.** Normalization and relationship resolution take an `observe:
   FnMut(Event) -> Result<(), E>` callback (`normalization.rs:590`,
   `relationships.rs:138`). The session passes a closure that charges the
   meter. So "normalize" cannot be run to completion and *then* charged — the
   charge order is the contract (ADR-0002). `PreparedFrameRender`
   (`mod.rs:212`) materializes that journal for frame batches, proving the
   stage *could* have a pure output.
2. **Normalization already knows about resource costs.**
   `planned_kind_working_bytes`, `kind_working_bytes`,
   `normalized_working_bytes` (`normalization.rs:1274-1476, 3207`) compute
   §13.2 ledger rows inside the normalizer.
3. **Resolution and accounting are interleaved in `resolve_scene`**
   (`mod.rs:1309-1745`). `account_fallback_semantics` (2608) then *mutates*
   the already-built `ResolvedScene` (`text.base_element_work_units = …` at
   2680). The "immutable paint-ordered semantic graph" of §9 is mutated once
   after assembly.
4. **Backends carry accounting hooks.** `PngPlan::{vector_coverage_area,
   image_projection_area, base_element_work_units, fallback_surface_area,
   paint_mask_surface_area, omit_element}` (`png.rs:1334-1581`) exist so
   `encode_png` can charge and *remove* elements from the plan after planning.
5. **`png.rs` imports `crate::normalization::NormalizedImageStatus`**
   (`png.rs:92, 275, 669, 1130-1157`): a stage-2 enum reaches the stage-5
   backend without being re-expressed in the resolved graph.
6. **Manual error-path cleanup in `encode.rs`.** `encode_svg` (13-197) repeats
   `release_svg_font_encoding_records(...)?; selection_working.release(...)?;
   return Err(...)` in eight arms. Forgetting one release changes the canonical
   trace (which ADR-0002 pins cross-target).
7. **Three near-identical element enums.** `SvgElement` (`svg.rs:105`),
   `PngElement` (`png.rs:25`), `PdfElement` (`pdf.rs:86`) each re-carry
   `source`, `frame_clips`, `transform`, `opacity`, `link`, `width`, `height`
   per variant. A common `PlannedElement<Extra>` header would cut ~250 lines
   of duplication.

---

## 4. Generated code

| File | Lines | Generator | Source of truth | Drift check |
|---|---|---|---|---|
| `excalidraw-render/src/generated/scene_surface_revision_1.rs` | 5,946 | `xtask::catalog::generated_scene_surface` (`xtask/src/catalog.rs:461, 507`) | `tests/oracle/scene-surfaces/revision-1.json` | `catalog verify` re-generates and compares (`catalog.rs:750-756`) |
| `assets-base/src/generated/revision_1.rs` | 6,551 | `generate_rust` (`catalog.rs:450, 498, 740`) | `catalog/revision-1.json` | `catalog.rs:737-743` |
| `assets-base/src/generated/xiaolai_faces_1.rs` | 2,532 | `generate_xiaolai_faces_rust` | `catalog/order/Xiaolai-index.ts` | `catalog.rs:744-749` |

- Generator is checked in (`xtask`), runs offline, never in `build.rs`.
- `catalog verify` is wired into CI as the Dagger `conformance.catalog` check
  and in `packaged-public-facade.yml:47,98`.

Weak spots:
- `SceneFieldPolicy` and friends are `#[allow(dead_code)]`
  (`scene_surface.rs:20, 31, 48, 56, 65`).
- `catalog.rs` (lib) **re-parses the 364 KB `scene-surface.json` and the 333
  KB `revision-1.json` at render time** with `serde_json::Value`
  (`catalog.rs:95-97, 134-141`) to look up notice text and font attribution,
  and scrapes the license bundle by splitting on human-readable headings like
  `"Official rendering font notices\n"` (`catalog.rs:116-120`). Error type is
  `()`. The notice catalog is the one part of the sealed policy that escaped
  code generation.
- `xtask/src/catalog.rs` itself is 3,538 lines and contains importer,
  verifier, materializer, Rust emitter, license-bundle builder, and
  reviewed-decode logic.

---

## 5. Error handling & diagnostics model

### Shape

- Five public error types, one per facade family, each with `.descriptor()`
  (`compatibility.rs:819`). `ErrorFamily` (`compatibility.rs:229`) and
  `RenderErrorCategory` (`outcome.rs:352`) are `#[non_exhaustive]`.
- `RenderError` carries the partial `CompatibilityReport` (`outcome.rs:388`).
- Stable codes are `&'static str` newtypes: `ErrorCode` (46 consts,
  `compatibility.rs:264-400`) and `DiagnosticCode` (43 consts,
  `outcome.rs:14-120`).
- Internal error types are stage-local and mapped at the session through a
  small set of `RenderSession::*_failure` constructors (`mod.rs:4523-4770`).
- Memory exhaustion is modelled, not panicked: 350 `try_reserve` calls,
  `RESOURCE_MEMORY_UNAVAILABLE`.

### Enforced by types or by convention?

Mostly **convention**:

1. **Code ↔ details coupling is unenforced.** `CompatibilityDiagnostic { code,
   details: Option<Box<ErrorDetails>>, effects: Vec<_> }` (`outcome.rs:156-167`)
   is constructed as a struct literal in 27 places across 5 files.
   `diagnostic_replaces_element` (`diagnostics.rs:3-18`) reverse-engineers
   meaning by matching seven `ErrorDetails` variants. A `enum DiagnosticKind`
   whose variants carry exactly their required details would make the registry
   a type.
2. **`DiagnosticCode::manifested(&'static str)`** (`outcome.rs:17`) lets the
   generated policy mint codes at runtime from strings.
3. **Recipe identities are string literals** compared by `==`
   (`diagnostics.rs:43`).
4. **Diagnostic ordering** is a hidden `DiagnosticOrder` (`outcome.rs:170`)
   with `u64::MAX` sentinels, sorted once in `take_report`
   (`mod.rs:4349-4361`).
5. `catalog.rs` uses `Result<_, ()>` (`catalog.rs:70, 93, 133, 145`).

Positive: `NormalizationError<E>`/`RelationshipError<E>` being generic over
the observer's error means those modules genuinely do not know `RenderError`.

---

## 6. Dependency hygiene

### Pinning

Every `[workspace.dependencies]` entry is `=x.y.z` (`Cargo.toml:21-60`).

- **Pro:** artifact-byte stability is the product; any transitive bump can
  change bytes.
- **Con:** exact pins on *libraries* prevent downstream consumers from
  unifying versions. This should be stated in the README for library users.
- Spec §10 lists `cargo-deny`, `cargo-vet`, RustSec, build-script, and
  native-link graph checks as release gates. **None exist in the repo** (see
  issue #87).
- No `rust-version` in any manifest and no `rust-toolchain.toml`.

### Declared-but-unused / oddities

- `read-fonts = { =0.42.1, features = ["experimental_traverse"] }` is a direct
  dependency of the lib (`excalidraw-render/Cargo.toml:31`) with **zero
  references** in any `.rs` file in the workspace.
- `anyhow` in the CLI: unused.

### Duplicated functionality

- **Two PNG encoders:** the owned miniz_oxide encoder (`png.rs:1694-2061`) and
  `png::Encoder` for normalized nested rasters (`asset.rs:1013-1029`).
- **Two of every codec via `resvg`'s `raster-images` feature**, which pulls
  the `image` crate: `Cargo.lock` has `png 0.17.16 + 0.18.1`, `zune-jpeg
  0.4.21 + 0.5.15`, `gif 0.13.3 + 0.14.2`, `moxcms 0.8.1 + 0.9.0`. The
  renderer decodes nested rasters itself and re-encodes to PNG, so resvg's
  `image` path only ever sees renderer-produced PNGs.
- **Three CSS colour libraries** in `color.rs`: `cssparser` +
  `cssparser-color` and `csscolorparser`.
- Hand-rolled SHA-256 in the CLI while `sha2` is in the tree.
- `write-fonts 0.48.1 + 0.51.0`, `read-fonts 0.39.2 + 0.42.1`, `skrifa 0.42.1
  + 0.45.1`: `skera 0.5.1` pulls older fontations crates than the renderer
  pins.

---

## 7. Determinism architecture

### Controlled well

- **No `HashMap` iteration anywhere.** The single `HashSet`
  (`svg_fonts.rs:17`) uses `BuildHasherDefault<DefaultHasher>` and is only
  used for membership. All keyed collections are `BTreeMap`.
- **No `std::time`, no `rand`, no `RandomState`, no `thread_local`, no `static
  mut`, no `OnceLock`**.
- **Rough PRNG** is a port of the JS `Math.imul`-based generator
  (`rough.rs:2105-2122`).
- **Float → text** is centralized: SVG uses Rust `Display` after `-0.0 → 0.0`
  (`svg.rs:1628-1641`); PDF uses `Display` then expands exponents
  (`pdf.rs:3042-3122`); Rough path numbers use an exact fixed-two-decimal
  conversion (`rough.rs:366-404`); freedraw uses an ECMAScript
  `Number.toString` emulation (`freedraw.rs:524`).
- **Sorting**: float sorts use `total_cmp`; stable `sort_by` where JS
  stability matters.
- **sRGB TRC frozen as data** (`pdf.rs:19-40`).
- **tiny-skia without `simd`**.
- **brotli `portable-float`** (ADR-0003).

### Not controlled (risks)

1. **Transcendental `f64` functions are used freely**: `rough.rs` (14 sites:
   `sin`/`cos` for ellipse points at 1466-1542, `powi`, `sqrt`), `color.rs`
   (15: `hue.cos()/sin()` at 140-141, `powf(563/256)` and `powf(1.8)` at
   200-210), `geometry.rs` (8), `pdf.rs`, `freedraw.rs`, `bound_text.rs`.
   `sqrt` is correctly-rounded; `sin`, `cos`, `powf`, `atan2` are **not** —
   on `x86_64-linux-gnu` they resolve to glibc's libm, on
   `wasm32-unknown-unknown` to Rust's `compiler-builtins` libm port. The
   2-decimal quantization absorbs most of it, but a value within an ulp of a
   rounding boundary will flip a digit. A pure-Rust `libm` crate would make the
   promise structural. This is the single largest determinism gap.
2. **`f64 → f32` at the tiny-skia boundary** (`png.rs:3030 finite_f32`).
3. **No `rust-version` pin**.
4. **`println!` shim for WASM** (`lib.rs:17-25`).

---

## 8. Top 5 strengths

1. **The stage IRs exist and are the right ones.**
2. **Determinism is designed in, not bolted on.**
3. **Bounded, panic-free input surface.**
4. **Sealed, verified, regenerable catalog.**
5. **The font stack layering matches its spec.** `font_demand.rs` (adapter,
   knows scene) → `svg_fonts.rs` (knows families/CSS) → `font_subset.rs`
   (knows sfnt/WOFF2, no SVG or scene types).

## Top 5 risks / debts

1. **`render/mod.rs` is a 4,787-line God object with a flat namespace.**
2. **Accounting is woven through every stage instead of being a stage.**
3. **Transcendental libm calls without a portability strategy.**
4. **Runtime JSON/text scraping of sealed data** in `catalog.rs:70-174`.
5. **Diagnostics registry is convention, and supply-chain gates are promised
   but absent.**

### Minor notes

- `#[allow(dead_code)]` ×17 and `#[allow(clippy::too_many_arguments)]` ×43 in
  the lib — the latter concentrates in `resolve.rs`, `normalization.rs`,
  `mod.rs`.
- `Path`/`Point`/`PathOperation` are owned by the Rough port
  (`rough.rs:120-160`) but used as the crate's geometry primitives.
- `xtask/src/catalog.rs` (3,538 lines) and `xtask/src/conformance.rs` (2,583)
  are the tooling equivalents of `render/mod.rs`, but unpinned and freely
  splittable.
