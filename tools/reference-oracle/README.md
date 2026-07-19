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

The repository `xtask` owns explicit conformance generation, read-only
reproduction, and ordinary offline verification:

```sh
cargo run --locked -p xtask -- conformance generate focused.rectangle-outline.v1
cargo run --locked -p xtask -- conformance reproduce focused.rectangle-outline.v1
cargo run --locked -p xtask -- conformance verify focused.rectangle-outline.v1
```

Generation and reproduction use the same pinned container and frozen wrapper as
the reference profile. Generation refuses to replace approved evidence.
Reproduction writes no corpus files. Verification requires no Node, browser,
container, network, or system fonts.
