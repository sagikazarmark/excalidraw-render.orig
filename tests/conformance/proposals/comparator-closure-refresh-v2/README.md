# #208 sha2 comparator refresh: reviewed in Git

Status: applied with the #208 sha2 0.11 upgrade; reviewed through normal Git
review (ADR-0019). There is no approval hash.

sha2 0.11 moved packages the canonical visual comparator builds against
(`sha2`, `digest`, `block-buffer`, `crypto-common`, `cpufeatures`,
`hybrid-array`, `const-oid`), so `canonical-visual-v1.closure.json` and with it
the profile digest moved (ADR-0005). Its digests no longer implement
`LowerHex`, so the `ssim-adversarial` vector source,
`crates/excalidraw-render-compare/tests/ssim_vectors.rs`, now spells its two
kernel digests through a local helper. The digests it asserts are unchanged,
as are the comparator's sources, comparison protocol, Pixelmatch fixtures and
every other vector.

`before/` preserves the exact committed preimages of the profile, its sidecar
and its closure. Evidence measured under that profile (the current
candidate's viewer evidence and exception proposal) keeps its recorded
comparator binding. `comparator_closure_successor` in
`xtask/src/conformance_candidate_gate.rs` accepts such a binding only when it
equals an archived preimage, the live profile has the reviewed digest
(`pdf_viewers::COMPARATOR_DIGEST`), and the two profiles differ only in
`implementation.closure` and the reviewed `ssim-adversarial` digest move
listed in `COMPARATOR_REFRESHES`. Evidence measured under the #206 preimage
(`comparator-closure-refresh-v1/`) follows the same chain.

Live evidence that binds the profile was rebound to the new digest instead; see
the "Visual comparator" section of `tests/conformance/README.md`.
