# Compatibility revision 1 scene surface

`revision-1.json` is the normative machine-readable field/value disposition and
fallback-recipe manifest for compatibility revision 1. Its adjacent SHA-256 is
over the exact RFC 8785 canonical JSON bytes.

`scene-surface-v1.schema.json` defines the closed artifact shape. The
materializer also performs semantic validation that JSON Schema cannot express:
exhaustive record/element/path coverage, reference integrity, content-addressed
recipe IDs, registry consistency, and an empty unresolved-coverage set.

`evidence/revision-1-harmless-fields.json` records focused SVG/PNG A/B probes and
official placeholder snapshots produced by the pinned, offline reference
profile. It is evidence for this manifest, not production renderer output.

Regenerate after an approved scene-surface change:

```sh
tools/reference-oracle/run-scene-surface.sh update
```

Verify without changing checked artifacts:

```sh
tools/reference-oracle/run-scene-surface.sh verify
```

The production renderer does not depend on these development-only tools.
