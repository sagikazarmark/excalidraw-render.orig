# jsonschema 0.58 comparator refresh: reviewed in Git

Status: applied with the jsonschema 0.33 → 0.58.2 upgrade that #208 deferred;
reviewed through normal Git review (ADR-0019). There is no approval hash.

The comparator's profile test validates the profile with jsonschema, so the
packages the canonical visual comparator builds against moved (`jsonschema`,
`referencing` and their dependencies; the ICU/IDNA stack out,
`jsonschema-regex`, `jsonschema-value` and their dependencies in) and with them
`canonical-visual-v1.closure.json` and the profile digest (ADR-0005). Nothing
else in the profile changed: no comparator source or vector was edited.

`before/` preserves the exact committed preimages of the profile, its sidecar
and its closure. Evidence measured under that profile, or under an earlier
archived one, keeps its recorded comparator binding through
`comparator_closure_successor` and the `COMPARATOR_REFRESHES` chain in
`xtask/src/conformance_candidate_gate.rs`; see
`comparator-closure-refresh-v2/README.md`.

Live evidence that binds the profile was rebound to the new digest instead; see
the "Visual comparator" section of `tests/conformance/README.md`.
