# Representative intake CI

Run the independent Rust representative-intake check from the repository root:

```sh
dagger api call --load-module tools/representative-intake/ci verify
```

The module builds the actual renderer and `xtask` with the locked workspace
dependencies. Builds and tests share `/work/src/target`, use two Cargo build jobs,
and disable development/test debug information to reduce build cost.

`REPRESENTATIVE_RENDERER` points to the freshly built renderer. The
`representative_intake` and `representative_case` suites run with
`--include-ignored` to exercise their real-renderer contracts. The
`representative_numeric`, `representative_judge`, `representative_viewer_assets`,
`representative_retention`, and `representative_evidence` suites run their regular
tests; optional pinned-font and Docker viewer-harness tests remain ignored.

Smoke checks use the built Rust `xtask representative` commands to verify the
retained Velox intake, capture its six executions twice, verify that capture, and
verify the accepted case registry with `case verify-registry --renderer`.
Comparison runs in-process in Rust; no Python or standalone comparison executable
is needed.

The dedicated representative-intake GitHub Actions workflow has been removed.
To validate the module schema and list its functions without running the check:

```sh
dagger api functions --load-module tools/representative-intake/ci
```
