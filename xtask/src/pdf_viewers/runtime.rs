//! Current producer identity and fresh replay, separate from approved captures.
use super::{repo_root, sha256, verify_descriptor};
use serde_json::{json, Value};
use std::{fs, process::Command};

const RECORD: &str = "tools/pdf-harness/pdfium-runtime.json";
const LOCK: &str = "tools/pdf-harness/pdfium-native.lock.json";
const CASE: &str = "tests/conformance/pdf-viewer-qualification/rectangle-outline";
const LIBRARY: &str = "61c9f745c6296a1050599a99a1ed985036411b591a11bd2a41bafe530ecb4f33";
const ROLES: [(&str, &str); 3] = [
    ("valid", "candidate-control.pdf"),
    ("visual-fail", "visual-fail-structure-pass.pdf"),
    ("structure-fail", "visual-pass-structure-fail.pdf"),
];

fn descriptor(path: &str) -> Result<Value, String> {
    let bytes = fs::read(repo_root().join(path)).map_err(|e| format!("{path}: {e}"))?;
    Ok(json!({"path": path, "sha256": sha256(&bytes)}))
}

fn identity() -> Result<Value, String> {
    let bytes = fs::read(repo_root().join(LOCK)).map_err(|e| e.to_string())?;
    let lock: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if lock["library"]["sha256"] != LIBRARY
        || lock["library"]["version"] != "152.0.7947.0"
        || lock["library"]["build"] != 7947
        || lock["adapter"] != "pdfium-render/0.9.4"
        || lock["platform"] != "linux-x86_64"
        || lock["renderer"] != "AGG"
        || lock["font_backend"] != "FreeType"
    {
        return Err("native PDFium identity differs from qualified engine".into());
    }
    let mut paths = super::rust_pdfium_sources()?;
    paths.extend([
        "tools/pdf-harness/install.sh".into(),
        "tools/pdf-harness/run-pdfium.sh".into(),
        "tools/pdf-harness/pdfium-rust/tests/controls.rs".into(),
        "tools/pdf-harness/pdfium/empty-fonts/.keep".into(),
    ]);
    let sources = paths
        .iter()
        .map(|p| descriptor(p))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "schema": "excalidraw-render/pdfium-runtime/v1",
        "adapter": lock["adapter"],
        "pdfium": lock["library"]["version"],
        "library_sha256": lock["library"]["sha256"],
        "platform": lock["platform"],
        "renderer": lock["renderer"],
        "font_backend": lock["font_backend"],
        "native_lock": descriptor(LOCK)?,
        "sources": sources,
    }))
}

fn receipt(raster: &Value) -> Value {
    json!({
        "adapter": "pdfium-render/0.9.4", "pdfium": "152.0.7947.0",
        "library_sha256": LIBRARY, "width": 200, "height": 120,
        "byte_length": 200 * 120 * 4, "layout": "top-left-row-major",
        "pixel_format": "straight-alpha-srgb-rgba8", "sha256": raster["sha256"],
    })
}

fn validate(record: &Value) -> Result<(), String> {
    let mut producer = record.clone();
    let captures = producer
        .as_object_mut()
        .ok_or("runtime record must be an object")?
        .remove("captures")
        .ok_or("runtime record omits captures")?;
    let mut current = identity()?;
    // Source descriptors are a capture-time observation, not a maintenance gate.
    crate::conformance_candidate_gate::historical_sources(&producer["sources"])?;
    producer.as_object_mut().unwrap().remove("sources");
    current.as_object_mut().unwrap().remove("sources");
    if producer != current {
        return Err("stale PDFium runtime identity; run conformance refresh-pdfium-runtime".into());
    }
    let captures = captures.as_array().ok_or("invalid runtime captures")?;
    if captures.len() != ROLES.len() {
        return Err("incomplete PDFium runtime replay".into());
    }
    for (capture, (role, fixture)) in captures.iter().zip(ROLES) {
        let input = descriptor(&format!("{CASE}/{fixture}"))?;
        let raster = descriptor(&format!("{CASE}/evidence/pdfium-{role}.rgba"))?;
        let expected = receipt(&raster);
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

struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Refresh only after real fresh-process renders match the retained raster bytes.
pub fn refresh() -> Result<(), String> {
    let mut record = identity()?;
    let scratch = repo_root().join(format!("target/pdfium-runtime-{}", std::process::id()));
    fs::create_dir(&scratch).map_err(|e| e.to_string())?;
    let scratch = Scratch(scratch);
    let mut captures = Vec::new();
    for (role, fixture) in ROLES {
        let input = descriptor(&format!("{CASE}/{fixture}"))?;
        let raster = descriptor(&format!("{CASE}/evidence/pdfium-{role}.rgba"))?;
        let expected = fs::read(repo_root().join(raster["path"].as_str().unwrap()))
            .map_err(|e| e.to_string())?;
        let mut runs = Vec::new();
        for run in 0..2 {
            let output_path = scratch.0.join(format!("{role}-{run}.rgba"));
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
            if actual != receipt(&raster) {
                return Err(format!("invalid PDFium {role} receipt"));
            }
            runs.push(actual);
        }
        captures.push(json!({"role": role, "input": input, "raster": raster, "runs": runs}));
    }
    record["captures"] = json!(captures);
    if record["sources"] != identity()?["sources"] {
        return Err("PDFium producers changed during collection".into());
    }
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
        for mutation in 0..7 {
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
                _ => record["native_lock"]["path"] = json!("tools/pdf-harness/pdfium.lock.json"),
            }
            assert!(validate(&record).is_err(), "mutation {mutation}");
        }
    }
}
