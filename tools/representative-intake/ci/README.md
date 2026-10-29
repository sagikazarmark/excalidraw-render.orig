# Representative intake: real-renderer checks

The `representative_intake` and `representative_case` suites carry tests that
need a freshly built CLI and are `#[ignore]`d in the ordinary tiers. Run them
from the devenv shell:

```sh
cargo build --locked -p excalidraw-render-cli -p xtask
REPRESENTATIVE_RENDERER=target/debug/excalidraw-render \
  cargo test --locked -p xtask --test representative_intake --test representative_case -- --include-ignored
target/debug/xtask representative verify tests/conformance/intake/velox-design-v1
target/debug/xtask representative capture tests/conformance/intake/velox-design-v1 target/debug/excalidraw-render target/velox-intake-capture
target/debug/xtask representative verify-capture tests/conformance/intake/velox-design-v1 target/velox-intake-capture
target/debug/xtask representative case verify-registry --renderer target/debug/excalidraw-render
```

The Dagger module in this directory runs the same commands in a sealed
`linux/amd64` Rust container; it is an optional standalone check, not one of
the gates (ADR-0024):

```sh
dagger api call --load-module tools/representative-intake/ci verify
```

Comparison runs in-process in Rust; no Python or standalone comparison
executable is needed. Optional pinned-font and Docker viewer-harness tests stay
ignored in both forms.
