# Canvas image resampling (#198) and V26 successor — proposal

Four exact subjects for one #85 decision. Nothing is applied until a genuine
approval comment exists on #85.

| Subject | SHA-256 |
| --- | --- |
| Owner tranche (`owner-tranche.json`) | `b3b4bd24e93dc4d001fd6494a03e9da58ce69cae028a54bd89fedace6361127c` |
| Native baseline (`tests/conformance/coverage/native-baseline`) | `5c1e26e4123ef7e8597a6f419ca87877862aa752a428be57cbbff3b2163deb25` |
| Layer-integration record (`tests/conformance/coverage/layers.json`, regenerated) | `aaf11ea35a5c0b450494cade7bb7b0cbb3d3383fb4b84a00c8d6c3e52c020552` |
| Representative V26 (`representative.stage4-frames-text-assets.v26`) | `8be7a55b44d48f33d9fd7d7614bb2e54d9ffb8e2d182bb4e51861d5f8d5fb256` |

PNG now resamples raster images exactly as the pinned software canvas does.
The work is described in `docs/research/canvas-image-resampling.md` and spec
section 8.2.

- **Renderer:**
  - The image cache's pre-scaled level is chosen and built as Skia builds it.
  - Skia's legacy fixed-point sampler draws it.
  - A per-render cache of levels reproduces draw-order reuse.
  - It is verified with 0 differing pixels on 63 research probe scenes and on
    the new 73-row record `tests/oracle/image-resample`.
- **Native baseline:** two PNG keys change, with reports unchanged, and both
  are closer to their official evidence.
  - `focused.image-codecs.v1`: the differing pixels against official evidence
    fall from 6791 to 3655.
  - `focused.rounded-image.v1` scale-2: from 4850 to 4724.
- **Layers record:** the new oracle directory is declared, as the layers
  verifier requires. It maps to a new row, `layer:vector:image-resampling`,
  owned by `image_resample_contract::raster_images_resample_exactly_as_the_reference_canvas`.
  The regenerated record needs this fresh approval. Until then,
  `closure:all-five-layers` is a gap, and `apply.py` re-records its record
  owner.
- **V26:** the renderer and spec changes staled V25. V26 is collected with
  bridge v20 and reproduces V25 exactly.

## Dry run

With a placeholder approval, `apply.py` validated end to end and was
reverted. The results:

- `verify-layers --require-approval` passed with 19 oracle directories.
- `verify-native-baseline` passed, with all 168 reports equal to their
  expected reports.
- `verify-ownership` reported 2424 of 2440 assigned or excluded, and 16 gaps.
- V26 `candidate-verify` and all xtask tests passed.
- The Dagger native replay of a temporary dry-run commit matched all 168
  expectations.
- Before the dry run, the full library (472) and fast (682) suites and clippy
  passed, and the resampling record reproduces byte for byte.

## Applying

After a genuine approval comment on #85:

    PATH=$HOME/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:$PATH \
      python3 tests/conformance/proposals/stage4-v26-successor-v1/apply.py <github-user> <date> <comment-url>
    cargo run --locked --release -p xtask -- conformance verify-ownership
