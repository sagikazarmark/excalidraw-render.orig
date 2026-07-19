use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[test]
fn renders_tightly_packed_straight_alpha_rgba8() {
    let root = temporary_directory("canonical-rgba");
    let input = root.join("input.svg");
    let output = root.join("output.rgba");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        &input,
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1" viewBox="0 0 2 1"><path d="M0 0h1v1H0z" fill="#ff0000"/></svg>"##,
    )
    .unwrap();

    let result = viewer(&input, &output);

    assert!(
        result.status.success(),
        "viewer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(&output).unwrap(), [255, 0, 0, 255, 0, 0, 0, 0]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_ambient_and_undeclared_image_resources() {
    let root = temporary_directory("image-resources");
    fs::create_dir_all(&root).unwrap();

    for (index, (svg, expected)) in [
        (
            r#"<image href="file:///etc/passwd" width="1" height="1"/>"#,
            "canonical base64 data URL",
        ),
        (
            r#"<image href="https://example.invalid/image.png" width="1" height="1"/>"#,
            "canonical base64 data URL",
        ),
        (
            r#"<image href="data:image/png;base64,iVBORw0KGgo=" width="1" height="1"/>"#,
            "undeclared embedded image resource",
        ),
        (
            r##"<use href="https://example.invalid/sprites.svg#shape"/>"##,
            "forbidden external SVG resource",
        ),
        (
            r#"<style>@import url("https://example.invalid/style.css");</style>"#,
            "forbidden stylesheet resource",
        ),
        (
            r#"<rect width="1" height="1" fill="url(https://example.invalid/paint.svg#p)"/>"#,
            "forbidden CSS resource",
        ),
        (
            r#"<rect width="1" height="1" style="fill:url(file:///tmp/paint.svg#p)"/>"#,
            "forbidden CSS resource",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let input = root.join(format!("input-{index}.svg"));
        let output = root.join(format!("output-{index}.rgba"));
        fs::write(
            &input,
            format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1">{svg}</svg>"#),
        )
        .unwrap();

        let result = viewer(&input, &output);

        assert!(!result.status.success(), "viewer admitted {svg}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected),
            "unexpected error for {svg}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!output.exists());
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_text_without_a_declared_embedded_font() {
    let root = temporary_directory("font-resource");
    let input = root.join("input.svg");
    let output = root.join("output.rgba");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        &input,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><text x="0" y="12">ambient</text></svg>"#,
    )
    .unwrap();

    let result = viewer(&input, &output);

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("undeclared font resource"),
        "unexpected error: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn approved_rectangle_is_reproducible_across_fresh_processes() {
    let root = temporary_directory("approved-rectangle");
    let first = root.join("first.rgba");
    let second = root.join("second.rgba");
    fs::create_dir_all(&root).unwrap();
    let input = repository_root()
        .join("tests/conformance/cases/focused/rectangle-outline/evidence/official.svg");

    let declaration = repository_root()
        .join("tests/conformance/svg-viewer-qualification/rectangle-outline/assets.none.json");
    let first_result = viewer_with_declaration(&input, &first, &declaration);
    let second_result = viewer(&input, &second);

    for result in [&first_result, &second_result] {
        assert!(
            result.status.success(),
            "viewer failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout),
            "{\"width\":200,\"height\":120}\n"
        );
    }
    let first = fs::read(first).unwrap();
    let second = fs::read(second).unwrap();
    assert_eq!(first.len(), 200 * 120 * 4);
    assert_eq!(first, second);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn admits_only_a_declared_data_image_and_uses_its_canonical_descriptor() {
    let root = temporary_directory("declared-image");
    let input = root.join("input.svg");
    let output = root.join("output.rgba");
    let declaration = root.join("assets.json");
    fs::create_dir_all(&root).unwrap();
    let image_path = "tests/conformance/cases/focused/rectangle-outline/evidence/official.png";
    let image = fs::read(repository_root().join(image_path)).unwrap();
    let hash = sha256_hex(&image);
    let data = base64::engine::general_purpose::STANDARD.encode(&image);
    fs::write(
        &input,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="data:image/png;base64,{data}" width="1" height="1"/></svg>"#
        ),
    )
    .unwrap();
    write_declaration(
        &declaration,
        json!({
            "fonts": [],
            "images": [{
                "source": { "media_type": "image/png", "sha256": hash },
                "canonical": { "path": image_path, "media_type": "image/png", "sha256": hash },
            }],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
        }),
    );

    let result = viewer_with_declaration(&input, &output, &declaration);

    assert!(
        result.status.success(),
        "viewer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout),
        "{\"width\":1,\"height\":1}\n"
    );
    assert_eq!(fs::read(&output).unwrap().len(), 4);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn admits_only_a_sanitized_canonical_nested_svg() {
    let root = temporary_directory("canonical-nested-svg");
    let descriptor_root = repository_root()
        .join("target")
        .join(format!("svg-viewer-nested-svg-test-{}", std::process::id()));
    let canonical = descriptor_root.join("canonical.svg");
    let input = root.join("input.svg");
    let output = root.join("output.rgba");
    let declaration = root.join("assets.json");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&descriptor_root).unwrap();
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><!-- source --><rect width="1" height="1" fill="#ff0000"/></svg>"##;
    let source_hash = sha256_hex(source);
    let data = base64::engine::general_purpose::STANDARD.encode(source);
    fs::write(
        &input,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="data:image/svg+xml;base64,{data}" width="1" height="1"/></svg>"#
        ),
    )
    .unwrap();
    let canonical_bytes =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="#ff0000"/></svg>"##;
    fs::write(&canonical, canonical_bytes).unwrap();
    let canonical_path = canonical.strip_prefix(repository_root()).unwrap();
    write_declaration(
        &declaration,
        json!({
            "fonts": [],
            "images": [{
                "source": { "media_type": "image/svg+xml", "sha256": source_hash },
                "canonical": {
                    "path": canonical_path.to_str().unwrap(),
                    "media_type": "image/svg+xml",
                    "sha256": sha256_hex(canonical_bytes),
                },
            }],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
        }),
    );

    let result = viewer_with_declaration(&input, &output, &declaration);

    assert!(
        result.status.success(),
        "viewer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(&output).unwrap(), [255, 0, 0, 255]);

    let active = br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" width="1" height="1"><s:script/></svg>"#;
    fs::write(&canonical, active).unwrap();
    write_declaration(
        &declaration,
        json!({
            "fonts": [],
            "images": [{
                "source": { "media_type": "image/svg+xml", "sha256": sha256_hex(source) },
                "canonical": {
                    "path": canonical_path.to_str().unwrap(),
                    "media_type": "image/svg+xml",
                    "sha256": sha256_hex(active),
                },
            }],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
        }),
    );
    fs::remove_file(&output).unwrap();
    let result = viewer_with_declaration(&input, &output, &declaration);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("canonical nested SVG contains active content"),
        "unexpected error: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(descriptor_root).unwrap();
}

#[test]
fn rejects_a_used_descriptor_with_the_wrong_hash() {
    let root = temporary_directory("descriptor-hash");
    let input = root.join("input.svg");
    let output = root.join("output.rgba");
    let declaration = root.join("assets.json");
    fs::create_dir_all(&root).unwrap();
    let image_path = "tests/conformance/cases/focused/rectangle-outline/evidence/official.png";
    let image = fs::read(repository_root().join(image_path)).unwrap();
    let source_hash = sha256_hex(&image);
    let data = base64::engine::general_purpose::STANDARD.encode(&image);
    fs::write(
        &input,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="data:image/png;base64,{data}" width="1" height="1"/></svg>"#
        ),
    )
    .unwrap();
    write_declaration(
        &declaration,
        json!({
            "fonts": [],
            "images": [{
                "source": { "media_type": "image/png", "sha256": source_hash },
                "canonical": { "path": image_path, "media_type": "image/png", "sha256": "0".repeat(64) },
            }],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
        }),
    );

    let result = viewer_with_declaration(&input, &output, &declaration);

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("descriptor SHA-256 mismatch"),
        "unexpected error: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_non_closed_and_duplicate_font_declarations() {
    let root = temporary_directory("declaration-shape");
    let input = root.join("input.svg");
    let output = root.join("output.rgba");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        &input,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#,
    )
    .unwrap();

    let unknown = root.join("unknown.json");
    write_declaration(
        &unknown,
        json!({
            "fonts": [],
            "images": [],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
            "unexpected": true,
        }),
    );
    let result = viewer_with_declaration(&input, &output, &unknown);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("asset declaration must contain exactly")
    );

    let duplicate = root.join("duplicate.json");
    write_declaration(
        &duplicate,
        json!({
            "fonts": [font_declaration("0".repeat(64)), font_declaration("1".repeat(64))],
            "images": [],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
        }),
    );
    let result = viewer_with_declaration(&input, &output, &duplicate);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("duplicate font CSS face"),
        "unexpected error: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_font_face_metadata_that_does_not_match_its_identity() {
    let root = temporary_directory("font-metadata");
    let input = root.join("input.svg");
    let output = root.join("output.rgba");
    let declaration = root.join("assets.json");
    fs::create_dir_all(&root).unwrap();
    let woff2 = b"wOF2metadata-only";
    let hash = sha256_hex(woff2);
    let data = base64::engine::general_purpose::STANDARD.encode(woff2);
    fs::write(
        &input,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><style>@font-face{{font-family:'Other';src:url(data:font/woff2;base64,{data}) format('woff2');font-style:normal;font-weight:400;font-stretch:normal;unicode-range:U+0-10FFFF}}</style><text x="0" y="12" font-family="Other">x</text></svg>"#
        ),
    )
    .unwrap();
    write_declaration(
        &declaration,
        json!({
            "fonts": [font_declaration(hash)],
            "images": [],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
        }),
    );

    let result = viewer_with_declaration(&input, &output, &declaration);

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("@font-face metadata does not match declaration"),
        "unexpected error: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_text_that_names_an_undeclared_face() {
    let root = temporary_directory("text-face");
    let input = root.join("input.svg");
    let output = root.join("output.rgba");
    let declaration = root.join("assets.json");
    fs::create_dir_all(&root).unwrap();
    let woff2 = b"wOF2metadata-only";
    let hash = sha256_hex(woff2);
    let data = base64::engine::general_purpose::STANDARD.encode(woff2);
    fs::write(
        &input,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><style>@font-face{{font-family:'Declared';src:url(data:font/woff2;base64,{data}) format('woff2');font-style:normal;font-weight:400;font-stretch:normal;unicode-range:U+0-10FFFF}}</style><text x="0" y="12" font-family="Other">x</text></svg>"#
        ),
    )
    .unwrap();
    write_declaration(
        &declaration,
        json!({
            "fonts": [font_declaration(hash)],
            "images": [],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
        }),
    );

    let result = viewer_with_declaration(&input, &output, &declaration);

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("text uses family \"Other\" without a matching embedded face"),
        "unexpected error: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
}

fn viewer(input: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_excalidraw-render-svg-viewer"))
        .args(["render"])
        .arg(input)
        .arg(output)
        .output()
        .expect("run pure-Rust SVG viewer")
}

fn viewer_with_declaration(
    input: &Path,
    output: &Path,
    declaration: &Path,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_excalidraw-render-svg-viewer"))
        .args(["render"])
        .arg(input)
        .arg(output)
        .arg(declaration)
        .output()
        .expect("run pure-Rust SVG viewer with asset declaration")
}

fn write_declaration(path: &Path, value: Value) {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    fs::write(path, bytes).unwrap();
}

fn font_declaration(source_hash: String) -> Value {
    json!({
        "source": { "media_type": "font/woff2", "sha256": source_hash },
        "css": {
            "family": "Declared",
            "style": "normal",
            "weight": 400,
            "stretch": "normal",
            "unicode_range": ["U+0-10FFFF"],
        },
        "sfnt": {
            "path": "target/svg-viewer-rust-test/reviewed.ttf",
            "media_type": "font/ttf",
            "sha256": "2".repeat(64),
            "face_index": 0,
            "family": "Reviewed",
            "style": "normal",
            "weight": 400,
            "stretch": "normal",
        },
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn temporary_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "excalidraw-render-svg-viewer-{name}-{}",
        std::process::id()
    ))
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_owned()
}
