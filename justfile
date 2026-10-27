# The gates, run in the devenv shell (ADR-0024).

set shell := ["bash", "-euo", "pipefail", "-c"]

# The repo-authored crates
packages := "-p excalidraw-render -p excalidraw-render-cli -p excalidraw-render-compare -p excalidraw-render-fonts -p excalidraw-render-fonts-xiaolai -p excalidraw-render-fonts-xiaolai-extended -p xtask"

# The Rust target of this host, which the packaged consumers build for
host := "$(rustc -vV | sed -n 's/^host: //p')"

# List the recipes.
[private]
default:
    @just --list --unsorted

# Run every per-change gate except msrv, which needs its own toolchain.
ci: fmt clippy features test test-render test-features test-doc conformance packaging

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
conformance: xtask
    target/debug/xtask conformance verify-all
    target/debug/xtask conformance verify-inventory
    target/debug/xtask conformance verify-ownership
    target/debug/xtask catalog verify
    target/debug/xtask conformance verify-svg-font-subset-release
    target/debug/xtask conformance verify-svg-font-subsets
    target/debug/xtask conformance verify-svg-viewers
    target/debug/xtask conformance verify-pdf-viewers

# Check the published crates the way a downstream consumer uses them (packaging:*).
packaging: package-cli package-facade package-no-cjk

# Package the published crates and unpack them under target/package.
package:
    rm -rf target/package
    cargo package --locked --workspace --exclude xtask --exclude excalidraw-render-compare \
        --allow-dirty --no-verify --target-dir target
    for archive in target/package/*.crate; do tar -xzf "$archive" -C target/package; done

# Check the packaged CLI against the packaged renderer (packaging:cli).
package-cli: package
    bash tests/package-consumer/check-packaged-cli.sh "{{ host }}"

# Run the public facade against the packaged renderer (packaging:public-facade, wasm:public-facade).
package-facade target=host: package
    cd tests/package-consumer && cargo test --locked --release --target "{{ target }}" \
        --test public_facade --test optional_cjk

# Check that a packaged dependency can leave out the CJK payloads (packaging:no-cjk, wasm:no-cjk).
package-no-cjk target=host: package
    bash tests/package-consumer/check-no-cjk.sh "{{ target }}"

# The release checks. They run on the supported targets: browser WASM runs on any host, and
# native evidence needs x86_64-unknown-linux-gnu (ADR-0006, ADR-0024).

# Where the release checks write their logs, snapshots and reports
evidence := "target/release-evidence"

native := "x86_64-unknown-linux-gnu"
wasm := "wasm32-unknown-unknown"

# Run every release check on both supported targets.
release: (qualification native) (qualification wasm) packaging wasm-packaging full-equality

# Run the packaged facade and the no-CJK consumer in browser WASM (wasm:public-facade, wasm:no-cjk).
wasm-packaging: (package-facade wasm) (package-no-cjk wasm)

# Run a target's release qualification and materialize its report (packaging:qualification, wasm:qualification).
qualification target: xtask
    {{ if target == native { "just require-native" } else { "" } }}
    mkdir -p "{{ evidence }}/{{ target }}"
    bash tools/ci/qualification.sh "{{ target }}" "{{ evidence }}/{{ target }}/qualification.log"
    target/debug/xtask conformance materialize-svg-font-subset-target "{{ target }}" \
        "{{ evidence }}/{{ target }}/qualification.log" "{{ evidence }}/{{ target }}/{{ target }}.json"

# Collect native snapshots of the packaged facade and replay them against the approved baseline (packaging:snapshot-replay).
snapshot-replay: collect-native-snapshots
    target/debug/xtask conformance verify-native-baseline
    target/debug/xtask conformance replay-native-snapshots "{{ evidence }}/native-snapshots"

# Collect native snapshots of the packaged facade for the inventory's keys and check their form.
[private]
collect-native-snapshots: require-native package xtask
    target/debug/xtask conformance verify-inventory
    rm -rf "{{ evidence }}/native-snapshots"
    mkdir -p "{{ evidence }}"
    cd tests/package-consumer && cargo test --locked --release --target "{{ native }}" --example native_snapshots
    cd tests/package-consumer && cargo run --locked --release --target "{{ native }}" --example native_snapshots \
        -- collect ../.. "../../{{ evidence }}/native-snapshots"
    target/debug/xtask conformance verify-native-snapshots "{{ evidence }}/native-snapshots"

# Render every snapshot key in browser WASM and compare with the approved native baseline (wasm:snapshot-replay).
wasm-snapshot-replay: package xtask
    target/debug/xtask conformance stage-wasm-snapshots
    rm -rf "{{ evidence }}/wasm-snapshots"
    mkdir -p "{{ evidence }}"
    cd tests/package-consumer && bash ../../tools/ci/wasm-snapshots.sh "../../{{ evidence }}/wasm-snapshots"
    target/debug/xtask conformance report-wasm-snapshots "{{ evidence }}/wasm-snapshots"
    python3 tools/ci/release-results.py snapshots "{{ evidence }}/wasm-snapshots"

# Collect a target's test-only resource traces from an instrumented copy of the source.
trace-logs target:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ target }}" in
        {{ native }}) just require-native; dir=native ;;
        {{ wasm }}) dir=wasm ;;
        *) echo "traces cover only the supported targets, not {{ target }}" >&2; exit 2 ;;
    esac
    # Staging edits tracked sources, so it runs in a worktree of the current tree.
    # Its build stays apart from the workspace's, which the instrumentation would evict.
    root="$PWD"
    stage="$root/target/trace-source"
    export CARGO_TARGET_DIR="$root/target/trace-build"
    git worktree remove --force "$stage" 2>/dev/null || rm -rf "$stage"
    commit="$(git stash create)"
    git worktree add --quiet --detach "$stage" "${commit:-HEAD}"
    trap 'git -C "$root" worktree remove --force "$stage"' EXIT
    out="$root/{{ evidence }}/results/$dir"
    rm -rf "$out"
    cd "$stage"
    cargo run --locked -q -p xtask -- conformance stage-traces
    bash tools/ci/trace-snapshots.sh "{{ target }}" "$out"

# Join both targets' traces to the packaged runs and check their equality (full-equality).
full-equality: (trace-logs native) (trace-logs wasm) snapshot-replay wasm-snapshot-replay xtask
    target/debug/xtask conformance aggregate-traces "{{ evidence }}/results"
    rm -rf "{{ evidence }}/results/native-snapshots" "{{ evidence }}/results/wasm-snapshots"
    cp -R "{{ evidence }}/native-snapshots" "{{ evidence }}/results/native-snapshots"
    cp -R "{{ evidence }}/wasm-snapshots" "{{ evidence }}/results/wasm-snapshots"
    target/debug/xtask conformance join-trace-results "{{ evidence }}/results"
    python3 tools/ci/release-results.py joined "{{ evidence }}/results"

# Export both targets' qualification reports and their aggregate from a clean commit (target-evidence).
target-evidence: require-native
    bash tools/ci/require-clean-source.sh
    just release
    rm -rf "{{ evidence }}/reports"
    mkdir -p "{{ evidence }}/reports"
    cp "{{ evidence }}/{{ native }}/{{ native }}.json" "{{ evidence }}/{{ wasm }}/{{ wasm }}.json" "{{ evidence }}/reports/"
    target/debug/xtask conformance aggregate-svg-font-subset-targets "{{ evidence }}/reports" "{{ evidence }}/aggregate.json"

# Build the reproducible CLI release archive and its SHA-256 file into dist/.
release-archive version: require-native
    #!/usr/bin/env bash
    set -euo pipefail
    crate="$(awk -F'"' '/^version = /{print $2; exit}' Cargo.toml)"
    if [[ "$crate" != "{{ version }}" ]]; then
        echo "archive version {{ version }} does not match the workspace version $crate" >&2
        exit 1
    fi
    cargo build --locked --release -p excalidraw-render-cli --target-dir target
    name="excalidraw-render-{{ version }}-{{ native }}"
    rm -rf "dist/$name" "dist/$name.tar.gz" "dist/$name.tar.gz.sha256"
    mkdir -p "dist/$name"
    cp target/release/excalidraw-render README.md LICENSE-MIT LICENSE-APACHE \
        crates/excalidraw-render-cli/LICENSE-BUNDLE.txt "dist/$name/"
    tar -C dist --sort=name --owner=0 --group=0 --numeric-owner --mode=u+rwX,go+rX,go-w \
        --mtime=@0 -cf - "$name" | gzip -9n > "dist/$name.tar.gz"
    rm -rf "dist/$name"
    cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256"

# Restaging. These recipes regenerate approved evidence after a change that moves
# it. They write candidates for review; nothing here edits a manifest or the
# registry, and CI exports their output as artifacts (docs/ci.md).

# Stage a native baseline candidate from a clean commit into target/release-evidence/native-baseline.
stage-native-baseline: require-native
    bash tools/ci/require-clean-source.sh
    just collect-native-snapshots
    rm -rf "{{ evidence }}/native-baseline"
    target/debug/xtask conformance stage-native-baseline "{{ evidence }}/native-snapshots" "{{ evidence }}/native-baseline"

# Reproduce or generate the font-subset, SVG or PDF viewer qualification in its pinned containers.
viewer-evidence kind mode="reproduce":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ kind }}" in
        font-subset) script=tools/svg-font-subset-qualification.sh ;;
        svg) script=tools/svg-viewer-qualification.sh ;;
        pdf) script=tools/pdf-viewer-qualification.sh ;;
        *) echo "viewer evidence covers font-subset, svg and pdf, not {{ kind }}" >&2; exit 2 ;;
    esac
    case "{{ mode }}" in
        reproduce|generate) ;;
        *) echo "viewer evidence is reproduced or generated, not {{ mode }}" >&2; exit 2 ;;
    esac
    # The scripts render with the network off, so everything they need is acquired first.
    if [[ "{{ kind }}" == pdf ]]; then
        bash tools/pdf-harness/install.sh
    else
        bash tools/reference-oracle/run-container.sh install
        # The pure-Rust viewer builds offline from this host's registry, so both lockfiles are fetched.
        cargo fetch --locked
        cargo fetch --locked --manifest-path tools/svg-viewer-rust/Cargo.toml
    fi
    bash "$script" "{{ mode }}"

# Collect a representative candidate successor from the frozen V4 authoring with the current renderer.
candidate-successor dir: require-native xtask
    #!/usr/bin/env bash
    set -euo pipefail
    bash tools/reference-oracle/run-container.sh install
    bash tools/pdf-harness/install.sh
    cargo fetch --locked --manifest-path tools/svg-viewer-rust/Cargo.toml
    # Each viewer run has a 180-second deadline, so nothing may compile under it. The
    # views script sets these two variables, and a different pair would rebuild.
    CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo build --locked --manifest-path tools/svg-viewer-rust/Cargo.toml
    target/debug/xtask conformance candidate-revise "{{ dir }}"
    mkdir "{{ dir }}/assets" "{{ dir }}/viewer-evidence"
    target/debug/xtask conformance candidate-native-capture "{{ dir }}"
    node tools/reference-oracle/scripts/stage4-viewer-assets.mjs "{{ dir }}"
    target/debug/xtask conformance candidate-prepare-assets "{{ dir }}"
    node tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs "{{ dir }}" collect
    node tools/reference-oracle/scripts/stage4-candidate-views.mjs collect "{{ dir }}"
    target/debug/xtask conformance candidate-propose-exceptions "{{ dir }}"
    target/debug/xtask conformance candidate-inspect "{{ dir }}"
    target/debug/xtask conformance candidate-materialize "{{ dir }}"

# The randomized expansions the nightly schedule runs. They are bounded but long,
# so no per-change gate carries them.

# Expand the bounded font subset fuzz.
nightly-font-subset-fuzz cases="4096":
    EXCALIDRAW_FONT_SUBSET_FUZZ_CASES={{ cases }} cargo test --locked --release -p excalidraw-render --lib \
        render::font_subset_fuzz::nightly_font_subset_fuzz_is_bounded_and_deterministic -- --ignored --exact

# Expand the batch-versus-standalone frame property.
nightly-frame-batch-property cases="512":
    EXCALIDRAW_FRAME_BATCH_CASES={{ cases }} cargo test --locked --release -p excalidraw-render --lib \
        render::frame_batch_property::nightly_frame_batch_property_runs_a_deeper_sample -- --ignored --exact

# Expand the bounded scene admission and normalization fuzz.
nightly-scene-fuzz cases="4096":
    EXCALIDRAW_SCENE_FUZZ_CASES={{ cases }} cargo test --locked --release -p excalidraw-render --lib \
        scene_fuzz::nightly_scene_fuzz_is_bounded_and_deterministic -- --ignored --exact

# Exercise every admitted raster family and nested SVG under semantic limits.
nightly-asset-fuzz cases="4096":
    EXCALIDRAW_ASSET_FUZZ_CASES={{ cases }} cargo test --locked --release -p excalidraw-render --test asset_fuzz \
        asset_fuzz::nightly_asset_fuzz_is_bounded_and_deterministic -- --ignored --exact

# Exercise complete requests, metadata and atomic outcomes with debug assertions.
nightly-render-fuzz cases="4096":
    EXCALIDRAW_RENDER_FUZZ_CASES={{ cases }} EXCALIDRAW_RENDER_FUZZ_TRACE=1 cargo test --locked \
        -p excalidraw-render --test render_fuzz render_fuzz::nightly_complete_render_fuzz \
        -- --ignored --exact --nocapture

# Run the complete-render fuzz smoke in browser WASM.
render-fuzz-wasm:
    CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --target {{ wasm }} -p excalidraw-render --test render_fuzz \
        wasm::complete_render_fuzz_smoke_in_wasm

# Build xtask, which the verifiers and release checks run directly.
[private]
xtask:
    cargo build --locked -q -p xtask

# Refuse native release evidence on a host that is not the supported native target.
[private]
require-native:
    [[ "{{ host }}" == "{{ native }}" ]] || { echo "native release evidence needs {{ native }}, not {{ host }}" >&2; exit 1; }
