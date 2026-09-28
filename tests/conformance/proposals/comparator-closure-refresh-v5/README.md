# #211 serde_json comparator refresh: reviewed in Git

Status: applied with the #211 serde_json 1.0.151 upgrade; reviewed through
normal Git review (ADR-0019). There is no approval hash.

serde_json 1.0.151 formats floats through `zmij` instead of `ryu`, so the
packages the canonical visual comparator builds against moved (`serde_json`,
`zmij` in, `ryu` out of its closure) and with them
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
