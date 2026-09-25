//! Independent qpdf structural inspection and handwritten calibration fixtures.
mod checks;
mod content;
mod fixtures;
mod inventory;

use crate::conformance::sha256;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;
type Result<T> = std::result::Result<T, String>;

pub fn run(args: impl Iterator<Item = String>) -> Result<()> {
    let args = args.collect::<Vec<_>>();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["generate-fixtures",output]=>fixtures::generate(Path::new(output)),
        ["inspect",input,expectation,expected,output]=>{
            let expected=match *expected {"pass"=>true,"fail"=>false,_=>return Err("expected pass or fail".into())};
            let qpdf=std::env::var_os("QPDF").map(std::path::PathBuf::from).unwrap_or_else(||crate::conformance::repo_root().join("target/pdf-harness/qpdf/bin/qpdf"));
            let expectation=checks::Expectation::load(&fs::read(expectation).map_err(|e|e.to_string())?)?;
            let report=inspect(&qpdf,Path::new(input),&expectation,expected)?;
            fs::write(output,[canonical(&report)?,b"\n".to_vec()].concat()).map_err(|e|e.to_string())?;
            if report["passed"]==expected {Ok(())}else{Err("PDF structural outcome differs from expectation".into())}
        }
        _=>Err("usage: pdf-harness generate-fixtures <output> | inspect <input.pdf> <expectation.json> <pass|fail> <output.json>".into()),
    }
}

fn list(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(n) => {
            crate::representative::json::compare_numbers(n, &serde_json::Number::from(0))
                != std::cmp::Ordering::Equal
        }
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
fn pairs(v: &Value) -> Vec<(&str, &Value)> {
    v.as_object()
        .map(|m| m.iter().map(|(k, v)| (k.as_str(), v)).collect())
        .unwrap_or_default()
}
fn get(v: &Value, k: &str, default: Value) -> Value {
    v.get(k).cloned().unwrap_or(default)
}
fn reference(v: &Value) -> Value {
    if v.as_str().is_some_and(|s| s.ends_with(" R")) {
        v.clone()
    } else {
        Value::Null
    }
}
fn object_value(v: &Value) -> Value {
    v.get("value")
        .cloned()
        .or_else(|| v.get("stream").map(|s| get(s, "dict", json!({}))))
        .unwrap_or_else(|| v.clone())
}
fn resolve_value(objects: &Value, v: &Value) -> Value {
    if let Some(r) = v.as_str().filter(|s| s.ends_with(" R")) {
        object_value(&get(objects, &format!("obj:{r}"), json!({})))
    } else {
        v.clone()
    }
}
fn resolve(objects: &Value, v: &Value) -> Value {
    let v = resolve_value(objects, v);
    if v.is_object() {
        v
    } else {
        json!({})
    }
}
fn stream_dict(objects: &Value, v: &Value) -> Value {
    v.as_str()
        .map(|r| get(&objects[format!("obj:{r}")]["stream"], "dict", json!({})))
        .unwrap_or(json!({}))
}
fn stream_value(objects: &Value, v: &Value) -> Result<Option<Vec<u8>>> {
    v.as_str()
        .and_then(|r| objects[format!("obj:{r}")]["stream"]["data"].as_str())
        .map(|s| {
            base64::engine::GeneralPurpose::new(
                &base64::alphabet::STANDARD,
                base64::engine::general_purpose::GeneralPurposeConfig::new()
                    .with_decode_allow_trailing_bits(true),
            )
            .decode(s)
            .map_err(|e| e.to_string())
        })
        .transpose()
}
fn stream_descriptor(bytes: Option<&[u8]>) -> Value {
    bytes
        .map(|b| json!({"byte_length":b.len(),"sha256":sha256(b)}))
        .unwrap_or(Value::Null)
}
fn normalize(v: &Value) -> Value {
    match v {
        Value::String(s) => json!(s.strip_prefix("u:").unwrap_or(s)),
        Value::Array(a) => json!(a.iter().map(normalize).collect::<Vec<_>>()),
        Value::Object(o) => {
            Value::Object(o.iter().map(|(k, v)| (k.clone(), normalize(v))).collect())
        }
        _ => v.clone(),
    }
}

/// Python's ASCII-escaped sorted compact JSON; report hashes include numeric
/// spelling, including whole-valued floats and exponent zero padding.
fn canonical(value: &Value) -> Result<Vec<u8>> {
    let bytes = crate::representative::json::compact(value)?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii() && ch != '\u{7f}' {
            out.push(ch)
        } else {
            let mut units = [0; 2];
            for unit in ch.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    Ok(out.into_bytes())
}

fn inherited(objects: &Value, page: &Value, key: &str, default: Value) -> Value {
    let mut current = page.clone();
    let mut seen = BTreeSet::new();
    for _ in 0..64 {
        if let Some(v) = current.get(key) {
            return v.clone();
        }
        let Some(parent) = current["/Parent"].as_str() else {
            return default;
        };
        if !seen.insert(parent.to_owned()) {
            return default;
        }
        current = resolve(objects, &json!(parent));
    }
    default
}
fn messages(bytes: &[u8], path: &Path) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .replace(&path.to_string_lossy().into_owned(), "<input.pdf>")
        .lines()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn inspect(
    qpdf: &Path,
    input: &Path,
    expectation: &checks::Expectation,
    expected: bool,
) -> Result<Value> {
    let input = input.canonicalize().map_err(|e| e.to_string())?;
    let version = Command::new(qpdf)
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    if !version.status.success()
        || !String::from_utf8_lossy(&version.stdout).starts_with("qpdf version 12.3.2\n")
    {
        return Err("PDF inspector requires qpdf 12.3.2".into());
    }
    let raw = fs::read(&input).map_err(|e| e.to_string())?;
    let check = Command::new(qpdf)
        .arg("--check")
        .arg(&input)
        .output()
        .map_err(|e| e.to_string())?;
    let status = match check.status.code() {
        Some(0) => "clean",
        Some(3) => "warning",
        _ => "error",
    };
    let mut warnings = messages(&check.stderr, &input);
    let mut errors = if status == "error" {
        warnings.clone()
    } else {
        vec![]
    };
    let output = Command::new(qpdf)
        .args([
            "--json=2",
            "--json-stream-data=inline",
            "--decode-level=generalized",
        ])
        .arg(&input)
        .output()
        .map_err(|e| e.to_string())?;
    let more = messages(&output.stderr, &input);
    for message in &more {
        if !warnings.contains(message) {
            warnings.push(message.clone());
        }
    }
    if status == "error" {
        errors = warnings.clone();
    }
    if !matches!(output.status.code(), Some(0 | 3)) {
        for message in more {
            if !errors.contains(&message) {
                errors.push(message);
            }
        }
    }
    let source = match serde_json::from_slice::<Value>(&output.stdout) {
        Ok(v) => v,
        Err(e) => {
            let message = if output.stdout.iter().all(u8::is_ascii_whitespace) {
                "Expecting value: line 1 column 1 (char 0)".to_owned()
            } else {
                e.to_string()
            };
            errors.push(format!("qpdf JSON parse failed: {message}"));
            if status == "error" {
                warnings = errors.clone();
            }
            json!({"acroform":{"fields":[],"hasacroform":false},"attachments":{},"encrypt":{"encrypted":false},"pages":[],"qpdf":[{"pdfversion":null},{"trailer":{"value":{}}}]})
        }
    };
    let mut report = build_report(&source, &raw, status, warnings, errors)?;
    for page in report["pages"].as_array_mut().ok_or("pages")? {
        for image in page["images"].as_array_mut().ok_or("images")? {
            image["classification"] = list(&expectation.value["images"])
                .iter()
                .rev()
                .find(|e| e["resource"] == image["resource"])
                .map(|e| e["classification"].clone())
                .unwrap_or(json!("undeclared"));
        }
    }
    report["checks"] = checks::evaluate(&report, expectation)?;
    report["expected_pass"] = json!(expected);
    report["passed"] = json!(
        status == "clean"
            && list(&report["errors"]).is_empty()
            && list(&report["checks"]).iter().all(|c| c["passed"] == true)
    );
    Ok(report)
}

fn build_report(
    source: &Value,
    raw: &[u8],
    status: &str,
    warnings: Vec<String>,
    mut errors: Vec<String>,
) -> Result<Value> {
    let objects = &source["qpdf"][1];
    let trailer = object_value(&get(objects, "trailer", json!({})));
    let catalog = resolve(objects, &trailer["/Root"]);
    let mut pages = Vec::new();
    for row in list(&source["pages"]) {
        let page = resolve(objects, &row["object"]);
        let resources = resolve_value(objects, &inherited(objects, &page, "/Resources", json!({})));
        let media = inherited(objects, &page, "/MediaBox", Value::Null);
        let crop = inherited(objects, &page, "/CropBox", media.clone());
        let mut others = serde_json::Map::new();
        for key in ["/BleedBox", "/TrimBox", "/ArtBox"] {
            let v = inherited(objects, &page, key, Value::Null);
            if !v.is_null() {
                others.insert(key[1..].into(), v);
            }
        }
        let operations =
            content::operations(objects, list(&row["contents"]), &resources, &mut errors)?;
        let mut p = inventory::page_inventory(objects, &resources, &operations, &page)?;
        let clips = operations
            .iter()
            .filter(|o| matches!(o["operator"].as_str(), Some("W" | "W*")))
            .cloned()
            .collect::<Vec<_>>();
        let mut masks = list(&p["transparency"])
            .iter()
            .filter(|s| !s["soft_mask"].is_null())
            .cloned()
            .collect::<Vec<_>>();
        masks.extend(list(&p["images"]).iter().filter(|i|!i["soft_mask"].is_null()).map(|i|json!({"image_object":i["object"],"kind":"image-soft-mask","resource":i["resource"],"soft_mask":i["soft_mask"]})));
        p["media_box"] = media;
        p["crop_box"] = crop;
        p["other_boxes"] = json!(others);
        p["rotate"] = inherited(objects, &page, "/Rotate", json!(0));
        p["user_unit"] = get(&page, "/UserUnit", json!(1));
        p["clips"] = json!(clips);
        p["masks"] = json!(masks);
        p["text_operations"] = json!(operations
            .iter()
            .filter(|o| content::is_text(o["operator"].as_str().unwrap_or("")))
            .collect::<Vec<_>>());
        p["vector_operations"] = json!(operations
            .iter()
            .filter(|o| content::is_vector(o["operator"].as_str().unwrap_or("")))
            .collect::<Vec<_>>());
        p["operations"] = json!(operations);
        pages.push(p);
    }
    let mut document = inventory::document_inventory(source, objects, &catalog)?;
    document["encrypted"] = json!(truthy(&source["encrypt"]["encrypted"]));
    document["file_ids"] = normalize(&trailer["/ID"]);
    document["incremental_revisions"] = json!(raw
        .windows(9)
        .filter(|w| *w == b"startxref")
        .count()
        .saturating_sub(1));
    document["info_dictionary"] = if !truthy(&trailer["/Info"]) {
        Value::Null
    } else {
        normalize(&resolve(objects, &trailer["/Info"]))
    };
    document["linearized"] = json!(raw[..raw.len().min(1024)]
        .windows(11)
        .any(|w| w == b"/Linearized"));
    document["page_count"] = json!(pages.len());
    document["pdf_version"] = source["qpdf"][0]["pdfversion"].clone();
    document["xmp"] = stream_descriptor(stream_value(objects, &catalog["/Metadata"])?.as_deref());
    Ok(
        json!({"schema":"excalidraw-render/pdf-structure-report/v1","checks":[],"document":document,"errors":errors,"expected_pass":false,"pages":pages,"parser":{"name":"qpdf","status":status,"version":"12.3.2","warnings":warnings},"passed":false}),
    )
}
