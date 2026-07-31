---
status: accepted
---

# Treat SVG font subsets as Modified Versions

Runtime subsetting deletes or substitutes parts of the official font files, so revision 1 treats each OFL-licensed derived font subset as a Modified Version rather than claiming that it remains exact official content. This preserves a defensible license boundary while allowing semantic and typographic parity to remain the compatibility target instead of exact reference font bytes or names.

Derived subsets retain the approved copyright and license records and carry mandatory artifact notices. The Reserved Font Names `Virgil`, `Cascadia Code`, `Lilita`, and `Liberation` are replaced in both primary OpenType names and SVG CSS by `ExcalidrawRender-1`, `ExcalidrawRender-3`, `ExcalidrawRender-7`, and `ExcalidrawRender-9`; exact official source bytes and approved lossless decodes may retain the original names. The detailed name-record, identity, notice, and verification rules remain in the revision-1 renderer specification.
