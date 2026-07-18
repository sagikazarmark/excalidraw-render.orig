# Excalidraw Rendering

This context describes native rendering of Excalidraw scenes and the compatibility promise made against the official renderer.

## Language

**Scene document**:
A native Excalidraw JSON document containing scene elements, application state, and embedded files. Exported images, clipboard payloads, shared links, and collaboration state are not scene documents.
_Avoid_: Excalidraw project, Excalidraw artifact

**Reference renderer**:
The renderer from the pinned official Excalidraw release whose SVG and PNG output defines expected visual behavior.
_Avoid_: Real thing, upstream latest

**Perceptual parity**:
The state in which output is materially indistinguishable from the reference renderer at normal viewing, subject to explicit and tested tolerances rather than byte or pixel identity.
_Avoid_: Drop-in replacement, pixel-perfect rendering

**Static export**:
A whole-scene SVG, PNG, or PDF representation without interactive editor overlays or viewport state. Frames and their clipping behavior remain part of the scene; exporting one frame separately does not.
_Avoid_: Editor snapshot, canvas rendering

**Compatibility diagnostic**:
An explicit report that a scene uses behavior outside the pinned reference baseline, even when known content can still be rendered on a best-effort basis.
_Avoid_: Silent fallback, unsupported-element skip

**Conformance corpus**:
The versioned collection of focused, real-world, adversarial, and regression scene documents used to judge parity with the reference renderer.
_Avoid_: Golden tests, fixture folder
