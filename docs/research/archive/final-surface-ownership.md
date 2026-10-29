# Final current-surface ownership (#85)

Base: `0a254810dd54ca30032fdb6a587ad3ad975a851a`.

## Completed scope

Three public-facade selectors in `fast/final_surface_contract.rs` own the final
eight current surface obligations: element type (SVG/PNG/PDF), links (SVG/PDF),
and nested local SVG resources (SVG/PNG/PDF). The ledger records exact executable
selectors, source hashes and rationales. All **605 current surface obligations**
now have owners; the global ledger has **777 assigned / 151 gaps**.

Type evidence covers known-kind admission, missing/nonstring/empty/unknown type
discrimination, bounded placeholder versus unbounded omission, original-source
preservation, fresh calls and selected/lazy frame metadata. Independent
transformed-placeholder, SVG grammar, PNG pixel and PDF vector assertions check
rendering rather than merely diagnostics; omitted artifacts match removal of the
source occurrence.

Link evidence covers invalid values, unsafe URL schemes, canonical safe URLs,
PNG silence, source preservation, scene/frame behavior and annotation ordering.
A 90-degree-rotated, partially frame-clipped linked rectangle has an independently
worked PDF rectangle and ordered quadrilateral; fully clipped content produces
no annotation. Page `/Annots` references are followed to verify topmost-first
order independently of object serialization order.

Nested-resource evidence composes existing facade checks for local fragments,
data resources, bounded recursion/node/work limits, cycles, sanitization,
surviving siblings, nested path/effect/order diagnostics, intrinsic/reference
authority, vector projections and isolated PDF fallback. Added assertions decode
SVG references and require their targets, verify fragment-produced PNG pixels
and PDF vectors, and retain complete original scene bytes in standalone/lazy
frame metadata. The nested BMP positive-data-resource oracle also runs.

## Review and approval

Two-axis review found no hard Standards violations and two nonblocking test-helper
duplication/naming suggestions. Spec findings prompted independent placeholder,
PDF link hit-region/page-order, and local-fragment rendering evidence. All were
corrected and rechecked; final review supports the eight current-surface owners.
No production change was required.

The maintainer's continuing “approve all” / “do it” authorization covers these
eight selections. This completes the current surface inventory, not global #85
closure: 69 diagnostic, 50 codec/profile, 24 exception, four interaction and four
closure obligations remain gaps. Specialized qualification, profile expansion
and corpus/release prerequisites remain separately tracked.

Targeted tests and workspace/all-target typechecking pass. All canonical
conformance/lint checks pass after correcting the Clippy approximate-constant
finding. The final full workspace suite (`dagger check excalidraw-render:test`)
passed fast, render/parity and doctest tiers in 18m22s.
