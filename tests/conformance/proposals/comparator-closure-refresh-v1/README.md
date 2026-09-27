# #206 comparator closure refresh: reviewed in Git

Status: applied with the #206 dependency refresh; reviewed through normal Git
review (ADR-0019). There is no approval hash.

The #206 refresh moved packages the canonical visual comparator builds against
(`sha1`, `crc32fast`, `flate2`, `icu_*`, `syn`, ...), so
`canonical-visual-v1.closure.json` and with it the profile digest moved
(ADR-0005). Nothing else in the profile changed: the comparator's sources,
comparison protocol, Pixelmatch fixtures and adversarial vectors are identical.

`before/` preserves the exact committed preimages of the profile, its sidecar
and its closure. Evidence measured under the old profile — the stage4 PDF
typography diagnosis and the current candidate's viewer evidence — keeps its
recorded comparator binding. `comparator_closure_successor` in
`xtask/src/conformance_candidate_gate.rs` accepts such a binding only when it
equals an archived preimage, the live profile has the reviewed digest
(`pdf_viewers::COMPARATOR_DIGEST`), and the two profiles differ only in
`implementation.closure`.

Live evidence that binds the profile was rebound to the new digest instead; see
the "Visual comparator" section of `tests/conformance/README.md`.
