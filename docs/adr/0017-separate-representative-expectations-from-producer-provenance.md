---
status: accepted
---

# Separate representative expectations from producer provenance

The representative-case verifier required every current renderer source file,
Cargo manifest, adapter source, and executable hash to equal the run that produced
its approved evidence. Even repository metadata or crate documentation therefore
required a numbered successor and reapproval. The maintainer chose to simplify
this binding during 0.1.0 preparation: **approved output plus fresh renderer replay
is authoritative; source and build identities remain historical provenance.**

Existing manifests and approval subjects remain byte-for-byte intact. Their
historical producer descriptors are validated for shape, path, identity, and
required membership, and remain bound by the original approval. Replaying builds
the current renderer with the qualified toolchain and runs each format twice,
requiring exact approved artifact and report bytes. Current source identities
are captured before replay and checked afterwards, preventing a mixed-source run.
The replay reports the current build separately from the historical build.

Scene and request bindings, captured viewer inputs/rasters, semantic expectations,
comparison profiles, tolerance exceptions, and approval-subject validation stay
live. Changed evidence or judgments still require acceptance; a new producer
identity alone does not. Read-only PDF bridge validation likewise checks the
actual structural and raster equality instead of requiring today's renderer to
have the historical source hashes. Generation still binds its actual sources.

This supersedes the representative source-binding parts of the conformance
README and ADR-0015's owner verification, not their complete-evidence or
approval requirements. Other evidence families retain their existing policies.

The same distinction applies to the approved native baseline's and PDF judgment
record's inventory descriptors.
The inventory includes maintenance links to test source files, so refreshing a
companion test could previously invalidate all 168 unchanged expectations. Its
historical descriptor remains inside the signed-off baseline subject. Verification
compares every current execution key (including scene, profile, options and asset
input digests), the complete artifact/report bytes, and the collector mapping.
Changes to those inputs still fail; source-anchor bookkeeping alone does not.
