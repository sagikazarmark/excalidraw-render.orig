use std::collections::BTreeSet;
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const CASE_ID: &str = "focused.rectangle-outline.v1";
const CASE_RELATIVE_PATH: &str = "tests/conformance/cases/focused/rectangle-outline";
const TYPOGRAPHY_CASE_ID: &str = "focused.excalifont-text.v1";
const TYPOGRAPHY_CASE_RELATIVE_PATH: &str = "tests/conformance/cases/focused/excalifont-text";
const MANIFEST_SCHEMA: &str = "excalidraw-render/conformance-case/rectangle-reference/v1";
const APPROVAL_SCHEMA: &str = "excalidraw-render/conformance-approval-subject/v1";
const EMPTY_REPORT: &[u8] = br#"{"schema_version":1,"diagnostics":[]}"#;
const PROFILE_DIGEST: &str = "1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
const ASSET_DIGEST: &str = "02685099e6a12632d77a3cac4c6d3acef291520e4fef93a675ee8be471245d9a";
const SURFACE_DIGEST: &str = "d7dadf8165ef3e42a025e68ec0c1f0f67cc63c7c745199e85442e6938c0c5ade";

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is a workspace-root child")
        .to_owned()
}

pub fn verify(case_id: &str, require_approval: bool) -> Result<(), String> {
    let loaded = load(case_id)?;
    if case_id == TYPOGRAPHY_CASE_ID {
        return verify_typography(&loaded, require_approval);
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
    expect_string(&loaded.manifest, "case_id", CASE_ID)?;
    if loaded.manifest["compatibility_revision"] != json!(1) {
        return Err("compatibility_revision must be 1".to_owned());
    }
    if loaded.manifest["layer"] != json!("focused") {
        return Err("the first case must be in the focused corpus layer".to_owned());
    }
    for field in ["assets", "crops", "exceptions"] {
        if loaded.manifest[field] != json!([]) {
            return Err(format!("{field} must be explicitly empty for this case"));
        }
    }

    verify_schema(&loaded.manifest)?;
    verify_case_metadata(&loaded.manifest)?;
    verify_comparison(&loaded.manifest)?;
    verify_reference_identities(&loaded.manifest)?;
    let mut referenced = Vec::new();
    verify_descriptor(&loaded.root, &loaded.manifest["scene"], &mut referenced)?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["expected_report"],
        &mut referenced,
    )?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["provenance"],
        &mut referenced,
    )?;
    verify_scene(&loaded)?;
    verify_expected_report(&loaded)?;
    verify_variants(&loaded, &mut referenced)?;
    verify_provenance(&loaded)?;

    let subject = subject_digest(&loaded.manifest, referenced)?;
    match &loaded.manifest["approval"] {
        Value::Null if require_approval => return Err("case has no approval record".to_owned()),
        Value::Null => {}
        approval => verify_approval(approval, &subject)?,
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
    if loaded.manifest["case_id"] != json!(TYPOGRAPHY_CASE_ID)
        || loaded.manifest["compatibility_revision"] != json!(1)
        || loaded.manifest["layer"] != json!("focused")
        || loaded.manifest["crops"] != json!([])
        || loaded.manifest["exceptions"] != json!([])
        || loaded.manifest["originating_issue"]
            != json!("https://github.com/sagikazarmark/excalidraw-render.orig/issues/58")
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
    let mut referenced = Vec::new();
    verify_descriptor(&loaded.root, &loaded.manifest["scene"], &mut referenced)?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["expected_report"],
        &mut referenced,
    )?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["provenance"],
        &mut referenced,
    )?;
    verify_typography_scene(loaded)?;
    verify_expected_report(loaded)?;
    verify_typography_variants(loaded, &mut referenced)?;
    verify_typography_provenance(loaded)?;
    let subject = subject_digest(&loaded.manifest, referenced)?;
    match &loaded.manifest["approval"] {
        Value::Null if require_approval => Err("case has no approval record".to_owned()),
        Value::Null => Ok(()),
        approval => verify_approval_for_issue(approval, &subject, "58"),
    }
}

fn verify_schema(manifest: &Value) -> Result<(), String> {
    verify_schema_for(
        manifest,
        MANIFEST_SCHEMA,
        "tests/conformance/rectangle-reference-case-v1.schema.json",
    )
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

fn verify_case_metadata(manifest: &Value) -> Result<(), String> {
    let coverage = manifest["coverage"]
        .as_array()
        .ok_or_else(|| "coverage must be an array".to_owned())?;
    let expected_coverage = [
        "element.rectangle",
        "reference.direct-png",
        "reference.svg",
        "rough.outline",
    ];
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
    if actual_coverage != expected_coverage.into_iter().collect() {
        return Err("coverage identities do not match the first case".to_owned());
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
    if manifest["originating_issue"]
        != json!("https://github.com/sagikazarmark/excalidraw-render.orig/issues/32")
    {
        return Err("originating issue is stale".to_owned());
    }
    let sanitization = object(&manifest["sanitization"], "sanitization")?;
    verify_exact_keys(sanitization, &["performed", "reason"], "sanitization")?;
    if sanitization["performed"] != json!(false)
        || sanitization["reason"]
            != json!("Project-authored synthetic scene document; no external content.")
    {
        return Err("sanitization metadata is invalid".to_owned());
    }
    let expectations = object(&manifest["semantic_expectations"], "semantic expectations")?;
    verify_exact_keys(
        expectations,
        &[
            "element_count",
            "element_type",
            "embedded_scene",
            "images",
            "links",
            "masks",
            "selectable_text",
        ],
        "semantic expectations",
    )?;
    if Value::Object(expectations.clone())
        != json!({
            "element_count": 1,
            "element_type": "rectangle",
            "embedded_scene": false,
            "images": 0,
            "links": 0,
            "masks": 0,
            "selectable_text": false,
        })
    {
        return Err("semantic expectations do not match the first case".to_owned());
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
    verify_descriptor(&loaded.root, &loaded.manifest["scene"], &mut referenced)?;
    verify_descriptor(
        &loaded.root,
        &loaded.manifest["expected_report"],
        &mut referenced,
    )?;
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
    if case_id == TYPOGRAPHY_CASE_ID {
        verify_approval_for_issue(&approval, &subject, "58")?;
    } else {
        verify_approval(&approval, &subject)?;
    }
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
    root: PathBuf,
    manifest_path: PathBuf,
    manifest_bytes: Vec<u8>,
    manifest: Value,
}

fn load(case_id: &str) -> Result<LoadedCase, String> {
    let relative = match case_id {
        CASE_ID => CASE_RELATIVE_PATH,
        TYPOGRAPHY_CASE_ID => TYPOGRAPHY_CASE_RELATIVE_PATH,
        _ => return Err(format!("unknown conformance case {case_id:?}")),
    };
    let root = repo_root().join(relative);
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
    if elements.len() != 1 || elements[0]["type"] != json!("rectangle") {
        return Err("first case must contain exactly one rectangle".to_owned());
    }
    let element = object(&elements[0], "rectangle")?;
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
            return Err(format!("rectangle is missing {field}"));
        }
    }
    if element["id"].as_str().unwrap_or_default().is_empty()
        || element["seed"].as_i64().unwrap_or_default() == 0
        || element["isDeleted"] != json!(false)
        || element["roundness"] != Value::Null
    {
        return Err("rectangle contains a normalization repair trigger".to_owned());
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

fn verify_typography_variants(
    case_data: &LoadedCase,
    referenced: &mut Vec<Value>,
) -> Result<(), String> {
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
        verify_descriptor(&case_data.root, &value["artifact"], referenced)?;
        verify_descriptor(&case_data.root, &value["raw_raster"], referenced)?;
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
        verify_png_semantics(&bytes, &raw_bytes)
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
        || provenance["case_id"] != json!(TYPOGRAPHY_CASE_ID)
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

fn verify_expected_report(case_data: &LoadedCase) -> Result<(), String> {
    let path = descriptor_path(&case_data.root, &case_data.manifest["expected_report"])?;
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read expected report: {error}"))?;
    if bytes != EMPTY_REPORT {
        return Err("expected report is not the exact empty compatibility report".to_owned());
    }
    Ok(())
}

fn verify_variants(case_data: &LoadedCase, referenced: &mut Vec<Value>) -> Result<(), String> {
    let variants = case_data.manifest["variants"]
        .as_array()
        .ok_or_else(|| "variants must be an array".to_owned())?;
    if variants.len() != 1 || variants[0]["id"] != json!("default") {
        return Err("first case must have exactly the default variant".to_owned());
    }
    let options = object(&variants[0]["options"], "variant options")?;
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
    if options["exportScale"].is_null() == options["maxWidthOrHeight"].is_null() {
        return Err("exactly one sizing mode must be selected".to_owned());
    }
    if Value::Object(options.clone())
        != json!({
            "exportBackground": true,
            "exportEmbedScene": false,
            "exportPadding": 10,
            "exportScale": 1,
            "exportWithDarkMode": false,
            "frameRendering": {"clip": true, "enabled": true, "name": true, "outline": true},
            "maxWidthOrHeight": null,
            "viewBackgroundColor": "#ffffff",
        })
    {
        return Err("default variant options are not the approved first-case options".to_owned());
    }
    let formats = object(&variants[0]["formats"], "formats")?;
    verify_exact_keys(formats, &["png", "svg"], "formats")?;
    for format in ["png", "svg"] {
        let value = object(&formats[format], format)?;
        verify_exact_keys(value, &["artifact", "raw_raster", "visual_class"], format)?;
        if value["visual_class"] != json!("geometry/image") {
            return Err(format!("{format} has the wrong visual class"));
        }
        verify_descriptor(&case_data.root, &value["artifact"], referenced)?;
        verify_descriptor(&case_data.root, &value["raw_raster"], referenced)?;
        verify_format_dimensions(&case_data.root, format, value)?;
    }
    Ok(())
}

fn collect_variant_descriptors(
    case_data: &LoadedCase,
    referenced: &mut Vec<Value>,
) -> Result<(), String> {
    let formats = object(&case_data.manifest["variants"][0]["formats"], "formats")?;
    for format in ["png", "svg"] {
        verify_descriptor(&case_data.root, &formats[format]["artifact"], referenced)?;
        verify_descriptor(&case_data.root, &formats[format]["raw_raster"], referenced)?;
    }
    Ok(())
}

fn verify_format_dimensions(
    root: &Path,
    format: &str,
    value: &Map<String, Value>,
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
    if raw_bytes.chunks_exact(4).any(|pixel| pixel[3] != 255) {
        return Err(format!(
            "{format} raw raster must be opaque for this white-background case"
        ));
    }
    let artifact_path = descriptor_path(root, &value["artifact"])?;
    let bytes = fs::read(artifact_path)
        .map_err(|error| format!("failed to read {format} artifact: {error}"))?;
    let observed = if format == "png" {
        png_dimensions(&bytes)?
    } else {
        svg_dimensions(&bytes)?
    };
    if observed != (width, height) {
        return Err(format!(
            "{format} encoded dimensions do not match the manifest"
        ));
    }
    if format == "png" {
        verify_png_semantics(&bytes, &raw_bytes)?;
    } else {
        verify_svg_semantics(&bytes, width, height)?;
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
    if provenance["schema"] != json!("excalidraw-render/reference-evidence-provenance/v1")
        || provenance["case_id"] != json!(CASE_ID)
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
        || provenance["generated_at"] != json!("2026-07-19")
        || provenance["reference_profile_digest"] != json!(format!("sha256:{PROFILE_DIGEST}"))
        || provenance["generation_invocation"]
            != json!("cargo run --locked -p xtask -- conformance generate focused.rectangle-outline.v1")
        || provenance["reproduction_invocation"]
            != json!("cargo run --locked -p xtask -- conformance reproduce focused.rectangle-outline.v1")
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

fn verify_approval(approval: &Value, subject: &str) -> Result<(), String> {
    verify_approval_for_issue(approval, subject, "32")
}

fn verify_approval_for_issue(approval: &Value, subject: &str, issue: &str) -> Result<(), String> {
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
    let prefix = format!(
        "https://github.com/sagikazarmark/excalidraw-render.orig/issues/{issue}#issuecomment-"
    );
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

fn canonical_bytes(value: &Value) -> Result<Vec<u8>, String> {
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

fn object<'a>(value: &'a Value, subject: &str) -> Result<&'a Map<String, Value>, String> {
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

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn verify_png_semantics(bytes: &[u8], expected_rgba: &[u8]) -> Result<(), String> {
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
        if matches!(
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
    if chunk_types.first().map(Vec::as_slice) != Some(&b"IHDR"[..])
        || chunk_types.last().map(Vec::as_slice) != Some(&b"IEND"[..])
        || !chunk_types.iter().any(|chunk| chunk == b"IDAT")
    {
        return Err("PNG evidence has an invalid critical chunk sequence".to_owned());
    }
    Ok(())
}

fn verify_svg_semantics(bytes: &[u8], width: u32, height: u32) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("SVG is not UTF-8: {error}"))?;
    let document = roxmltree::Document::parse(text)
        .map_err(|error| format!("SVG evidence is not well-formed XML: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
        || root.attribute("width") != Some(width.to_string().as_str())
        || root.attribute("height") != Some(height.to_string().as_str())
        || root.attribute("viewBox") != Some(format!("0 0 {width} {height}").as_str())
    {
        return Err("SVG root semantics or dimensions are invalid".to_owned());
    }
    let element_count = |name| {
        document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == name)
            .count()
    };
    if element_count("rect") != 1 || element_count("path") != 1 {
        return Err(
            "SVG does not contain exactly one background and one rectangle path".to_owned(),
        );
    }
    for absent in ["a", "clipPath", "image", "mask", "text"] {
        if element_count(absent) != 0 {
            return Err(format!("SVG unexpectedly contains {absent} semantics"));
        }
    }
    let metadata = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "metadata")
        .ok_or_else(|| "SVG metadata marker is missing".to_owned())?;
    if metadata
        .children()
        .any(|child| child.is_element() || child.text().is_some_and(|text| !text.trim().is_empty()))
    {
        return Err("SVG unexpectedly embeds scene metadata".to_owned());
    }
    Ok(())
}

fn valid_github_login(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn valid_date(value: &str) -> bool {
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
