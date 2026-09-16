---
status: accepted
---

# Share PDF raster closure planning and encoding

The PDF diagnostic owner exposed two authorities: project preflight admitted a
clipped final grid while svg2pdf independently rasterized an unclipped local
layer. A 100-pixel crop could embed 200 pixels, and half-scale exports reported
10×5 while embedding 20×10. Use one occurrence-specific closure plan for resource
admission, diagnostics, rasterization and embedding, adapting the existing pinned
native converter rather than forking its page/text/vector implementation.

The plan records both the full filter-input grid and the clipped output grid.
When they differ, both selected surfaces are admitted and charged; a crop cannot
make its required input workspace free. Render the input once, copy the selected
output pixels without resampling, and give svg2pdf only native content and the
resulting PNGs. Patterns and masks are instantiated per reference, their consumer
transforms participate in sampling, and `feImage` dependencies merge into their
enclosing filtered closure. Native ancestors retain their own compositing scope.

The adapter necessarily crosses usvg's read-only normalized-tree interface via
SVG. Its pinned writer has a twelve-decimal precision ceiling and cannot name
anonymous `feImage` inputs reliably. The internal serializer adapter restores
round-trip path/transform tokens and materializes those input subtrees explicitly.
Nested-image bindings reuse data URLs already present in the serialized tree,
avoiding a second child serialization solely for lookup. These assumptions,
occurrence expansion and the qualified image-wrapper
compaction form one dependency qualification surface; upgrades must exercise all
three together. Stylesheet filters are resolved with the same already-transitive
`simplecss` version before adding private effect IDs, preserving source provenance
and avoiding selector feedback from those IDs.

ADR-0008's Flate bytes remain authoritative: ordinary nested-SVG chunks keep the
existing compressed path; adapted content streams are requested uncompressed,
and all stored streams except XMP are compressed by the owned stream writer.
Redundant image wrappers in Form and tiling-pattern content are compacted first.
This avoids an allocating
inflate pass and preserves the existing unit-scale qualification artifact and
readable structure. The original PDFium, PDF.js and qpdf gates remain required;
their evidence must not be silently relaxed to accommodate adapter changes.
