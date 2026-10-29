---
status: implemented (#210)
---

# Validate subset layout structurally with a typed walker

The SVG font-subset layout validator walked GSUB, GPOS and GDEF through
read-fonts' untyped `traversal` API, which read-fonts 0.44 removed
(googlefonts/fontations#2087) with no replacement. That pin held the renderer on
a second parser version (ADR-0003, 2026-09-10 amendment). The validator moves to
a typed walker over `write_fonts::read`, and the direct read-fonts dependency
goes (#210).

The validator is structural: every glyph ID, lookup index, feature index, class
value and offset in GSUB, GPOS and GDEF must refer to something that exists,
format 2 ranges must be ordered, and counts must match array lengths. It walks
everything present, including lookups no feature reaches, because it judges the
subsetter's bytes, not shaping. Whether the subset is closed under GSUB is a
different guarantee; the shaping probes cover it today, and a closure check
would be qualified separately.

Only bundled, digest-verified fonts reach the validator, and it checks our own
subsetter's output. A format the walker cannot model is therefore a subsetter
bug or a profile change, so it fails closed. The walker still covers the whole
OpenType layout specification, with write-fonts fixtures for formats no bundled
font uses: rejecting a valid format would surface as a false failure during an
unrelated catalog change.

Typed nesting has a fixed shape and nested lookups are referenced by index, so
the traversal depth limit is dropped. A named work budget stays, bounding
repeated visits through shared offsets, and is tested against the largest
bundled font. Neither number is part of the specification. The public
`Validate` failure stage is unchanged; a finer reason exists only internally,
for tests.

Probe selection for the qualification corpus no longer shares the walk. It
treats a glyph as layout-referenced when a GSUB or GPOS lookup's coverage
includes it, counting whole ranges. Traversal's accidental definition (range
endpoints only, GDEF classes included) is not preserved; the resulting corpus
change is reviewed under ADR-0019.

The traversal walker served as a test oracle on read-fonts 0.43.3, the last
release with traversal, until the typed walker matched it over bundled fonts,
corpus subsets, fixtures and seeded mutations; it is retired. Mutations that
exposed a disagreement remain as fixed fuzz regressions.
