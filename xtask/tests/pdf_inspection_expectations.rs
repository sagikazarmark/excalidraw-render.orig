//! Expectation admission and Python-compatible checks through the public CLI.
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

fn case() -> PathBuf {
    root().join("tests/conformance/pdf-viewer-qualification/rectangle-outline")
}

fn expectation() -> Value {
    serde_json::from_slice(&fs::read(case().join("expected-structure.json")).unwrap()).unwrap()
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pdf-inspection-expectations-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn inspect(&self, expectation: &str, expected: &str, qpdf: Option<&Path>) -> Output {
        let output = self.0.join("report.json");
        let _ = fs::remove_file(&output);
        let input = self.0.join("expectation.json");
        fs::write(&input, expectation).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
        command
            .args(["pdf-harness", "inspect"])
            .arg(case().join("candidate-control.pdf"))
            .arg(input)
            .arg(expected)
            .arg(output)
            .current_dir(root());
        if let Some(qpdf) = qpdf {
            command.env("QPDF", qpdf);
        }
        command.output().unwrap()
    }

    fn report(&self) -> Value {
        serde_json::from_slice(&fs::read(self.0.join("report.json")).unwrap()).unwrap()
    }

    fn rejected(&self, expectation: &str, field: &str) {
        for expected in ["pass", "fail"] {
            // Admission must precede qpdf execution, including for PDFs with no
            // pages or output intents on which to run the conditional checks.
            let output = self.inspect(expectation, expected, Some(&self.0.join("no-qpdf")));
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "accepted {field} with {expected}");
            assert!(error.contains("invalid expectation"), "{field}: {error}");
            assert!(error.contains(field), "{field}: {error}");
            assert!(!self.0.join("report.json").exists(), "{field}");
        }
    }

    #[cfg(unix)]
    fn model(&self, media: Value) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let qpdf = self.0.join("qpdf");
        fs::write(
            &qpdf,
            "#!/bin/sh\ncase \"$1\" in\n--version) printf 'qpdf version 12.3.2\\n';;\n--check) exit 0;;\n*) exec cat \"$0.json\";;\nesac\n",
        )
        .unwrap();
        fs::set_permissions(&qpdf, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            self.0.join("qpdf.json"),
            serde_json::to_vec(&json!({
                "qpdf": [{"pdfversion": "1.7"}, {
                    "trailer": {"value": {"/Root": "1 0 R"}},
                    "obj:1 0 R": {"value": {"/Type": "/Catalog"}},
                    "obj:2 0 R": {"value": {"/MediaBox": media, "/Resources": {}}},
                }],
                "pages": [{"object": "2 0 R", "contents": []}],
                "acroform": {"fields": []}, "attachments": {}, "encrypt": {"encrypted": false},
            }))
            .unwrap(),
        )
        .unwrap();
        qpdf
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn check<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == id)
        .unwrap()
}

#[test]
fn every_consumed_expectation_field_is_required_and_type_checked() {
    let scratch = Scratch::new();
    let baseline = expectation();
    for key in baseline
        .as_object()
        .unwrap()
        .keys()
        .filter(|k| *k != "schema")
    {
        let mut changed = baseline.clone();
        changed.as_object_mut().unwrap().remove(key);
        scratch.rejected(&changed.to_string(), key);
        for wrong in [Value::Null, json!({"wrong": true})] {
            changed[key] = wrong;
            scratch.rejected(&changed.to_string(), key);
        }
    }
    for (key, wrong) in [
        ("operation_minimums", json!([])),
        ("operation_minimums", json!({"S": -1})),
        ("operation_minimums", json!({"S": "1"})),
        ("operation_minimums", json!({"S": true})),
        ("required_icc_tags", json!(["rXYZ", 7])),
        ("required_icc_tags", json!("rXYZ")),
        ("action_kinds", json!([false])),
        ("images", json!([{}])),
        (
            "images",
            json!([{"resource": "x", "classification": false}]),
        ),
        ("media_box", json!([0, 0, 150])),
        ("crop_box", json!([0, 0, "150", 90])),
        ("page_count", json!(-1)),
        ("page_count", json!(1.5)),
        ("page_count", json!(true)),
        ("xmp_present", json!(0)),
    ] {
        let mut changed = baseline.clone();
        changed[key] = wrong;
        // Nested image diagnostics identify their missing/invalid member.
        let diagnostic = if key == "images" {
            if changed[key][0].get("resource").is_none() {
                "resource"
            } else {
                "classification"
            }
        } else {
            key
        };
        scratch.rejected(&changed.to_string(), diagnostic);
    }
    scratch.rejected("[]", "object");
    let duplicate = baseline
        .to_string()
        .replacen('{', "{\"operation_minimums\":{},", 1);
    scratch.rejected(&duplicate, "duplicate");
    let nonfinite = baseline.to_string().replace("\"S\":1", "\"S\":1e9999");
    scratch.rejected(&nonfinite, "non-finite");
}

#[test]
fn minimum_checks_keep_input_order_and_python_numeric_spelling() {
    let scratch = Scratch::new();
    let mut expected = expectation();
    expected["page_count"] = json!(1.0);
    expected["operation_minimums"] = json!({});
    // Deliberately unsorted, with an escaped key and Python float formatting.
    let input = expected.to_string().replace(
        "\"operation_minimums\":{}",
        "\"operation_minimums\":{\"re\":1.0,\"\\u006d\":0.5,\"S\":1e-7,\"c\":-0.0}",
    );
    let output = scratch.inspect(&input, "pass", None);
    assert!(output.status.success(), "{:?}", output.stderr);
    let report = scratch.report();
    let checks = report["checks"].as_array().unwrap();
    let start = checks
        .iter()
        .position(|c| c["id"] == "operation-re")
        .unwrap();
    assert_eq!(
        checks[start..start + 4]
            .iter()
            .map(|c| (&c["id"], &c["expected"]))
            .collect::<Vec<_>>(),
        vec![
            (&json!("operation-re"), &json!(">=1.0")),
            (&json!("operation-m"), &json!(">=0.5")),
            (&json!("operation-S"), &json!(">=1e-07")),
            (&json!("operation-c"), &json!(">=-0.0")),
        ]
    );
    assert_eq!(checks[start - 1]["id"], "structural-inventory-sha256");
    assert_eq!(checks[start + 4]["id"], "encrypted");
    expected["operation_minimums"] = json!({"S": 1e20});
    let output = scratch.inspect(&expected.to_string(), "fail", None);
    assert!(output.status.success(), "{:?}", output.stderr);
    let report = scratch.report();
    assert_eq!(check(&report, "operation-S")["passed"], false);
    assert_eq!(check(&report, "operation-S")["expected"], ">=1e+20");
}

#[test]
fn declared_icc_tags_cannot_silently_disappear() {
    let scratch = Scratch::new();
    let mut expected = expectation();
    expected["required_icc_tags"] = json!(["zzzz", "aaaa", "zzzz"]);
    let output = scratch.inspect(&expected.to_string(), "fail", None);
    assert!(output.status.success(), "{:?}", output.stderr);
    let report = scratch.report();
    assert_eq!(
        check(&report, "output-profile-tags")["actual"],
        json!(["aaaa", "zzzz"])
    );
    assert_eq!(check(&report, "output-profile-tags")["passed"], false);
}

#[test]
#[cfg(unix)]
fn equality_compares_integer_float_values_without_rounding_integers() {
    let scratch = Scratch::new();
    for (actual, expected, passed) in [
        ("9007199254740993", "9007199254740992", false),
        ("9007199254740993", "9007199254740992.0", false),
        ("9007199254740992.0", "9007199254740993", false),
        ("9007199254740992", "9007199254740992.0", true),
        ("18446744073709551617", "18446744073709551616", false),
        ("-9007199254740993", "-9007199254740992.0", false),
        ("1000000000000000000000000000000", "1e30", false),
        ("1000000000000000019884624838656", "1e30", true),
        ("1", "1.0", true),
        ("0", "-0.0", true),
        ("1", "1.5", false),
        ("true", "1", true),
        ("false", "0.0", true),
    ] {
        let qpdf = scratch.model(serde_json::from_str(&format!("[0,0,{actual},90]")).unwrap());
        let mut input = expectation();
        input["media_box"] = serde_json::from_str(&format!("[0,0,{expected},90]")).unwrap();
        let output = scratch.inspect(&input.to_string(), "fail", Some(&qpdf));
        assert!(
            output.status.success(),
            "{actual}/{expected}: {:?}",
            output.stderr
        );
        assert_eq!(
            check(&scratch.report(), "media-box")["passed"],
            passed,
            "{actual}/{expected}"
        );
    }
}
