# wasm-bindgen 0.2.129 comparator refresh: reviewed in Git

Status: applied with the #207 wasm-bindgen-test 0.3.79 / wasm-bindgen 0.2.129
upgrade; reviewed through normal Git review (ADR-0019). There is no approval
hash.

The comparator's closure reaches `wasm-bindgen` and `js-sys` through its
dependencies' wasm32 edges, so the packages the canonical visual comparator
builds against moved (`wasm-bindgen`, `wasm-bindgen-macro`,
`wasm-bindgen-macro-support` and `wasm-bindgen-shared` 0.2.126 → 0.2.129,
`js-sys` 0.3.103 → 0.3.106) and with them `canonical-visual-v1.closure.json`
and the profile digest (ADR-0005). Nothing else in the profile changed: no
comparator source or vector was edited.

`before/` preserves the exact committed preimages of the profile, its sidecar
and its closure. Evidence measured under that profile, or under an earlier
archived one, keeps its recorded comparator binding through
`comparator_closure_successor` and the `COMPARATOR_REFRESHES` chain in
`xtask/src/conformance_candidate_gate.rs`; see
`comparator-closure-refresh-v2/README.md`.

Live evidence that binds the profile was rebound to the new digest instead; see
the "Visual comparator" section of `tests/conformance/README.md`.
