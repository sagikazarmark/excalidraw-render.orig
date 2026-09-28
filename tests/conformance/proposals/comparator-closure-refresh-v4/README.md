# #208 png comparator refresh: reviewed in Git

Status: applied with the #208 png 0.18 upgrade; reviewed through normal Git
review (ADR-0019). There is no approval hash.

png 0.18 moved packages the canonical visual comparator builds against
(`png`, and `bitflags` 1.3.2 that only png 0.17 held), so
`canonical-visual-v1.closure.json` and with it the profile digest moved
(ADR-0005). png 0.18 returns `Decoder::output_buffer_size` as an `Option`, so
the `pixelmatch-upstream-harness` vector source,
`crates/excalidraw-render-compare/tests/pixelmatch_upstream.rs`, unwraps it
when it decodes a fixture. The fixtures it decodes and the comparisons it
asserts are unchanged, as are the comparator's sources, comparison protocol
and every other vector.

`before/` preserves the exact committed preimages of the profile, its sidecar
and its closure. Evidence measured under that profile, or under an earlier
archived one, keeps its recorded comparator binding through
`comparator_closure_successor` and the `COMPARATOR_REFRESHES` chain in
`xtask/src/conformance_candidate_gate.rs`, which lists this refresh's
`pixelmatch-upstream-harness` digest move; see
`comparator-closure-refresh-v2/README.md`.

Live evidence that binds the profile was rebound to the new digest instead; see
the "Visual comparator" section of `tests/conformance/README.md`.
