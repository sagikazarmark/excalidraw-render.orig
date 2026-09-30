# #212 comparator identity split: reviewed in Git

Status: applied with #212 (ADR-0005, amended 2026-09-30) and reviewed through
normal Git review (ADR-0019). There is no approval hash.

The profile stopped binding the comparator's dependency closure:
`implementation.closure` is gone from `canonical-visual-v1.json` and from
`comparator-profile-v1.schema.json`. The closure file stays committed and
checked against `Cargo.lock` by the profile test, but no digest binds it, so a
later closure-only dependency move changes that file and nothing else.

Removing the field moved the profile, its sidecar and its schema digest one
last time. No comparator source, protocol value, limit, vector or fixture
changed.

`before/` preserves the exact committed preimages of the profile, its sidecar,
its closure and the schema. Evidence measured under that profile, or under an
earlier archived one, keeps its recorded comparator binding through
`comparator_closure_successor`. The `COMPARATOR_REFRESHES` row for this
refresh migrates the schema digest, as earlier rows migrated vector digests;
see `comparator-closure-refresh-v2/README.md`.

This is the last closure refresh. Live evidence that binds the profile was
rebound to the new digest; see the "Visual comparator" section of
`tests/conformance/README.md`.
