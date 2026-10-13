# The Cargo gates of the Dagger module (`dagger.dang`), run in the devenv shell.
# Packaging, wasm, and full equality stay in Dagger.

set shell := ["bash", "-euo", "pipefail", "-c"]

# The repo-authored crates
packages := "-p excalidraw-render -p excalidraw-render-cli -p excalidraw-render-compare -p excalidraw-render-fonts -p excalidraw-render-fonts-xiaolai -p excalidraw-render-fonts-xiaolai-extended -p xtask"

# List the recipes.
[private]
default:
    @just --list --unsorted

# Run all CI jobs.
ci: fmt clippy features msrv test test-render test-features test-doc conformance

# Check the formatting of every repo-authored crate (lint:fmt).
fmt:
    cargo fmt --check {{ packages }}

# Lint the renderer, the CLI, and xtask (lint:clippy).
clippy:
    cargo clippy --locked --no-deps --all-targets -p excalidraw-render -p excalidraw-render-cli -p xtask -- -D warnings

# Lint the renderer without its default features and with each one alone (lint:features).
features:
    for features in '' png pdf cjk; do \
        cargo clippy --locked --no-deps --all-targets -p excalidraw-render --no-default-features --features "$features" -- -D warnings; \
    done

# Run it on that toolchain: devenv --option languages.rust.version:string 1.92.0 shell -- just msrv
# Check the published crates on the workspace rust-version (lint:msrv).
msrv:
    cargo check --locked -p excalidraw-render -p excalidraw-render-cli

# Run the fast tier: every test outside the parity, limits, and harness binaries (test:fast).
test:
    python3 -m unittest discover -s tools/ci -p 'test_*.py'
    cargo nextest run --locked --workspace --profile ci-fast

# Run the render tier: the parity harnesses and the fixed parser ceilings (test:render).
test-render:
    cargo nextest run --locked --workspace --profile ci-render

# Test the builds that leave out the png or pdf feature (test:features).
test-features:
    bash tools/ci/check-format-features.sh

# Run every documentation test in the workspace (test:doc).
test-doc:
    cargo test --locked --workspace --doc

# Verify the cases, the coverage ledgers, the sealed catalog, and the approved evidence (conformance:*).
conformance:
    cargo build --locked -p xtask
    target/debug/xtask conformance verify-all
    target/debug/xtask conformance verify-inventory
    target/debug/xtask conformance verify-ownership
    target/debug/xtask catalog verify
    target/debug/xtask conformance verify-svg-font-subset-release
    target/debug/xtask conformance verify-svg-font-subsets
    target/debug/xtask conformance verify-svg-viewers
    target/debug/xtask conformance verify-pdf-viewers
