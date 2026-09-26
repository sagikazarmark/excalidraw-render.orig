//! Intake contracts through the command boundary. Capture tests opt into an
//! actual separately built renderer with REPRESENTATIVE_RENDERER.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture {
    root: PathBuf,
    scene: Vec<u8>,
    plan: Value,
    provenance: Value,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn write_json(path: impl AsRef<Path>, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "rust-intake-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("source")).unwrap();
        let scene = b"{\"type\":\"excalidraw\",\"version\":2,\"elements\":[],\"appState\":{},\"files\":{}}\n".to_vec();
        let provenance = json!({"identity":"fixture", "upstream":{"repository":"https://github.com/example/board","commit":"a".repeat(40),"path":"scene.excalidraw","permalink":format!("https://github.com/example/board/blob/{}/scene.excalidraw","a".repeat(40))},"license":{"spdx":"MIT","text":"LICENSE"},"scene":{"file":"scene.excalidraw","bytes":scene.len(),"sha256":hash(&scene)}});
        let plan = json!({"schema":"excalidraw-render/representative-intake-plan/v1","case_id":"representative.fixture.v1","source_identity":"fixture","purpose":"Command-boundary regression fixture", "sanitization":{"operation":"reviewed-no-change","review_notes":"Authored fixture"},"variants":[{"id":"light","color_mode":"light","include_background":true,"padding":10,"scale":1,"metadata":true}],"formats":["svg","png","pdf"],"applicability":{"frame_exports":"No frames","assets":"No assets","sizing":"Scale one"},"regions":[{"id":"whole","class":"mixed-representative","element_ids":[]}]});
        let fixture = Self {
            root,
            scene,
            plan,
            provenance,
        };
        fixture.save();
        fixture
    }
    fn save(&self) {
        fs::write(self.root.join("source/scene.excalidraw"), &self.scene).unwrap();
        fs::write(
            self.root.join("source/LICENSE"),
            "MIT fixture attribution\n",
        )
        .unwrap();
        write_json(self.root.join("source/PROVENANCE.json"), &self.provenance);
        write_json(self.root.join("plan.json"), &self.plan);
    }
    fn run(&self, args: &[&str], success: bool) -> Output {
        let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("representative")
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            success,
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }
    fn prepare(&self, success: bool) {
        self.run(&["prepare", "source", "plan.json", "bundle"], success);
    }
    fn json(&self, file: &str) -> Value {
        serde_json::from_slice(&fs::read(self.root.join(file)).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn prepare_preserves_bytes_and_cannot_confer_approval_or_overwrite() {
    let f = Fixture::new();
    f.prepare(true);
    assert_eq!(
        fs::read(f.root.join("bundle/scene.excalidraw")).unwrap(),
        f.scene
    );
    let before = fs::read(f.root.join("bundle/manifest.json")).unwrap();
    let manifest = f.json("bundle/manifest.json");
    assert_eq!(manifest["status"], "pending-review");
    assert!(manifest["approval"].is_null());
    assert_eq!(manifest["eligible_executions"], json!([]));
    assert_eq!(manifest["proposed_executions"].as_array().unwrap().len(), 3);
    f.run(&["verify", "bundle"], true);
    f.prepare(false);
    assert_eq!(
        fs::read(f.root.join("bundle/manifest.json")).unwrap(),
        before
    );
    let mut forged = manifest;
    forged["approval"] = json!({"approved_by":"invented"});
    write_json(f.root.join("bundle/manifest.json"), &forged);
    f.run(&["verify", "bundle"], true);
    fs::write(f.root.join("bundle/manifest.json"), before).unwrap();
    fs::write(
        f.root.join("bundle/scene.excalidraw"),
        [f.scene.as_slice(), b" "].concat(),
    )
    .unwrap();
    f.run(&["verify", "bundle"], false);
}

#[test]
fn invalid_plans_provenance_and_duplicate_json_never_create_output() {
    for mutation in [
        json!({"formats":["svg","png"]}),
        json!({"approval":true}),
        json!({"variants":[]}),
        json!({"variants":[{"id":"bad"}]}),
        json!({"regions":[{"id":"whole","class":"typography","element_ids":["missing"]}]}),
    ] {
        let mut f = Fixture::new();
        for (key, value) in mutation.as_object().unwrap() {
            f.plan[key] = value.clone();
        }
        f.save();
        f.prepare(false);
        assert!(!f.root.join("bundle").exists());
    }
    let mut f = Fixture::new();
    f.provenance["scene"]["sha256"] = json!("0".repeat(64));
    f.save();
    f.prepare(false);
    assert!(!f.root.join("bundle").exists());
    let f = Fixture::new();
    let bytes =
        serde_json::to_string(&f.plan)
            .unwrap()
            .replacen('{', "{\"schema\":\"duplicate\",", 1);
    fs::write(f.root.join("plan.json"), bytes).unwrap();
    f.prepare(false);
    assert!(!f.root.join("bundle").exists());
}

#[test]
fn index_repair_preserves_escaped_strings_unicode_and_number_spelling_across_key_rollover() {
    let mut f = Fixture::new();
    let elements = (0..64)
        .map(|i| {
            format!("{{\"id\":\"é{i}\",\"type\":\"rectangle\",\"x\":1.10,\"custom\":\"{{\\\"}}\"}}")
        })
        .collect::<Vec<_>>();
    f.scene = format!("{{\"custom\":{{\"elements\":[1]}},\"type\":\"excalidraw\",\"version\":2,\"elements\":[{}],\"appState\":{{}},\"files\":{{}}}}\n",elements.join(", ")).into_bytes();
    f.provenance["scene"]["bytes"] = json!(f.scene.len());
    f.provenance["scene"]["sha256"] = json!(hash(&f.scene));
    f.plan["sanitization"]["operation"] = json!("reviewed-index-repair");
    f.save();
    f.prepare(true);
    let retained = fs::read(f.root.join("bundle/scene.excalidraw")).unwrap();
    let text = String::from_utf8(retained.clone()).unwrap();
    assert!(text.contains("\"index\":\"a0\""));
    assert!(text.contains("\"index\":\"az\""));
    assert!(text.contains("\"index\":\"b00\""));
    assert!(text.contains("\"index\":\"b01\""));
    f.run(&["verify", "bundle"], true);
    fs::write(
        f.root.join("bundle/scene.excalidraw"),
        text.replace("1.10", "1.1"),
    )
    .unwrap();
    f.run(&["verify", "bundle"], false);
    fs::remove_dir_all(f.root.join("bundle")).unwrap();
    f.scene = retained;
    f.provenance["scene"]["bytes"] = json!(f.scene.len());
    f.provenance["scene"]["sha256"] = json!(hash(&f.scene));
    f.save();
    f.prepare(false);
    assert!(!f.root.join("bundle").exists());
}

#[test]
fn legacy_capture_verification_uses_observations_not_retired_python_source() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let f = Fixture::new();
    let bundle = repo.join("tests/conformance/intake/velox-design-v1");
    let source = repo.join("tests/conformance/proposals/velox84-evidence-v1/native");
    let output = f.root.join("legacy");
    fs::create_dir(&output).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let e = entry.unwrap();
        fs::copy(e.path(), output.join(e.file_name())).unwrap();
    }
    let path = output.join("capture.json");
    let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["tool"]["sha256"] = json!("0".repeat(64));
    write_json(&path, &record);
    f.run(
        &[
            "verify-capture",
            bundle.to_str().unwrap(),
            output.to_str().unwrap(),
        ],
        true,
    );
    fs::write(output.join("light-svg.svg"), b"changed").unwrap();
    f.run(
        &[
            "verify-capture",
            bundle.to_str().unwrap(),
            output.to_str().unwrap(),
        ],
        false,
    );
}

#[test]
fn retained_intake_bundles_verify_byte_for_byte() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let f = Fixture::new();
    for bundle in fs::read_dir(repo.join("tests/conformance/intake")).unwrap() {
        let path = bundle.unwrap().path();
        if path.join("manifest.json").is_file() {
            f.run(&["verify", path.to_str().unwrap()], true);
        }
    }
}

#[cfg(unix)]
#[test]
fn symlinks_extra_files_and_invalid_timeout_are_rejected() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.prepare(true);
    fs::write(f.root.join("bundle/extra"), b"extra").unwrap();
    f.run(&["verify", "bundle"], false);
    fs::remove_file(f.root.join("bundle/extra")).unwrap();
    fs::remove_file(f.root.join("bundle/LICENSE")).unwrap();
    symlink(f.root.join("source/LICENSE"), f.root.join("bundle/LICENSE")).unwrap();
    f.run(&["verify", "bundle"], false);
    fs::remove_file(f.root.join("bundle/LICENSE")).unwrap();
    fs::copy(f.root.join("source/LICENSE"), f.root.join("bundle/LICENSE")).unwrap();
    for timeout in ["NaN", "0", "-1", "1801"] {
        let output = f.run(
            &[
                "capture",
                "bundle",
                "missing-renderer",
                "capture",
                "--timeout",
                timeout,
            ],
            false,
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid process timeout"));
    }
    assert!(!f.root.join("capture").exists());
}

#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to a freshly built CLI"]
fn real_capture_repeats_and_rejects_resealed_report_tampering() {
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").expect("set REPRESENTATIVE_RENDERER");
    let f = Fixture::new();
    f.prepare(true);
    f.run(&["capture", "bundle", &renderer, "capture"], true);
    f.run(&["verify-capture", "bundle", "capture"], true);
    let record = f.json("capture/capture.json");
    assert_eq!(
        record["schema"],
        "excalidraw-render/representative-intake-capture/v2"
    );
    assert!(record["approval"].is_null());
    assert_eq!(record["rows"].as_array().unwrap().len(), 3);
    assert_eq!(
        record["tool"]["implementation"],
        "cargo-xtask-representative"
    );
    let original = f.json("capture/light-png-report.json");
    for mutation in [
        json!({"schema_version":true}),
        json!({"format":"svg"}),
        json!({"diagnostics":[{"code":"color.invalid","severity":"warning","format":"png","effects":["visual","visual"],"element_id":null,"json_pointer":"","message":"observed","details":null}]}),
    ] {
        let mut report = original.clone();
        for (k, v) in mutation.as_object().unwrap() {
            report[k] = v.clone();
        }
        let bytes = serde_json::to_vec(&report).unwrap();
        fs::write(f.root.join("capture/light-png-report.json"), &bytes).unwrap();
        let mut resealed = record.clone();
        resealed["files"]["light-png-report.json"] =
            json!({"bytes":bytes.len(),"sha256":hash(&bytes)});
        write_json(f.root.join("capture/capture.json"), &resealed);
        f.run(&["verify-capture", "bundle", "capture"], false);
    }
}

#[cfg(unix)]
#[test]
fn capture_timeout_and_failure_terminate_wrapper_descendants() {
    use std::os::unix::fs::PermissionsExt;
    for (ending, expected) in [("wait", "process timeout"), ("exit 1", "render failed")] {
        let f = Fixture::new();
        f.prepare(true);
        fs::write(
            f.root.join("renderer"),
            format!("#!/bin/sh\n(sleep 1; printf leaked > descendant-finished) &\n{ending}\n"),
        )
        .unwrap();
        fs::set_permissions(f.root.join("renderer"), fs::Permissions::from_mode(0o755)).unwrap();
        let output = f.run(
            &[
                "capture",
                "bundle",
                "renderer",
                "capture",
                "--timeout",
                "0.2",
            ],
            false,
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        std::thread::sleep(std::time::Duration::from_millis(1200));
        assert!(!f.root.join("descendant-finished").exists());
        assert!(!f.root.join("capture").exists());
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires REPRESENTATIVE_RENDERER pointing to a freshly built CLI"]
fn real_process_order_drift_failure_and_timeout_leave_no_capture() {
    use std::os::unix::fs::PermissionsExt;
    let renderer = std::env::var("REPRESENTATIVE_RENDERER").expect("set REPRESENTATIVE_RENDERER");
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let f = Fixture::new();
    f.prepare(true);
    for (mode, fault) in [
        ("observe", ""),
        (
            "drift",
            "if [ \"$count\" -ge 3 ]; then printf ' ' >> \"$report\"; fi",
        ),
        ("failure", "exit 1"),
        ("timeout", "exec sleep 5"),
    ] {
        let _ = fs::remove_file(f.root.join("calls"));
        let script = format!("#!/bin/sh\ncount=0\nif [ -f calls ]; then count=$(wc -l < calls); fi\nprevious=''\nfor arg do\n if [ \"$previous\" = '--format' ]; then printf '%s\\n' \"$arg\" >> calls; fi\n if [ \"$previous\" = '--report-output' ]; then report=$arg; fi\n previous=$arg\ndone\n{} \"$@\" || exit $?\n{fault}\n",quote(&renderer));
        fs::write(f.root.join("renderer"), script).unwrap();
        fs::set_permissions(f.root.join("renderer"), fs::Permissions::from_mode(0o755)).unwrap();
        f.run(
            &[
                "capture",
                "bundle",
                "renderer",
                mode,
                "--timeout",
                if mode == "timeout" { "0.5" } else { "30" },
            ],
            mode == "observe",
        );
        if mode == "observe" {
            assert_eq!(
                fs::read_to_string(f.root.join("calls")).unwrap(),
                "svg\npng\npdf\npdf\npng\nsvg\n"
            );
        } else {
            assert!(!f.root.join(mode).exists());
        }
    }
}
