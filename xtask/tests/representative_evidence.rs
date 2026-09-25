//! Evidence collection boundary tests. Full viewer runs are opt-in and require
//! the installed pinned harnesses and a freshly built native renderer.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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
        let parent = root().join("target");
        fs::create_dir_all(&parent).unwrap();
        let path = parent.join(format!(
            "evidence-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(args: &[&str], success: bool) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["representative"])
        .args(args)
        .current_dir(root())
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        success,
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(if success {
        output.stdout
    } else {
        output.stderr
    })
    .unwrap()
}
fn write(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

#[test]
fn invalid_evidence_inputs_fail_without_creating_output() {
    let scratch = Scratch::new();
    let output = scratch.0.join("viewers");
    run(
        &[
            "evidence",
            "collect",
            "missing-bundle",
            "missing-reference",
            "missing-native",
            output.to_str().unwrap(),
        ],
        false,
    );
    assert!(!output.exists());
    run(&["case", "verify-registry", "--viewers"], false);
}

#[test]
#[ignore = "requires pinned Docker SVG/PDF viewers, Rust viewer font helper, and REPRESENTATIVE_RENDERER"]
fn collection_rederives_assets_crops_receipts_and_measurements() {
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").unwrap();
    let scratch = Scratch::new();
    let native = scratch.0.join("native");
    let viewers = scratch.0.join("viewers");
    let bundle = "tests/conformance/intake/velox-design-v2";
    let reference = "tests/conformance/cases/representative/velox-design/reference";
    run(
        &["capture", bundle, &renderer, native.to_str().unwrap()],
        true,
    );
    let commands = |mode| {
        vec![
            "evidence",
            mode,
            bundle,
            reference,
            native.to_str().unwrap(),
            viewers.to_str().unwrap(),
        ]
    };
    run(&commands("collect"), true);
    run(&commands("verify"), true);
    let record_path = viewers.join("viewers.json");
    let bytes = fs::read(&record_path).unwrap();
    let original: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        original["schema"],
        "excalidraw-render/representative-intake-viewers/v2"
    );
    assert_eq!(original["captures"].as_array().unwrap().len(), 12);
    assert_eq!(original["judgments"].as_array().unwrap().len(), 10);
    for mutation in 0..6 {
        let mut record = original.clone();
        match mutation {
            0 => record["sources"] = json!([]),
            1 => record["inputs"][0]["sha256"] = json!("0".repeat(64)),
            2 => record["captures"][0]["runs"][0]["raw_capture"]["sha256"] = json!("0".repeat(64)),
            3 => record["judgments"][0]["result"]["passed"] = json!(true),
            4 => record["profiles"][0]["sha256"] = json!("0".repeat(64)),
            _ => record["viewport_adapter"] = json!("resize to fit"),
        }
        if record == original {
            record["judgments"][0]["result"]["passed"] = json!(false);
        }
        write(&record_path, &record);
        run(&commands("verify"), false);
    }
    for (file, replacement, expected) in [
        ("light/regions.json", None, "crop plan detached"),
        (
            "light/official-rust.svg",
            Some(b"changed".to_vec()),
            "derived viewer resource differs",
        ),
    ] {
        let path = viewers.join(file);
        let saved = fs::read(&path).unwrap();
        let changed = replacement.unwrap_or_else(|| {
            let mut region: Value = serde_json::from_slice(&saved).unwrap();
            region["crops"][0]["x"] = json!(0);
            region["crops"][0]["width"] = json!(11);
            serde_json::to_vec(&region).unwrap()
        });
        fs::write(&path, &changed).unwrap();
        let mut record = original.clone();
        record["files"][file] =
            json!({"bytes":changed.len(),"sha256":format!("{:x}",Sha256::digest(&changed))});
        write(&record_path, &record);
        assert!(run(&commands("verify"), false).contains(expected));
        fs::write(path, saved).unwrap();
    }
    fs::write(&record_path, &bytes).unwrap();
    let compact = scratch.0.join("compact");
    run(
        &[
            "compact",
            "retain",
            bundle,
            reference,
            native.to_str().unwrap(),
            viewers.to_str().unwrap(),
            compact.to_str().unwrap(),
        ],
        true,
    );
    run(&["compact", "verify", compact.to_str().unwrap()], true);
    run(&["compact", "check", compact.to_str().unwrap()], true);
    run(
        &[
            "compact",
            "retain",
            bundle,
            reference,
            native.to_str().unwrap(),
            viewers.to_str().unwrap(),
            compact.to_str().unwrap(),
        ],
        false,
    );
    // A modified historical tool identity is allowed across fresh captures;
    // changed observed bytes are not. Reseal only the capture receipt binding.
    let capture_path = native.join("capture.json");
    let capture_bytes = fs::read(&capture_path).unwrap();
    let mut capture: Value = serde_json::from_slice(&capture_bytes).unwrap();
    capture["renderer_binary"]["sha256"] = json!("0".repeat(64));
    write(&capture_path, &capture);
    let changed = fs::read(&capture_path).unwrap();
    let mut viewer = original.clone();
    let binding = json!({"path":original["native"]["path"],"bytes":changed.len(),"sha256":format!("{:x}",Sha256::digest(&changed))});
    viewer["native"] = binding.clone();
    viewer["inputs"][4] = binding;
    viewer["producer"]["executable"]["sha256"] = json!("1".repeat(64));
    viewer["sources"][0]["sha256"] = json!("2".repeat(64));
    for c in viewer["captures"].as_array_mut().unwrap() {
        if c["viewer"].as_str().unwrap().ends_with("-chromium") {
            for (index, receipt) in c["runs"].as_array_mut().unwrap().iter_mut().enumerate() {
                receipt["browser_process_id"] = json!(100 + index);
                receipt["container_namespace"] = json!(format!("new-process-{index}"));
                receipt["virtual_time_ticks_base"] = json!(200 + index);
            }
        }
    }
    write(&record_path, &viewer);
    run(&["compact", "check", compact.to_str().unwrap()], true);
    fs::write(capture_path, capture_bytes).unwrap();
    fs::write(&record_path, &bytes).unwrap();
    // A fully self-consistent fresh collection may include a new observed file;
    // compact replay must still reject a changed non-receipt inventory.
    let extra = viewers.join("new-observation.json");
    fs::write(&extra, b"{}").unwrap();
    let mut changed = original.clone();
    changed["files"]["new-observation.json"] =
        json!({"bytes":2,"sha256":format!("{:x}",Sha256::digest(b"{}"))});
    write(&record_path, &changed);
    run(&commands("verify"), true);
    assert!(run(&["compact", "check", compact.to_str().unwrap()], false)
        .contains("fresh viewers collection differs"));
    fs::remove_file(extra).unwrap();
    // The glyph count is a valid but changed observation, not process identity.
    let mut changed = original.clone();
    let fonts = changed["captures"][0]["runs"][0]["platform_fonts"]
        .as_array_mut()
        .unwrap();
    fonts[0]["glyphCount"] = json!(fonts[0]["glyphCount"].as_u64().unwrap() + 1);
    write(&record_path, &changed);
    run(&commands("verify"), true);
    assert!(run(&["compact", "check", compact.to_str().unwrap()], false)
        .contains("fresh viewers receipt differs"));
    // Invalid full evidence must not leave a retained directory.
    changed["captures"] = json!([]);
    write(&record_path, &changed);
    let rejected = scratch.0.join("rejected-compact");
    run(
        &[
            "compact",
            "retain",
            bundle,
            reference,
            native.to_str().unwrap(),
            viewers.to_str().unwrap(),
            rejected.to_str().unwrap(),
        ],
        false,
    );
    assert!(!rejected.exists());
    fs::write(&record_path, &bytes).unwrap();
    run(&commands("collect"), false);
    assert_eq!(fs::read(record_path).unwrap(), bytes);
}

#[test]
#[ignore = "requires pinned viewers and REPRESENTATIVE_RENDERER; exercises multi-face assets and observed font substitution"]
fn registered_cases_pass_all_viewers_with_rust_collection() {
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").unwrap();
    let output = run(
        &[
            "case",
            "verify-registry",
            "--renderer",
            &renderer,
            "--viewers",
        ],
        true,
    );
    let registry: Value = serde_json::from_slice(
        &fs::read(root().join("tests/conformance/cases/representative/registry.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        output.lines().count(),
        registry["cases"].as_array().unwrap().len()
    );
    for line in output.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        assert!(value["viewers"].is_string());
    }
}
