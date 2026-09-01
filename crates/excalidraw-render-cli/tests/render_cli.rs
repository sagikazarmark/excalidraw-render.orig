use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use excalidraw_render::{CompatibilityRevision, Renderer};

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

    let ordered_asset_failure = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args([
            fixture.to_str().unwrap(),
            "-o",
            temp.join("ordered-asset.svg").to_str().unwrap(),
            "--asset",
            "z=missing-z",
            "--asset",
            "a=missing-a",
            "--report-format=json",
        ])
        .output()
        .expect("run ordered asset failure");
    let ordered_asset_failure: serde_json::Value =
        serde_json::from_slice(&ordered_asset_failure.stderr).expect("JSON asset failure");
    assert_eq!(ordered_asset_failure["error"]["details"]["asset_id"], "a");

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

#[test]
fn rejects_existing_and_nonexistent_path_aliases_without_modifying_sources() {
    let temp = temp_directory_named("path-aliases");
    let input = temp.join("input.excalidraw");
    fs::copy(corpus_scene("basic-shapes.excalidraw"), &input).expect("copy scene input");
    let alias = temp.join("input-alias.excalidraw");
    fs::hard_link(&input, &alias).expect("create input hard link");
    let original = fs::read(&input).expect("read original scene");

    assert_failure(
        &[
            input.to_str().unwrap(),
            "-o",
            alias.to_str().unwrap(),
            "--format=svg",
            "--force",
            "--report-format=json",
        ],
        2,
        "cli.path-conflict",
    );
    assert_eq!(fs::read(&input).unwrap(), original);

    assert_failure(
        &[
            "frames",
            input.to_str().unwrap(),
            "--output",
            input.to_str().unwrap(),
            "--format=svg",
            "--report-format=json",
        ],
        2,
        "cli.path-conflict",
    );
    assert_eq!(fs::read(&input).unwrap(), original);

    let output = temp.join("future.svg");
    let report_alias = temp.join("nested").join("..").join("future.svg");
    assert_failure(
        &[
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-output",
            report_alias.to_str().unwrap(),
            "--report-format=json",
        ],
        2,
        "cli.path-conflict",
    );
    assert!(!output.exists());

    fs::remove_dir_all(temp).expect("remove path-alias temp directory");
}

#[cfg(unix)]
#[test]
fn treats_dangling_symlink_destinations_as_existing_paths() {
    use std::os::unix::fs::symlink;

    let temp = temp_directory_named("dangling-destination");
    let output = temp.join("result.svg");
    symlink(temp.join("missing-target"), &output).expect("create dangling destination symlink");

    assert_failure(
        &[
            corpus_scene("basic-shapes.excalidraw").to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--report-format=json",
        ],
        2,
        "cli.destination-exists",
    );

    fs::remove_dir_all(temp).expect("remove dangling-destination temp directory");
}

#[test]
fn rejects_scene_and_asset_payloads_beyond_fixed_transport_bounds() {
    let temp = temp_directory_named("bounded-reads");
    let oversized_scene = temp.join("oversized.excalidraw");
    fs::File::create(&oversized_scene)
        .and_then(|file| file.set_len(67_108_865))
        .expect("create oversized sparse scene");
    assert_failure(
        &[
            oversized_scene.to_str().unwrap(),
            "-o",
            temp.join("scene.svg").to_str().unwrap(),
            "--report-format=json",
        ],
        1,
        "scene.input-too-large",
    );

    let oversized_asset = temp.join("oversized.asset");
    fs::File::create(&oversized_asset)
        .and_then(|file| file.set_len(33_554_433))
        .expect("create oversized sparse asset");
    assert_failure(
        &[
            corpus_scene("basic-shapes.excalidraw").to_str().unwrap(),
            "-o",
            temp.join("asset.svg").to_str().unwrap(),
            "--asset",
            &format!("asset={}", oversized_asset.display()),
            "--report-format=json",
        ],
        1,
        "resource.asset-bytes-exceeded",
    );

    fs::remove_dir_all(temp).expect("remove bounded-reads temp directory");
}

#[test]
fn stages_both_paths_before_delivery_and_cleans_temporary_files() {
    let temp = temp_directory_named("staging");
    let output = temp.join("result.svg");
    let unavailable_report = temp.join("missing-parent").join("report.json");
    let result = render_command(&output)
        .args([
            "--report-output",
            unavailable_report.to_str().unwrap(),
            "--report-format=json",
        ])
        .output()
        .expect("run report staging failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(!output.exists(), "artifact must not precede report staging");
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("emergency JSON report");
    assert_eq!(report["error"]["code"], "cli.report-write-failed");
    assert_eq!(report["error"]["details"]["artifact_state"], "none");
    assert_no_staging_files(&temp);

    fs::remove_dir_all(temp).expect("remove staging temp directory");
}

#[test]
fn delivers_artifact_before_report_and_emits_complete_emergency_json() {
    let temp = temp_directory_named("report-delivery");
    let output = temp.join("result.svg");
    let report_destination = temp.join("report.json");
    fs::create_dir(&report_destination).expect("create undeliverable report destination");
    let result = render_command(&output)
        .args([
            "--report-output",
            report_destination.to_str().unwrap(),
            "--report-format=json",
            "--force",
        ])
        .output()
        .expect("run report delivery failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(fs::read(&output)
        .expect("delivered artifact")
        .starts_with(b"<svg "));
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("canonical emergency JSON report");
    assert_eq!(report["status"], "failure");
    assert_eq!(report["artifact"], serde_json::Value::Null);
    assert_eq!(report["error"]["code"], "cli.report-write-failed");
    assert_eq!(report["error"]["details"]["artifact_state"], "complete");
    assert!(result.stderr.ends_with(b"\n"));
    assert_eq!(
        result.stderr.iter().filter(|&&byte| byte == b'\n').count(),
        1
    );
    assert_no_staging_files(&temp);

    fs::remove_dir_all(temp).expect("remove report-delivery temp directory");
}

#[test]
fn failed_non_json_report_delivery_emits_one_emergency_line() {
    let temp = temp_directory_named("non-json-emergency");
    let input = temp.join("diagnosed.excalidraw");
    fs::write(
        &input,
        br#"{"type":"excalidraw","version":2,"elements":[{"id":"unknown","type":"future","x":0,"y":0,"width":100,"height":50,"updated":1}],"appState":{},"files":{}}"#,
    )
    .expect("write diagnosed scene");
    let output = temp.join("result.svg");
    let report_destination = temp.join("report.txt");
    fs::create_dir(&report_destination).expect("create undeliverable report destination");
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .args([
            "--report-output",
            report_destination.to_str().unwrap(),
            "--report-format=human",
            "--force",
        ])
        .output()
        .expect("run non-JSON report delivery failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(output.exists());
    assert_eq!(
        result.stderr.iter().filter(|&&byte| byte == b'\n').count(),
        1
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("cli.report-write-failed"));

    fs::remove_dir_all(temp).expect("remove non-JSON emergency temp directory");
}

#[test]
fn stdout_failure_before_first_byte_reports_that_no_artifact_bytes_escaped() {
    let temp = temp_directory_named("closed-stdout");
    let report_path = temp.join("report.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"));
    command
        .arg(corpus_scene("basic-shapes.excalidraw"))
        .args([
            "-o",
            "-",
            "--format=svg",
            "--report-output",
            report_path.to_str().unwrap(),
            "--report-format=json",
        ])
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.stdout(Stdio::from(
        fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .expect("open always-failing stdout"),
    ));
    #[cfg(not(unix))]
    command.stdout(Stdio::piped());
    let child = command.spawn().expect("spawn renderer with closed stdout");
    #[cfg(not(unix))]
    let mut child = child;
    #[cfg(not(unix))]
    drop(child.stdout.take().expect("take stdout pipe"));
    let result = child.wait_with_output().expect("wait for stdout failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(&report_path).expect("read stdout failure report"))
            .expect("JSON stdout failure report");
    assert_eq!(report["error"]["code"], "cli.output-write-failed");
    assert_eq!(report["error"]["details"]["artifact_state"], "none");
    assert_no_staging_files(&temp);

    fs::remove_dir_all(temp).expect("remove closed-stdout temp directory");
}

#[test]
fn interrupted_stdout_after_bytes_escape_reports_possibly_partial() {
    let temp = temp_directory_named("partial-stdout");
    let report_path = temp.join("report.json");
    let mut child = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg(corpus_scene("basic-shapes.excalidraw"))
        .args([
            "-o",
            "-",
            "--format=pdf",
            "--report-output",
            report_path.to_str().unwrap(),
            "--report-format=json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn renderer with interrupted stdout");
    let mut stdout = child.stdout.take().expect("take stdout pipe");
    let mut first_byte = [0_u8; 1];
    stdout
        .read_exact(&mut first_byte)
        .expect("receive one artifact byte");
    drop(stdout);
    let result = child
        .wait_with_output()
        .expect("wait for partial stdout failure");

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(&report_path).expect("read partial failure report"))
            .expect("JSON partial stdout failure report");
    assert_eq!(report["error"]["code"], "cli.output-write-failed");
    assert_eq!(
        report["error"]["details"]["artifact_state"],
        "possibly_partial"
    );

    fs::remove_dir_all(temp).expect("remove partial-stdout temp directory");
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
    assert!(svg_text.contains(r#"data-source-text="Text rendered by the complete CLI""#));

    let pixmap = tiny_skia::Pixmap::decode_png(&png_bytes).expect("decode PNG output");
    assert_eq!(pixmap.width(), 3020);
    assert_eq!(pixmap.height(), 2220);
    assert!(pdf_bytes.starts_with(b"%PDF-"));
    let pdf_text = String::from_utf8_lossy(&pdf_bytes);
    assert!(pdf_text.contains("/BaseFont /Excalifont"));

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

#[test]
fn preserves_embedded_images_across_cli_formats() {
    let temp = temp_directory_named("embedded-image");
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/embedded-image.excalidraw");
    let svg = temp.join("image.svg");
    let png = temp.join("image.png");
    let pdf = temp.join("image.pdf");

    run(&fixture, &svg, &[]);
    run(&fixture, &png, &[]);
    run(&fixture, &pdf, &[]);

    let svg = fs::read_to_string(svg).expect("read SVG image output");
    assert!(svg.contains("data:image/png;base64,"));
    assert!(svg.contains(r#"width="10" height="10""#));

    let png = tiny_skia::Pixmap::load_png(png).expect("decode PNG image output");
    assert_eq!((png.width(), png.height()), (30, 30));
    assert_eq!(
        png.pixel(15, 15).expect("center pixel").demultiply(),
        tiny_skia::ColorU8::from_rgba(0, 0, 0, 255)
    );

    let pdf = fs::read(pdf).expect("read PDF image output");
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(pdf.contains("/Subtype /Image"));
    assert!(pdf.contains("/Im0 Do"));

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

#[test]
fn frames_list_reports_the_selection_without_rendering_anything() {
    let temp = temp_directory_named("frames-list");
    let input = temp.join("scene.excalidraw");
    let output = temp.join("frames");
    fs::write(
        &input,
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"one","type":"frame","name":"Road Map","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"two","type":"frame","name":"Road Map","x":50,"y":0,"width":40,"height":30,"updated":1},
                {"id":"three","type":"magicframe","name":null,"x":100,"y":0,"width":40,"height":30,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("write frame scene");

    // Neither --output nor --format is required, and nothing is written.
    let human = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .arg("--list")
        .output()
        .expect("run frame listing");
    assert!(human.status.success());
    assert!(human.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&human.stderr),
        "1\tframe\tRoad Map\tRoad-Map\n2\tframe\tRoad Map\tRoad-Map-2\n3\tmagicframe\t\tframe-3\n"
    );
    assert!(!output.exists());

    // With a format, the derived basenames carry the extension the export would
    // have used, and the ordinals, kinds, and names match the export order.
    let json = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--format", "svg", "--report-format", "json"])
        .output()
        .expect("run frame listing");
    assert!(json.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&json.stderr).expect("frame list JSON report");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["status"], "success");
    assert_eq!(report["mode"], "list");
    assert_eq!(report["compatibility_revision"], 1);
    assert_eq!(
        report["reference_profile_digest"],
        Renderer::frame_profile_identity(CompatibilityRevision::REVISION_1)
            .expect("frame profile")
            .profile_digest()
    );
    assert_eq!(report["frames"].as_array().map(Vec::len), Some(3));
    assert_eq!(report["frames"][0]["ordinal"], 1);
    assert_eq!(report["frames"][0]["kind"], "frame");
    assert_eq!(report["frames"][0]["name"], "Road Map");
    assert_eq!(report["frames"][0]["output_basename"], "Road-Map.svg");
    assert_eq!(report["frames"][2]["kind"], "magicframe");
    assert_eq!(report["frames"][2]["name"], serde_json::Value::Null);
    assert_eq!(report["frames"][2]["output_basename"], "frame-3.svg");
    assert!(!output.exists());

    // --name-regex and --max-frames apply exactly as they do to an export.
    let filtered = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--name-regex", "^Road"])
        .output()
        .expect("run filtered frame listing");
    assert!(filtered.status.success());
    assert_eq!(
        String::from_utf8_lossy(&filtered.stderr),
        "1\tframe\tRoad Map\tRoad-Map\n2\tframe\tRoad Map\tRoad-Map-2\n"
    );

    let capped = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--max-frames", "2"])
        .output()
        .expect("run capped frame listing");
    assert_eq!(capped.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&capped.stderr).contains("cli.frame-limit-exceeded"));

    // An empty selection is a valid answer to a listing, unlike to an export.
    let empty = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args([
            "--list",
            "--name-regex",
            "^Missing$",
            "--report-format",
            "json",
        ])
        .output()
        .expect("run empty frame listing");
    assert!(empty.status.success());
    let empty_report: serde_json::Value =
        serde_json::from_slice(&empty.stderr).expect("empty frame list JSON report");
    assert_eq!(empty_report["frames"].as_array().map(Vec::len), Some(0));

    let suppressed = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--list", "--report-format", "none"])
        .output()
        .expect("run suppressed frame listing");
    assert_eq!(suppressed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&suppressed.stderr).contains("cli.invalid-arguments"));

    // Outside --list, both --output and --format remain required.
    let missing = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .args(["--format", "svg"])
        .output()
        .expect("run frame export without an output");
    assert_eq!(missing.status.code(), Some(2));

    // A dry run is exempt from performing an export's work, not from rejecting
    // what an export would reject: a listing that succeeded here while the
    // matching export failed would be a false preview.
    for rejected in [
        vec!["--scale", "0"],
        vec!["--max-dimension", "-1"],
        vec!["--asset", "bogus-no-equals"],
        vec!["--asset-mime", "unmatched=image/png"],
    ] {
        let listed = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
            .arg("frames")
            .arg(&input)
            .arg("--list")
            .args(&rejected)
            .output()
            .expect("run rejected frame listing");
        let exported = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
            .arg("frames")
            .arg(&input)
            .arg("--output")
            .arg(temp.join("rejected"))
            .args(["--format", "svg"])
            .args(&rejected)
            .output()
            .expect("run rejected frame export");
        assert_eq!(
            listed.status.code(),
            exported.status.code(),
            "{rejected:?} is accepted by --list but rejected by an export"
        );
        assert_eq!(
            listed.status.code(),
            Some(2),
            "{rejected:?} is a usage error"
        );
    }

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

#[test]
fn frames_command_filters_names_and_writes_one_deterministically_named_artifact_per_frame() {
    let temp = temp_directory_named("frames-command");
    let input = temp.join("scene.excalidraw");
    let output = temp.join("frames");
    fs::write(
        &input,
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"one","type":"frame","name":"Road Map","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"one-child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"one","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
                {"id":"two","type":"frame","name":"Road Map","x":50,"y":0,"width":40,"height":30,"updated":1},
                {"id":"two-child","type":"rectangle","x":55,"y":5,"width":10,"height":10,"frameId":"two","strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1},
                {"id":"three","type":"magicframe","name":null,"x":100,"y":0,"width":40,"height":30,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("write frame scene");

    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .args(["--format", "svg", "--report-format", "json"])
        .output()
        .expect("run frame export");
    assert!(
        result.status.success(),
        "frame export failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(output.join("Road-Map.svg").is_file());
    assert!(output.join("Road-Map-2.svg").is_file());
    assert!(output.join("frame-3.svg").is_file());
    let report: serde_json::Value =
        serde_json::from_slice(&result.stderr).expect("frame batch JSON report");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["status"], "success");
    assert_eq!(report["format"], "svg");
    assert_eq!(report["frames"].as_array().map(Vec::len), Some(3));
    assert_eq!(report["frames"][0]["ordinal"], 1);
    assert_eq!(
        report["frames"][0]["output_path"],
        output.join("Road-Map.svg").to_string_lossy().as_ref()
    );
    assert_eq!(
        report["reference_profile_digest"],
        Renderer::frame_profile_identity(CompatibilityRevision::REVISION_1)
            .expect("frame profile")
            .profile_digest()
    );

    let no_match_output = temp.join("no-match");
    let no_match = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .arg("--output")
        .arg(&no_match_output)
        .args(["--format", "svg", "--name-regex", "^Missing$"])
        .output()
        .expect("run no-match frame export");
    assert_eq!(no_match.status.code(), Some(1));
    assert!(!no_match_output.exists());
    assert!(String::from_utf8_lossy(&no_match.stderr).contains("cli.frame-no-match"));

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

#[test]
fn frame_filenames_use_unicode_case_folding_and_preserve_long_name_hashes() {
    let temp = temp_directory_named("frame-portable-filenames");
    let input = temp.join("scene.excalidraw");
    let output = temp.join("frames");
    let long_name = "A".repeat(140);
    let scene = format!(
        r##"{{
            "type":"excalidraw","version":2,
            "elements":[
                {{"id":"one","type":"frame","name":"Straße","x":0,"y":0,"width":20,"height":20,"updated":1}},
                {{"id":"two","type":"frame","name":"STRASSE","x":30,"y":0,"width":20,"height":20,"updated":1}},
                {{"id":"three","type":"frame","name":"{long_name}","x":60,"y":0,"width":20,"height":20,"updated":1}},
                {{"id":"four","type":"frame","name":"{long_name}","x":90,"y":0,"width":20,"height":20,"updated":1}}
            ],
            "appState":{{}},"files":{{}}
        }}"##
    );
    fs::write(&input, scene).expect("write portable filename scene");

    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .arg("frames")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .args(["--format", "svg"])
        .output()
        .expect("run portable filename export");
    assert!(
        result.status.success(),
        "frame export failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.join("Straße.svg").is_file());
    assert!(output.join("STRASSE-2.svg").is_file());

    let long_names = fs::read_dir(&output)
        .expect("read frame output directory")
        .map(|entry| {
            entry
                .expect("read frame output entry")
                .file_name()
                .into_string()
                .expect("portable filename is UTF-8")
        })
        .filter(|name| name.starts_with('A'))
        .collect::<Vec<_>>();
    assert_eq!(long_names.len(), 2);
    let pattern =
        regex::Regex::new(r"-([0-9a-f]{8})(?:-2)?\.svg$").expect("long-name suffix regex is valid");
    let hashes = long_names
        .iter()
        .map(|name| {
            assert!(
                name.len() <= 124,
                "filename exceeds component limit: {name}"
            );
            pattern
                .captures(name)
                .and_then(|captures| captures.get(1))
                .map(|capture| capture.as_str().to_owned())
                .expect("long filename retains its hash suffix")
        })
        .collect::<Vec<_>>();
    assert_eq!(hashes[0], hashes[1]);
    assert!(long_names.iter().any(|name| name.ends_with("-2.svg")));

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

#[test]
fn frame_batches_are_stage_all_by_default_and_partial_only_when_requested() {
    let temp = temp_directory_named("frame-publication-policy");
    let input = temp.join("scene.excalidraw");
    fs::write(
        &input,
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"one","type":"frame","name":"One","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"one-child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"one","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1},
                {"id":"two","type":"frame","name":"Two","x":50,"y":0,"width":40,"height":30,"updated":1},
                {"id":"two-child","type":"rectangle","x":55,"y":5,"width":10,"height":10,"frameId":"two","strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("write frame scene");

    let atomic = temp.join("atomic");
    fs::create_dir(&atomic).expect("create atomic output");
    fs::write(atomic.join("One.svg"), b"existing").expect("write collision");
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&atomic)
        .args(["--format", "svg", "--report-format", "json"])
        .output()
        .expect("run atomic frame export");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(atomic.join("One.svg")).unwrap(), b"existing");
    assert!(!atomic.join("Two.svg").exists());
    assert_no_staging_files(&atomic);
    let report: serde_json::Value = serde_json::from_slice(&result.stderr).expect("atomic report");
    assert_eq!(report["status"], "failure");
    assert!(report["frames"][0]["artifact"].is_object());

    let partial = temp.join("partial");
    fs::create_dir(&partial).expect("create partial output");
    fs::write(partial.join("One.svg"), b"existing").expect("write collision");
    let result = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&partial)
        .args([
            "--format",
            "svg",
            "--allow-partial",
            "--report-format",
            "json",
        ])
        .output()
        .expect("run partial frame export");
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(partial.join("One.svg")).unwrap(), b"existing");
    assert!(partial.join("Two.svg").is_file());
    assert_no_staging_files(&partial);
    let report: serde_json::Value = serde_json::from_slice(&result.stderr).expect("partial report");
    assert_eq!(report["status"], "partial");
    assert_eq!(report["frames"][0]["delivery_state"], "none");
    assert_eq!(report["frames"][1]["delivery_state"], "complete");
    assert_eq!(report["frames"][0]["error"]["family"], "cli");
    assert_eq!(
        report["frames"][0]["error"]["category"],
        serde_json::Value::Null
    );
    assert_eq!(report["frames"][0]["error"]["details"]["kind"], "delivery");
    let raw_report = std::str::from_utf8(&result.stderr).expect("frame report is UTF-8");
    assert!(raw_report
        .starts_with("{\"schema_version\":1,\"status\":\"partial\",\"renderer_version\":"));
    assert!(raw_report.contains(
        "\"error\":{\"family\":\"cli\",\"category\":null,\"code\":\"cli.destination-exists\""
    ));

    let human = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&atomic)
        .args(["--format", "svg"])
        .output()
        .expect("run human collision report");
    assert_eq!(human.status.code(), Some(1));
    let human = String::from_utf8(human.stderr).expect("human report is UTF-8");
    assert!(human.contains("cli cli.destination-exists: frame 1 ("));
    assert!(!human.contains("frame-batch-"));

    let none = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&atomic)
        .args(["--format", "svg", "--report-format", "none"])
        .output()
        .expect("run suppressed collision report");
    assert_eq!(none.status.code(), Some(1));
    let none = String::from_utf8(none.stderr).expect("none report is UTF-8");
    assert!(none.contains("cli cli.destination-exists: output destination already exists"));
    assert!(!none.contains("frame-batch-"));

    let missing_asset = temp.join("missing-asset.bin");
    let precedence = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"))
        .args(["frames"])
        .arg(&input)
        .arg("--output")
        .arg(&atomic)
        .args(["--format", "svg", "--report-format", "json", "--asset"])
        .arg(format!("missing={}", missing_asset.display()))
        .output()
        .expect("run collision and missing asset");
    assert_eq!(precedence.status.code(), Some(2));
    let precedence: serde_json::Value =
        serde_json::from_slice(&precedence.stderr).expect("precedence report is JSON");
    assert_eq!(precedence["error"]["code"], "cli.destination-exists");

    fs::remove_dir_all(temp).expect("remove test temp directory");
}

#[test]
fn frame_batch_report_staging_failure_prevents_all_artifact_publication() {
    let temp = temp_directory_named("frame-report-staging");
    let input = temp.join("scene.excalidraw");
    fs::write(
        &input,
        br##"{
            "type":"excalidraw","version":2,
            "elements":[
                {"id":"one","type":"frame","name":"One","x":0,"y":0,"width":40,"height":30,"updated":1},
                {"id":"one-child","type":"rectangle","x":5,"y":5,"width":10,"height":10,"frameId":"one","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1}
            ],
            "appState":{},"files":{}
        }"##,
    )
    .expect("write frame scene");

    for (mode, allow_partial) in [("default", false), ("partial", true)] {
        let output = temp.join(format!("{mode}-frames"));
        let report = temp
            .join(format!("{mode}-missing-parent"))
            .join("report.json");
        let mut command = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"));
        command
            .arg("frames")
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .args(["--format", "svg", "--report-format", "json"])
            .arg("--report-output")
            .arg(&report);
        if allow_partial {
            command.arg("--allow-partial");
        }
        let result = command.output().expect("run frame report staging failure");

        assert_eq!(result.status.code(), Some(1));
        assert!(
            !output.exists(),
            "{mode} mode published output before report staging"
        );
        assert!(!report.exists());
        let emergency: serde_json::Value =
            serde_json::from_slice(&result.stderr).expect("emergency JSON report");
        assert_eq!(emergency["error"]["code"], "cli.report-write-failed");
        assert_eq!(emergency["error"]["details"]["artifact_state"], "none");
        assert_no_staging_files(&temp);
    }

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

fn render_command(output: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_excalidraw-render"));
    command
        .arg(corpus_scene("basic-shapes.excalidraw"))
        .arg("-o")
        .arg(output);
    command
}

fn corpus_scene(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn assert_no_staging_files(directory: &Path) {
    let staging_files = fs::read_dir(directory)
        .expect("read temp directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert!(
        staging_files.is_empty(),
        "staging files remain: {staging_files:?}"
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
