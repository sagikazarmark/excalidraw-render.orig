# Bounded image ICC binding bridge — evidence, not approval

The fixed PDF is bound to the unchanged v4 authoring and original typography
diagnosis. Only the image colour-space entry and derived IDs differ; all raw
streams, fonts, text coordinates and other objects are identical. Both pinned
viewer rasters repeat and match the diagnosed rasters byte-for-byte.

The exception adapter validates this bridge before accepting the new PDF hash;
an unrelated PDF or stale proof is rejected. See
`docs/research/archive/stage4-image-icc-fix.md` for identities, metrics and review steps.
