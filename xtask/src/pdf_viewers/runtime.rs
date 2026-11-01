//! Current producer identity and fresh replay, separate from approved captures.
use super::{is_library_sha256, repo_root, sha256, verify_descriptor};
use serde_json::{json, Value};
use std::{fs, process::Command};

const RECORD: &str = "tools/pdf-harness/pdfium-runtime.json";
const CASE: &str = "tests/conformance/pdf-viewer-qualification/rectangle-outline";
const ROLES: [(&str, &str); 3] = [
    ("valid", "candidate-control.pdf"),
    ("visual-fail", "visual-fail-structure-pass.pdf"),
    ("structure-fail", "visual-pass-structure-fail.pdf"),
];

fn descriptor(path: &str) -> Result<Value, String> {
    let bytes = fs::read(repo_root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    Ok(json!({"path": path, "sha256": sha256(&bytes)}))
}

/// The worker configuration every replay shares. The library that ran and the
/// host it ran on are observations recorded beside it (ADR-0024).
fn identity() -> Value {
    json!({
        "schema": "excalidraw-render/pdfium-runtime/v1",
        "adapter": "pdfium-render/0.9.4",
        "pdfium": "152.0.7947.0",
        "renderer": "AGG",
        "font_backend": "FreeType",
    })
}

fn sources() -> Result<Value, String> {
    let mut paths = super::rust_pdfium_sources()?;
    paths.extend([
        "devenv.nix".into(),
        "tools/pdf-harness/run-pdfium.sh".into(),
        "tools/pdf-harness/pdfium-rust/tests/controls.rs".into(),
        "tools/pdf-harness/pdfium/empty-fonts/.keep".into(),
    ]);
    Ok(json!(paths
        .iter()
        .map(|p| descriptor(p))
        .collect::<Result<Vec<_>, _>>()?))
}

fn host() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn receipt(raster: &Value, library: &Value) -> Value {
    json!({
        "adapter": "pdfium-render/0.9.4", "pdfium": "152.0.7947.0",
        "library_sha256": library, "width": 200, "height": 120,
        "byte_length": 200 * 120 * 4, "layout": "top-left-row-major",
        "pixel_format": "straight-alpha-srgb-rgba8", "sha256": raster["sha256"],
    })
}

fn validate(record: &Value) -> Result<(), String> {
    let mut producer = record.clone();
    let fields = producer
        .as_object_mut()
        .ok_or("runtime record must be an object")?;
    let mut take = |key: &str| {
        fields
            .remove(key)
            .ok_or_else(|| format!("runtime record omits {key}"))
    };
    let captures = take("captures")?;
    let library = take("library_sha256")?;
    let platform = take("platform")?;
    // Source descriptors are a capture-time observation, not a maintenance gate.
    crate::conformance_candidate_gate::historical_sources(&take("sources")?)?;
    if !is_library_sha256(&library) || !platform.as_str().is_some_and(|p| !p.trim().is_empty()) {
        return Err("invalid PDFium runtime observation".into());
    }
    if producer != identity() {
        return Err("stale PDFium runtime identity; run conformance refresh-pdfium-runtime".into());
    }
    let captures = captures.as_array().ok_or("invalid runtime captures")?;
    if captures.len() != ROLES.len() {
        return Err("incomplete PDFium runtime replay".into());
    }
    for (capture, (role, fixture)) in captures.iter().zip(ROLES) {
        let input = descriptor(&format!("{CASE}/{fixture}"))?;
        let raster = descriptor(&format!("{CASE}/evidence/pdfium-{role}.rgba"))?;
        let expected = receipt(&raster, &library);
        if capture
            != &json!({
                "role": role, "input": input, "raster": raster,
                "runs": [expected, expected],
            })
        {
            return Err(format!("invalid PDFium {role} runtime replay"));
        }
        verify_descriptor(&capture["input"])?;
        verify_descriptor(&capture["raster"])?;
    }
    Ok(())
}

pub(super) fn verify() -> Result<(), String> {
    let bytes = fs::read(repo_root().join(RECORD))
        .map_err(|e| format!("{RECORD}: {e}; run conformance refresh-pdfium-runtime"))?;
    validate(&serde_json::from_slice(&bytes).map_err(|e| e.to_string())?)
}

/// Refresh only after real fresh-process renders match the retained raster bytes.
pub fn refresh() -> Result<(), String> {
    let producers = sources()?;
    let scratch = tempfile::Builder::new()
        .prefix("pdfium-runtime-")
        .tempdir_in(repo_root().join("target"))
        .map_err(|e| e.to_string())?;
    let mut library = None;
    let mut captures = Vec::new();
    for (role, fixture) in ROLES {
        let input = descriptor(&format!("{CASE}/{fixture}"))?;
        let raster = descriptor(&format!("{CASE}/evidence/pdfium-{role}.rgba"))?;
        let expected = fs::read(repo_root().join(raster["path"].as_str().unwrap()))
            .map_err(|e| e.to_string())?;
        let mut runs = Vec::new();
        for run in 0..2 {
            let output_path = scratch.path().join(format!("{role}-{run}.rgba"));
            let output = Command::new("bash")
                .current_dir(repo_root())
                .arg("tools/pdf-harness/run-pdfium.sh")
                .arg("render")
                .arg(input["path"].as_str().unwrap())
                .arg(&output_path)
                .args(["200", "120"])
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(format!(
                    "PDFium {role} failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            if fs::read(output_path).map_err(|e| e.to_string())? != expected {
                return Err(format!("PDFium {role} differs from retained raster"));
            }
            let actual: Value =
                serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
            // Every run of one refresh loads the same library.
            let ran = library.get_or_insert_with(|| actual["library_sha256"].clone());
            if actual != receipt(&raster, ran) {
                return Err(format!("invalid PDFium {role} receipt"));
            }
            runs.push(actual);
        }
        captures.push(json!({"role": role, "input": input, "raster": raster, "runs": runs}));
    }
    if producers != sources()? {
        return Err("PDFium producers changed during collection".into());
    }
    let mut record = identity();
    record["library_sha256"] = library.ok_or("PDFium runtime replay rendered nothing")?;
    record["platform"] = json!(host());
    record["sources"] = producers;
    record["captures"] = json!(captures);
    validate(&record)?;
    let bytes = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())? + "\n";
    fs::write(repo_root().join(RECORD), bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_record_preserves_provenance_but_rejects_detached_receipts() {
        let original: Value =
            serde_json::from_slice(&fs::read(repo_root().join(RECORD)).unwrap()).unwrap();
        validate(&original).unwrap();
        let mut older = original.clone();
        older["sources"][0]["sha256"] = json!("a".repeat(64));
        validate(&older).unwrap();
        // Another host's library and platform are observations, not drift.
        let mut elsewhere = original.clone();
        let other = json!("b".repeat(64));
        elsewhere["library_sha256"] = other.clone();
        elsewhere["platform"] = json!("other-host");
        for capture in elsewhere["captures"].as_array_mut().unwrap() {
            for run in capture["runs"].as_array_mut().unwrap() {
                run["library_sha256"] = other.clone();
            }
        }
        validate(&elsewhere).unwrap();
        for mutation in 0..10 {
            let mut record = original.clone();
            match mutation {
                0 => record["adapter"] = json!("pypdfium2/5.12.1"),
                1 => record["sources"][0]["sha256"] = json!("invalid"),
                2 => record["captures"][0]["runs"][0]["sha256"] = json!("a".repeat(64)),
                3 => record["captures"][0]["runs"][1]["pypdfium2"] = json!("5.12.1"),
                4 => {
                    record["captures"].as_array_mut().unwrap().pop();
                }
                5 => record["captures"][0]["input"] = record["captures"][1]["input"].clone(),
                6 => record["captures"][0]["runs"][0]["library_sha256"] = json!("c".repeat(64)),
                7 => record["library_sha256"] = json!("00"),
                8 => record["platform"] = json!(" "),
                _ => {
                    record["native_lock"] =
                        json!({"path": "tools/pdf-harness/pdfium-native.lock.json"})
                }
            }
            assert!(validate(&record).is_err(), "mutation {mutation}");
        }
    }
}
