use std::collections::BTreeSet;
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const REGISTRY_RELATIVE_PATH: &str = "tests/conformance/cases/registry.json";
const REGISTRY_SCHEMA: &str = "excalidraw-render/conformance-case-registry/v1";
const APPROVAL_SCHEMA: &str = "excalidraw-render/conformance-approval-subject/v1";
const REPOSITORY_ISSUE_PREFIX: &str =
    "https://github.com/sagikazarmark/excalidraw-render.orig/issues/";
const EMPTY_REPORT: &[u8] = br#"{"schema_version":1,"diagnostics":[]}"#;
const PROFILE_DIGEST: &str = "1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
const ASSET_DIGEST: &str = "02685099e6a12632d77a3cac4c6d3acef291520e4fef93a675ee8be471245d9a";
const SURFACE_DIGEST: &str = "4cc295860c1627485717ba1292add5ee6c28c7115ffda1019122ffff5d4c46cb";
const FRAME_PROFILE_DIGEST: &str =
    "63bf16ee197e7c6e79f917988e04aafec4c89c39de87b08ad10dd19712cde77e";

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace-root child")
        .to_owned()
}

/// One registered conformance case: the identity, layout, and approval issue
/// that used to be a per-case constant in this file.
struct CaseEntry {
    id: String,
    kind: String,
    relative_path: String,
    approving_issue: String,
    runner: String,
    schema_id: Option<String>,
    originating_issue: Option<String>,
    coverage: Vec<String>,
    semantic_expectations: Value,
    sanitization_reason: Option<String>,
    svg_element_counts: Value,
    generated_at: Option<String>,
    empty_fields: Vec<String>,
    expected_diagnostics: Value,
    variants: Value,
    exceptions: Value,
}

fn registry() -> Result<Vec<CaseEntry>, String> {
    let path = repo_root().join(REGISTRY_RELATIVE_PATH);
    let bytes = fs::read(&path)
        .map_err(|error| format!("failed to read {REGISTRY_RELATIVE_PATH}: {error}"))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid case registry JSON: {error}"))?;
    if value["schema"] != json!(REGISTRY_SCHEMA) {
        return Err(format!("case registry schema must be {REGISTRY_SCHEMA}"));
    }
    let cases = value["cases"]
        .as_array()
        .ok_or_else(|| "case registry cases must be an array".to_owned())?;
    let mut entries = Vec::with_capacity(cases.len());
    for case in cases {
        let field = |key: &str| -> Result<String, String> {
            case[key]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("case registry entry is missing a string {key}"))
        };
        let kind = field("kind")?;
        if !matches!(kind.as_str(), "scene" | "typography" | "frame") {
            return Err(format!("unknown registered case kind {kind:?}"));
        }
        entries.push(CaseEntry {
            id: field("id")?,
            kind,
            relative_path: field("path")?,
            approving_issue: field("approving_issue")?,
            runner: field("runner")?,
            schema_id: case["schema"]["id"].as_str().map(str::to_owned),
            originating_issue: case["originating_issue"].as_str().map(str::to_owned),
            coverage: case["coverage"]
                .as_array()
                .map(|records| {
                    records
                        .iter()
                        .filter_map(|record| record["identity"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            semantic_expectations: case["semantic_expectations"].clone(),
            sanitization_reason: case["sanitization"]["reason"].as_str().map(str::to_owned),
            svg_element_counts: case["svg_element_counts"].clone(),
            generated_at: case["generated_at"].as_str().map(str::to_owned),
            empty_fields: case["empty_fields"]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
            expected_diagnostics: case["expected_diagnostics"].clone(),
            variants: case["variants"].clone(),
            // A case that registers no tolerance exceptions carries none; the
            // manifest field is then required to be explicitly empty.
            exceptions: match &case["exceptions"] {
                Value::Null => json!([]),
                exceptions => exceptions.clone(),
            },
        });
    }
    let mut seen = BTreeSet::new();
    for entry in &entries {
        if !seen.insert(entry.id.clone()) {
            return Err(format!("case registry has a duplicate id {:?}", entry.id));
        }
    }
    Ok(entries)
}

fn registered(case_id: &str) -> Result<CaseEntry, String> {
    registry()?
        .into_iter()
        .find(|entry| entry.id == case_id)
        .ok_or_else(|| format!("unknown conformance case {case_id:?}"))
}

/// The runner script that materializes evidence for a registered case.
pub fn case_runner(case_id: &str) -> Result<String, String> {
    Ok(registered(case_id)?.runner)
}

/// Every registered case id, in registry order.
pub fn case_ids() -> Result<Vec<String>, String> {
    Ok(registry()?.into_iter().map(|entry| entry.id).collect())
}

pub fn verify(case_id: &str, require_approval: bool) -> Result<(), String> {
    let loaded = load(case_id)?;
    match loaded.entry.kind.as_str() {
        "frame" => return verify_frame(&loaded, require_approval),
        "typography" => return verify_typography(&loaded, require_approval),
        _ => {}
    }
    verify_manifest_digest(&loaded)?;
    verify_exact_keys(
        object(&loaded.manifest, "manifest")?,
        &[
            "approval",
            "assets",
            "case_id",
            "comparison",
            "compatibility_revision",
            "coverage",
            "crops",
            "determinism",
            "exceptions",
            "feature_tags",
            "layer",
            "license",
            "originating_issue",
            "provenance",
            "purpose",
            "reference",
            "sanitization",
            "scene",
            "schema",
            "semantic_expectations",
            "variants",
        ],
        "manifest",
    )?;
    expect_string(&loaded.manifest, "case_id", &loaded.entry.id)?;
    if loaded.manifest["compatibility_revision"] != json!(1) {
        return Err("compatibility_revision must be 1".to_owned());
    }
    if loaded.manifest["layer"] != json!("focused") {
        return Err("registered scene cases must be in the focused corpus layer".to_owned());
    }
    // Which of these a case leaves empty is a property of the case, not of the
    // corpus: an asset-bearing case necessarily populates `assets`.
    for field in loaded.entry.empty_fields.iter() {
        if loaded.manifest[field.as_str()] != json!([]) {
            return Err(format!("{field} must be explicitly empty for this case"));
        }
    }
    verify_assets(&loaded)?;

    let schema_id = loaded
        .entry
        .schema_id
        .as_deref()
        .ok_or_else(|| format!("case {} has no registered schema id", loaded.entry.id))?;
    verify_schema(&loaded.manifest, schema_id)?;
    verify_case_metadata(&loaded.manifest, &loaded.entry)?;
    verify_exceptions(
        &loaded.manifest["exceptions"],
        &loaded.entry.exceptions,
        &loaded.entry.variants,
    )?;
    verify_comparison(&loaded.manifest)?;
    verify_reference_identities(&loaded.manifest)?;
    let mut referenced = Vec::new();
    verify_descriptor(&loaded.root, &loaded.manifest["scene"], &mut referenced)?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["provenance"],
        &mut referenced,
    )?;
    verify_scene(&loaded)?;
    verify_variants(&loaded, &mut referenced)?;
    verify_provenance(&loaded)?;

    let subject = subject_digest(&loaded.manifest, referenced)?;
    match &loaded.manifest["approval"] {
        Value::Null if require_approval => return Err("case has no approval record".to_owned()),
        Value::Null => {}
        approval => verify_approval_for_issue(approval, &subject, &loaded.entry.approving_issue)?,
    }
    Ok(())
}

fn verify_frame(loaded: &LoadedCase, require_approval: bool) -> Result<(), String> {
    verify_manifest_digest(loaded)?;
    let manifest = object(&loaded.manifest, "frame manifest")?;
    verify_exact_keys(
        manifest,
        &[
            "approval",
            "case_id",
            "compatibility_revision",
            "expected_report",
            "options",
            "originating_issue",
            "profile",
            "provenance",
            "scene",
            "schema",
            "targets",
        ],
        "frame manifest",
    )?;
    if loaded.manifest["schema"] != json!("excalidraw-render/frame-conformance-case/v1")
        || loaded.manifest["case_id"] != json!(loaded.entry.id)
        || loaded.manifest["compatibility_revision"] != json!(1)
        || loaded.manifest["originating_issue"] != json!(format!("{REPOSITORY_ISSUE_PREFIX}97"))
        || loaded.manifest["options"]
            != json!({
                "exportBackground": true,
                "exportEmbedScene": false,
                "exportScale": 1,
                "exportWithDarkMode": false,
                "maxWidthOrHeight": null,
                "viewBackgroundColor": "#ffffff"
            })
    {
        return Err("frame conformance identity or invocation is stale".to_owned());
    }
    let profile = object(&loaded.manifest["profile"], "frame profile")?;
    verify_exact_keys(profile, &["digest", "path"], "frame profile")?;
    if profile["digest"] != json!(format!("sha256:{FRAME_PROFILE_DIGEST}"))
        || profile["path"] != json!("tests/oracle/reference-profiles/revision-1-frame.json")
    {
        return Err("frame conformance profile identity is stale".to_owned());
    }
    let profile_bytes = fs::read(repo_root().join(profile["path"].as_str().unwrap()))
        .map_err(|error| format!("failed to read frame profile: {error}"))?;
    if sha256(&profile_bytes) != FRAME_PROFILE_DIGEST {
        return Err("frame conformance profile bytes are stale".to_owned());
    }

    let mut referenced = Vec::new();
    collect_frame_descriptors(loaded, &mut referenced)?;
    if fs::read(loaded.root.join("expected-report.json"))
        .map_err(|error| format!("failed to read frame expected report: {error}"))?
        != EMPTY_REPORT
    {
        return Err("frame expected report is not the canonical empty report".to_owned());
    }
    let scene: Value = serde_json::from_slice(
        &fs::read(loaded.root.join("scene.excalidraw"))
            .map_err(|error| format!("failed to read frame scene: {error}"))?,
    )
    .map_err(|error| format!("invalid frame scene: {error}"))?;
    if scene["type"] != json!("excalidraw")
        || scene["version"] != json!(2)
        || scene["appState"]["frameRendering"]["enabled"] != json!(false)
    {
        return Err("frame conformance scene does not exercise the declared target".to_owned());
    }
    verify_frame_coverage(&scene)?;
    verify_frame_targets(loaded, &scene)?;
    verify_frame_provenance(loaded)?;
    let subject = subject_digest(&loaded.manifest, referenced)?;
    match &loaded.manifest["approval"] {
        Value::Null if require_approval => Err("frame case has no approval record".to_owned()),
        Value::Null => Ok(()),
        approval => verify_approval_for_issue(approval, &subject, &loaded.entry.approving_issue),
    }
}

fn collect_frame_descriptors(
    loaded: &LoadedCase,
    referenced: &mut Vec<Value>,
) -> Result<(), String> {
    verify_descriptor(&loaded.root, &loaded.manifest["scene"], referenced)?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["expected_report"],
        referenced,
    )?;
    verify_descriptor(&loaded.root, &loaded.manifest["provenance"], referenced)?;
    let targets = loaded.manifest["targets"]
        .as_array()
        .ok_or_else(|| "frame targets must be an array".to_owned())?;
    for target in targets {
        for format in ["png", "svg"] {
            verify_descriptor(
                &loaded.root,
                &target["outputs"][format]["artifact"],
                referenced,
            )?;
            verify_descriptor(
                &loaded.root,
                &target["outputs"][format]["raw_raster"],
                referenced,
            )?;
        }
    }
    Ok(())
}

fn verify_frame_targets(loaded: &LoadedCase, scene: &Value) -> Result<(), String> {
    let frames = scene["elements"]
        .as_array()
        .ok_or_else(|| "frame scene elements must be an array".to_owned())?
        .iter()
        .filter(|element| {
            element["isDeleted"] != json!(true)
                && matches!(element["type"].as_str(), Some("frame" | "magicframe"))
        })
        .collect::<Vec<_>>();
    let targets = loaded.manifest["targets"]
        .as_array()
        .filter(|targets| targets.len() == frames.len() && targets.len() == 7)
        .ok_or_else(|| "frame manifest must bind all seven live reference targets".to_owned())?;
    for (index, (target, frame)) in targets.iter().zip(frames).enumerate() {
        verify_exact_keys(
            object(target, "frame target")?,
            &["frame_kind", "frame_name", "frame_ordinal", "outputs"],
            "frame target",
        )?;
        if target["frame_ordinal"] != json!(index + 1)
            || target["frame_kind"] != frame["type"]
            || target["frame_name"] != frame.get("name").cloned().unwrap_or(Value::Null)
        {
            return Err("frame target identity does not match source order".to_owned());
        }
        verify_exact_keys(
            object(&target["outputs"], "frame outputs")?,
            &["png", "svg"],
            "frame outputs",
        )?;
        for format in ["png", "svg"] {
            let output = &target["outputs"][format];
            verify_exact_keys(
                object(output, "frame format output")?,
                &["artifact", "raw_raster"],
                "frame format output",
            )?;
            let artifact = object(&output["artifact"], "frame artifact")?;
            let raw = object(&output["raw_raster"], "frame raw raster")?;
            let width = u32_field(artifact, "width")?;
            let height = u32_field(artifact, "height")?;
            if raw["width"] != json!(width)
                || raw["height"] != json!(height)
                || raw["layout"] != json!("top-left-row-major")
                || raw["pixel_format"] != json!("straight-alpha-srgb-rgba8")
                || raw["byte_length"]
                    != json!(u64::from(width)
                        .saturating_mul(u64::from(height))
                        .saturating_mul(4))
            {
                return Err(format!(
                    "frame {} {format} raster shape is stale",
                    index + 1
                ));
            }
            let bytes =
                fs::read(descriptor_path(&loaded.root, &output["artifact"])?).map_err(|error| {
                    format!(
                        "failed to read frame {} {format} artifact: {error}",
                        index + 1
                    )
                })?;
            if format == "png" {
                if artifact["media_type"] != json!("image/png") {
                    return Err("frame PNG media type is stale".to_owned());
                }
                if png_dimensions(&bytes)? != (width, height) {
                    return Err(format!("frame {} PNG dimensions are stale", index + 1));
                }
            } else {
                if artifact["media_type"] != json!("image/svg+xml") {
                    return Err("frame SVG media type is stale".to_owned());
                }
                let (svg_width, svg_height) = svg_f64_dimensions(&bytes)?;
                if svg_width.round() != f64::from(width) || svg_height.round() != f64::from(height)
                {
                    return Err(format!("frame {} SVG dimensions are stale", index + 1));
                }
            }
        }
    }
    Ok(())
}

fn verify_frame_coverage(scene: &Value) -> Result<(), String> {
    let elements = scene["elements"]
        .as_array()
        .ok_or_else(|| "frame scene elements must be an array".to_owned())?;
    let by_id = |id: &str| {
        elements
            .iter()
            .find(|element| element["id"] == json!(id))
            .ok_or_else(|| format!("frame coverage element {id:?} is missing"))
    };
    let target = by_id("target-frame")?;
    let edge = by_id("edge-contact")?;
    let outside = by_id("outside-direct-member")?;
    let inner = by_id("inner-magic")?;
    let grandchild = by_id("nested-grandchild")?;
    let rotated = by_id("rotated-frame")?;
    let empty = by_id("empty-frame")?;
    let container = by_id("binding-container")?;
    let text = by_id("bound-text")?;
    let linked = by_id("linked-member")?;
    let deleted = by_id("deleted-frame")?;
    if edge["x"].as_f64()
        != target["x"]
            .as_f64()
            .zip(target["width"].as_f64())
            .map(|(x, width)| x + width)
        || outside["frameId"] != json!("target-frame")
        || outside["x"].as_f64().is_none_or(|x| x <= 120.0)
        || inner["type"] != json!("magicframe")
        || inner["frameId"] != json!("outer-frame")
        || !inner.get("name").is_none()
        || grandchild["frameId"] != json!("inner-magic")
        || rotated["angle"].as_f64().is_none_or(|angle| angle == 0.0)
        || empty["name"] != json!("")
        || container["boundElements"][0]["id"] != json!("bound-text")
        || text["containerId"] != json!("binding-container")
        || linked["frameId"] != json!("binding-frame")
        || linked["link"] != json!("https://example.com/frame")
        || deleted["isDeleted"] != json!(true)
    {
        return Err("frame conformance semantic coverage is stale".to_owned());
    }
    Ok(())
}

fn verify_frame_provenance(loaded: &LoadedCase) -> Result<(), String> {
    let provenance: Value = serde_json::from_slice(
        &fs::read(loaded.root.join("evidence/provenance.json"))
            .map_err(|error| format!("failed to read frame provenance: {error}"))?,
    )
    .map_err(|error| format!("invalid frame provenance: {error}"))?;
    if provenance["schema"] != json!("excalidraw-render/frame-reference-evidence-provenance/v1")
        || provenance["case_id"] != json!(loaded.entry.id)
        || provenance["frame_reference_profile_digest"]
            != json!(format!("sha256:{FRAME_PROFILE_DIGEST}"))
    {
        return Err("frame provenance identity is stale".to_owned());
    }
    verify_repo_descriptor(&provenance["generator"], "frame generator")?;
    verify_repo_descriptor(&provenance["runner"], "frame runner")?;
    let runs = provenance["runs"]
        .as_array()
        .filter(|runs| runs.len() == 2)
        .ok_or_else(|| "frame provenance must contain two runs".to_owned())?;
    if runs[0]["requests"] != runs[1]["requests"] {
        return Err("frame provenance request sets differ".to_owned());
    }
    let targets = loaded.manifest["targets"]
        .as_array()
        .ok_or_else(|| "frame targets must be an array".to_owned())?;
    if runs.iter().any(|run| {
        run["targets"]
            .as_array()
            .is_none_or(|run_targets| run_targets.len() != targets.len())
    }) {
        return Err("frame provenance target count is stale".to_owned());
    }
    for (index, target) in targets.iter().enumerate() {
        let first = &runs[0]["targets"][index];
        let second = &runs[1]["targets"][index];
        if first != second || first["ordinal"] != target["frame_ordinal"] {
            return Err(format!("frame {} provenance is not repeatable", index + 1));
        }
        for format in ["png", "svg"] {
            if first["outputs"][format]["artifact_sha256"]
                != target["outputs"][format]["artifact"]["sha256"]
                || first["outputs"][format]["raw_raster_sha256"]
                    != target["outputs"][format]["raw_raster"]["sha256"]
                || first["outputs"][format]["width"]
                    != target["outputs"][format]["artifact"]["width"]
                || first["outputs"][format]["height"]
                    != target["outputs"][format]["artifact"]["height"]
            {
                return Err(format!(
                    "frame {} {format} provenance does not match evidence",
                    index + 1
                ));
            }
        }
    }
    Ok(())
}

fn verify_typography(loaded: &LoadedCase, require_approval: bool) -> Result<(), String> {
    verify_manifest_digest(loaded)?;
    verify_exact_keys(
        object(&loaded.manifest, "manifest")?,
        &[
            "approval",
            "assets",
            "case_id",
            "comparison",
            "compatibility_revision",
            "coverage",
            "crops",
            "determinism",
            "exceptions",
            "expected_report",
            "feature_tags",
            "layer",
            "license",
            "originating_issue",
            "provenance",
            "purpose",
            "reference",
            "sanitization",
            "scene",
            "schema",
            "semantic_expectations",
            "variants",
        ],
        "manifest",
    )?;
    if loaded.manifest["case_id"] != json!(loaded.entry.id)
        || loaded.manifest["compatibility_revision"] != json!(1)
        || loaded.manifest["layer"] != json!("focused")
        || loaded.manifest["crops"] != json!([])
        || loaded.manifest["exceptions"] != json!([])
        || loaded.manifest["originating_issue"] != json!(format!("{REPOSITORY_ISSUE_PREFIX}58"))
    {
        return Err("typography case identity or policy metadata is invalid".to_owned());
    }
    if loaded.manifest["assets"]
        != json!([{
            "byte_length": 24956,
            "path": "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
            "role": "official-rendering-font",
            "sha256": "e4423b318e11432aff2e6e865e300b7ca270f92321a3f56632268fede01c1b48"
        }])
        || loaded.manifest["semantic_expectations"]
            != json!({
                "element_count": 1, "element_type": "text", "embedded_scene": false,
                "images": 0, "links": 0, "masks": 0, "selectable_text": true
            })
    {
        return Err("typography assets or semantic expectations are invalid".to_owned());
    }
    verify_schema_for(
        &loaded.manifest,
        "excalidraw-render/conformance-case/typography-reference/v1",
        "tests/conformance/typography-reference-case-v1.schema.json",
    )?;
    verify_comparison(&loaded.manifest)?;
    verify_reference_identities(&loaded.manifest)?;
    verify_typography_scene(loaded)?;
    verify_expected_report(loaded, &loaded.manifest["expected_report"], &json!([]))?;
    verify_typography_variants(loaded)?;
    verify_typography_provenance(loaded)?;
    let mut referenced = Vec::new();
    collect_typography_descriptors(loaded, &mut referenced)?;
    let subject = subject_digest(&loaded.manifest, referenced)?;
    match &loaded.manifest["approval"] {
        Value::Null if require_approval => Err("case has no approval record".to_owned()),
        Value::Null => Ok(()),
        approval => verify_approval_for_issue(approval, &subject, &loaded.entry.approving_issue),
    }
}

/// The typography manifest carries one `expected_report` for both formats and
/// per-format artifacts and rasters under its single variant; the approval
/// subject binds exactly those files.
fn collect_typography_descriptors(
    loaded: &LoadedCase,
    referenced: &mut Vec<Value>,
) -> Result<(), String> {
    verify_descriptor(&loaded.root, &loaded.manifest["scene"], referenced)?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["expected_report"],
        referenced,
    )?;
    verify_descriptor(&loaded.root, &loaded.manifest["provenance"], referenced)?;
    let variants = loaded.manifest["variants"]
        .as_array()
        .ok_or_else(|| "variants must be an array".to_owned())?;
    for variant in variants {
        let formats = object(&variant["formats"], "formats")?;
        for format in ["png", "svg"] {
            verify_descriptor(&loaded.root, &formats[format]["artifact"], referenced)?;
            verify_descriptor(&loaded.root, &formats[format]["raw_raster"], referenced)?;
        }
    }
    Ok(())
}

fn verify_schema(manifest: &Value, schema_id: &str) -> Result<(), String> {
    let schema_path = manifest["schema"]["path"]
        .as_str()
        .ok_or_else(|| "schema.path must be a string".to_owned())?
        .to_owned();
    verify_schema_for(manifest, schema_id, &schema_path)
}

fn verify_schema_for(manifest: &Value, schema_id: &str, schema_path: &str) -> Result<(), String> {
    let schema = object(&manifest["schema"], "schema")?;
    verify_exact_keys(schema, &["digest", "id", "path"], "schema")?;
    if schema["id"] != json!(schema_id) || schema["path"] != json!(schema_path) {
        return Err("manifest references an unknown case schema".to_owned());
    }
    let path = schema["path"].as_str().unwrap();
    let bytes = fs::read(repo_root().join(path))
        .map_err(|error| format!("failed to read {path}: {error}"))?;
    if schema["digest"] != json!(format!("sha256:{}", sha256(&bytes))) {
        return Err("case schema digest is stale".to_owned());
    }
    let definition: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid case schema JSON: {error}"))?;
    if definition["$schema"] != json!("https://json-schema.org/draft/2020-12/schema")
        || definition["type"] != json!("object")
        || definition["additionalProperties"] != json!(false)
    {
        return Err("case schema is not the recognized closed draft-2020-12 schema".to_owned());
    }
    let required: BTreeSet<&str> = definition["required"]
        .as_array()
        .ok_or_else(|| "case schema required set is missing".to_owned())?
        .iter()
        .map(|value| value.as_str().unwrap_or_default())
        .collect();
    let manifest_keys: BTreeSet<&str> = object(manifest, "manifest")?
        .keys()
        .map(String::as_str)
        .collect();
    if required != manifest_keys {
        return Err("case schema required set does not match manifest keys".to_owned());
    }
    jsonschema::draft202012::meta::validate(&definition)
        .map_err(|error| format!("case schema does not satisfy its meta-schema: {error}"))?;
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&definition)
        .map_err(|error| format!("case schema could not be compiled: {error}"))?;
    validator
        .validate(manifest)
        .map_err(|error| format!("manifest does not satisfy its declared schema: {error}"))?;
    Ok(())
}

/// Every declared asset must match a scene file payload by kind and decoded
/// bytes, and every payload must be declared exactly once.
fn verify_assets(case_data: &LoadedCase) -> Result<(), String> {
    let declared = case_data.manifest["assets"]
        .as_array()
        .ok_or_else(|| "assets must be an array".to_owned())?;
    let path = descriptor_path(&case_data.root, &case_data.manifest["scene"])?;
    let scene: Value = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("failed to read scene document: {error}"))?,
    )
    .map_err(|error| format!("invalid scene document: {error}"))?;
    let files = scene["files"]
        .as_object()
        .ok_or_else(|| "scene.files must be an object".to_owned())?;
    if declared.len() != files.len() {
        return Err(format!(
            "manifest declares {} assets; the scene carries {} file payloads",
            declared.len(),
            files.len()
        ));
    }
    for record in declared {
        let record = object(record, "asset record")?;
        verify_exact_keys(
            record,
            &["byte_length", "file_id", "media_type", "sha256"],
            "asset record",
        )?;
        let file_id = record["file_id"]
            .as_str()
            .ok_or_else(|| "asset file_id must be a string".to_owned())?;
        let file = files
            .get(file_id)
            .ok_or_else(|| format!("asset {file_id:?} is not a scene file payload"))?;
        if record["media_type"] != file["mimeType"] {
            return Err(format!(
                "asset {file_id:?} media type does not match the scene"
            ));
        }
        let data_url = file["dataURL"]
            .as_str()
            .ok_or_else(|| format!("scene file {file_id:?} has no dataURL"))?;
        let encoded = data_url
            .split_once(";base64,")
            .map(|(_, encoded)| encoded)
            .ok_or_else(|| format!("scene file {file_id:?} is not a base64 data URL"))?;
        let bytes = base64_decode(encoded)
            .ok_or_else(|| format!("scene file {file_id:?} payload is not valid base64"))?;
        if record["byte_length"] != json!(bytes.len()) || record["sha256"] != json!(sha256(&bytes))
        {
            return Err(format!(
                "asset {file_id:?} does not match its decoded payload"
            ));
        }
    }
    Ok(())
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (index, byte) in TABLE.iter().enumerate() {
        lookup[*byte as usize] = index as u8;
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut accumulator = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = lookup[byte as usize];
        if value == 255 {
            return None;
        }
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
        }
    }
    Some(out)
}

fn verify_case_metadata(manifest: &Value, entry: &CaseEntry) -> Result<(), String> {
    let coverage = manifest["coverage"]
        .as_array()
        .ok_or_else(|| "coverage must be an array".to_owned())?;
    let expected_coverage: BTreeSet<&str> = entry.coverage.iter().map(String::as_str).collect();
    if expected_coverage.is_empty() {
        return Err(format!(
            "case {} registers no coverage identities",
            entry.id
        ));
    }
    let actual_coverage: BTreeSet<&str> = coverage
        .iter()
        .map(|entry| {
            let entry = object(entry, "coverage record")?;
            verify_exact_keys(entry, &["identity", "role"], "coverage record")?;
            if entry["role"] != json!("primary") {
                return Err("coverage role must be primary".to_owned());
            }
            entry["identity"]
                .as_str()
                .ok_or_else(|| "coverage identity must be a string".to_owned())
        })
        .collect::<Result<_, _>>()?;
    if actual_coverage.len() != coverage.len() {
        return Err("coverage contains a duplicate primary record".to_owned());
    }
    if actual_coverage != expected_coverage {
        return Err("coverage identities do not match the registered case".to_owned());
    }
    let determinism = object(&manifest["determinism"], "determinism")?;
    verify_exact_keys(
        determinism,
        &["animation_offset_microseconds", "fresh_process_runs"],
        "determinism",
    )?;
    if determinism["animation_offset_microseconds"] != json!(0)
        || determinism["fresh_process_runs"] != json!(2)
    {
        return Err("determinism settings do not describe two offset-zero runs".to_owned());
    }
    let license = object(&manifest["license"], "license")?;
    verify_exact_keys(license, &["copyright", "expression"], "license")?;
    if license["expression"] != json!("MIT OR Apache-2.0")
        || license["copyright"] != json!("Copyright 2026 excalidraw-render contributors")
    {
        return Err("case license metadata is invalid".to_owned());
    }
    let originating_issue = entry
        .originating_issue
        .as_deref()
        .ok_or_else(|| format!("case {} registers no originating issue", entry.id))?;
    if manifest["originating_issue"] != json!(originating_issue) {
        return Err("originating issue is stale".to_owned());
    }
    let sanitization = object(&manifest["sanitization"], "sanitization")?;
    verify_exact_keys(sanitization, &["performed", "reason"], "sanitization")?;
    let sanitization_reason = entry
        .sanitization_reason
        .as_deref()
        .ok_or_else(|| format!("case {} registers no sanitization reason", entry.id))?;
    if sanitization["performed"] != json!(false)
        || sanitization["reason"] != json!(sanitization_reason)
    {
        return Err("sanitization metadata is invalid".to_owned());
    }
    let expectations = object(&manifest["semantic_expectations"], "semantic expectations")?;
    // the registered entry is the authority on shape as well as values, so the
    // key set follows it rather than a literal that only fits one schema
    let registered = object(
        &entry.semantic_expectations,
        "registered semantic expectations",
    )?;
    let expected_keys: Vec<&str> = registered.keys().map(String::as_str).collect();
    verify_exact_keys(expectations, &expected_keys, "semantic expectations")?;
    if Value::Object(expectations.clone()) != entry.semantic_expectations {
        return Err("semantic expectations do not match the registered case".to_owned());
    }
    Ok(())
}

/// A tolerance exception names one variant, format, viewer, and region, sets
/// exact ceilings, and names the issue that carries the decision (spec 14.4).
/// The immutable approval is the case approval itself: the record is part of
/// the manifest, so the approval subject binds it and any change to a ceiling
/// or cause invalidates the approval. The registered entry is the authority on
/// which exceptions a case carries, as it is for coverage and semantic
/// expectations, so the manifest must reproduce it exactly. Only the scope the
/// direct-PNG harness judges today is admitted: whole-artifact PNG with no
/// viewer.
fn verify_exceptions(
    exceptions: &Value,
    registered: &Value,
    variants: &Value,
) -> Result<(), String> {
    let exceptions = exceptions
        .as_array()
        .ok_or_else(|| "exceptions must be an array".to_owned())?;
    if Value::Array(exceptions.clone()) != *registered {
        return Err("exceptions do not match the registered case".to_owned());
    }
    let declared_variants: BTreeSet<&str> = variants
        .as_array()
        .ok_or_else(|| "registered variants must be an array".to_owned())?
        .iter()
        .filter_map(|variant| variant["id"].as_str())
        .collect();
    let mut excepted_variants = BTreeSet::new();
    for exception in exceptions {
        let exception = object(exception, "tolerance exception")?;
        verify_exact_keys(
            exception,
            &[
                "approving_issue",
                "format",
                "max_changed_pixels",
                "minimum_ssim_bits",
                "reason",
                "region",
                "variant",
                "viewer_profile",
            ],
            "tolerance exception",
        )?;
        let variant = exception["variant"]
            .as_str()
            .ok_or_else(|| "tolerance exception variant must be a string".to_owned())?;
        if !declared_variants.contains(variant) {
            return Err(format!(
                "tolerance exception names undeclared variant {variant:?}"
            ));
        }
        if exception["format"] != json!("png")
            || exception["region"] != json!("whole")
            || !exception["viewer_profile"].is_null()
        {
            return Err(
                "tolerance exception scope must be the whole direct PNG with no viewer".to_owned(),
            );
        }
        if !excepted_variants.insert(variant) {
            return Err(format!(
                "variant {variant:?} carries more than one tolerance exception for one scope"
            ));
        }
        if exception["max_changed_pixels"].as_u64().is_none() {
            return Err(
                "tolerance exception max_changed_pixels must be a non-negative integer".to_owned(),
            );
        }
        let bits = exception["minimum_ssim_bits"]
            .as_str()
            .filter(|bits| bits.len() == 16)
            .and_then(|bits| u64::from_str_radix(bits, 16).ok())
            .ok_or_else(|| {
                "tolerance exception minimum_ssim_bits must be 16 hex digits".to_owned()
            })?;
        if !f64::from_bits(bits).is_finite() {
            return Err("tolerance exception SSIM floor must be finite".to_owned());
        }
        if exception["reason"].as_str().is_none_or(str::is_empty) {
            return Err("tolerance exception must document a non-defect cause".to_owned());
        }
        let issue = exception["approving_issue"]
            .as_str()
            .and_then(|url| url.strip_prefix(REPOSITORY_ISSUE_PREFIX))
            .filter(|number| !number.is_empty() && !number.starts_with('0'))
            .filter(|number| number.chars().all(|character| character.is_ascii_digit()));
        if issue.is_none() {
            return Err(
                "tolerance exception approving issue must be an issue in this repository"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn verify_comparison(manifest: &Value) -> Result<(), String> {
    let comparison = object(&manifest["comparison"], "comparison")?;
    verify_exact_keys(
        comparison,
        &["comparator_profiles", "viewer_profiles"],
        "comparison",
    )?;
    if comparison["comparator_profiles"] != json!([]) {
        return Err("the evidence-only case must not claim a comparator profile".to_owned());
    }
    let viewers = comparison["viewer_profiles"]
        .as_array()
        .ok_or_else(|| "viewer_profiles must be an array".to_owned())?;
    if viewers.len() != 1 {
        return Err("exactly one evidence raster viewer profile is required".to_owned());
    }
    let viewer = object(&viewers[0], "viewer profile")?;
    verify_exact_keys(viewer, &["digest", "id", "scope"], "viewer profile")?;
    if viewer["id"] != json!("reference-evidence.browser-raster.v1")
        || viewer["scope"] != json!("reference evidence repeatability only")
    {
        return Err("viewer profile identity or scope is invalid".to_owned());
    }
    Ok(())
}

pub fn approval_subject(case_id: &str) -> Result<String, String> {
    verify(case_id, false)?;
    let loaded = load(case_id)?;
    let mut referenced = Vec::new();
    if loaded.entry.kind == "frame" {
        collect_frame_descriptors(&loaded, &mut referenced)?;
        return subject_digest(&loaded.manifest, referenced);
    }
    if loaded.entry.kind == "typography" {
        collect_typography_descriptors(&loaded, &mut referenced)?;
        return subject_digest(&loaded.manifest, referenced);
    }
    verify_descriptor(&loaded.root, &loaded.manifest["scene"], &mut referenced)?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["provenance"],
        &mut referenced,
    )?;
    collect_variant_descriptors(&loaded, &mut referenced)?;
    subject_digest(&loaded.manifest, referenced)
}

pub fn record_approval(
    case_id: &str,
    approved_by: &str,
    approved_at: &str,
    approval_url: &str,
    expected_subject: &str,
) -> Result<(), String> {
    verify(case_id, false)?;
    let mut loaded = load(case_id)?;
    if !loaded.manifest["approval"].is_null() {
        return Err("case is already approved".to_owned());
    }
    let subject = approval_subject(case_id)?;
    let expected = expected_subject
        .strip_prefix("sha256:")
        .unwrap_or(expected_subject);
    if subject != expected {
        return Err(format!(
            "approval subject mismatch: expected {expected}, computed {subject}"
        ));
    }
    let approval = json!({
        "approved_at": approved_at,
        "approved_by": approved_by,
        "approval_subject_sha256": format!("sha256:{subject}"),
        "approval_url": approval_url,
    });
    verify_approval_for_issue(&approval, &subject, &loaded.entry.approving_issue)?;
    loaded.manifest["approval"] = approval;
    let bytes = canonical_bytes(&loaded.manifest)?;
    fs::write(&loaded.manifest_path, [&bytes[..], b"\n"].concat())
        .map_err(|error| format!("failed to write manifest: {error}"))?;
    fs::write(
        loaded.root.join("manifest.sha256"),
        format!("{}\n", sha256(&bytes)),
    )
    .map_err(|error| format!("failed to write manifest digest: {error}"))?;
    verify(case_id, true)
}

struct LoadedCase {
    entry: CaseEntry,
    root: PathBuf,
    manifest_path: PathBuf,
    manifest_bytes: Vec<u8>,
    manifest: Value,
}

fn load(case_id: &str) -> Result<LoadedCase, String> {
    let entry = registered(case_id)?;
    let root = repo_root().join(&entry.relative_path);
    let manifest_path = root.join("manifest.json");
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("invalid manifest JSON: {error}"))?;
    let canonical = canonical_bytes(&manifest)?;
    if manifest_bytes != [&canonical[..], b"\n"].concat() {
        return Err("manifest.json is not canonical JSON followed by one LF".to_owned());
    }
    Ok(LoadedCase {
        entry,
        root,
        manifest_path,
        manifest_bytes,
        manifest,
    })
}

fn verify_manifest_digest(case_data: &LoadedCase) -> Result<(), String> {
    let expected = fs::read_to_string(case_data.root.join("manifest.sha256"))
        .map_err(|error| format!("failed to read manifest.sha256: {error}"))?;
    if expected
        != format!(
            "{}\n",
            sha256(case_data.manifest_bytes.strip_suffix(b"\n").unwrap())
        )
    {
        return Err("manifest.sha256 does not match canonical manifest bytes".to_owned());
    }
    Ok(())
}

fn verify_reference_identities(manifest: &Value) -> Result<(), String> {
    let reference = object(&manifest["reference"], "reference")?;
    verify_exact_keys(
        reference,
        &["asset_manifest", "profile", "scene_surface"],
        "reference",
    )?;
    let identities = [
        (
            "profile",
            "tests/oracle/reference-profiles/revision-1.sha256",
            PROFILE_DIGEST,
        ),
        (
            "asset_manifest",
            "tests/oracle/reference-profiles/revision-1-assets.sha256",
            ASSET_DIGEST,
        ),
        (
            "scene_surface",
            "tests/oracle/scene-surfaces/revision-1.sha256",
            SURFACE_DIGEST,
        ),
    ];
    for (key, sidecar, digest) in identities {
        let value = object(&reference[key], key)?;
        verify_exact_keys(value, &["digest", "path"], key)?;
        if value["digest"] != json!(format!("sha256:{digest}")) {
            return Err(format!("unexpected {key} digest"));
        }
        let actual = fs::read_to_string(repo_root().join(sidecar))
            .map_err(|error| format!("failed to read {sidecar}: {error}"))?;
        if actual.trim() != digest {
            return Err(format!("{sidecar} does not contain the approved digest"));
        }
        let path = value["path"]
            .as_str()
            .ok_or_else(|| format!("{key}.path must be a string"))?;
        let bytes = fs::read(repo_root().join(path))
            .map_err(|error| format!("failed to read {path}: {error}"))?;
        if sha256(&bytes) != digest {
            return Err(format!("{path} does not match the approved digest"));
        }
    }
    Ok(())
}

fn verify_scene(case_data: &LoadedCase) -> Result<(), String> {
    let path = descriptor_path(&case_data.root, &case_data.manifest["scene"])?;
    let scene: Value = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("failed to read scene document: {error}"))?,
    )
    .map_err(|error| format!("invalid scene document: {error}"))?;
    if scene["type"] != json!("excalidraw") || scene["version"] != json!(2) {
        return Err("scene must be a complete Excalidraw version 2 scene document".to_owned());
    }
    let elements = scene["elements"]
        .as_array()
        .ok_or_else(|| "scene.elements must be an array".to_owned())?;

    let expectations = &case_data.entry.semantic_expectations;
    // rectangle-reference/v1 declares one type and a count; focused-scene/v1
    // declares a per-type census. Derive the census from whichever is present.
    let census: Map<String, Value> = match expectations.get("element_counts") {
        Some(value) => object(value, "element_counts")?.clone(),
        None => {
            let kind = expectations["element_type"]
                .as_str()
                .ok_or_else(|| "registered element_type must be a string".to_owned())?;
            let count = expectations["element_count"]
                .as_u64()
                .ok_or_else(|| "registered element_count must be an integer".to_owned())?;
            let mut single = Map::new();
            single.insert(kind.to_owned(), json!(count));
            single
        }
    };
    let counts = &census;
    let expected_total: u64 = counts.values().filter_map(Value::as_u64).sum();
    if elements.len() as u64 != expected_total {
        return Err(format!(
            "scene has {} elements; the registered case expects {expected_total}",
            elements.len()
        ));
    }
    let mut observed: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
    for value in elements {
        let kind = value["type"]
            .as_str()
            .ok_or_else(|| "scene element type must be a string".to_owned())?;
        *observed.entry(kind).or_default() += 1;
    }
    for (kind, count) in counts {
        let want = count.as_u64().unwrap_or_default();
        let have = observed.remove(kind.as_str()).unwrap_or_default();
        if have != want {
            return Err(format!(
                "scene has {have} {kind} elements; the registered case expects {want}"
            ));
        }
    }
    if let Some((kind, count)) = observed.into_iter().next() {
        return Err(format!("scene has {count} undeclared {kind} elements"));
    }

    for (position, value) in elements.iter().enumerate() {
        let element = object(value, "scene element")?;
        for field in [
            "id",
            "type",
            "x",
            "y",
            "width",
            "height",
            "angle",
            "strokeColor",
            "backgroundColor",
            "fillStyle",
            "strokeWidth",
            "strokeStyle",
            "roughness",
            "opacity",
            "groupIds",
            "frameId",
            "index",
            "seed",
            "version",
            "versionNonce",
            "isDeleted",
            "boundElements",
            "updated",
            "link",
            "locked",
            "roundness",
        ] {
            if !element.contains_key(field) {
                return Err(format!("element {position} is missing {field}"));
            }
        }
        if element["id"].as_str().unwrap_or_default().is_empty()
            || element["seed"].as_i64().unwrap_or_default() == 0
            || element["isDeleted"] != json!(false)
        {
            return Err(format!(
                "element {position} contains a normalization repair trigger"
            ));
        }
    }

    if !scene["appState"].is_object() || !scene["files"].is_object() {
        return Err("scene must explicitly contain appState and files objects".to_owned());
    }
    Ok(())
}

fn verify_typography_scene(case_data: &LoadedCase) -> Result<(), String> {
    let path = descriptor_path(&case_data.root, &case_data.manifest["scene"])?;
    let scene: Value = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("failed to read text scene: {error}"))?,
    )
    .map_err(|error| format!("invalid text scene: {error}"))?;
    let elements = scene["elements"]
        .as_array()
        .ok_or_else(|| "text scene elements must be an array".to_owned())?;
    if scene["type"] != json!("excalidraw")
        || scene["version"] != json!(2)
        || elements.len() != 1
        || elements[0]["type"] != json!("text")
        || elements[0]["fontFamily"] != json!(5)
        || elements[0]["text"] != json!("One two three\nFour")
        || elements[0]["width"] != json!(90)
        || elements[0]["fontSize"] != json!(20)
        || !scene["appState"].is_object()
        || !scene["files"].is_object()
    {
        return Err("typography scene does not isolate the approved wrapped text".to_owned());
    }
    Ok(())
}

fn verify_typography_variants(case_data: &LoadedCase) -> Result<(), String> {
    let variants = case_data.manifest["variants"]
        .as_array()
        .ok_or_else(|| "variants must be an array".to_owned())?;
    if variants.len() != 1 || variants[0]["id"] != json!("default") {
        return Err("typography case must have one default variant".to_owned());
    }
    if variants[0]["options"]
        != json!({
            "exportBackground": true,
            "exportEmbedScene": false,
            "exportPadding": 10,
            "exportScale": 1,
            "exportWithDarkMode": false,
            "frameRendering": {"clip": true, "enabled": true, "name": true, "outline": true},
            "maxWidthOrHeight": null,
            "viewBackgroundColor": "#ffffff"
        })
    {
        return Err("typography variant options are invalid".to_owned());
    }
    let formats = object(&variants[0]["formats"], "formats")?;
    verify_exact_keys(formats, &["png", "svg"], "formats")?;
    for format in ["png", "svg"] {
        let value = object(&formats[format], format)?;
        verify_exact_keys(value, &["artifact", "raw_raster", "visual_class"], format)?;
        if value["visual_class"] != json!("geometry/image") {
            return Err(format!("{format} has the wrong visual class"));
        }
        verify_typography_format(&case_data.root, format, value)?;
    }
    Ok(())
}

fn verify_typography_format(
    root: &Path,
    format: &str,
    value: &Map<String, Value>,
) -> Result<(), String> {
    let artifact = object(&value["artifact"], "artifact")?;
    let raw = object(&value["raw_raster"], "raw raster")?;
    let width = u32_field(artifact, "width")?;
    let height = u32_field(artifact, "height")?;
    if width != u32_field(raw, "width")? || height != u32_field(raw, "height")? {
        return Err(format!("{format} typography dimensions differ"));
    }
    let raw_bytes = fs::read(descriptor_path(root, &value["raw_raster"])?)
        .map_err(|error| format!("failed to read typography raster: {error}"))?;
    if raw_bytes.len() != width as usize * height as usize * 4 {
        return Err(format!("{format} typography raster length is invalid"));
    }
    let bytes = fs::read(descriptor_path(root, &value["artifact"])?)
        .map_err(|error| format!("failed to read typography artifact: {error}"))?;
    if format == "png" {
        if png_dimensions(&bytes)? != (width, height) {
            return Err("typography PNG dimensions are invalid".to_owned());
        }
        verify_png_semantics(&bytes, &raw_bytes, false)
    } else {
        if svg_dimensions(&bytes)? != (width, height) {
            return Err("typography SVG dimensions are invalid".to_owned());
        }
        let document = roxmltree::Document::parse(
            std::str::from_utf8(&bytes).map_err(|error| format!("SVG is not UTF-8: {error}"))?,
        )
        .map_err(|error| format!("typography SVG is invalid: {error}"))?;
        let texts: Vec<_> = document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "text")
            .collect();
        if texts.len() != 2
            || texts[0].text() != Some("One two three")
            || texts[1].text() != Some("Four")
            || document.descendants().any(|node| {
                node.is_element() && matches!(node.tag_name().name(), "a" | "image" | "mask")
            })
            || !std::str::from_utf8(&bytes)
                .unwrap()
                .contains("font-family: Excalifont")
        {
            return Err("typography SVG selectable text or embedded font is invalid".to_owned());
        }
        Ok(())
    }
}

fn verify_typography_provenance(case_data: &LoadedCase) -> Result<(), String> {
    let path = descriptor_path(&case_data.root, &case_data.manifest["provenance"])?;
    let bytes = fs::read(path).map_err(|error| format!("failed to read provenance: {error}"))?;
    let provenance: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid provenance: {error}"))?;
    if bytes != [&canonical_bytes(&provenance)?[..], b"\n"].concat()
        || provenance["case_id"] != json!(case_data.entry.id)
        || provenance["generated_at"] != json!("2026-07-22")
        || provenance["png_independent_from_svg"] != json!(true)
        || provenance["reference_profile_digest"] != json!(format!("sha256:{PROFILE_DIGEST}"))
        || provenance["generation_invocation"]
            != json!(
                "cargo run --locked -p xtask -- conformance generate focused.excalifont-text.v1"
            )
        || provenance["reproduction_invocation"]
            != json!(
                "cargo run --locked -p xtask -- conformance reproduce focused.excalifont-text.v1"
            )
    {
        return Err("typography provenance identity is invalid".to_owned());
    }
    verify_repo_descriptor(&provenance["generator"], "generator")?;
    verify_repo_descriptor(&provenance["runner"], "runner")?;
    let runs = provenance["runs"]
        .as_array()
        .ok_or_else(|| "typography provenance runs are missing".to_owned())?;
    if runs.len() != 2 || runs[0]["requests"] != runs[1]["requests"] {
        return Err("typography evidence was not repeated in fresh processes".to_owned());
    }
    for format in ["png", "svg"] {
        let manifest = &case_data.manifest["variants"][0]["formats"][format];
        if runs[0]["outputs"][format]["artifact_sha256"] != manifest["artifact"]["sha256"]
            || runs[0]["outputs"][format]["raw_raster_sha256"] != manifest["raw_raster"]["sha256"]
            || runs[0]["outputs"][format] != runs[1]["outputs"][format]
        {
            return Err(format!("typography {format} evidence is not repeatable"));
        }
    }
    Ok(())
}

fn verify_expected_report(
    case_data: &LoadedCase,
    descriptor: &Value,
    declared: &Value,
) -> Result<(), String> {
    let path = descriptor_path(&case_data.root, descriptor)?;
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read expected report: {error}"))?;
    let list = declared
        .as_array()
        .map(|values| serde_json::to_string(values).unwrap_or_default())
        .unwrap_or_else(|| "[]".to_owned());
    let expected = format!(r#"{{"schema_version":1,"diagnostics":{list}}}"#);
    if bytes != expected.as_bytes() {
        return Err(format!(
            "expected report does not match the {} diagnostics the case declares",
            declared.as_array().map_or(0, Vec::len)
        ));
    }
    Ok(())
}

fn verify_variants(case_data: &LoadedCase, referenced: &mut Vec<Value>) -> Result<(), String> {
    let variants = case_data.manifest["variants"]
        .as_array()
        .ok_or_else(|| "variants must be an array".to_owned())?;
    let declared = case_data
        .entry
        .variants
        .as_array()
        .ok_or_else(|| format!("case {} registers no export variants", case_data.entry.id))?;
    if variants.len() != declared.len() {
        return Err(format!(
            "manifest carries {} variants; the registered case declares {}",
            variants.len(),
            declared.len()
        ));
    }
    for (variant, registered) in variants.iter().zip(declared) {
        if variant["id"] != registered["id"] {
            return Err(format!(
                "variant {:?} is not the registered {:?}",
                variant["id"], registered["id"]
            ));
        }
        let options = object(&variant["options"], "variant options")?;
        verify_exact_keys(
            options,
            &[
                "exportBackground",
                "exportEmbedScene",
                "exportPadding",
                "exportScale",
                "exportWithDarkMode",
                "frameRendering",
                "maxWidthOrHeight",
                "viewBackgroundColor",
            ],
            "variant options",
        )?;
        if Value::Object(options.clone()) != registered["options"] {
            return Err(format!(
                "variant {:?} options do not match the registered case",
                variant["id"]
            ));
        }
        let sizing = variant_sizing(options)?;
        let formats = object(&variant["formats"], "formats")?;
        verify_exact_keys(formats, &["png", "svg"], "formats")?;
        // The SVG viewBox is the scene's logical extent, which both formats'
        // rasterized dimensions derive from under the variant's sizing.
        let logical = {
            let svg_path = descriptor_path(&case_data.root, &formats["svg"]["artifact"])?;
            let bytes = fs::read(&svg_path)
                .map_err(|error| format!("failed to read svg artifact: {error}"))?;
            let view_box = svg_view_box(&bytes)?;
            (view_box[2], view_box[3])
        };
        for format in ["png", "svg"] {
            let value = object(&formats[format], format)?;
            verify_exact_keys(
                value,
                &["artifact", "expected_report", "raw_raster", "visual_class"],
                format,
            )?;
            if value["visual_class"] != registered["visual_class"] {
                return Err(format!("{format} has the wrong visual class"));
            }
            verify_descriptor(&case_data.root, &value["artifact"], referenced)?;
            verify_descriptor(&case_data.root, &value["raw_raster"], referenced)?;
            verify_descriptor(&case_data.root, &value["expected_report"], referenced)?;
            verify_format_dimensions(
                &case_data.root,
                format,
                value,
                sizing,
                logical,
                ExportOptions::from_variant(options),
                &case_data.entry,
            )?;
            // A raster carries no hyperlink to sanitize, so a case can legitimately
            // report diagnostics for one format and none for another.
            verify_expected_report(
                case_data,
                &value["expected_report"],
                &case_data.entry.expected_diagnostics[format],
            )?;
        }
    }
    Ok(())
}

fn collect_variant_descriptors(
    case_data: &LoadedCase,
    referenced: &mut Vec<Value>,
) -> Result<(), String> {
    let variants = case_data.manifest["variants"]
        .as_array()
        .ok_or_else(|| "variants must be an array".to_owned())?;
    for variant in variants {
        let formats = object(&variant["formats"], "formats")?;
        for format in ["png", "svg"] {
            verify_descriptor(&case_data.root, &formats[format]["artifact"], referenced)?;
            verify_descriptor(&case_data.root, &formats[format]["raw_raster"], referenced)?;
            verify_descriptor(
                &case_data.root,
                &formats[format]["expected_report"],
                referenced,
            )?;
        }
    }
    Ok(())
}

/// The two registered export options, besides sizing and dark mode, that
/// change what the official evidence must contain: whether the export paints
/// the scene background and whether it embeds the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ExportOptions {
    background: bool,
    embed_scene: bool,
}

impl ExportOptions {
    fn from_variant(options: &Map<String, Value>) -> Self {
        Self {
            background: options["exportBackground"] != json!(false),
            embed_scene: options["exportEmbedScene"] == json!(true),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_format_dimensions(
    root: &Path,
    format: &str,
    value: &Map<String, Value>,
    sizing: Sizing,
    logical: (f64, f64),
    export: ExportOptions,
    entry: &CaseEntry,
) -> Result<(), String> {
    let artifact = object(&value["artifact"], "artifact")?;
    let raw = object(&value["raw_raster"], "raw raster")?;
    let width = u32_field(artifact, "width")?;
    let height = u32_field(artifact, "height")?;
    if width != u32_field(raw, "width")? || height != u32_field(raw, "height")? {
        return Err(format!(
            "{format} artifact and raw raster dimensions differ"
        ));
    }
    if raw["layout"] != json!("top-left-row-major")
        || raw["pixel_format"] != json!("straight-alpha-srgb-rgba8")
    {
        return Err(format!(
            "{format} raw raster has the wrong canonical layout"
        ));
    }
    let raw_path = descriptor_path(root, &value["raw_raster"])?;
    let raw_bytes = fs::read(&raw_path)
        .map_err(|error| format!("failed to read {format} raw raster: {error}"))?;
    let expected_len = u64::from(width) * u64::from(height) * 4;
    if u64::try_from(raw_bytes.len()).unwrap() != expected_len {
        return Err(format!(
            "{format} raw raster length does not match its dimensions"
        ));
    }
    // Rasterizing an SVG document antialiases its outer border, so those edge
    // pixels carry partial alpha whenever the extent is not pixel-aligned. The
    // interior must still be fully opaque for a white-background case. The
    // direct-canvas PNG is pixel-aligned by construction and stays strict over
    // the whole surface, which is where an unintended hole would show. A
    // variant exported without the scene background has no opaque surface to
    // hold to; it must instead show the transparency it exists to evidence.
    let artifact_path = descriptor_path(root, &value["artifact"])?;
    let bytes = fs::read(artifact_path)
        .map_err(|error| format!("failed to read {format} artifact: {error}"))?;
    let border = u32::from(format != "png");
    if width <= 2 * border || height <= 2 * border {
        return Err(format!("{format} raster is too small to verify"));
    }
    let alpha = |row: u32, column: u32| raw_bytes[((row * width + column) as usize) * 4 + 3];
    if export.background {
        for row in border..height - border {
            for column in border..width - border {
                if alpha(row, column) != 255 {
                    return Err(format!(
                        "{format} raw raster is not opaque at {column},{row} for this white-background case"
                    ));
                }
            }
        }
    } else if !(0..height).any(|row| (0..width).any(|column| alpha(row, column) == 0)) {
        return Err(format!(
            "{format} raw raster has no transparent pixel for this transparent-background variant"
        ));
    }
    if format == "png" {
        if png_dimensions(&bytes)? != (width, height) {
            return Err("png encoded dimensions do not match the manifest".to_owned());
        }
        verify_png_extent(logical, sizing, width, height)?;
        verify_png_semantics(&bytes, &raw_bytes, export.embed_scene)?;
    } else {
        let (svg_width, svg_height) = svg_f64_dimensions(&bytes)?;
        if svg_width.round() != f64::from(width) || svg_height.round() != f64::from(height) {
            return Err("svg encoded dimensions do not match the manifest".to_owned());
        }
        verify_svg_semantics(&bytes, width, height, sizing.svg_factor(), export, entry)?;
    }
    Ok(())
}

fn verify_provenance(case_data: &LoadedCase) -> Result<(), String> {
    let path = descriptor_path(&case_data.root, &case_data.manifest["provenance"])?;
    let bytes = fs::read(path).map_err(|error| format!("failed to read provenance: {error}"))?;
    let provenance: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid provenance JSON: {error}"))?;
    if bytes != [&canonical_bytes(&provenance)?[..], b"\n"].concat() {
        return Err("provenance is not canonical JSON followed by one LF".to_owned());
    }
    let generated_at = case_data
        .entry
        .generated_at
        .as_deref()
        .ok_or_else(|| format!("case {} registers no generation date", case_data.entry.id))?;
    if provenance["schema"] != json!("excalidraw-render/reference-evidence-provenance/v1")
        || provenance["case_id"] != json!(case_data.entry.id)
        || provenance["png_independent_from_svg"] != json!(true)
    {
        return Err("provenance identity or independent PNG declaration is invalid".to_owned());
    }
    verify_exact_keys(
        object(&provenance, "provenance")?,
        &[
            "browser",
            "case_id",
            "container_image",
            "generated_at",
            "generation_invocation",
            "generator",
            "node_version",
            "png_independent_from_svg",
            "raw_raster_profile",
            "reference_profile_digest",
            "reproduction_invocation",
            "runner",
            "runs",
            "schema",
        ],
        "provenance",
    )?;
    if provenance["container_image"]
        != json!("mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9")
        || provenance["node_version"] != json!("v24.16.0")
        || provenance["generated_at"] != json!(generated_at)
        || provenance["reference_profile_digest"] != json!(format!("sha256:{PROFILE_DIGEST}"))
        || provenance["generation_invocation"]
            != json!(format!(
                "cargo run --locked -p xtask -- conformance generate {}",
                case_data.entry.id
            ))
        || provenance["reproduction_invocation"]
            != json!(format!(
                "cargo run --locked -p xtask -- conformance reproduce {}",
                case_data.entry.id
            ))
    {
        return Err("generation provenance identity is invalid".to_owned());
    }
    let browser = object(&provenance["browser"], "browser provenance")?;
    verify_exact_keys(
        browser,
        &["arguments", "context", "version"],
        "browser provenance",
    )?;
    if browser["arguments"]
        != json!([
            "--disable-gpu",
            "--font-render-hinting=none",
            "--force-color-profile=srgb"
        ])
        || browser["context"]
            != json!({
                "colorScheme": "light",
                "deviceScaleFactor": 1,
                "forcedColors": "none",
                "locale": "en-US",
                "reducedMotion": "reduce",
                "serviceWorkers": "block",
                "timezoneId": "UTC",
                "viewport": {"height": 720, "width": 1280},
            })
        || browser["version"] != json!("149.0.7827.55")
    {
        return Err("browser provenance does not match the approved reference profile".to_owned());
    }
    verify_repo_descriptor(&provenance["generator"], "generator")?;
    verify_repo_descriptor(&provenance["runner"], "runner")?;
    let raster_profile = object(&provenance["raw_raster_profile"], "raw raster profile")?;
    verify_exact_keys(
        raster_profile,
        &["definition", "digest", "id"],
        "raw raster profile",
    )?;
    let profile_digest = format!(
        "sha256:{}",
        sha256(&canonical_bytes(&raster_profile["definition"])?)
    );
    if raster_profile["id"] != json!("reference-evidence.browser-raster.v1")
        || raster_profile["digest"] != json!(profile_digest)
        || case_data.manifest["comparison"]["viewer_profiles"][0]["digest"]
            != raster_profile["digest"]
    {
        return Err("raw raster viewer profile digest is invalid".to_owned());
    }
    let runs = provenance["runs"]
        .as_array()
        .ok_or_else(|| "provenance.runs must be an array".to_owned())?;
    if runs.len() != 2
        || runs
            .iter()
            .any(|run| run["fresh_browser_process"] != json!(true))
    {
        return Err("provenance must record two fresh browser-process runs".to_owned());
    }
    if runs[0]["browser_process_id"] == runs[1]["browser_process_id"] {
        return Err("provenance runs reused a browser process ID".to_owned());
    }
    for (index, run) in runs.iter().enumerate() {
        let run = object(run, "run provenance")?;
        verify_exact_keys(
            run,
            &[
                "browser_process_id",
                "fresh_browser_process",
                "ordinal",
                "outputs",
                "requests",
            ],
            "run provenance",
        )?;
        if run["ordinal"] != json!(index + 1)
            || run["browser_process_id"].as_u64().is_none_or(|id| id == 0)
        {
            return Err("run ordinal or browser process ID is invalid".to_owned());
        }
        let outputs = object(&run["outputs"], "run outputs")?;
        verify_exact_keys(outputs, &["png", "svg"], "run outputs")?;
        for format in ["png", "svg"] {
            verify_exact_keys(
                object(&outputs[format], "run format output")?,
                &["artifact_sha256", "height", "raw_raster_sha256", "width"],
                "run format output",
            )?;
        }
        let requests = run["requests"]
            .as_array()
            .ok_or_else(|| "run requests must be an array".to_owned())?;
        if requests.is_empty()
            || requests.iter().any(|request| {
                request
                    .as_str()
                    .is_none_or(|url| !url.starts_with("http://reference.invalid/"))
            })
        {
            return Err("run request set is empty or escapes the fixed origin".to_owned());
        }
    }
    for format in ["png", "svg"] {
        let manifest_format = &case_data.manifest["variants"][0]["formats"][format];
        for field in ["artifact_sha256", "raw_raster_sha256"] {
            if runs[0]["outputs"][format][field] != runs[1]["outputs"][format][field] {
                return Err(format!("{format} {field} differs across fresh processes"));
            }
        }
        if runs[0]["outputs"][format]["artifact_sha256"] != manifest_format["artifact"]["sha256"]
            || runs[0]["outputs"][format]["raw_raster_sha256"]
                != manifest_format["raw_raster"]["sha256"]
            || runs[0]["outputs"][format]["width"] != manifest_format["artifact"]["width"]
            || runs[0]["outputs"][format]["height"] != manifest_format["artifact"]["height"]
        {
            return Err(format!(
                "{format} provenance does not match checked evidence"
            ));
        }
    }
    if runs[0]["requests"] != runs[1]["requests"] {
        return Err("request sets differ across fresh processes".to_owned());
    }
    Ok(())
}

fn verify_repo_descriptor(value: &Value, subject: &str) -> Result<(), String> {
    let descriptor = object(value, subject)?;
    verify_exact_keys(descriptor, &["byte_length", "path", "sha256"], subject)?;
    let relative = descriptor["path"]
        .as_str()
        .ok_or_else(|| format!("{subject}.path must be a string"))?;
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("unsafe repository-relative path {relative:?}"));
    }
    let root = fs::canonicalize(repo_root())
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;
    let path = fs::canonicalize(root.join(path))
        .map_err(|error| format!("failed to resolve {relative}: {error}"))?;
    if !path.starts_with(&root) {
        return Err(format!("{subject} path escapes the repository"));
    }
    let bytes = fs::read(path).map_err(|error| format!("failed to read {relative}: {error}"))?;
    if descriptor["byte_length"] != json!(bytes.len())
        || descriptor["sha256"] != json!(sha256(&bytes))
    {
        return Err(format!("{subject} descriptor is stale"));
    }
    Ok(())
}

/// Checks one approval object against its subject and the issue whose comment
/// thread must carry it: exactly the four approval keys, the subject digest,
/// a GitHub login, a calendar date, and a comment URL on that issue. Shared by
/// every approval-bearing record that is decided on one issue.
pub(crate) fn verify_approval_for_issue(
    approval: &Value,
    subject: &str,
    issue: &str,
) -> Result<(), String> {
    let approval = object(approval, "approval")?;
    verify_exact_keys(
        approval,
        &[
            "approved_at",
            "approved_by",
            "approval_subject_sha256",
            "approval_url",
        ],
        "approval",
    )?;
    if approval["approval_subject_sha256"] != json!(format!("sha256:{subject}")) {
        return Err("approval subject does not match case contents".to_owned());
    }
    let approved_by = approval["approved_by"].as_str().unwrap_or_default();
    let approved_at = approval["approved_at"].as_str().unwrap_or_default();
    let approval_url = approval["approval_url"].as_str().unwrap_or_default();
    let prefix = format!("{REPOSITORY_ISSUE_PREFIX}{issue}#issuecomment-");
    let comment_id = approval_url.strip_prefix(&prefix);
    if !valid_github_login(approved_by)
        || !valid_date(approved_at)
        || comment_id.is_none_or(str::is_empty)
        || comment_id.is_some_and(|id| !id.chars().all(|character| character.is_ascii_digit()))
    {
        return Err("approval identity, date, or URL is invalid".to_owned());
    }
    Ok(())
}

fn verify_descriptor(
    root: &Path,
    value: &Value,
    referenced: &mut Vec<Value>,
) -> Result<(), String> {
    let descriptor = object(value, "file descriptor")?;
    let expected_keys = if descriptor.contains_key("pixel_format") {
        &[
            "byte_length",
            "height",
            "layout",
            "path",
            "pixel_format",
            "sha256",
            "width",
        ][..]
    } else if descriptor.contains_key("media_type") {
        &[
            "byte_length",
            "height",
            "media_type",
            "path",
            "sha256",
            "width",
        ][..]
    } else {
        &["byte_length", "path", "sha256"][..]
    };
    verify_exact_keys(descriptor, expected_keys, "file descriptor")?;
    let path = descriptor_path(root, value)?;
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if descriptor["byte_length"] != json!(bytes.len())
        || descriptor["sha256"] != json!(sha256(&bytes))
    {
        return Err(format!("file descriptor does not match {}", path.display()));
    }
    referenced.push(json!({
        "byte_length": bytes.len(),
        "path": descriptor["path"],
        "sha256": descriptor["sha256"],
    }));
    Ok(())
}

fn descriptor_path(root: &Path, value: &Value) -> Result<PathBuf, String> {
    let relative = value["path"]
        .as_str()
        .ok_or_else(|| "file descriptor path must be a string".to_owned())?;
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("unsafe case-relative path {relative:?}"));
    }
    let root =
        fs::canonicalize(root).map_err(|error| format!("failed to resolve case root: {error}"))?;
    let joined = fs::canonicalize(root.join(path))
        .map_err(|error| format!("failed to resolve {relative}: {error}"))?;
    if !joined.starts_with(&root) {
        return Err(format!("case path escapes its directory: {relative}"));
    }
    Ok(joined)
}

fn subject_digest(manifest: &Value, mut files: Vec<Value>) -> Result<String, String> {
    files.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    files.dedup_by(|left, right| left["path"] == right["path"]);
    let mut manifest = manifest.clone();
    manifest
        .as_object_mut()
        .ok_or_else(|| "manifest must be an object".to_owned())?
        .remove("approval");
    Ok(sha256(&canonical_bytes(&json!({
        "files": files,
        "manifest": manifest,
        "schema": APPROVAL_SCHEMA,
    }))?))
}

/// Canonical JSON for approval subjects and digest sidecars, shared by every
/// evidence family that is not pinned by an approval record. Object keys must
/// be ASCII so byte-order key sorting is a total canonical ordering.
pub(crate) fn canonical_bytes(value: &Value) -> Result<Vec<u8>, String> {
    let mut output = String::new();
    write_canonical(value, &mut output)?;
    Ok(output.into_bytes())
}

fn write_canonical(value: &Value, output: &mut String) -> Result<(), String> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => output.push_str(
            &serde_json::to_string(value)
                .map_err(|error| format!("invalid JSON string: {error}"))?,
        ),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical(value, output)?;
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys: Vec<&String> = values.keys().collect();
            if keys.iter().any(|key| !key.is_ascii()) {
                return Err("manifest keys must be ASCII for canonical ordering".to_owned());
            }
            keys.sort();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key).unwrap());
                output.push(':');
                write_canonical(&values[key], output)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

pub(crate) fn object<'a>(
    value: &'a Value,
    subject: &str,
) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{subject} must be an object"))
}

fn verify_exact_keys(
    object: &Map<String, Value>,
    expected: &[&str],
    subject: &str,
) -> Result<(), String> {
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    if actual != expected {
        return Err(format!("{subject} keys differ: {actual:?} != {expected:?}"));
    }
    Ok(())
}

fn expect_string(value: &Value, key: &str, expected: &str) -> Result<(), String> {
    if value[key] != json!(expected) {
        return Err(format!("{key} must be {expected:?}"));
    }
    Ok(())
}

fn u32_field(value: &Map<String, Value>, key: &str) -> Result<u32, String> {
    value[key]
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| format!("{key} must be a u32"))
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Reads a repository-relative file named by an evidence manifest, refusing
/// absolute paths, parent components, and symlinks that escape the repository.
pub(crate) fn read_repository_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe repository-relative path {relative:?}"));
    }
    let root = fs::canonicalize(root)
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;
    let resolved = fs::canonicalize(root.join(path))
        .map_err(|error| format!("failed to resolve {relative}: {error}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!("path escapes repository: {relative}"));
    }
    fs::read(resolved).map_err(|error| format!("failed to read {relative}: {error}"))
}

/// The keyword of the `tEXt` chunk the reference's `encodePngMetadata` writes
/// before `IEND` when `exportEmbedScene` is on, and the MIME type its SVG
/// `payload-type` comment names.
const EMBEDDED_SCENE_MIME_TYPE: &str = "application/vnd.excalidraw+json";

fn verify_png_semantics(
    bytes: &[u8],
    expected_rgba: &[u8],
    embed_scene: bool,
) -> Result<(), String> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("PNG evidence could not be decoded: {error}"))?;
    let mut decoded = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut decoded)
        .map_err(|error| format!("PNG evidence frame is invalid: {error}"))?;
    if output.color_type != png::ColorType::Rgba || output.bit_depth != png::BitDepth::Eight {
        return Err("PNG evidence must be untagged RGBA8".to_owned());
    }
    if &decoded[..output.buffer_size()] != expected_rgba {
        return Err("decoded PNG pixels do not match the checked raw raster".to_owned());
    }
    let mut offset = 8;
    let mut chunk_types = Vec::new();
    let mut embedded_scenes = 0;
    while offset < bytes.len() {
        if offset + 12 > bytes.len() {
            return Err("truncated PNG chunk".to_owned());
        }
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset + 12 + length;
        if end > bytes.len() {
            return Err("invalid PNG chunk length".to_owned());
        }
        let chunk = &bytes[offset + 4..offset + 8];
        chunk_types.push(chunk.to_vec());
        // The one metadata chunk the reference writes is the embedded scene,
        // a `tEXt` keyed by the Excalidraw MIME type, and only when the
        // variant asks for it; every other metadata chunk is unexpected.
        let embedded_scene = embed_scene
            && chunk == b"tEXt"
            && bytes[offset + 8..end - 4]
                .split(|byte| *byte == 0)
                .next()
                .is_some_and(|keyword| keyword == EMBEDDED_SCENE_MIME_TYPE.as_bytes());
        if embedded_scene {
            embedded_scenes += 1;
        } else if matches!(
            chunk,
            b"tEXt" | b"zTXt" | b"iTXt" | b"iCCP" | b"sRGB" | b"gAMA" | b"cHRM" | b"eXIf" | b"tIME"
        ) {
            return Err(format!(
                "unexpected PNG metadata chunk {}",
                String::from_utf8_lossy(chunk)
            ));
        }
        offset = end;
    }
    if embed_scene && embedded_scenes != 1 {
        return Err(format!(
            "PNG evidence carries {embedded_scenes} embedded-scene tEXt chunks; the variant embeds the scene once"
        ));
    }
    if chunk_types.first().map(Vec::as_slice) != Some(&b"IHDR"[..])
        || chunk_types.last().map(Vec::as_slice) != Some(&b"IEND"[..])
        || !chunk_types.iter().any(|chunk| chunk == b"IDAT")
    {
        return Err("PNG evidence has an invalid critical chunk sequence".to_owned());
    }
    Ok(())
}

/// The one sizing mode a registered variant selects, as the reference wrapper
/// and the renderer both understand it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Sizing {
    Scale(f64),
    MaxDimension(f64),
}

impl Sizing {
    /// The factor the wrapper applied to its canvas: the explicit scale, or
    /// `maximum / largest` when the maximum is below the largest logical
    /// dimension. Maximum dimension is downscale-only and never upscales.
    fn raster_factor(self, logical: (f64, f64)) -> f64 {
        match self {
            Self::Scale(scale) => scale,
            Self::MaxDimension(maximum) => {
                let largest = logical.0.max(logical.1);
                if maximum < largest {
                    maximum / largest
                } else {
                    1.0
                }
            }
        }
    }

    /// The factor applied to the SVG root. A maximum-dimension export leaves
    /// the official SVG untouched.
    fn svg_factor(self) -> f64 {
        match self {
            Self::Scale(scale) => scale,
            Self::MaxDimension(_) => 1.0,
        }
    }
}

/// A variant selects exactly one sizing mode, and whichever it selects must be
/// a finite positive number.
fn variant_sizing(options: &Map<String, Value>) -> Result<Sizing, String> {
    let positive = |key: &str| -> Result<Option<f64>, String> {
        match &options[key] {
            Value::Null => Ok(None),
            value => value
                .as_f64()
                .filter(|number| number.is_finite() && *number > 0.0)
                .map(Some)
                .ok_or_else(|| format!("{key} must be null or a finite positive number")),
        }
    };
    match (positive("exportScale")?, positive("maxWidthOrHeight")?) {
        (Some(scale), None) => Ok(Sizing::Scale(scale)),
        (None, Some(maximum)) => Ok(Sizing::MaxDimension(maximum)),
        _ => Err("exactly one sizing mode must be selected".to_owned()),
    }
}

/// The reference wrapper sizes its canvas as the logical extent times the
/// raster factor and lets the canvas truncate, so the official PNG must be
/// exactly that truncation of the SVG's logical `viewBox`. This is the check
/// that catches an upscaled or rounded raster that its own header would not.
fn verify_png_extent(
    logical: (f64, f64),
    sizing: Sizing,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let factor = sizing.raster_factor(logical);
    let expected = ((logical.0 * factor).trunc(), (logical.1 * factor).trunc());
    if expected != (f64::from(width), f64::from(height)) {
        return Err(format!(
            "PNG is {width}x{height}; the logical extent under {sizing:?} truncates to {}x{}",
            expected.0, expected.1
        ));
    }
    Ok(())
}

/// The reference export keeps the SVG `viewBox` in logical units and multiplies
/// only `width` and `height` by the export scale; a maximum-dimension export
/// leaves the SVG untouched. Chromium rasterizes the document at its rounded
/// declared size, so both the root and the scaled logical extent must round to
/// the rasterized dimensions.
fn verify_svg_root_extent(
    root_width: f64,
    root_height: f64,
    view_box: &[f64],
    export_scale: f64,
    width: u32,
    height: u32,
) -> Result<(), String> {
    if root_width.round() != f64::from(width) || root_height.round() != f64::from(height) {
        return Err("SVG root dimensions do not match the rasterized size".to_owned());
    }
    if view_box.len() != 4
        || view_box[0] != 0.0
        || view_box[1] != 0.0
        || (view_box[2] * export_scale).round() != f64::from(width)
        || (view_box[3] * export_scale).round() != f64::from(height)
    {
        return Err(
            "SVG viewBox scaled by the export scale does not match the rasterized size".to_owned(),
        );
    }
    Ok(())
}

fn verify_svg_semantics(
    bytes: &[u8],
    width: u32,
    height: u32,
    export_scale: f64,
    export: ExportOptions,
    entry: &CaseEntry,
) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("SVG is not UTF-8: {error}"))?;
    let document = roxmltree::Document::parse(text)
        .map_err(|error| format!("SVG evidence is not well-formed XML: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
    {
        return Err("SVG root semantics are invalid".to_owned());
    }
    let attribute = |name: &str| -> Result<f64, String> {
        root.attribute(name)
            .ok_or_else(|| format!("SVG root is missing {name}"))?
            .parse::<f64>()
            .map_err(|error| format!("SVG root {name} is not a number: {error}"))
    };
    let view_box: Vec<f64> = root
        .attribute("viewBox")
        .ok_or_else(|| "SVG root is missing viewBox".to_owned())?
        .split_whitespace()
        .map(|part| part.parse::<f64>().map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()
        .map_err(|error| format!("SVG viewBox is not numeric: {error}"))?;
    verify_svg_root_extent(
        attribute("width")?,
        attribute("height")?,
        &view_box,
        export_scale,
        width,
        height,
    )?;
    let element_count = |name| {
        document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == name)
            .count()
    };
    let declared = object(&entry.svg_element_counts, "svg_element_counts")?;
    for name in ["rect", "path", "a", "clipPath", "image", "mask", "text"] {
        let mut expected = declared.get(name).and_then(Value::as_u64).unwrap_or(0);
        // The registered counts describe the default export, whose one
        // background `<rect>` the reference emits only when the export paints
        // the scene background; a variant without it has exactly one fewer.
        if name == "rect" && !export.background {
            expected = expected.checked_sub(1).ok_or_else(|| {
                "the registered svg_element_counts declare no background rect for the \
                 transparent-background variant to omit"
                    .to_owned()
            })?;
        }
        let observed = element_count(name) as u64;
        if observed != expected {
            return Err(format!(
                "SVG contains {observed} {name} elements; the registered case declares {expected}"
            ));
        }
    }
    verify_svg_metadata(&document, export.embed_scene)
}

/// The `<metadata>` element every reference SVG carries: empty for the default
/// export, and for an `exportEmbedScene` variant the `encodeSvgMetadata`
/// payload — the type, version, start and end markers as comments around the
/// encoded scene text — and nothing else in either case.
fn verify_svg_metadata(document: &roxmltree::Document, embed_scene: bool) -> Result<(), String> {
    let metadata = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "metadata")
        .ok_or_else(|| "SVG metadata marker is missing".to_owned())?;
    if metadata.children().any(|child| child.is_element()) {
        return Err("SVG metadata carries an element".to_owned());
    }
    let payload_text = metadata
        .children()
        .any(|child| child.is_text() && child.text().is_some_and(|text| !text.trim().is_empty()));
    let comments: Vec<&str> = metadata
        .children()
        .filter(|child| child.is_comment())
        .filter_map(|child| child.text())
        .map(str::trim)
        .collect();
    if embed_scene {
        let expected = [
            format!("payload-type:{EMBEDDED_SCENE_MIME_TYPE}"),
            "payload-version:2".to_owned(),
            "payload-start".to_owned(),
            "payload-end".to_owned(),
        ];
        if comments != expected || !payload_text {
            return Err(
                "SVG metadata does not carry the reference's embedded-scene payload comments and text"
                    .to_owned(),
            );
        }
    } else if payload_text || !comments.is_empty() {
        return Err("SVG unexpectedly embeds scene metadata".to_owned());
    }
    Ok(())
}

pub(crate) fn valid_github_login(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

pub(crate) fn valid_date(value: &str) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let Ok(year) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(month) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(day) = parts[2].parse::<u32>() else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err("invalid PNG evidence".to_owned());
    }
    Ok((
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    ))
}

fn svg_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("SVG is not UTF-8: {error}"))?;
    let start = text
        .find("<svg")
        .ok_or_else(|| "SVG root is missing".to_owned())?;
    let end = text[start..]
        .find('>')
        .map(|offset| start + offset)
        .ok_or_else(|| "SVG root is incomplete".to_owned())?;
    let root = &text[start..=end];
    Ok((
        svg_u32_attribute(root, "width")?,
        svg_u32_attribute(root, "height")?,
    ))
}

/// The root `viewBox` as four numbers; the last two are the logical extent.
fn svg_view_box(bytes: &[u8]) -> Result<[f64; 4], String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("SVG is not UTF-8: {error}"))?;
    let document = roxmltree::Document::parse(text)
        .map_err(|error| format!("SVG evidence is not well-formed XML: {error}"))?;
    let values: Vec<f64> = document
        .root_element()
        .attribute("viewBox")
        .ok_or_else(|| "SVG root is missing viewBox".to_owned())?
        .split_whitespace()
        .map(|part| part.parse::<f64>().map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()
        .map_err(|error| format!("SVG viewBox is not numeric: {error}"))?;
    <[f64; 4]>::try_from(values).map_err(|_| "SVG viewBox does not have four values".to_owned())
}

fn svg_f64_dimensions(bytes: &[u8]) -> Result<(f64, f64), String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("SVG is not UTF-8: {error}"))?;
    let start = text
        .find("<svg")
        .ok_or_else(|| "SVG root is missing".to_owned())?;
    let end = text[start..]
        .find('>')
        .map(|offset| start + offset)
        .ok_or_else(|| "SVG root is incomplete".to_owned())?;
    let root = &text[start..=end];
    Ok((
        svg_f64_attribute(root, "width")?,
        svg_f64_attribute(root, "height")?,
    ))
}

fn svg_u32_attribute(root: &str, name: &str) -> Result<u32, String> {
    let marker = format!(r#"{name}=""#);
    let value = root
        .split_once(&marker)
        .and_then(|(_, tail)| tail.split_once('"'))
        .map(|(value, _)| value)
        .ok_or_else(|| format!("SVG root has no {name}"))?;
    value
        .parse()
        .map_err(|_| format!("SVG {name} is not an integer: {value}"))
}

fn svg_f64_attribute(root: &str, name: &str) -> Result<f64, String> {
    let marker = format!(r#"{name}=""#);
    let value = root
        .split_once(&marker)
        .and_then(|(_, tail)| tail.split_once('"'))
        .map(|(value, _)| value)
        .ok_or_else(|| format!("SVG root has no {name}"))?;
    let value: f64 = value
        .parse()
        .map_err(|_| format!("SVG {name} is not numeric: {value}"))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("SVG {name} is not positive and finite"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        verify_exceptions, verify_png_extent, verify_png_semantics, verify_svg_metadata,
        verify_svg_root_extent, Sizing, EMBEDDED_SCENE_MIME_TYPE,
    };

    /// A one-pixel untagged RGBA8 PNG and its raw raster, with `chunks`
    /// spliced in before `IEND` as the reference's `encodePngMetadata` does.
    fn png_with_chunks(chunks: &[(&[u8; 4], &[u8])]) -> (Vec<u8>, Vec<u8>) {
        let rgba = vec![0x12, 0x34, 0x56, 0xff];
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("header writes");
            writer.write_image_data(&rgba).expect("pixels write");
        }
        let iend = bytes.len() - 12;
        let mut spliced = bytes[..iend].to_vec();
        for (kind, data) in chunks {
            spliced.extend_from_slice(&(data.len() as u32).to_be_bytes());
            spliced.extend_from_slice(*kind);
            spliced.extend_from_slice(data);
            // The verifier reads chunk boundaries, not CRCs.
            spliced.extend_from_slice(&[0; 4]);
        }
        spliced.extend_from_slice(&bytes[iend..]);
        (spliced, rgba)
    }

    #[test]
    fn png_semantics_admit_the_embedded_scene_chunk_only_for_an_embedding_variant() {
        let (plain, rgba) = png_with_chunks(&[]);
        verify_png_semantics(&plain, &rgba, false).expect("a plain PNG verifies");
        assert!(verify_png_semantics(&plain, &rgba, true)
            .expect_err("an embedding variant must carry the scene")
            .contains("0 embedded-scene tEXt chunks"));

        let payload = [
            EMBEDDED_SCENE_MIME_TYPE.as_bytes(),
            b"\0{\"encoded\":\"...\"}",
        ]
        .concat();
        let (embedded, rgba) = png_with_chunks(&[(b"tEXt", &payload)]);
        verify_png_semantics(&embedded, &rgba, true).expect("the reference's scene chunk verifies");
        assert!(verify_png_semantics(&embedded, &rgba, false)
            .expect_err("a default export carries no metadata chunk")
            .contains("unexpected PNG metadata chunk tEXt"));

        let (foreign, rgba) = png_with_chunks(&[(b"tEXt", b"Comment\0anything")]);
        assert!(verify_png_semantics(&foreign, &rgba, true)
            .expect_err("only the Excalidraw-keyed chunk is the embedded scene")
            .contains("unexpected PNG metadata chunk tEXt"));
    }

    #[test]
    fn svg_metadata_carries_the_embedded_scene_only_for_an_embedding_variant() {
        let verify = |metadata: &str, embed_scene: bool| {
            let text = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{metadata}</svg>");
            let document = roxmltree::Document::parse(&text).expect("test document parses");
            verify_svg_metadata(&document, embed_scene)
        };
        verify("<metadata/>", false).expect("an empty marker verifies");
        assert!(verify("<metadata/>", true)
            .expect_err("an embedding variant must carry the payload")
            .contains("embedded-scene payload"));

        let embedded = "<metadata><!-- payload-type:application/vnd.excalidraw+json -->\
                        <!-- payload-version:2 --><!-- payload-start -->eJzT0yMAAGTvBe8=\
                        <!-- payload-end --></metadata>";
        verify(embedded, true).expect("the reference's payload verifies");
        assert!(verify(embedded, false)
            .expect_err("a default export embeds nothing")
            .contains("unexpectedly embeds"));

        let markers_only = "<metadata><!-- payload-type:application/vnd.excalidraw+json -->\
                            <!-- payload-version:2 --><!-- payload-start --><!-- payload-end -->\
                            </metadata>";
        assert!(verify(markers_only, true).is_err());
        assert!(verify("<metadata><x/></metadata>", true).is_err());
        assert!(verify("<metadata><x/></metadata>", false).is_err());
    }

    fn record() -> Value {
        json!({
            "approving_issue": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/104",
            "format": "png",
            "max_changed_pixels": 188,
            "minimum_ssim_bits": "3fef9fd8a6b58839",
            "reason": "sub-pixel glyph coverage on exact glyph bounding boxes",
            "region": "whole",
            "variant": "default",
            "viewer_profile": null,
        })
    }

    fn variants() -> Value {
        json!([{"id": "default"}, {"id": "dark-mode"}])
    }

    /// The rejection must come from the rule under test, not from an earlier
    /// check tripping over the same mutation.
    fn assert_rejected(manifest: Value, registered: Value, rule: &str) {
        let error = verify_exceptions(&manifest, &registered, &variants())
            .expect_err("an invalid exception record must fail closed");
        assert!(
            error.contains(rule),
            "expected a rejection mentioning {rule:?}, got {error:?}"
        );
    }

    /// A record that is registered verbatim, so only the record's own content
    /// is under test.
    fn assert_registered_rejected(record: Value, rule: &str) {
        assert_rejected(json!([record.clone()]), json!([record]), rule);
    }

    /// The reference wrapper sizes its canvas as the logical extent times the
    /// scale and lets the canvas truncate, so the freedraw case (204 x 245.424)
    /// is 408 x 490 at scale 2 and 132 x 160 under a 160-pixel maximum.
    #[test]
    fn png_extent_accepts_the_truncated_scaled_extent() {
        verify_png_extent((204.0, 245.42399999999998), Sizing::Scale(2.0), 408, 490)
            .expect("scale 2 truncates 490.848 to 490");
        verify_png_extent(
            (204.0, 245.42399999999998),
            Sizing::MaxDimension(160.0),
            132,
            160,
        )
        .expect("a 160-pixel maximum downscales the largest dimension to exactly 160");
        verify_png_extent((340.0, 275.0), Sizing::MaxDimension(200.0), 200, 161)
            .expect("a 200-pixel maximum truncates 161.76 to 161");
        verify_png_extent((730.0, 362.85758946736456), Sizing::Scale(1.0), 730, 362)
            .expect("the default export truncates a fractional extent");
    }

    /// Maximum dimension is downscale-only and never upscales.
    #[test]
    fn png_extent_treats_a_maximum_above_the_extent_as_a_no_op() {
        verify_png_extent((340.0, 275.0), Sizing::MaxDimension(400.0), 340, 275)
            .expect("a maximum above the largest dimension leaves the raster unscaled");
        let error = verify_png_extent((340.0, 275.0), Sizing::MaxDimension(400.0), 400, 323)
            .expect_err("an upscaled raster is rejected");
        assert!(error.contains("PNG"), "{error}");
    }

    #[test]
    fn png_extent_rejects_a_rounded_rather_than_truncated_raster() {
        let error = verify_png_extent((204.0, 245.42399999999998), Sizing::Scale(2.0), 408, 491)
            .expect_err("491 is the rounded height, which is the SVG rule, not the PNG rule");
        assert!(error.contains("PNG"), "{error}");
    }

    /// Chromium rounds an SVG image's natural size, so a fractional extent
    /// rasterizes to the nearest pixel: the arrow suite's 362.857 is 363 tall.
    #[test]
    fn svg_root_extent_accepts_the_rounded_unscaled_extent() {
        verify_svg_root_extent(
            730.0,
            362.85758946736456,
            &[0.0, 0.0, 730.0, 362.85758946736456],
            1.0,
            730,
            363,
        )
        .expect("an unscaled root whose extent rounds to the raster size verifies");
    }

    /// An exportScale multiplies width and height but leaves the viewBox in
    /// logical units, so the freedraw case at scale 2 is 408 x 491 over a
    /// 204 x 245.424 viewBox.
    #[test]
    fn svg_root_extent_accepts_a_scaled_root_over_its_logical_view_box() {
        verify_svg_root_extent(
            408.0,
            490.848,
            &[0.0, 0.0, 204.0, 245.42399999999998],
            2.0,
            408,
            491,
        )
        .expect("a scaled root verifies against its logical viewBox");
    }

    #[test]
    fn svg_root_extent_rejects_a_view_box_that_is_not_the_logical_extent() {
        // The viewBox was scaled along with the root, which is not what the
        // reference export does.
        let error =
            verify_svg_root_extent(408.0, 490.848, &[0.0, 0.0, 408.0, 490.848], 2.0, 408, 491)
                .expect_err("a scaled viewBox is rejected");
        assert!(error.contains("viewBox"), "{error}");
        // The root does not match the rasterized size at all.
        let error =
            verify_svg_root_extent(204.0, 245.424, &[0.0, 0.0, 204.0, 245.424], 1.0, 408, 491)
                .expect_err("a root smaller than its raster is rejected");
        assert!(error.contains("root dimensions"), "{error}");
        // A viewBox with an origin or the wrong arity.
        let error =
            verify_svg_root_extent(204.0, 245.424, &[1.0, 0.0, 204.0, 245.424], 1.0, 204, 245)
                .expect_err("a viewBox with an origin is rejected");
        assert!(error.contains("viewBox"), "{error}");
    }

    #[test]
    fn exceptions_accept_the_registered_record_for_a_declared_variant() {
        verify_exceptions(&json!([record()]), &json!([record()]), &variants())
            .expect("a registered, well-formed exception verifies");
        verify_exceptions(&json!([]), &json!([]), &variants())
            .expect("a case with no exceptions verifies");
    }

    #[test]
    fn exceptions_must_match_the_registered_case_exactly() {
        let rule = "do not match the registered case";
        assert_rejected(json!([record()]), json!([]), rule);
        assert_rejected(json!([]), json!([record()]), rule);
        let mut relaxed = record();
        relaxed["max_changed_pixels"] = json!(189);
        assert_rejected(json!([relaxed]), json!([record()]), rule);
    }

    #[test]
    fn exceptions_reject_an_undeclared_variant() {
        let mut record = record();
        record["variant"] = json!("high-dpi");
        assert_registered_rejected(record, "undeclared variant");
    }

    #[test]
    fn exceptions_reject_two_records_for_one_scope() {
        let mut second = record();
        second["max_changed_pixels"] = json!(1);
        assert_rejected(
            json!([record(), second.clone()]),
            json!([record(), second]),
            "more than one tolerance exception for one scope",
        );
    }

    #[test]
    fn exceptions_reject_a_non_finite_ssim_floor() {
        let mut record = record();
        record["minimum_ssim_bits"] = json!("7ff8000000000000");
        assert_registered_rejected(record, "SSIM floor must be finite");
        let mut record = self::record();
        record["minimum_ssim_bits"] = json!("3fef");
        assert_registered_rejected(record, "16 hex digits");
    }

    #[test]
    fn exceptions_reject_an_approving_issue_outside_this_repository() {
        for url in [
            "https://github.com/excalidraw/excalidraw/issues/1",
            "https://github.com/sagikazarmark/excalidraw-render.orig/issues/",
            "https://github.com/sagikazarmark/excalidraw-render.orig/issues/0104",
            "https://github.com/sagikazarmark/excalidraw-render.orig/pull/104",
        ] {
            let mut record = record();
            record["approving_issue"] = json!(url);
            assert_registered_rejected(
                record,
                "approving issue must be an issue in this repository",
            );
        }
    }

    #[test]
    fn exceptions_reject_scopes_the_harness_does_not_judge() {
        for (key, value) in [
            ("format", json!("svg")),
            ("region", json!("crop-1")),
            ("viewer_profile", json!("canonical-svg.chromium.v1")),
        ] {
            let mut record = record();
            record[key] = value;
            assert_registered_rejected(record, "whole direct PNG with no viewer");
        }
    }

    #[test]
    fn exceptions_reject_an_undocumented_cause() {
        let mut record = record();
        record["reason"] = json!("");
        assert_registered_rejected(record, "must document a non-defect cause");
    }

    #[test]
    fn exceptions_reject_unknown_or_missing_keys() {
        let mut extra = record();
        extra["visual_class"] = json!("geometry/image");
        assert_registered_rejected(extra, "tolerance exception keys differ");
        let mut missing = record();
        missing.as_object_mut().unwrap().remove("reason");
        assert_registered_rejected(missing, "tolerance exception keys differ");
    }
}
