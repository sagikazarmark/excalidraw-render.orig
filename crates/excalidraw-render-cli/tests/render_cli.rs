use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn renders_fixture_to_svg_png_and_pdf_deterministically() {
    let temp = temp_directory();
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic-shapes.excalidraw");
    let svg = temp.join("basic.svg");
    let second_svg = temp.join("basic-second.svg");
    let png = temp.join("basic.png");
    let pdf = temp.join("basic.pdf");

    run(&fixture, &svg, &[]);
    run(&fixture, &second_svg, &[]);
    run(&fixture, &png, &["--scale", "2"]);
    run(&fixture, &pdf, &[]);

    let svg_bytes = fs::read(&svg).expect("read SVG output");
    let second_svg_bytes = fs::read(&second_svg).expect("read second SVG output");
    let png_bytes = fs::read(&png).expect("read PNG output");
    let pdf_bytes = fs::read(&pdf).expect("read PDF output");

    assert!(!svg_bytes.is_empty());
    assert!(!png_bytes.is_empty());
    assert!(!pdf_bytes.is_empty());
    assert_eq!(svg_bytes, second_svg_bytes);

    let svg_text = String::from_utf8(svg_bytes).expect("SVG is UTF-8");
    assert!(svg_text.contains(r#"width="380" height="210""#));
    assert_eq!(svg_text.matches("data-element-id=").count(), 5);
    assert!(!svg_text.contains("unsupported-text"));

    let pixmap = tiny_skia::Pixmap::decode_png(&png_bytes).expect("decode PNG output");
    assert_eq!(pixmap.width(), 760);
    assert_eq!(pixmap.height(), 420);
    assert!(pdf_bytes.starts_with(b"%PDF-"));

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

fn run(input: &Path, output: &Path, extra_args: &[&str]) {
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg(input)
        .arg("-o")
        .arg(output)
        .args(extra_args)
        .output()
        .expect("run excalidraw-render");

    assert!(
        result.status.success(),
        "renderer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn temp_directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!("excalidraw-render-test-{}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path).expect("remove stale test temp directory");
    }
    fs::create_dir(&path).expect("create test temp directory");
    path
}
