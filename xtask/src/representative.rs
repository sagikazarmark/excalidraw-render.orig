//! Representative scene intake and native observations. These commands prepare
//! review material; neither bundles nor captures confer conformance approval.
mod capture;
mod case;
mod compact;
mod comparison;
mod evidence;
pub(crate) mod json;
mod viewer_assets;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use self::json::{decode, encode};
use crate::cli::RepresentativeCommand;
use crate::conformance::sha256;

type Result<T> = std::result::Result<T, String>;
const FORMATS: [&str; 3] = ["svg", "png", "pdf"];
const FILES: [&str; 4] = [
    "scene.excalidraw",
    "LICENSE",
    "PROVENANCE.json",
    "plan.json",
];
type Files = BTreeMap<String, Vec<u8>>;

pub fn run(command: RepresentativeCommand) -> Result<()> {
    let record = match command {
        RepresentativeCommand::Case(command) => return case::run(command),
        RepresentativeCommand::Controls(args) => return comparison::controls(args),
        RepresentativeCommand::Evidence(command) => return evidence::run(command),
        RepresentativeCommand::ViewerAssets(command) => return viewer_assets::run(command),
        RepresentativeCommand::Compact(command) => return compact::run(command),
        RepresentativeCommand::Prepare {
            source,
            plan,
            output,
        } => prepare(&source, &plan, &output)?,
        RepresentativeCommand::Verify { bundle } => verify(&bundle)?,
        RepresentativeCommand::Capture {
            bundle,
            renderer,
            output,
            timeout,
        } => capture::capture(&bundle, &renderer, &output, timeout)?,
        RepresentativeCommand::VerifyCapture { bundle, output } => {
            capture::verify_capture(&bundle, &output)?
        }
    };
    print!("{}", String::from_utf8(encode(&json!({"case_id": record["case_id"], "status": record["status"], "eligible_executions": []}))?).map_err(|e| e.to_string())?);
    Ok(())
}

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| format!("missing or invalid {field}"))
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value[field]
        .as_array()
        .ok_or_else(|| format!("missing or invalid {field}"))
}

fn fields(value: &Value, expected: &[&str], message: &str) -> Result<()> {
    require(
        value.as_object().is_some_and(|o| {
            o.keys().map(String::as_str).collect::<BTreeSet<_>>()
                == expected.iter().copied().collect()
        }),
        message,
    )
}

fn descriptor(bytes: &[u8]) -> Value {
    json!({"bytes": bytes.len(), "sha256": sha256(bytes)})
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn read(root: &Path, name: &str) -> Result<Vec<u8>> {
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    require(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        &format!("not a regular immutable intake file: {name}"),
    )?;
    fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
}

fn file_names(root: &Path) -> Result<BTreeSet<String>> {
    fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .map(|e| {
            e.map_err(|e| e.to_string())?
                .file_name()
                .into_string()
                .map_err(|_| "non-UTF8 intake filename".into())
        })
        .collect()
}

fn bundle_files(root: &Path) -> Result<Files> {
    FILES
        .iter()
        .map(|name| Ok(((*name).to_owned(), read(root, name)?)))
        .collect()
}

// Validate before creating a destination. Remove incomplete publication on an
// I/O failure, but never delete or overwrite a pre-existing destination.
fn publish(output: &Path, files: &Files, marker: &str, record: &Value) -> Result<()> {
    let record = encode(record)?;
    fs::create_dir(output).map_err(|e| format!("cannot create {}: {e}", output.display()))?;
    let result = (|| {
        for (name, bytes) in files {
            fs::write(output.join(name), bytes).map_err(|e| e.to_string())?;
        }
        fs::write(output.join(marker), record).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(output);
    }
    result
}

fn valid_id(id: &str) -> bool {
    id.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn valid_case(id: &str) -> bool {
    id.strip_prefix("representative.")
        .and_then(|s| s.rsplit_once(".v"))
        .is_some_and(|(name, version)| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && version
                    .bytes()
                    .next()
                    .is_some_and(|b| (b'1'..=b'9').contains(&b))
                && version.bytes().all(|b| b.is_ascii_digit())
        })
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => {
            json::compare_numbers(v, &serde_json::Number::from(0)) != std::cmp::Ordering::Equal
        }
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}

fn validate_plan(plan: &Value, scene: &Value, provenance: &Value) -> Result<()> {
    fields(
        plan,
        &[
            "schema",
            "case_id",
            "source_identity",
            "purpose",
            "sanitization",
            "variants",
            "formats",
            "applicability",
            "regions",
        ],
        "unknown or missing intake plan field",
    )?;
    fields(
        &plan["sanitization"],
        &["operation", "review_notes"],
        "invalid sanitization fields",
    )?;
    require(
        plan["schema"] == "excalidraw-render/representative-intake-plan/v1",
        "unsupported plan schema",
    )?;
    require(valid_case(text(plan, "case_id")?), "invalid case identity")?;
    require(
        provenance
            .get("identity")
            .is_some_and(|identity| identity == &plan["source_identity"]),
        "source identity mismatch",
    )?;
    require(
        !text(plan, "purpose")?.trim().is_empty(),
        "missing representative purpose",
    )?;
    require(
        matches!(
            text(&plan["sanitization"], "operation")?,
            "reviewed-no-change" | "reviewed-index-repair"
        ),
        "unsupported sanitization operation",
    )?;
    require(
        !text(&plan["sanitization"], "review_notes")?
            .trim()
            .is_empty(),
        "missing sanitization review notes",
    )?;
    require(
        plan["formats"] == json!(FORMATS),
        "intake requires SVG, PNG and PDF",
    )?;
    let variants = array(plan, "variants")?;
    require(!variants.is_empty(), "missing variants")?;
    let mut ids = BTreeSet::new();
    for variant in variants {
        fields(
            variant,
            &[
                "id",
                "color_mode",
                "include_background",
                "padding",
                "scale",
                "metadata",
            ],
            "unknown or missing variant option",
        )?;
        let id = text(variant, "id")?;
        require(
            valid_id(id) && ids.insert(id),
            "invalid or duplicate variant",
        )?;
        require(
            matches!(text(variant, "color_mode")?, "light" | "dark"),
            "invalid color mode",
        )?;
        require(
            variant["include_background"].is_boolean() && variant["metadata"].is_boolean(),
            "invalid background/metadata option",
        )?;
        let padding = variant["padding"].as_f64().ok_or("invalid padding")?;
        let scale = variant["scale"].as_f64().ok_or("invalid scale")?;
        require(
            (0.0..=100.0).contains(&padding) && scale > 0.0 && scale <= 4.0,
            "request outside intake envelope",
        )?;
    }
    fields(
        &plan["applicability"],
        &["frame_exports", "assets", "sizing"],
        "incomplete applicability decisions",
    )?;
    for value in plan["applicability"]
        .as_object()
        .ok_or("invalid applicability")?
        .values()
    {
        require(
            value.as_str().is_some_and(|s| !s.trim().is_empty()),
            "empty applicability reason",
        )?;
    }
    let elements = array(scene, "elements")?
        .iter()
        .filter(|e| !truthy(&e["isDeleted"]))
        .map(|e| text(e, "id"))
        .collect::<Result<BTreeSet<_>>>()?;
    let mut ids = BTreeSet::new();
    for region in array(plan, "regions")? {
        fields(
            region,
            &["id", "class", "element_ids"],
            "invalid region fields",
        )?;
        let id = text(region, "id")?;
        require(
            valid_id(id) && ids.insert(id),
            "invalid or duplicate region",
        )?;
        require(
            matches!(
                text(region, "class")?,
                "geometry/image" | "typography" | "mixed-representative"
            ),
            "invalid visual class",
        )?;
        let anchors = array(region, "element_ids")?;
        require(
            anchors
                .iter()
                .all(|v| v.as_str().is_some_and(|s| elements.contains(s))),
            "unknown crop element",
        )?;
        require(
            id == "whole" || !anchors.is_empty(),
            "crop needs source anchors",
        )?;
    }
    require(ids.contains("whole"), "missing whole-scene judgment")
}

fn census(scene: &Value) -> Result<Value> {
    let live = array(scene, "elements")?
        .iter()
        .filter(|e| !truthy(&e["isDeleted"]))
        .collect::<Vec<_>>();
    let mut kinds = BTreeMap::<&str, usize>::new();
    let mut fonts = Vec::<serde_json::Number>::new();
    for e in &live {
        *kinds.entry(text(e, "type")?).or_default() += 1;
        if e["type"] == "text" {
            let font = e["fontFamily"].as_number().ok_or("invalid font family")?;
            if !fonts
                .iter()
                .any(|existing| json::compare_numbers(existing, font).is_eq())
            {
                fonts.push(font.clone());
            }
        }
    }
    fonts.sort_by(json::compare_numbers);
    Ok(json!({"live_elements": live.len(), "element_types": kinds,
        "bound_text": live.iter().filter(|e| e["type"] == "text" && truthy(&e["containerId"])).count(),
        "bound_arrows": live.iter().filter(|e| e["type"] == "arrow" && (truthy(&e["startBinding"]) || truthy(&e["endBinding"]))).count(),
        "font_families": fonts, "assets": scene["files"].as_object().ok_or("invalid scene files")?.len()}))
}

fn materialize(files: &Files) -> Result<Value> {
    let provenance = decode(&files["PROVENANCE.json"])?;
    let plan = decode(&files["plan.json"])?;
    let scene = decode(&files["scene.excalidraw"])?;
    require(
        scene["type"] == "excalidraw"
            && scene["version"] == 2
            && scene["elements"].is_array()
            && scene["appState"].is_object()
            && scene["files"].is_object(),
        "scene envelope",
    )?;
    let original = if plan["sanitization"]["operation"] == "reviewed-index-repair" {
        let original = repair_indices(&files["scene.excalidraw"], true)?;
        require(
            repair_indices(&original, false)? == files["scene.excalidraw"],
            "retained scene is not an exact index repair",
        )?;
        original
    } else {
        files["scene.excalidraw"].clone()
    };
    require(
        provenance["scene"]["file"] == "scene.excalidraw"
            && provenance["license"]["text"] == "LICENSE",
        "source file names must be local fixed names",
    )?;
    require(
        descriptor(&original)
            == json!({"bytes": provenance["scene"]["bytes"], "sha256": provenance["scene"]["sha256"]}),
        "upstream scene digest/length mismatch",
    )?;
    let upstream = &provenance["upstream"];
    let commit = text(upstream, "commit")?;
    require(
        commit.len() == 40
            && commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "upstream commit must be immutable",
    )?;
    let repository = text(upstream, "repository")?;
    require(
        repository.starts_with("https://github.com/")
            && text(upstream, "permalink")?
                == format!("{repository}/blob/{commit}/{}", text(upstream, "path")?),
        "upstream permalink mismatch",
    )?;
    require(
        !text(&provenance["license"], "spdx")?.trim().is_empty()
            && files["LICENSE"].iter().any(|b| !b.is_ascii_whitespace()),
        "missing source license",
    )?;
    validate_plan(&plan, &scene, &provenance)?;
    let mut sanitization = plan["sanitization"].clone();
    sanitization["original_sha256"] = json!(sha256(&original));
    sanitization["retained_sha256"] = json!(sha256(&files["scene.excalidraw"]));
    sanitization["sign_off"] = Value::Null;
    let mut executions = Vec::new();
    for variant in array(&plan, "variants")? {
        for format in FORMATS {
            executions.push(format!(
                "{}:{}:{format}",
                text(&plan, "case_id")?,
                text(variant, "id")?
            ));
        }
    }
    Ok(
        json!({"schema": "excalidraw-render/representative-intake/v1", "case_id": plan["case_id"], "status": "pending-review", "approval": null,
        "eligible_executions": [], "files": files.iter().map(|(name, data)| (name, descriptor(data))).collect::<BTreeMap<_,_>>(),
        "source": upstream, "license": provenance["license"]["spdx"], "sanitization": sanitization, "census": census(&scene)?, "proposed_executions": executions,
        "pending": ["sanitization-and-applicability-review", "reference-and-viewer-evidence", "independent-semantics-reports-notices", "resource-outcomes", "canonical-crops", "exact-subject-approval", "registry-and-target-integration"]}),
    )
}

fn prepare(source: &Path, plan: &Path, output: &Path) -> Result<Value> {
    let mut files = FILES
        .iter()
        .filter(|n| **n != "plan.json")
        .map(|name| Ok(((*name).to_owned(), read(source, name)?)))
        .collect::<Result<Files>>()?;
    files.insert(
        "plan.json".into(),
        fs::read(plan).map_err(|e| e.to_string())?,
    );
    if decode(&files["plan.json"])?["sanitization"]["operation"] == "reviewed-index-repair" {
        files.insert(
            "scene.excalidraw".into(),
            repair_indices(&files["scene.excalidraw"], false)?,
        );
    }
    let manifest = materialize(&files)?;
    publish(output, &files, "manifest.json", &manifest)?;
    Ok(manifest)
}

fn verify(root: &Path) -> Result<Value> {
    require(
        file_names(root)?
            == FILES
                .iter()
                .copied()
                .chain(["manifest.json"])
                .map(str::to_owned)
                .collect(),
        "intake file inventory changed",
    )?;
    let manifest = materialize(&bundle_files(root)?)?;
    let mut retained = decode(&read(root, "manifest.json")?)?;
    retained["approval"] = Value::Null;
    require(
        retained == manifest,
        "intake manifest or file binding changed",
    )?;
    Ok(manifest)
}

fn repair_indices(data: &[u8], undo: bool) -> Result<Vec<u8>> {
    let scene = decode(data)?;
    let elements = array(&scene, "elements")?;
    require(
        elements
            .iter()
            .all(|e| e.is_object() && (undo || e.get("index").is_none())),
        "index repair covers only scenes in which no element has an index",
    )?;
    let offsets = json::element_offsets(data)?;
    require(
        offsets.len() == elements.len(),
        "element scan disagrees with parsed scene",
    )?;
    let alphabet = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut key = b"a0".to_vec();
    let mut output = Vec::new();
    let mut cursor = 0;
    for offset in offsets {
        let inserted = format!(
            "\"index\":\"{}\",",
            std::str::from_utf8(&key).map_err(|e| e.to_string())?
        );
        output.extend_from_slice(&data[cursor..offset + 1]);
        cursor = offset + 1;
        if undo {
            require(
                data[cursor..].starts_with(inserted.as_bytes()),
                "retained scene is not an exact index repair",
            )?;
            cursor += inserted.len();
        } else {
            output.extend_from_slice(inserted.as_bytes());
        }
        let mut carry = true;
        for digit in key[1..].iter_mut().rev() {
            let next = alphabet
                .iter()
                .position(|b| b == digit)
                .ok_or("invalid fractional key")?
                + 1;
            *digit = alphabet[next % 62];
            if next < 62 {
                carry = false;
                break;
            }
        }
        if carry {
            require(key[0] < b'z', "fractional index keys exhausted")?;
            let head = key[0] + 1;
            key = vec![b'0'; (head - b'a') as usize + 2];
            key[0] = head;
        }
    }
    output.extend_from_slice(&data[cursor..]);
    Ok(output)
}
