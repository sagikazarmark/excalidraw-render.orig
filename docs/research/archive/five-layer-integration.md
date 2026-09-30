# Five-layer integration record (`closure:all-five-layers`, #85)

Base: `a275ef3` plus the uncommitted stage-1 layer rows.

## Status: record generated, unapproved; completeness waits on the tranche

`tests/conformance/coverage/layers.json` is generated with `approval: null`.
The `layer-owner-tranche.json` owners have not been applied to the ledger, so
`verify-layers` lists 42 completeness problems. They are the 21 `layer:*` gaps,
the frame-export case, and the representative pin described below.

A dry run recorded every tranche owner in memory. It found two problems, and
both are the representative owner's adapter-source pin. That pin stays until a
V25 successor candidate is collected, because xtask sources changed.

## What the spec asks

Spec 14.2 lists five corpus layers. They are exact compatibility-critical
vectors, focused scene documents, representative scenes, adversarial and
resource-boundary documents, and permanent minimized regressions. ADR-0015 turns
`closure:all-five-layers` into a record: every registered vector family, fuzz
risk, regression corpus and case, in every layer, maps to a ledger row.

## Bindings

The record avoids bindings that churn routinely:

- **Candidate registry and superseded `candidates/*` directories.** These
  change with every representative successor, so they are checked by rule
  instead (step 5 below).
- **Spec digest.** Only the family sentence's normalized text is bound.
- **Catalog digest.** Selectors are expanded, and orphans checked, against the
  current catalog.
- **Focused registry digest.** Only the case ids are bound.
- **Oracle and fuzz source bytes.** Their owners bind them.

`derive.rs` stays bound because it is the rule set. A simulated V25 apply on a
scratch copy moved the registry and the ledger binding, and `layers.json` still
reproduced.

## Rules

`xtask/src/coverage/layers/derive.rs` holds every discovery, derivation and
completeness rule, and the record binds its digest. Each rule in the record's
`rules` list carries a spec anchor. The rules read the repository and the
ownership catalog, never the ledger's dispositions. So recording an owner
changes whether the record passes, but not the record itself.

1. **Vector families.** The families are parsed from the spec sentence
   beginning "Exact vectors cover". It is split on commas and ends at the first
   period that is followed by whitespace, so the period in `Rough.js` does not
   end it. `declarations.vector_families` must name the same families in the
   same order.
2. **Vector corpora.** Stored corpora are discovered by path:
   - `tests/conformance/comparator-vectors/*/`, which must hold `manifest.json`;
   - `crates/*/tests/fixtures/*-vectors.json`;
   - `tools/*-vectors/`.

   Each one needs exactly one declaration.
3. **Oracles.** Every `tests/oracle/*/` directory is declared as `vector` (with
   rows), `authority` or `profile`.
4. **Registered cases.** Every case in `cases/registry.json` maps to the
   `case-label` and `layer` rows that name it as a candidate. It must be the
   case owner of at least one of them.
5. **Representative.** Each `candidates/registry.json` entry's coverage rows
   must be owned by that entry's representative binding. The #84 cases are
   checked by the record itself; see below.
6. **Adversarial targets.** A target is found by one of these paths:
   - `crates/*/src/**/*_fuzz*`, as a module file or directory (outermost match);
   - `crates/*/tests/*_fuzz*` or `crates/*/tests/limits_*`, as a test binary
     with its module directory;
   - `crates/*/{src,tests}/**/frame_batch_property`.

   A target maps to `layer:adversarial:<name>`, where the name uses `-` for
   `_`. `REQUIRED_RISKS` is parsed with `syn`, not by text search. The parser
   takes a `const REQUIRED_RISKS: [T; N] = [..]` array, checks that its length
   matches `N` and that the risks are unique, and allows at most one such
   array per target.
7. **Regressions.** Each `crates/*/tests/qualification/*-regressions.json` maps
   to `layer:regression:<name>`. The record keeps its digest, case ids,
   public-facade case ids and risks.
8. **No orphans.** Every `layer:*` catalog row must be referenced by an item of
   its own layer. No item may map to a `closure:*` row.

Completeness then reads the ledger through a read-only `LedgerView`. The view
applies `validate`'s per-assignment checks to one row at a time. It never
validates closure rows, so the `layers` verifier cannot recurse into its own
record owner. The requirements are:

- Each vector item needs at least one owned row.
- Every mapped `layer:*` row must be owned.
- Every owned mapped row's owner must pass its checks. A case owner must also
  pass conformance verification.

Owners are cached per (category, test selection), so the 700 cICP rows cost one
module resolution.

## Vector-family mapping

| Spec family | Row | Owner (tranche) |
| --- | --- | --- |
| Rough.js | `layer:vector:rough-js` | `render::tests::outline_operations_match_the_pinned_roughjs_vectors` |
| the qualified Perfect Freehand path | `layer:vector:perfect-freehand` | `render::tests::freedraw_strokes_match_the_pinned_perfect_freehand_vectors` |
| PRNG consumption | `layer:vector:prng` | `render::rough::tests::architect_solid_ellipse_consumes_both_overlap_pairs` |
| normalization | `layer:vector:fractional-index` | `normalization::fractional_index::tests::repair_matches_the_pinned_source_order_vectors` |
| geometry | `layer:vector:geometry` (new) | `render::tests::rounded_diamond_radii_match_the_pinned_roughjs_vectors` |
| color | `layer:vector:color` (new) | `color::tests::pinned_chromium_color_level_4_forms_lower_to_canonical_srgb` |
| shaping/fallback | `layer:vector:shaping-fallback` (new) | `render::tests::a_font_fallback_probe_is_one_consulted_face_per_shaped_scalar_occurrence` |
| serialization | `layer:vector:number-format` | `render::number_format::tests::ecmascript_spelling_matches_pinned_chromium_vectors` |
| diagnostic order | `interaction:diagnostic-order` (existing, owned) | `stage4_diagnostic_contract::mixed_normalization_render_resource_metadata_preserve_source_order` |
| format planning | `layer:vector:format-planning` (new) | `render::plan_tests::png_surface_follows_the_sizing_rule` |

The existing owned rows were reviewed for geometry, color, normalization and
serialization, and none of them hold exact vectors:

- The `surface:*` geometry and color rows are field-recovery contracts.
- The `codec:*:color-profile` rows cover image color management, not color
  values.
- The `diagnostic:normalization.*` rows assert repair diagnostics.

Geometry and color therefore got new rows. Normalization and serialization
reuse the stage-1 fractional-index and number-format rows.

The comparator is declared as a further vector domain, anchored in the
Pixelmatch paragraph and mapped to `layer:vector:pixelmatch` and `ssim`. The
stored corpora map as follows:

- `number-format-vectors.json` and `tools/number-format-vectors` map to
  number-format.
- `pixelmatch-v7.2.0` maps to pixelmatch.
- `tools/rough-filler-vectors` maps to rough-js and geometry.

Fifteen oracle directories are vector oracles, each mapped to the `codec:*`
rows whose tests read it; `png-color` maps to 721 rows. Three are inputs:

- `scene-surfaces` is the authority from which `surface:*` and `diagnostic:*`
  rows derive.
- `reference-profiles` and `compatibility-artifacts` are profiles.

`jpeg-color` and `jpeg-gain-map` include 14 open #187/#190 gap rows. The rule
is at least one owned row per item, so those gaps stay visible in the ledger
without blocking this record.

## The #84 check

`tools/representative-intake/case.py` is ported to the checks the offline
`validate` runs, plus the five-layer requirements. For each registered #84
case:

- The identity is checked: schema, `layer: representative`, the #84
  originating issue, and the registry id.
- The retained-file inventory (every file except `case.json`) is current.
- The intake manifest's bytes descriptor is current, and its file inventory
  and digests are current.
- `sanitization.retained_sha256` equals the retained `scene.excalidraw` digest.
- A license is present: an SPDX value and non-empty `LICENSE` bytes.
- The comparisons are non-empty and equal the list derived from the formats.
- The plan has `whole` and at least one crop anchored to scene elements.
- `case.json` is approved on a #85 comment, and the approval binds the subject
  that case.py computes. That subject is `json.dumps(sort_keys=True,
  separators=(",", ":"), ensure_ascii=False)`, ported exactly, including
  Python's float `repr`.

The three approved cases reproduce their recorded subjects.

## Tests

`xtask` tests override repository files in memory, as the `pdf-judgments`
tests do. Each of the following fails verification:

- a new vector fixture, `tools/*-vectors` directory, comparator directory or
  oracle directory;
- a new registered case, which fails the rebuild and, once regenerated, lacks
  a case owner;
- a new candidate directory or intake bundle;
- a new `*_fuzz` or `limits_*` target;
- a new required risk, or an inconsistent risk length;
- a new or duplicate regression id, or a new regression corpus;
- an orphan layer row, a closure or unmatched selector, or a missing layer
  owner;
- a #84 case with a bad approval URL, a missing approval, a changed subject,
  empty comparisons, a stale intake, a bad `retained_sha256`, or no crop.

An in-memory recorder test shows that assigning the reviewed owners clears
exactly their items.
