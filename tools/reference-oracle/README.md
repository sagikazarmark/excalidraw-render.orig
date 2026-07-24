# Reference oracle

This development-only oracle materializes compatibility revision 1 against the
official Excalidraw `v0.18.1` browser exports. It is not linked into the Rust
renderer and is not required by normal builds or tests.

The canonical environment is the `linux/amd64` Playwright image pinned by
digest in `run-container.sh`. Dependency acquisition may use the network;
profile generation and rendering run in a container with `--network=none`.
The browser sees only `http://reference.invalid/`, the checked wrapper bundle,
and font files admitted by the checked asset manifest.

## Commands

Acquire the exact npm dependency closure:

```sh
tools/reference-oracle/run-container.sh install
```

Rebuild and verify the checked profile and its two-process rendering smoke
test:

```sh
tools/reference-oracle/run-container.sh verify
```

After an explicitly reviewed profile-input change, regenerate canonical
manifests and digests, then run the same smoke test:

```sh
tools/reference-oracle/run-container.sh update
```

The public profile identity is the SHA-256 in
`tests/oracle/reference-profiles/revision-1.sha256`. It hashes the exact RFC
8785 canonical bytes of `revision-1.json`. The profile binds the npm lock,
wrapper source records, generated browser bundle, official rendering assets,
container host fonts, browser settings, and evidence invocation rules.

The smoke fixture is profile validation, not approved conformance evidence.
Conformance cases and their review protocol belong to the conformance corpus.

Frame export uses a sibling target-scoped wrapper so the approved scene wrapper
and profile digest remain immutable. Rebuild and verify its browser bundle and
checked profile without touching the scene bundle with:

```sh
bash tools/reference-oracle/run-frame-profile.sh verify
```

After an explicitly reviewed frame-wrapper change, regenerate the frame profile
with `bash tools/reference-oracle/run-frame-profile.sh update`.

The checked frame profile and digest are
`tests/oracle/reference-profiles/revision-1-frame.json` and
`revision-1-frame.sha256`. The profile binds the frame HTML, browser entry,
build configuration, conformance generator and runner, every generated bundle
file, zero-padding invocation, and the immutable scene profile it extends.

The repository `xtask` owns explicit conformance generation, read-only
reproduction, and ordinary offline verification:

```sh
cargo run --locked -p xtask -- conformance generate focused.rectangle-outline.v1
cargo run --locked -p xtask -- conformance reproduce focused.rectangle-outline.v1
cargo run --locked -p xtask -- conformance verify focused.rectangle-outline.v1
cargo run --locked -p xtask -- conformance reproduce focused.frame-export.v1
cargo run --locked -p xtask -- conformance approval-subject focused.frame-export.v1
```

Generation and reproduction use the same pinned container and frozen wrapper as
the reference profile. Generation refuses to replace approved evidence.
Reproduction writes no corpus files. Verification requires no Node, browser,
container, network, or system fonts.

## Canonical SVG viewer

The candidate-comparison Chromium viewer is separate from the reference
evidence rasterizer above. It loads each SVG as an isolated top-level document,
installs virtual time before navigation, blocks every undeclared request,
audits text fonts through DevTools, completes two lifecycle updates, and emits
canonical straight-alpha RGBA8.

Run its pinned-container qualification with:

```sh
bash tools/reference-oracle/run-svg-viewer.sh test
```

Reproduce both the Chromium and pure-Rust qualification buffers without
changing checked evidence with:

```sh
bash tools/svg-viewer-qualification.sh reproduce
```
