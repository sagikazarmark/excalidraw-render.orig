//! The approved rectangle-outline SVG through both canonical SVG viewers: the
//! pinned Chromium container (`tools/reference-oracle/run-svg-viewer.sh`) and
//! the pinned pure-Rust viewer (`tools/svg-viewer-rust`), each compared byte
//! for byte with its approved reference raster (issue #34). The test is
//! `#[ignore]`d for plain `cargo test`; the `harness` nextest profile selects
//! this binary by name and runs it with `--run-ignored only`.

use std::fs;
use std::process::Command;

mod common;

use common::approved_scenes::render_rectangle_outline_svg;
use common::repository_root;

#[test]
#[ignore = "requires the pinned Chromium container and Rust 1.97.1 viewer toolchain"]
fn approved_rectangle_passes_both_canonical_svg_visual_gates() {
    let root = repository_root();
    let stem = format!("rectangle-candidate-{}", std::process::id());
    let candidate = format!("target/{stem}.svg");
    let chromium = format!("target/{stem}-chromium.rgba");
    let pure_rust = format!("target/{stem}-pure-rust.rgba");
    fs::write(
        root.join(&candidate),
        render_rectangle_outline_svg().artifact().bytes(),
    )
    .expect("write renderer-owned candidate SVG under target");

    let chromium_status = Command::new("bash")
        .current_dir(&root)
        .args([
            "tools/reference-oracle/run-svg-viewer.sh",
            "render",
            &candidate,
            &chromium,
            "tests/conformance/svg-viewer-qualification/rectangle-outline/assets.none.json",
        ])
        .status()
        .expect("launch canonical Chromium SVG viewer");
    assert!(chromium_status.success());

    let pure_rust_status = Command::new("cargo")
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target/svg-viewer-rust"))
        .args([
            "run",
            "--quiet",
            "--locked",
            "--release",
            "--manifest-path",
            "tools/svg-viewer-rust/Cargo.toml",
            "--",
            "render",
            &candidate,
            &pure_rust,
            "tests/conformance/svg-viewer-qualification/rectangle-outline/assets.none.json",
        ])
        .status()
        .expect("launch canonical pure-Rust SVG viewer");
    assert!(pure_rust_status.success());

    assert_eq!(
        fs::read(root.join(chromium)).unwrap(),
        fs::read(root.join(concat!(
            "tests/conformance/svg-viewer-qualification/rectangle-outline/",
            "evidence/chromium-reference.rgba"
        )))
        .unwrap()
    );
    assert_eq!(
        fs::read(root.join(pure_rust)).unwrap(),
        fs::read(root.join(concat!(
            "tests/conformance/svg-viewer-qualification/rectangle-outline/",
            "evidence/pure-rust-reference.rgba"
        )))
        .unwrap()
    );
}
