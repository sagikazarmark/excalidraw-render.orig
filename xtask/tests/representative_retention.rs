//! Materialization and compact records at their command boundaries.
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
fn run(args: &[&str], success: bool) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("representative")
        .args(args)
        .current_dir(root())
        .output()
        .unwrap();
    assert_eq!(
        out.status.success(),
        success,
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(if success { out.stdout } else { out.stderr }).unwrap()
}
fn copy(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
fn read(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn write(path: impl AsRef<Path>, v: &Value) {
    fs::write(path, serde_json::to_vec(v).unwrap()).unwrap();
}

const CASE: &str = "tests/conformance/cases/representative/excali-local-slides";
fn spec() -> Value {
    let case = read(root().join(CASE).join("case.json"));
    json!({"case_id":case["case_id"],"formats":case["formats"],"semantic_expectations":case["semantic_expectations"],"exceptions":case["exceptions"],"resource_outcomes":case["resource_outcomes"],
        "expected_reports":{"svg":read(root().join(CASE).join("expected-report-svg.json")),"png":read(root().join(CASE).join("expected-report-png.json")),"pdf":read(root().join(CASE).join("expected-report-pdf.json"))}})
}
fn materialize(output: &Path, spec: &Path, success: bool) -> String {
    run(
        &[
            "case",
            "materialize",
            output.to_str().unwrap(),
            "--intake",
            "tests/conformance/intake/excali-local-slides-v1",
            "--reference",
            "tests/conformance/cases/representative/excali-local-slides/reference",
            "--evidence-record",
            "tests/conformance/proposals/slides84-evidence-v2/compact.json",
            "--spec",
            spec.to_str().unwrap(),
        ],
        success,
    )
}

#[test]
fn case_materialization_is_create_only_unapproved_and_preserves_reference_bytes() {
    let scratch = tempfile::tempdir().unwrap();
    let spec_path = scratch.path().join("spec.json");
    write(&spec_path, &spec());
    let output = scratch.path().join("case");
    let summary: Value = serde_json::from_str(&materialize(&output, &spec_path, true)).unwrap();
    let case = read(output.join("case.json"));
    assert!(case["approval"].is_null());
    assert!(summary["approval"].is_null());
    let verify: Value =
        serde_json::from_str(&run(&["case", "verify", output.to_str().unwrap()], true)).unwrap();
    assert_eq!(verify["subject"], summary["subject"]);
    assert_eq!(verify["approved"], false);
    assert_eq!(
        run(&["case", "subject", output.to_str().unwrap()], true).trim(),
        summary["subject"].as_str().unwrap()
    );
    assert_eq!(
        fs::read(output.join("reference/light/official.svg")).unwrap(),
        fs::read(root().join(CASE).join("reference/light/official.svg")).unwrap()
    );
    let before = fs::read(output.join("case.json")).unwrap();
    materialize(&output, &spec_path, false);
    assert_eq!(fs::read(output.join("case.json")).unwrap(), before);
}

#[test]
fn invalid_materialization_never_publishes_a_case() {
    for mutation in 0..5 {
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("spec.json");
        let output = scratch.path().join("case");
        let mut value = spec();
        match mutation {
            0 => value["case_id"] = json!("representative.other.v1"),
            1 => value["formats"] = json!(["png", "png"]),
            2 => value["expected_reports"]["png"] = json!({"status":"success"}),
            3 => value["semantic_expectations"] = json!({}),
            _ => value["approval"] = json!({"approved_by":"forged"}),
        }
        write(&path, &value);
        materialize(&output, &path, false);
        assert!(!output.exists());
    }
}

#[test]
fn materialization_rejects_invalid_semantics_without_changing_sources() {
    let source_case = read(root().join(CASE).join("case.json"));
    let sources = source_case["files"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|name| name.starts_with("reference/"))
        .map(|name| root().join(CASE).join(name))
        .chain([
            root().join("tests/conformance/intake/excali-local-slides-v1/manifest.json"),
            root().join("tests/conformance/intake/excali-local-slides-v1/scene.excalidraw"),
            root().join("tests/conformance/proposals/slides84-evidence-v2/compact.json"),
        ])
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
    let fields = [
        (
            "svg_size",
            vec![
                json!(["1"]),
                json!([1, 2]),
                json!(["0", "2"]),
                json!(["NaN", "2"]),
            ],
        ),
        ("min_text_elements", vec![json!(-1), json!(1.5), json!("1")]),
        ("links", vec![json!(-1), json!(1.5), json!("1")]),
        ("embedded_scene", vec![json!(0)]),
        ("root_theme_filter", vec![json!("false")]),
        (
            "notices",
            vec![
                json!("font.test"),
                json!([1]),
                json!([""]),
                json!([" "]),
                json!(["font.test", "font.test"]),
            ],
        ),
        (
            "png_size",
            vec![
                json!([1]),
                json!([1, 2, 3]),
                json!(["1", "2"]),
                json!([0, 2]),
                json!([-1, 2]),
                json!([1.5, 2]),
                json!([4294967296_u64, 2]),
            ],
        ),
        (
            "pdf_media_box",
            vec![
                json!(["1"]),
                json!([1, 2]),
                json!(["-1", "2"]),
                json!(["inf", "2"]),
                json!(["1e999", "2"]),
                json!(["wide", "2"]),
            ],
        ),
    ];
    let mut invalid = vec![json!({}), Value::Null];
    for (field, values) in fields {
        let mut missing = spec()["semantic_expectations"]["light"].clone();
        missing.as_object_mut().unwrap().remove(field);
        invalid.push(missing);
        for value in values {
            let mut expected = spec()["semantic_expectations"]["light"].clone();
            expected[field] = value;
            invalid.push(expected);
        }
    }
    for expected in invalid {
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("spec.json");
        let output = scratch.path().join("case");
        let mut value = spec();
        value["semantic_expectations"]["light"] = expected.clone();
        write(&path, &value);
        let before = fs::read(&path).unwrap();
        let error = materialize(&output, &path, false);
        assert!(
            error.contains("light: invalid semantic expectation"),
            "{expected}: {error}"
        );
        assert!(!output.exists());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    for (path, bytes) in sources {
        assert_eq!(fs::read(&path).unwrap(), bytes, "{}", path.display());
    }
}

#[test]
fn materialization_requires_only_selected_format_semantics_and_preserves_authored_values() {
    for (format, expected) in [
        (
            "svg",
            json!({"svg_size":["1.25","2"],"min_text_elements":0,"links":0,"embedded_scene":false,"root_theme_filter":true,"notices":[],"png_size":"unused"}),
        ),
        ("png", json!({"png_size":[1,u32::MAX],"notices":false})),
        (
            "pdf",
            json!({"pdf_media_box":["1e2","2.5"],"notices":["font.test"],"svg_size":null}),
        ),
    ] {
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("spec.json");
        let output = scratch.path().join("case");
        let mut value = spec();
        value["formats"] = json!([format]);
        value["expected_reports"] = json!({format: value["expected_reports"][format]});
        for semantics in value["semantic_expectations"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            *semantics = expected.clone();
        }
        write(&path, &value);
        materialize(&output, &path, true);
        let case = read(output.join("case.json"));
        assert_eq!(
            case["semantic_expectations"],
            value["semantic_expectations"]
        );
        assert!(case["approval"].is_null());
        assert_eq!(read(&path), value);
    }
}

#[test]
fn all_legacy_compact_records_verify_without_full_rasters() {
    for name in [
        "varmq84-evidence-v3",
        "slides84-evidence-v2",
        "velox84-evidence-v2",
    ] {
        run(
            &[
                "compact",
                "verify",
                &format!("tests/conformance/proposals/{name}"),
            ],
            true,
        );
    }
}

#[test]
fn compact_rejects_changed_extra_missing_and_detached_records() {
    let source = root().join("tests/conformance/proposals/varmq84-evidence-v3");
    for mutation in 0..6 {
        let scratch = tempfile::tempdir().unwrap();
        let output = scratch.path().join("compact");
        copy(&source, &output);
        match mutation {
            0 => fs::write(output.join("viewers/light/regions.json"), b"{}").unwrap(),
            1 => fs::write(output.join("viewers/light/native.png"), b"extra").unwrap(),
            2 => fs::remove_file(output.join("native/capture.json")).unwrap(),
            _ => {
                let mut record = read(output.join("compact.json"));
                match mutation {
                    3 => record["eligible_executions"] = json!(["invented"]),
                    4 => record["retained"]["viewers"] = json!([]),
                    _ => {
                        record["inventory"]["viewers"]["../escape"] =
                            json!({"bytes":1,"sha256":"0".repeat(64)})
                    }
                };
                write(output.join("compact.json"), &record);
            }
        }
        run(&["compact", "verify", output.to_str().unwrap()], false);
    }
}

#[test]
fn compact_reference_artifact_descriptors_must_match_retained_receipt() {
    let scratch = tempfile::tempdir().unwrap();
    let output = scratch.path().join("compact");
    copy(
        &root().join("tests/conformance/proposals/varmq84-evidence-v3"),
        &output,
    );
    let mut record = read(output.join("compact.json"));
    record["inventory"]["reference"]["light/official.png"]["sha256"] = json!("0".repeat(64));
    write(output.join("compact.json"), &record);
    assert!(run(&["compact", "verify", output.to_str().unwrap()], false)
        .contains("reference inventory detached"));
}
