# WebP umbrella facets, ICC exceptions and V18 successor — pending approval

Three exact subjects for one #85 decision. Nothing here is approved or applied.

| Subject | SHA-256 |
| --- | --- |
| WebP ICC exceptions (`tests/oracle/webp-container/icc-exceptions.json` without `approval`) | `e8f408e8263f78f9e9ece56546a7253b50d53c988cae507c493501b85a38da66` |
| Owner tranche (`owner-tranche.json`) | `84ab9c95eb2e0d55bb376e472f45e9d4fc1d36fbc7d60db9ed7e19c16f2af351` |
| Representative V18 (`representative.stage4-frames-text-assets.v18`) | `6b88396cffe18ac6bb47c68fa5eedc0dee3f69514b311440374e3b78c49d91a7` |

Evidence and renderer changes are in `docs/research/webp-umbrella-facets.md`
(f339cbc, b9abd9a).

- **Owner tranche:** owners for the eight WebP facets, plus reassignment of the
  12 #191 control rows to their reference-backed owners. The control criteria
  are rewritten in `ownership-policy.json` (b9abd9a): `vp8x-reserved-flag`
  becomes rejected, while `anmf-reserved-flag` and `intrinsic-frame-mismatch`
  become accepted, as the reference does.
- **ICC exceptions:** 34 color-managed 9×7 views, each held at its exact
  observed values: at most 12 differing pixels, maximum channel difference 4,
  0 Pixelmatch changes (spec 14.4, small-artifact form).
- **Pinned divergence:** one row stays pinned to its current output (#197).
- **V18:** the 8.2 edit staled V17. V18 is collected with bridge v12 and
  reproduces V17 exactly.

## Dry run

With placeholder approvals, `apply.py` validated end to end and was reverted.
All WebP tests and all xtask tests passed. `verify-ownership` reported 2367 of
2418 assigned/excluded and 51 gaps. The full library (413) and fast (644)
suites passed at b9abd9a. The WebP record reproduces byte for byte, and the
Dagger native replay matches all 168 approved snapshots.

## Applying

After a genuine approval comment on #85:

    python3 tests/conformance/proposals/stage4-v18-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
