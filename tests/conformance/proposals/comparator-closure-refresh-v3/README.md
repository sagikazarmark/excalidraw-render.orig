# #208 sha1 comparator refresh: reviewed in Git

Status: applied with the #208 sha1 0.11 upgrade; reviewed through normal Git
review (ADR-0019). There is no approval hash.

sha1 0.11 moved the packages the canonical visual comparator builds against:
`sha1` itself, and the digest 0.10 line (`digest`, `block-buffer`,
`crypto-common`, `cpufeatures`, `generic-array`) that only sha1 0.10 still
held, so `canonical-visual-v1.closure.json` and with it the profile digest
moved (ADR-0005). Nothing else in the profile changed: the only migrated
source is the profile test's git blob hash, which is not a profile vector.

`before/` preserves the exact committed preimages of the profile, its sidecar
and its closure. Evidence measured under that profile, or under an earlier
archived one, keeps its recorded comparator binding through
`comparator_closure_successor` and the `COMPARATOR_REFRESHES` chain in
`xtask/src/conformance_candidate_gate.rs`; see
`comparator-closure-refresh-v2/README.md`.

Live evidence that binds the profile was rebound to the new digest instead; see
the "Visual comparator" section of `tests/conformance/README.md`.
