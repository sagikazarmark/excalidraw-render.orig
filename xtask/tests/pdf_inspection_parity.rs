//! Protocol regressions and retained report bytes at the actual xtask CLI.
//! Scratch qpdf models exercise malformed and boundary inputs independently.

use base64::Engine;
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

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pdf-inspection-parity-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn inspect(&self, qpdf: &Path, input: &Path, expectation: &Path, pass: bool) -> Output {
        let output = self.0.join("report.json");
        let _ = fs::remove_file(&output);
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["pdf-harness", "inspect"])
            .arg(input)
            .arg(expectation)
            .arg(if pass { "pass" } else { "fail" })
            .arg(output)
            .env("QPDF", qpdf)
            .current_dir(root())
            .output()
            .unwrap()
    }

    #[cfg(unix)]
    fn model(&self, source: &Value) -> Output {
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
            serde_json::to_vec(source).unwrap(),
        )
        .unwrap();
        let input = self.0.join("input.pdf");
        fs::write(&input, b"%PDF-1.7\nstartxref\n0\n%%EOF\n").unwrap();
        self.inspect(
            &qpdf,
            &input,
            &root().join(
                "tests/conformance/pdf-backend-qualification/project-operations-structure.json",
            ),
            false,
        )
    }

    fn report(&self) -> Value {
        serde_json::from_slice(&fs::read(self.0.join("report.json")).unwrap()).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stream(data: &[u8], dictionary: Value) -> Value {
    json!({"stream": {"dict": dictionary, "data": base64::engine::general_purpose::STANDARD.encode(data)}})
}

fn model() -> Value {
    json!({
        "qpdf": [{"pdfversion": "1.7"}, {
            "trailer": {"value": {"/Root": "1 0 R"}},
            "obj:1 0 R": {"value": {"/Type": "/Catalog"}},
            "obj:2 0 R": {"value": {"/MediaBox": [0, 0, 100, 100], "/Resources": {}}},
            "obj:3 0 R": stream(b"", json!({})),
        }],
        "pages": [{"object": "2 0 R", "contents": ["3 0 R"]}],
        "acroform": {"fields": []}, "attachments": {}, "encrypt": {"encrypted": false},
    })
}

#[test]
#[cfg(unix)]
fn fatal_parser_errors_keep_diagnostics_in_both_lists() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let qpdf = scratch.0.join("qpdf");
    fs::write(&qpdf, "#!/bin/sh\ncase \"$1\" in\n--version) printf 'qpdf version 12.3.2\\n';;\n--check) printf 'bad pdf\\n' >&2; exit 2;;\n*) printf 'cannot parse\\n' >&2; exit 2;;\nesac\n").unwrap();
    fs::set_permissions(&qpdf, fs::Permissions::from_mode(0o755)).unwrap();
    let input = scratch.0.join("input.pdf");
    fs::write(&input, b"not pdf").unwrap();
    let expectation = root().join(
        "tests/conformance/pdf-viewer-qualification/rectangle-outline/expected-structure.json",
    );
    let result = scratch.inspect(&qpdf, &input, &expectation, false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report = scratch.report();
    assert_eq!(
        report["errors"],
        json!([
            "bad pdf",
            "cannot parse",
            "qpdf JSON parse failed: Expecting value: line 1 column 1 (char 0)"
        ])
    );
    assert_eq!(report["parser"]["warnings"], report["errors"]);
    assert_eq!(report["passed"], false);
}

#[test]
#[cfg(unix)]
fn ascii_del_is_escaped_in_report_bytes_and_operation_hashes() {
    let scratch = Scratch::new();
    let mut source = model();
    source["qpdf"][1]["obj:3 0 R"] = stream(b"BT (\x7f) Tj ET", json!({}));
    assert!(scratch.model(&source).status.success());
    let bytes = fs::read(scratch.0.join("report.json")).unwrap();
    assert!(!bytes.contains(&127));
    assert!(String::from_utf8(bytes).unwrap().contains("\\u007f"));
}

#[test]
#[cfg(unix)]
fn uri_actions_retain_urlsplit_validation_and_scheme_classification() {
    let scratch = Scratch::new();
    for uri in [
        "http://[",
        "http://[xyz]",
        "http://[127.0.0.1]",
        "http://prefix[::1]",
        "http://[::1]suffix",
        "http://[v1.]",
        "http://[::1%]",
        "https://example.org：80",
        "https://example.org℀host",
        "//example.org＠host",
    ] {
        let mut source = model();
        source["qpdf"][1]["obj:1 0 R"]["value"]["/OpenAction"] =
            json!({"/S": "/URI", "/URI": format!("u:{uri}")});
        let output = scratch.model(&source);
        assert!(!output.status.success(), "accepted malformed URI {uri}");
        assert!(!scratch.0.join("report.json").exists(), "{uri}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("panicked"),
            "{uri}"
        );
    }
    for (uri, classification) in [
        ("\0 \nHtTpS://example.org", "uri"),
        ("http://[::1]", "uri"),
        ("http://[fe80::1%25eth0]:invalid-port", "uri"),
        ("http://[v1.xyz]", "uri"),
        ("https://example.org/＠path", "uri"),
        ("mailto:a@example.org", "uri"),
        ("../relative/path", "uri"),
        ("java\tscript:alert(1)", "prohibited"),
        ("file:///tmp/x", "prohibited"),
    ] {
        let mut source = model();
        source["qpdf"][1]["obj:1 0 R"]["value"]["/OpenAction"] =
            json!({"/S": "/URI", "/URI": format!("u:{uri}")});
        let output = scratch.model(&source);
        assert!(output.status.success(), "{uri}: {:?}", output.stderr);
        let action = &scratch.report()["document"]["actions"][0];
        assert_eq!(action["classification"], classification, "{uri}");
        assert_eq!(action["details"]["URI"], uri, "URI must remain unmodified");
    }
}

#[test]
#[cfg(unix)]
fn inline_images_and_icc_preserve_types_bytes_and_truncated_tag_tables() {
    let scratch = Scratch::new();
    let mut source = model();
    let objects = &mut source["qpdf"][1];
    objects["obj:3 0 R"] = stream(
        b"BI /W +000123 /H -0 /BPC 1_6 /IM false /I true ID\r\n\xff\r\nEI Q",
        json!({}),
    );
    let mut icc = vec![0; 150];
    icc[..4].copy_from_slice(&150_u32.to_be_bytes());
    icc[8..10].copy_from_slice(&[2, 0x1f]);
    icc[12..16].copy_from_slice(b"mntr");
    icc[16..20].copy_from_slice(b"RGB\x1c");
    icc[20..24].copy_from_slice(b"XYZ ");
    icc[36..40].copy_from_slice(b"acsp");
    icc[128..132].copy_from_slice(&u32::MAX.to_be_bytes());
    icc[132..136].copy_from_slice(&[b'r', 0xc3, 0xa9, 0xff]);
    objects["obj:4 0 R"] = stream(&icc, json!({}));
    objects["obj:1 0 R"]["value"]["/OutputIntents"] = json!([{ "/DestOutputProfile": "4 0 R" }]);
    let output = scratch.model(&source);
    assert!(output.status.success(), "{:?}", output.stderr);
    let report = scratch.report();
    let image = &report["pages"][0]["images"][0];
    assert_eq!(image["width"], 123);
    assert_eq!(image["height"], 0);
    assert_eq!(image["bits_per_component"], 16);
    assert_eq!(image["mask"], "false");
    assert_eq!(image["interpolate"], true);
    assert_eq!(image["byte_length"], 3);
    assert_eq!(image["resource"], "page/inline-image-1");
    assert!(image["object"].is_null());
    let profile = &report["document"]["output_intents"][0]["profile"];
    assert_eq!(profile["tags"], json!(["r���"]));
    assert_eq!(profile["color_space"], "RGB");
    assert_eq!(profile["version"], "2.1");
    assert_eq!(profile["valid_header"], true);
}

#[test]
#[cfg(unix)]
fn action_cycles_are_path_local_and_deep_chains_fail_without_panicking() {
    let scratch = Scratch::new();
    let mut source = model();
    source["qpdf"][1]["obj:1 0 R"]["value"]["/OpenAction"] = json!(["4 0 R", "4 0 R"]);
    source["qpdf"][1]["obj:4 0 R"] =
        json!({"value": {"/S": "/URI", "/URI": "u:https://example.org", "/Next": "4 0 R"}});
    assert!(scratch.model(&source).status.success());
    let report = scratch.report();
    let actions = report["document"]["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 4);
    for (index, location) in [
        (1, "catalog/OpenAction[0]/Next"),
        (3, "catalog/OpenAction[1]/Next"),
    ] {
        assert_eq!(actions[index]["kind"], "cycle");
        assert_eq!(actions[index]["location"], location);
    }
    for index in 4..145 {
        source["qpdf"][1][format!("obj:{index} 0 R")] = json!({"value": {
            "/S": "/URI", "/URI": "u:https://example.org", "/Next": format!("{} 0 R", index + 1),
        }});
    }
    let output = scratch.model(&source);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("actions exceed inventory depth limit")
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
}

// Reconstruct the expectation inputs preserved in a retained report. This
// checks the complete report, including historically failed checks, rather than
// incorrectly applying today's expectation to an older renderer artifact.
fn expectation_from_report(report: &Value) -> Value {
    let mut expectation = json!({"operation_minimums": {}, "required_icc_tags": ["rXYZ", "gXYZ", "bXYZ", "wtpt", "rTRC", "gTRC", "bTRC"]});
    for check in report["checks"].as_array().unwrap() {
        let id = check["id"].as_str().unwrap();
        let key = match id {
            "operation-sequence-sha256" => "operation_sha256".to_owned(),
            "structural-inventory-sha256" => "inventory_sha256".to_owned(),
            "image-inventory" => "images".to_owned(),
            id if id.starts_with("operation-") && id != "operation-count" => {
                expectation["operation_minimums"][&id[10..]] = json!(check["expected"]
                    .as_str()
                    .unwrap()
                    .strip_prefix(">=")
                    .unwrap()
                    .parse::<u64>()
                    .unwrap());
                continue;
            }
            _ => id.replace('-', "_"),
        };
        expectation[key] = check["expected"].clone();
    }
    expectation
}

#[test]
fn backend_requalification_reports_retain_exact_bytes() {
    let scratch = Scratch::new();
    let root = root();
    let qpdf = std::env::var_os("QPDF")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/pdf-harness/qpdf/bin/qpdf"));
    for version in ["v1", "v2"] {
        let directory = root.join(format!(
            "tests/conformance/candidates/stage4-pdf-backend-requalification-{version}"
        ));
        for role in ["approved", "candidate"] {
            for repeat in ["", "-repeat"] {
                let retained =
                    fs::read(directory.join(format!("{role}-structure{repeat}.json"))).unwrap();
                let report: Value = serde_json::from_slice(&retained).unwrap();
                let expectation = scratch.0.join("expectation.json");
                fs::write(
                    &expectation,
                    serde_json::to_vec(&expectation_from_report(&report)).unwrap(),
                )
                .unwrap();
                let output = scratch.inspect(
                    &qpdf,
                    &directory.join(format!("{role}{repeat}.pdf")),
                    &expectation,
                    report["expected_pass"].as_bool().unwrap(),
                );
                assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
                let actual = fs::read(scratch.0.join("report.json")).unwrap();
                assert!(
                    actual == retained,
                    "{version}/{role}{repeat} report bytes differ"
                );
            }
        }
    }
}
