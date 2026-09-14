# Surface-field ownership audit (#85)

Base: `91b7970f45b5be29dd6aa9c5f2eca5d542c40c35`.

## Authority and method

Reviewed the 202 field rows (605 applicable field/format obligations) in
`tests/oracle/scene-surfaces/revision-1.json`. The first batch closes the common
nonpainting/preservation contracts and request-only controls. It does not infer
coverage from a field's presence in a scene or from a passing broad render test.

Eight authored public-facade tests in `fast/surface_metadata_contract.rs` supply
351 new format-scoped owners. Field lists and expectations are independent of
the generated renderer policy. Each test renders through `SceneDocument` and
`Renderer`; metadata assertions decode the artifact's actual embedded scene.

| Owner family | Fields | Obligations | Evidence |
| --- | ---: | ---: | --- |
| Editor-only app state | 84 | 252 | Individual field mutations, absent baseline, all JSON types, explicit valid enum values, mixed geometry/text/image/frame scene, exact output/report invariance |
| Scene source/scroll/library metadata | 3 | 9 | Individual values, arbitrary descendants, exact preserved original bytes and scene/frame silence |
| Common element metadata/index | 5 | 15 | Mixed participating/deleted records; overlapping geometry; duplicate, descending and malformed indices retain paint order |
| Kind-specific editor metadata | 10 | 30 | Text sizing metadata, line/arrow/freedraw committed points, elbow routing metadata, iframe generation container and embeddable validation |
| File editor metadata | 2 | 6 | Referenced and deleted/unreferenced authoritative image records; every JSON type; source preservation |
| Nested generation/fixed-segment members | 7 | 21 | Each member absent or mutated; conflicting coordinates and inert HTML/code; source preservation |
| Persisted request-only controls | 4 | 12 | Exact invalid-value diagnostics, request precedence for background/dark/scale/embedding, invalid-original preservation and frame reports |
| Line-only arrowheads | 2 | 6 | All accepted head kinds remain nonpainting; exact invalid recovery, deleted/unselected silence, source preservation and selected-frame reports |
| **Total** | **117** | **351** | |

Preservation checks compare complete decoded scene bytes against the submitted
compact JSON, not a renderer-produced expectation. Fresh render sessions and
direct/lazy frame results agree. Source whitespace/number-token edge cases remain
owned by the separate canonical-scene tests; these tests use ordinary JSON values.

## Recording and review

`record-test-owner MODULE FUNCTION RATIONALE ID...` validates the whole proposed
batch before writing: identities must be unique existing gaps, sources and
unconditional selectors must be valid, and rationales nonempty. It neither runs
tests nor grants approval. Negative CLI tests prove rejection leaves the ledger
unchanged, including a valid first selection followed by an invalid selection.

`refresh-test-owner-source MODULE` updates only live source descriptors and
revalidates every selector. It refuses an approved global ledger. Existing
producer-only approval policy is retained; selecting a new or weakened owner
still requires assertion review. This command is for the incomplete proposal.

Two-axis review found missing valid structured/enum probes, generation-container
mutation at the wrong boundary and missing invalid-original/frame assertions.
Those were added before recording completion. The review also identified the
approval-refresh risk; refreshing an approved ledger is now refused.

## Remaining work

Current totals: **517 assigned / 411 gaps**, including **254 surface gaps**.
The remaining surface rows cover parser admission, timestamps, geometry/style,
text/font normalization, relationships/bindings, frames, asset records, crop and
nested SVG. Other families retain 75 diagnostic, 50 codec, 24 exception,
4 interaction and 4 closure gaps. These need dedicated contracts/adapters and
the #83/#84/#178 prerequisite work; they are not exclusions.

The current 168-key canonical baseline is unaffected: these tests and ledger
assignments do not modify renderer producers, case manifests, or execution keys.
Global approval remains pending and `verify-closure` must continue failing.

## Verification and scoped approval

- All eight new facade contract tests pass; the recorder's five CLI contract
  tests pass, including all-or-nothing validation of rejected batches.
- Workspace/all-target typechecking passes.
- All ten Dagger conformance/lint checks pass.
- Full final workspace suite: `dagger check excalidraw-render:test` passed all
  three tiers (fast, render/parity, doctests) in 13m9s.
- Two-axis review: no hard Standards violations; duplicated command construction
  was consolidated. All actionable Spec findings were corrected and rechecked.

The maintainer's “#85 and approve all”, followed by “do it”, authorizes these
351 explicitly reviewed assignments. The committed ledger records their exact
selectors, source hashes and rationales. This scoped approval does not turn the
incomplete global ledger into a completed approval subject.
