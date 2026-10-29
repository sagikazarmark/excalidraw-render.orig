# Geometry, style and color ownership (#85)

Base: `c54dcfeb9fdbabf44c720dc70d825a9d765962f2`.

## Completed assignments

Five public-facade contract selectors in `fast/geometry_style_contract.rs` own
54 obligations, all in SVG/PNG/PDF:

| Contract | Fields | Obligations |
| --- | --- | ---: |
| Coordinates/dimensions/rotation | x, y, width, height, angle | 15 |
| Styles | strokeWidth, roughness, opacity, fillStyle, strokeStyle | 15 |
| Colors | strokeColor, backgroundColor, viewBackgroundColor | 9 |
| Roundness | roundness, strokeSharpness, nested type/value | 12 |
| Seed | seed | 3 |

The matrices explicitly author field defaults and ignored-kind participation.
They cover fourteen kinds including legacy draw and silent selection, absent
fields, wrong JSON types, invalid domain boundaries, valid enums and representative
numbers. Exact ordered reports are compared independently of runtime policy.
Artifact comparisons use explicit clean repairs or specified equivalences;
embedded original bytes, fresh sessions, direct/lazy frames, excluded targets
and deleted silence are checked at the facade.

Positive semantics come from existing independently pinned Rough.js paths,
color/alpha/dark-mode consumer assertions, negative-dimension origin shifts,
linear-point authority, anchored translation/extent/rotation probes, and explicit
adaptive radius versus proportional-radius clamping. The independent seed vector
for identity `paint` is 438155844, calculated with hashlib over the frozen
revision/identity transcript, not obtained from renderer output.

Roundness tests cover malformed container/nested members at the narrowest
pointer, absent type/value, modes 1/2/3, ignored kinds, legacy round/sharp/other
strings, and explicit modern-record precedence. Color tests cover named,
short/full hex and functional CSS equivalence, malformed values, scene
background inclusion and light/dark requests. Invalid global background values
are preserved and diagnosed in populated and otherwise empty frame exports.

## Production fix

The first coordinate/default matrix failed on a valid image with zero destination
width/height: PNG encoding attempted to invert its singular transform and
returned `encode.png-failed`. The PNG image paint branch now skips zero-area
destinations. Admission, normalization, diagnostics, bounds and original-source
preservation still occur normally. The facade matrix passes both defaulted and
invalid-dimension recovery branches after the fix.

## Review and verification

Two-axis review found no hard Standards violations and two low-priority helper
naming/duplication suggestions. Spec review requested positive explicit-radius
evidence and global background preservation/frame diagnostics; both were added
and passed. The final Spec review supports all 54 assignments with no remaining
actionable findings in this batch.

Workspace/all-target typechecking, targeted contract tests, ownership verification,
and all ten canonical Dagger conformance/lint checks pass. The final full workspace
suite (`dagger check excalidraw-render:test`) passed fast, render/parity and
doctest tiers in 16m0s.

## Approval and remaining scope

The maintainer's continuing “approve all” and “do it” instructions authorize
these 54 reviewed assignments and the demonstrated zero-destination PNG fix.
Exact executable selectors, source digests and rationales are in the ledger.
Current totals: **604 assigned / 324 gaps** (173 surface and 151 other).
Global approval remains pending; #85 closure is still incomplete.
