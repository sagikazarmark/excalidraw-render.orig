use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[test]
fn standalone_commands_expose_revision_identity_and_exact_licenses() {
    let revisions = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["--list-revisions", "--report-format", "json"])
        .output()
        .expect("list revisions");
    assert!(revisions.status.success());
    assert!(revisions.stdout.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&revisions.stderr).expect("revision JSON report");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["renderer_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["revisions"][0]["compatibility_revision"], 1);
    assert_eq!(report["revisions"][0]["default"], true);

    let version = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("--version")
        .output()
        .expect("print version");
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).expect("version is UTF-8");
    assert!(version.contains(env!("CARGO_PKG_VERSION")));
    assert!(version.contains("compatibility revision 1"));
    assert!(version.contains("Excalidraw v0.18.1"));

    let licenses = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("--licenses")
        .output()
        .expect("print licenses");
    assert!(licenses.status.success());
    assert_eq!(
        licenses.stdout,
        excalidraw_render::license_bundle().as_bytes()
    );
    assert!(licenses.stderr.is_empty());
}

#[test]
fn only_one_valid_json_report_selection_changes_usage_error_transport() {
    let json = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["--report-format=json", "--not-an-option"])
        .output()
        .expect("run invalid JSON-mode command");
    assert_eq!(json.status.code(), Some(2));
    assert!(json.stdout.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&json.stderr).expect("JSON failure");
    assert_eq!(report["status"], "failure");
    assert_eq!(report["error"]["family"], "cli");
    assert_eq!(report["error"]["code"], "cli.invalid-arguments");

    let duplicate = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            "--report-format=json",
            "--report-format",
            "json",
            "--not-an-option",
        ])
        .output()
        .expect("run duplicate report-mode command");
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(serde_json::from_slice::<serde_json::Value>(&duplicate.stderr).is_err());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("cli.invalid-arguments"));
}

#[test]
fn validation_uses_normative_failure_order_and_stable_exit_statuses() {
    let temp = temp_directory_named("validation");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic-shapes.excalidraw");
    let output = temp.join("result.svg");

    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            "-",
            "--compatibility-revision",
            "999",
            "--padding=-1",
            "--report-format=json",
        ],
        2,
        "revision.unsupported",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            "-",
            "--padding=-1",
            "--report-format=json",
        ],
        2,
        "cli.format-required",
    );
    let format_required = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([fixture.to_str().unwrap(), "-o", "-", "--report-format=json"])
        .output()
        .expect("run format-required command");
    let format_required: serde_json::Value =
        serde_json::from_slice(&format_required.stderr).expect("JSON format failure");
    assert_eq!(format_required["error"]["details"]["kind"], "cli_argument");
    let invalid_none = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            fixture.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-format",
            "none",
            "--report-output",
            "stderr",
        ])
        .output()
        .expect("run invalid none-mode command");
    assert_eq!(invalid_none.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid_none.stderr).contains("cli cli.invalid-arguments:"));
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-output=-",
            "--report-format=json",
        ],
        2,
        "cli.invalid-arguments",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            fixture.to_str().unwrap(),
            "--format",
            "svg",
            "--asset",
            "broken",
            "--report-format=json",
        ],
        2,
        "cli.invalid-asset-option",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--asset",
            "asset=-",
            "--report-format=json",
        ],
        2,
        "cli.invalid-asset-option",
    );
    let invalid_mime = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            fixture.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--asset-mime",
            "broken",
            "--report-format=json",
        ])
        .output()
        .expect("run malformed asset MIME declaration");
    let invalid_mime: serde_json::Value =
        serde_json::from_slice(&invalid_mime.stderr).expect("JSON failure report");
    assert_eq!(invalid_mime["error"]["details"]["argument"], "--asset-mime");
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            fixture.to_str().unwrap(),
            "--format",
            "svg",
            "--report-format=json",
        ],
        2,
        "cli.path-conflict",
    );

    fs::write(&output, b"old output").expect("create existing destination");
    let missing = temp.join("missing.excalidraw");
    assert_failure(
        &[
            missing.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-format=json",
        ],
        2,
        "cli.destination-exists",
    );
    assert_failure(
        &[
            missing.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--force",
            "--report-format=json",
        ],
        1,
        "cli.input-read-failed",
    );
    assert_eq!(fs::read(&output).unwrap(), b"old output");

    let original_fixture = fs::read(&fixture).expect("read fixture before conflict test");
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            temp.join("conflict.svg").to_str().unwrap(),
            "--report-output",
            fixture.to_str().unwrap(),
            "--report-format=json",
            "--force",
        ],
        2,
        "cli.path-conflict",
    );
    assert_eq!(fs::read(&fixture).unwrap(), original_fixture);

    let invalid_scene = temp.join("invalid.excalidraw");
    fs::write(&invalid_scene, b"not JSON").expect("write invalid scene");
    assert_failure(
        &[
            invalid_scene.to_str().unwrap(),
            "-o",
            temp.join("parse.svg").to_str().unwrap(),
            "--asset",
            "z=missing-z",
            "--report-format=json",
        ],
        1,
        "scene.invalid-json",
    );
    assert_failure(
        &[
            fixture.to_str().unwrap(),
            "-o",
            temp.join("asset.svg").to_str().unwrap(),
            "--asset",
            "z=missing-z",
            "--asset",
            "a=missing-a",
            "--report-format=json",
        ],
        1,
        "cli.asset-read-failed",
    );

    let none = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            missing.to_str().unwrap(),
            "-o",
            temp.join("none.svg").to_str().unwrap(),
            "--report-format",
            "none",
        ])
        .output()
        .expect("run none-mode failure");
    assert_eq!(none.status.code(), Some(1));
    assert!(serde_json::from_slice::<serde_json::Value>(&none.stderr).is_err());
    assert!(String::from_utf8_lossy(&none.stderr).contains("cli cli.input-read-failed:"));

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

fn assert_failure(args: &[&str], exit: i32, code: &str) {
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(args)
        .output()
        .expect("run failing command");
    assert_eq!(
        result.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).unwrap_or_else(|error| {
            panic!(
                "JSON failure report for {args:?}: {error}; stderr={}",
                String::from_utf8_lossy(&result.stderr)
            )
        });
    assert_eq!(report["status"], "failure");
    assert_eq!(report["error"]["code"], code);
    assert_eq!(report["artifact"], serde_json::Value::Null);
}

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
    let stdin_svg = run_stdio(&fixture, &["--format", "svg", "--report-format", "json"]);
    fs::write(&second_svg, &stdin_svg).expect("write captured SVG");
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
    assert!(svg_text.contains(r#"width="1510" height="1110""#));
    assert!(svg_text.starts_with("<svg "));

    let pixmap = tiny_skia::Pixmap::decode_png(&png_bytes).expect("decode PNG output");
    assert_eq!(pixmap.width(), 3020);
    assert_eq!(pixmap.height(), 2220);
    assert!(pdf_bytes.starts_with(b"%PDF-"));

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

fn run_stdio(input: &Path, extra_args: &[&str]) -> Vec<u8> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["-", "--output", "-"])
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn excalidraw-render");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(&fs::read(input).expect("read fixture"))
        .expect("write scene to stdin");
    let result = child.wait_with_output().expect("wait for renderer");
    assert!(
        result.status.success(),
        "renderer failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("JSON success report");
    assert_eq!(report["status"], "success");
    assert_eq!(report["format"], "svg");
    assert_eq!(report["artifact"]["byte_length"], result.stdout.len());
    assert!(report["artifact"]["sha256"]
        .as_str()
        .expect("artifact digest")
        .starts_with("sha256:"));
    result.stdout
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
    temp_directory_named("render")
}

fn temp_directory_named(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "excalidraw-render-test-{}-{name}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path).expect("remove stale test temp directory");
    }
    fs::create_dir(&path).expect("create test temp directory");
    path
}
