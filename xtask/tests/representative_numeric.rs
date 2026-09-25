//! Numeric contracts through the command boundary, with literal expected tokens.
//! Fixture inputs deliberately bypass JSON serialization so they cannot be rounded
//! or normalized before the command reads them. No renderer is needed.
use serde_json::{json, value::RawValue, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new(fonts: &str, metadata: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "representative-numeric-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("source")).unwrap();
        let elements = fonts
            .split(',')
            .filter(|font| !font.is_empty())
            .enumerate()
            .map(|(i, font)| format!(r#"{{"id":"e{i}","type":"text","fontFamily":{font}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let scene = format!("{{\"type\":\"excalidraw\",\"version\":2,\"elements\":[{elements}],\"appState\":{{}},\"files\":{{}}}}\n");
        fs::write(root.join("source/scene.excalidraw"), &scene).unwrap();
        fs::write(root.join("source/LICENSE"), "MIT fixture\n").unwrap();
        let provenance = json!({
            "identity": "fixture",
            "upstream": {
                "repository": "https://github.com/example/board",
                "commit": "a".repeat(40),
                "path": "scene.excalidraw",
                "permalink": format!("https://github.com/example/board/blob/{}/scene.excalidraw", "a".repeat(40)),
            },
            "license": {"spdx": "MIT", "text": "LICENSE"},
            "scene": {"file": "scene.excalidraw", "bytes": scene.len(), "sha256": format!("{:x}", Sha256::digest(scene.as_bytes()))},
        });
        // Raw insertion also permits invalid JSON and duplicate-member controls.
        let provenance = serde_json::to_string(&provenance).unwrap().replace(
            "\"upstream\":{",
            &format!("\"upstream\":{{\"numeric\":{metadata},"),
        );
        fs::write(root.join("source/PROVENANCE.json"), provenance).unwrap();
        let plan = json!({
            "schema": "excalidraw-render/representative-intake-plan/v1",
            "case_id": "representative.numeric.v1", "source_identity": "fixture",
            "purpose": "Numeric contracts",
            "sanitization": {"operation": "reviewed-no-change", "review_notes": "Authored fixture"},
            "variants": [{"id": "light", "color_mode": "light", "include_background": true, "padding": 10, "scale": 1, "metadata": false}],
            "formats": ["svg", "png", "pdf"],
            "applicability": {"frame_exports": "None", "assets": "None", "sizing": "Scale one"},
            "regions": [{"id": "whole", "class": "typography", "element_ids": []}],
        });
        fs::write(root.join("plan.json"), serde_json::to_vec(&plan).unwrap()).unwrap();
        Self(root)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .arg("representative")
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }

    fn prepare(&self) -> String {
        let output = self.run(&["prepare", "source", "plan.json", "bundle"]);
        assert_success(&output);
        assert_eq!(
            output.stdout,
            b"{\n  \"case_id\": \"representative.numeric.v1\",\n  \"eligible_executions\": [],\n  \"status\": \"pending-review\"\n}\n"
        );
        for name in [
            "scene.excalidraw",
            "LICENSE",
            "PROVENANCE.json",
            "plan.json",
        ] {
            let source = if name == "plan.json" {
                self.0.clone()
            } else {
                self.0.join("source")
            };
            assert_eq!(
                fs::read(self.0.join("bundle").join(name)).unwrap(),
                fs::read(source.join(name)).unwrap(),
                "retained {name} bytes"
            );
        }
        assert_success(&self.run(&["verify", "bundle"]));
        fs::read_to_string(self.0.join("bundle/manifest.json")).unwrap()
    }

    fn reject(&self, diagnostic: &str) {
        let output = self.run(&["prepare", "source", "plan.json", "bundle"]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{:?}",
            output.stderr
        );
        assert!(!self.0.join("bundle").exists());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// Inspect exact emitted tokens without serde's numeric normalization or private
// sentinel interpretation. The expected bytes below do not use the tool encoder.
fn field(raw: &str, path: &[&str]) -> String {
    let Some((key, rest)) = path.split_first() else {
        return raw.to_owned();
    };
    let object: BTreeMap<String, Box<RawValue>> = serde_json::from_str(raw).unwrap();
    field(object[*key].get(), rest)
}

fn assert_census(manifest: &str, live: usize, expected_fonts: &str) {
    let fonts = if expected_fonts.is_empty() {
        "[]".to_owned()
    } else {
        format!(
            "[\n      {}\n    ]",
            expected_fonts
                .split(',')
                .collect::<Vec<_>>()
                .join(",\n      ")
        )
    };
    let kinds = if live == 0 {
        "{}".to_owned()
    } else {
        format!("{{\n      \"text\": {live}\n    }}")
    };
    assert_eq!(field(manifest, &["census"]), format!(
        "{{\n    \"assets\": 0,\n    \"bound_arrows\": 0,\n    \"bound_text\": 0,\n    \"element_types\": {kinds},\n    \"font_families\": {fonts},\n    \"live_elements\": {live}\n  }}"
    ));
}

#[test]
fn integer_metadata_and_binary64_float_spelling_have_known_manifest_bytes() {
    let huge = "9".repeat(400);
    let input = format!("[18446744073709551617,-18446744073709551617,{huge},-{huge},-0,-0.0,0e0,-0e0,1.234567890123456789,1e-4,1e-5,1e15,1e16,1e23,5e-324,-1e-999,1.7976931348623157e308]");
    let manifest = Fixture::new("", &input).prepare();
    assert_eq!(field(&manifest, &["source", "numeric"]), format!(
        "[\n      18446744073709551617,\n      -18446744073709551617,\n      {huge},\n      -{huge},\n      0,\n      -0.0,\n      0.0,\n      -0.0,\n      1.2345678901234567,\n      0.0001,\n      1e-05,\n      1000000000000000.0,\n      1e+16,\n      1e+23,\n      5e-324,\n      -0.0,\n      1.7976931348623157e+308\n    ]"
    ));
    assert_census(&manifest, 0, "");
}

#[test]
fn census_deduplicates_and_orders_numbers_without_rounding_integers() {
    // Numerically equal values keep the first token's integer/float kind.
    // 1e23 is exactly 99999999999999991611392 in binary64, not 10^23.
    for (fonts, live, expected) in [
        ("1,1.0,2.0,2,-0,-0.0", 6, "0,1,2.0"),
        ("1.0,1,-0.0,-0,2,2.0", 6, "-0.0,1.0,2"),
        ("1e23,99999999999999991611392,100000000000000000000000,-1e23,-99999999999999991611392,-100000000000000000000000", 6,
         "-100000000000000000000000,-1e+23,1e+23,100000000000000000000000"),
        ("9007199254740993,9007199254740992.0,9007199254740992,18446744073709551617,18446744073709551616.0,-9007199254740993,-9007199254740992.0,-1.5,-1,-0.5,0,0.5,1.5", 13,
         "-9007199254740993,-9007199254740992.0,-1.5,-1,-0.5,0,0.5,1.5,9007199254740992.0,9007199254740993,1.8446744073709552e+19,18446744073709551617"),
    ] {
        assert_census(&Fixture::new(fonts, "null").prepare(), live, expected);
    }
    let huge = "9".repeat(400);
    assert_census(
        &Fixture::new(&format!("{huge},1e308,-{huge},-1e308"), "null").prepare(),
        4,
        &format!("-{huge},-1e+308,1e+308,{huge}"),
    );
}

#[test]
fn duplicate_members_including_serde_private_sentinels_are_rejected() {
    for metadata in [
        r#"{"nested":{"x":1,"x":2}}"#,
        r#"{"$serde_json::private::Number":"1","$serde_json::private::Number":"2"}"#,
        r#"{"$serde_json::private::RawValue":"1","$serde_json::private::RawValue":"2"}"#,
    ] {
        Fixture::new("", metadata).reject("duplicate JSON member");
    }
}

#[test]
fn serde_private_sentinels_remain_literal_user_objects() {
    for (metadata, expected) in [
        (r#"{"$serde_json::private::Number":"123"}"#, "{\n      \"$serde_json::private::Number\": \"123\"\n    }"),
        (r#"{"$serde_json::private::RawValue":"456"}"#, "{\n      \"$serde_json::private::RawValue\": \"456\"\n    }"),
        (r#"{"$serde_json::private::Number":"123","$serde_json::private::RawValue":"456"}"#, "{\n      \"$serde_json::private::Number\": \"123\",\n      \"$serde_json::private::RawValue\": \"456\"\n    }"),
    ] {
        let manifest = Fixture::new("", metadata).prepare();
        assert_eq!(field(&manifest, &["source", "numeric"]), expected);
        assert_census(&manifest, 0, "");
    }
}

#[test]
fn non_finite_numeric_metadata_is_rejected_without_publication() {
    for metadata in ["NaN", "Infinity", "-Infinity", "1e999", "-1e999"] {
        Fixture::new("", metadata).reject("invalid JSON");
    }
}

#[test]
fn huge_integer_truthiness_excludes_deleted_elements_from_census() {
    let f = Fixture::new("1", "null");
    let scene_path = f.0.join("source/scene.excalidraw");
    let scene = fs::read_to_string(&scene_path).unwrap().replace(
        "\"type\":\"text\"",
        &format!("\"type\":\"text\",\"isDeleted\":{}", "9".repeat(400)),
    );
    fs::write(scene_path, &scene).unwrap();
    let provenance_path = f.0.join("source/PROVENANCE.json");
    let mut provenance: Value =
        serde_json::from_slice(&fs::read(&provenance_path).unwrap()).unwrap();
    provenance["scene"]["bytes"] = json!(scene.len());
    provenance["scene"]["sha256"] = json!(format!("{:x}", Sha256::digest(scene.as_bytes())));
    fs::write(provenance_path, serde_json::to_vec(&provenance).unwrap()).unwrap();
    assert_census(&f.prepare(), 0, "");
}

#[test]
fn missing_provenance_identity_cannot_match_null_source_identity() {
    let f = Fixture::new("", "null");
    let path = f.0.join("source/PROVENANCE.json");
    let mut provenance: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    provenance.as_object_mut().unwrap().remove("identity");
    fs::write(path, serde_json::to_vec(&provenance).unwrap()).unwrap();
    let path = f.0.join("plan.json");
    let mut plan: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    plan["source_identity"] = Value::Null;
    fs::write(path, serde_json::to_vec(&plan).unwrap()).unwrap();
    f.reject("source identity mismatch");
}
