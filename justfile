# The gates, run in the devenv shell (ADR-0024). The base shell runs every
# per-change gate; the WASM recipes need the `browser` profile
# (`devenv --profile browser shell`), which adds the wasm-bindgen runner and
# headless Firefox, and the release checks and `candidate-successor` the
# `release` profile, which extends it with Node.

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
ci: fmt clippy features deny test test-render test-features test-doc conformance packaging

# Check the formatting of every repo-authored crate.
fmt:
    cargo fmt --check {{ packages }}

# Lint the renderer, the CLI, and xtask.
clippy:
    cargo clippy --locked --no-deps --all-targets -p excalidraw-render -p excalidraw-render-cli -p xtask -- -D warnings

# Lint the renderer without its default features and with each one alone.
features:
    for features in '' png pdf cjk; do \
        cargo clippy --locked --no-deps --all-targets -p excalidraw-render --no-default-features --features "$features" -- -D warnings; \
    done

# Run it on that toolchain: devenv --option languages.rust.version:string 1.92.0 shell -- just msrv
# Check advisories, licenses and sources of every dependency (deny.toml).
deny:
    cargo deny --locked check

# Check the published crates on the workspace rust-version.
msrv:
    cargo check --locked -p excalidraw-render -p excalidraw-render-cli

# Run the fast tier: every test outside the parity, limits, and harness binaries.
test:
    cargo nextest run --locked --workspace --profile ci-fast

# Run the render tier: the parity harnesses and the fixed parser ceilings.
test-render:
    cargo nextest run --locked --workspace --profile ci-render

# CI runs the fast and render tiers as one archived build and several shards.

# Build every test target once into a nextest archive.
test-archive archive="target/nextest-archive.tar.zst":
    cargo nextest archive --locked --workspace --archive-file {{ archive }}

# Run one shard, such as 1/4, of the fast and render tiers from an archive. It
# extracts into the checkout, where every binary is at the path its build
# compiled in.
test-shard shard archive="target/nextest-archive.tar.zst":
    cargo nextest run --archive-file {{ archive }} --extract-to . --extract-overwrite \
        --profile default --no-fail-fast --partition slice:{{ shard }}

# Run the release-tier tests: the xtask tests that build the qualified release
# CLI and replay the representative candidate through it, the ones that judge
# the live coverage inventory and ownership ledger, the ones that run host tools
# (qpdf, Node; the `release` devenv profile has both), and the case generators'
# replacement guard. Building the CLI first keeps the one-thread group from
# serializing on Cargo's build lock.
test-release:
    cargo build --release --locked -p excalidraw-render-cli
    cargo nextest run --locked --workspace --profile default --no-fail-fast --run-ignored only \
        -E 'group(release-tier) | group(host-tools)'
    node --test tools/reference-oracle/scripts/generation-guard.test.mjs

# Test the builds that leave out the png or pdf feature.
test-features:
    bash tools/ci/check-format-features.sh

# Run every documentation test in the workspace.
test-doc:
    cargo test --locked --workspace --doc

# Run the PDF gates in PDFium, PDF.js and qpdf. PDF.js runs in its pinned image, so this needs Docker.
pdf-harness:
    bash tools/pdf-harness/install.sh
    cargo nextest run --locked --release -p excalidraw-render --profile harness --run-ignored only \
        -E 'binary(/^harness_pdf_/)'

# Run the SVG gate in Chromium and the pure-Rust viewer. Chromium runs in its pinned image, so this needs Docker.
svg-harness:
    bash tools/reference-oracle/run-container.sh install
    cargo nextest run --locked --release -p excalidraw-render --profile harness --run-ignored only \
        -E 'binary(/^harness_svg_/)'

# Verify the registered cases, the sealed catalog and the viewer qualification records.
conformance: xtask
    target/debug/xtask conformance verify-all
    target/debug/xtask catalog verify
    target/debug/xtask conformance verify-svg-viewers
    target/debug/xtask conformance verify-pdf-viewers

# Verify the release-tier records: the representative candidates against a fresh
# release build, the coverage inventory and ownership ledgers, and the font-subset
# viewer qualification. They bind evidence that only the release checks consume
# and move on every intentional output change, so no per-change gate carries them.
conformance-release: xtask
    target/debug/xtask conformance verify-candidates
    target/debug/xtask conformance verify-inventory
    target/debug/xtask conformance verify-ownership
    target/debug/xtask conformance verify-svg-font-subsets

# Check the published crates the way a downstream consumer uses them.
packaging: package-cli package-facade package-no-cjk

# Package the published crates and unpack them under target/package.
package:
    rm -rf target/package
    cargo package --locked --workspace --exclude xtask --exclude excalidraw-render-compare \
        --allow-dirty --no-verify --target-dir target
    for archive in target/package/*.crate; do tar -xzf "$archive" -C target/package; done

# Check the packaged CLI against the packaged renderer.
package-cli: package
    bash tests/package-consumer/check-packaged-cli.sh "{{ host }}"

# Run the public facade against the packaged renderer.
package-facade target=host: package
    cd tests/package-consumer && cargo test --locked --release --target "{{ target }}" \
        --test public_facade --test optional_cjk

# Check that a packaged dependency can leave out the CJK payloads.
package-no-cjk target=host: package
    bash tests/package-consumer/check-no-cjk.sh "{{ target }}"

# The release checks. They run on the supported targets (ADR-0006): the native
# checks on any host, since the bytes do not depend on it (ADR-0024), and the
# browser-WASM checks in headless Firefox, all from the `release` devenv profile
# (`devenv --profile release shell -- just release`).

# Where the release checks write their logs
evidence := "target/release-evidence"

native := "x86_64-unknown-linux-gnu"
wasm := "wasm32-unknown-unknown"

# Run every release check on both supported targets.
release: conformance-release test-release (qualification native) (qualification wasm) packaging wasm-packaging golden-wasm

# Run the packaged facade and the no-CJK consumer in browser WASM.
wasm-packaging: (package-facade wasm) (package-no-cjk wasm)

# Run a target's release qualification: the renderer's unit target in release
# mode with the corpus and frame-batch tests admitted, logged under the evidence
# directory.
qualification target:
    mkdir -p "{{ evidence }}/{{ target }}"
    bash tools/ci/qualification.sh "{{ target }}" "{{ evidence }}/{{ target }}/qualification.log"

# Render every golden target in browser WASM against the native golden record,
# accepting only the #178 differences tests/conformance/golden/experimental-wasm.json records.
golden-wasm:
    CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --target {{ wasm }} -p excalidraw-render --test golden_native

# Adopt an intentional output change: rerender every golden target and rewrite
# tests/conformance/golden for review (the render tier's golden_native test).
# When the registry moved, the first run regenerates the embedded inputs module
# and asks for a rebuild, so the recipe runs the test again.
update-golden:
    #!/usr/bin/env bash
    set -euo pipefail
    export UPDATE_GOLDEN=1
    cargo test --locked -p excalidraw-render --test golden_native \
        || cargo test --locked -p excalidraw-render --test golden_native

# Restaging. These recipes regenerate committed evidence after a change that moves
# it. They write candidates for review; nothing here edits a manifest or the
# registry, and CI exports their output as artifacts (docs/development/ci.md).

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

# After a subset-only profile change, rebind the font-subset qualification's subset fields to fresh viewer inputs.
font-subset-rebind: xtask
    cargo test --locked --release -p excalidraw-render --lib \
        render::font_subset_qualification::materialize_complete_font_subset_viewer_inputs \
        -- --ignored --exact
    target/debug/xtask conformance rebind-svg-font-subsets target/svg-font-subset-viewer-qualification/inputs.json

# Verify or update the scene or frame reference profile in the pinned oracle container.
reference-profile mode="verify" profile="scene":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ mode }}" in
        verify|update) ;;
        *) echo "a reference profile is verified or updated, not {{ mode }}" >&2; exit 2 ;;
    esac
    case "{{ profile }}" in
        scene) script=tools/reference-oracle/run-container.sh ;;
        frame) script=tools/reference-oracle/run-frame-profile.sh ;;
        *) echo "the reference profiles are scene and frame, not {{ profile }}" >&2; exit 2 ;;
    esac
    bash tools/reference-oracle/run-container.sh install
    bash "$script" "{{ mode }}"

# Generating over a manifested case needs --supersede, which keeps its approval block.
# Reproduce a registered case with its reference runner into target/reference-evidence, or generate it.
reference-case id mode="reproduce" *flags: xtask
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ mode }}" in
        reproduce|generate) ;;
        *) echo "a registered case is reproduced or generated, not {{ mode }}" >&2; exit 2 ;;
    esac
    bash tools/reference-oracle/run-container.sh install
    target/debug/xtask conformance "{{ mode }}" "{{ id }}" {{ flags }}

# Every case is manifested, so regenerating the registry is `just reference-cases generate --supersede`.
# Reproduce every registered case in one container session (the weekly oracle drift check), or generate them all.
reference-cases mode="reproduce" *flags:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ mode }}" in
        reproduce|generate) ;;
        *) echo "the registered cases are reproduced or generated, not {{ mode }}" >&2; exit 2 ;;
    esac
    bash tools/reference-oracle/run-container.sh install
    bash tools/reference-oracle/run-all-cases.sh "{{ mode }}" {{ flags }}

# Regenerate the per-cell font subset corpus into target/font-corpus for review.
font-corpus:
    rm -rf target/font-corpus target/tmp/svg-font-subset-qualification-corpus-*
    cargo test --locked --release -p excalidraw-render --lib \
        render::font_subset_qualification::materialize_complete_font_subset_qualification_corpus \
        -- --ignored --exact
    mkdir -p target/font-corpus
    cp target/tmp/svg-font-subset-qualification-corpus-*/* target/font-corpus/

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
# so no per-change gate carries them. Each takes a case count; with
# EXCALIDRAW_FUZZ_SEED set (the workflow passes the run id) a family mixes it
# into its fixed seed and prints the result, so a night searches new cases and a
# failure names the seed that replays it. Unset, they replay the fixed seed.

# Run every nightly expansion in one build.
nightly: nightly-font-subset-fuzz nightly-frame-batch-property nightly-scene-fuzz nightly-asset-fuzz nightly-render-fuzz

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

# Expand the complete-render fuzz under the revision-1 default resource limits.
nightly-render-fuzz-default-limits cases="4096":
    EXCALIDRAW_RENDER_FUZZ_DEFAULT_LIMITS=1 just nightly-render-fuzz {{ cases }}

# Run the complete-render fuzz smoke in browser WASM.
render-fuzz-wasm:
    CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --target {{ wasm }} -p excalidraw-render --test render_fuzz \
        wasm::complete_render_fuzz_smoke_in_wasm

# Build xtask, which the verifiers and release checks run directly.
[private]
xtask:
    cargo build --locked -q -p xtask

# Refuse to produce the representative candidate on a host that is not the
# supported native target: the candidate's pinned images are linux/amd64.
[private]
require-native:
    [[ "{{ host }}" == "{{ native }}" ]] || { echo "this needs {{ native }}, not {{ host }}" >&2; exit 1; }
