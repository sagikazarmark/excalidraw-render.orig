use std::fs;
use std::path::{Component, Path};

use serde_json::{json, Value};

use crate::conformance::{
    canonical_bytes, object, repo_root, sha256, valid_date, valid_github_login,
};

const MANIFEST_PATH: &str =
    "tests/conformance/svg-viewer-qualification/rectangle-outline/manifest.json";
const MANIFEST_DIGEST_PATH: &str =
    "tests/conformance/svg-viewer-qualification/rectangle-outline/manifest.sha256";
const PROFILE_SCHEMA_ID: &str = "excalidraw-render/conformance-svg-viewer-profile/v1";
const QUALIFICATION_SCHEMA_ID: &str = "excalidraw-render/conformance-svg-viewer-qualification/v1";
const REFERENCE_PROFILE_DIGEST: &str =
    "sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";

pub fn verify(require_approval: bool) -> Result<(), String> {
    let manifest_bytes = fs::read(repo_root().join(MANIFEST_PATH))
        .map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("invalid SVG viewer qualification manifest: {error}"))?;
    let canonical_manifest = canonical_bytes(&manifest)?;
    if manifest_bytes != [&canonical_manifest[..], b"\n"].concat() {
        return Err("SVG viewer qualification manifest is not canonical JSON plus LF".to_owned());
    }
    verify_sidecar(MANIFEST_DIGEST_PATH, &canonical_manifest)?;
    verify_schema(&manifest, QUALIFICATION_SCHEMA_ID)?;
    if manifest["id"] != json!("svg-viewers.rectangle-outline.v1")
        || manifest["originating_issue"]
            != json!("https://github.com/sagikazarmark/excalidraw-render.orig/issues/34")
        || manifest["case"]["id"] != json!("focused.rectangle-outline.v1")
        || manifest["case"]["candidate_control"]["same_bytes_as_reference"] != json!(false)
    {
        return Err("SVG viewer qualification identity is invalid".to_owned());
    }

    let reference_svg = verify_artifact_descriptor(&manifest["case"]["reference_svg"])?;
    let candidate_svg = verify_artifact_descriptor(&manifest["case"]["candidate_svg"])?;
    if reference_svg == candidate_svg {
        return Err("candidate control SVG must have distinct source bytes".to_owned());
    }
    verify_asset_declaration(
        &manifest["case"]["asset_schema"],
        &manifest["case"]["asset_declaration"],
    )?;
    verify_descriptor(&manifest["comparator"])?;
    for source in array(&manifest["implementation"], "qualification implementation")? {
        verify_descriptor(source)?;
    }
    verify_profile(&manifest["profiles"]["chromium"], "chromium")?;
    verify_profile(&manifest["profiles"]["pure_rust"], "pure-rust")?;
    verify_viewer_output(&manifest["outputs"]["chromium"], "chromium")?;
    verify_viewer_output(&manifest["outputs"]["pure_rust"], "pure-rust")?;
    for proof in array(&manifest["rejections"]["proofs"], "rejection proofs")? {
        verify_descriptor(proof)?;
    }
    if manifest["rejections"]
        != json!({
            "ambient_fonts": "rejected",
            "local_files": "rejected",
            "network": "rejected",
            "proofs": manifest["rejections"]["proofs"],
            "undeclared_data_assets": "rejected",
        })
    {
        return Err("ambient dependency rejection record is invalid".to_owned());
    }
    if manifest["determinism"]
        != json!({
            "fresh_processes": true,
            "samples_per_role": 2,
            "shared_clock": false,
            "shared_context": false,
            "shared_font_database": false,
        })
    {
        return Err("viewer determinism record is invalid".to_owned());
    }

    let subject = approval_subject_from_manifest(&manifest)?;
    match &manifest["approval"] {
        Value::Null if require_approval => {
            return Err("SVG viewer qualification has no approval record".to_owned())
        }
        Value::Null => {}
        approval => verify_approval(approval, &subject)?,
    }
    Ok(())
}

pub fn approval_subject() -> Result<String, String> {
    verify(false)?;
    let manifest: Value = serde_json::from_slice(
        &fs::read(repo_root().join(MANIFEST_PATH))
            .map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?,
    )
    .map_err(|error| format!("invalid qualification manifest: {error}"))?;
    approval_subject_from_manifest(&manifest)
}

pub fn record_approval(
    approved_by: &str,
    approved_at: &str,
    approval_url: &str,
    expected_subject: &str,
) -> Result<(), String> {
    verify(false)?;
    let path = repo_root().join(MANIFEST_PATH);
    let mut manifest: Value = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?,
    )
    .map_err(|error| format!("invalid qualification manifest: {error}"))?;
    if !manifest["approval"].is_null() {
        return Err("SVG viewer qualification is already approved".to_owned());
    }
    let subject = approval_subject_from_manifest(&manifest)?;
    let expected = expected_subject
        .strip_prefix("sha256:")
        .unwrap_or(expected_subject);
    if subject != expected {
        return Err(format!(
            "approval subject mismatch: expected {expected}, computed {subject}"
        ));
    }
    manifest["approval"] = json!({
        "approved_at": approved_at,
        "approved_by": approved_by,
        "approval_subject_sha256": format!("sha256:{subject}"),
        "approval_url": approval_url,
    });
    verify_approval(&manifest["approval"], &subject)?;
    let canonical = canonical_bytes(&manifest)?;
    let bytes = [&canonical[..], b"\n"].concat();
    fs::write(&path, &bytes)
        .map_err(|error| format!("failed to write {MANIFEST_PATH}: {error}"))?;
    fs::write(
        repo_root().join(MANIFEST_DIGEST_PATH),
        format!("{}\n", sha256(&canonical)),
    )
    .map_err(|error| format!("failed to write {MANIFEST_DIGEST_PATH}: {error}"))?;
    verify(true)
}

fn verify_profile(descriptor: &Value, viewer: &str) -> Result<(), String> {
    let bytes = verify_descriptor(descriptor)?;
    let path = descriptor["path"]
        .as_str()
        .ok_or_else(|| "viewer profile path must be a string".to_owned())?;
    verify_sidecar(&path.replace(".json", ".sha256"), &bytes)?;
    let profile: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {viewer} viewer profile: {error}"))?;
    verify_schema(&profile, PROFILE_SCHEMA_ID)?;
    if profile["identity"]["reference_profile_digest"] != json!(REFERENCE_PROFILE_DIGEST)
        || profile["qualification"]["case_id"] != json!("focused.rectangle-outline.v1")
        || profile["qualification"]["declared_font_faces"] != json!(0)
        || profile["qualification"]["declared_image_assets"] != json!(0)
    {
        return Err(format!("{viewer} viewer profile identity is invalid"));
    }
    verify_descriptor(&profile["implementation"]["lockfile"])?;
    for source in array(&profile["implementation"]["sources"], "profile sources")? {
        verify_descriptor(source)?;
    }
    match viewer {
        "chromium" => verify_chromium_profile(&profile),
        "pure-rust" => verify_rust_profile(&profile),
        _ => Err(format!("unknown viewer profile {viewer}")),
    }
}

fn verify_chromium_profile(profile: &Value) -> Result<(), String> {
    if profile["id"] != json!("canonical-svg.chromium.v1")
        || profile["identity"]["viewer"] != json!("chromium")
        || profile["runtime"]
            .as_array()
            .is_none_or(|records| !has_runtime(records, "chromium", "149.0.7827.55"))
        || profile["settings"]["dimensions"]["device_pixel_ratio"] != json!("1")
        || profile["settings"]["locale"]["locale"] != json!("en-US")
        || profile["settings"]["locale"]["timezone"] != json!("UTC")
        || profile["settings"]["resources"]["container_network"] != json!("none")
        || profile["settings"]["dimensions"]["capture"]
            != json!("exact integer CSS dimensions at x=0,y=0,scale=1 with beyond-viewport capture")
        || profile["settings"]["state"]["sharing"]
            != json!("official and candidate inputs share no mutable viewer state")
    {
        return Err("canonical Chromium viewer settings are invalid".to_owned());
    }
    Ok(())
}

fn verify_rust_profile(profile: &Value) -> Result<(), String> {
    let runtime = array(&profile["runtime"], "pure-Rust runtime records")?;
    for (name, version) in [
        ("resvg", "0.47.0"),
        ("usvg", "0.47.0"),
        ("tiny-skia", "0.12.0"),
        ("fontdb", "0.23.0"),
        ("rustybuzz", "0.20.1"),
        ("ttf-parser", "0.25.1"),
        ("simplecss", "0.2.2"),
        ("rustc", "1.97.1 (8bab26f4f 2026-07-14)"),
        ("target", "x86_64-unknown-linux-gnu"),
    ] {
        if !has_runtime(runtime, name, version) {
            return Err(format!("pure-Rust profile does not pin {name} {version}"));
        }
    }
    if profile["id"] != json!("canonical-svg.pure-rust.v1")
        || profile["identity"]["upstream_commit"]
            != json!("3a0fdba53ccf2d346b54cc53ba7adf0ee60d0707")
        || profile["implementation"]["features"]
            != json!(["resvg:raster-images", "resvg:text", "usvg:text"])
        || profile["implementation"]["forbidden_features"]
            != json!([
                "memmap-fonts",
                "system-fonts",
                "file-backed-faces",
                "ambient-fonts"
            ])
        || profile["settings"]["locale"]["dpi"] != json!("96")
        || profile["settings"]["locale"]["languages"] != json!("en-US")
        || profile["settings"]["resources"]["resources_dir"] != json!("none")
        || profile["settings"]["resources"]["style_sheet"] != json!("none")
    {
        return Err("canonical pure-Rust viewer settings are invalid".to_owned());
    }
    Ok(())
}

fn verify_viewer_output(output: &Value, viewer: &str) -> Result<(), String> {
    let reference = verify_raster_descriptor(&output["reference"])?;
    let candidate = verify_raster_descriptor(&output["candidate"])?;
    let reference_hash = output["reference"]["sha256"].clone();
    let candidate_hash = output["candidate"]["sha256"].clone();
    if reference != candidate {
        return Err(format!("{viewer} qualification control buffers differ"));
    }
    if output["reference_run_sha256"] != json!([reference_hash, reference_hash])
        || output["candidate_run_sha256"] != json!([candidate_hash, candidate_hash])
    {
        return Err(format!("{viewer} repeat-run hashes differ"));
    }
    let report = verify_descriptor(&output["comparison"])?;
    let expected = format!(
        "{{\"changed_pixels\":0,\"height\":120,\"mode\":\"{viewer}\",\"passed\":true,\"ssim_bits\":\"3ff0000000000000\",\"width\":200}}\n"
    );
    if report != expected.as_bytes() {
        return Err(format!(
            "{viewer} comparator report is not the exact passing result"
        ));
    }
    Ok(())
}

fn verify_raster_descriptor(descriptor: &Value) -> Result<Vec<u8>, String> {
    let bytes = verify_descriptor(descriptor)?;
    if descriptor["width"] != json!(200)
        || descriptor["height"] != json!(120)
        || descriptor["byte_length"] != json!(96_000)
        || descriptor["layout"] != json!("top-left-row-major")
        || descriptor["pixel_format"] != json!("straight-alpha-srgb-rgba8")
        || bytes.len() != 96_000
    {
        return Err("viewer raster descriptor is invalid".to_owned());
    }
    if bytes
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[3] == 0 && pixel[..3] != [0, 0, 0])
    {
        return Err("viewer raster has nonzero RGB below zero alpha".to_owned());
    }
    Ok(bytes)
}

fn verify_artifact_descriptor(descriptor: &Value) -> Result<Vec<u8>, String> {
    let bytes = verify_descriptor(descriptor)?;
    if descriptor["byte_length"] != json!(bytes.len())
        || descriptor["width"] != json!(200)
        || descriptor["height"] != json!(120)
    {
        return Err("qualification SVG descriptor is invalid".to_owned());
    }
    Ok(bytes)
}

fn verify_asset_declaration(schema: &Value, declaration: &Value) -> Result<(), String> {
    let schema_bytes = verify_descriptor(schema)?;
    let declaration_bytes = verify_descriptor(declaration)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("invalid SVG viewer asset schema: {error}"))?;
    let declaration: Value = serde_json::from_slice(&declaration_bytes)
        .map_err(|error| format!("invalid SVG viewer asset declaration: {error}"))?;
    jsonschema::draft202012::meta::validate(&schema)
        .map_err(|error| format!("SVG viewer asset schema fails its meta-schema: {error}"))?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| format!("failed to compile SVG viewer asset schema: {error}"))?
        .validate(&declaration)
        .map_err(|error| format!("asset declaration fails its schema: {error}"))?;
    if declaration
        != json!({
            "fonts": [],
            "images": [],
            "schema": "excalidraw-render/svg-viewer-asset-declaration/v1",
        })
    {
        return Err("rectangle qualification must declare an explicit empty asset set".to_owned());
    }
    Ok(())
}

fn verify_schema(value: &Value, expected_id: &str) -> Result<(), String> {
    let schema = object(&value["schema"], "schema descriptor")?;
    if schema["id"] != json!(expected_id) {
        return Err(format!("unexpected schema identity for {expected_id}"));
    }
    let bytes = verify_descriptor(&value["schema"])?;
    let definition: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {expected_id} schema: {error}"))?;
    jsonschema::draft202012::meta::validate(&definition)
        .map_err(|error| format!("{expected_id} schema fails its meta-schema: {error}"))?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&definition)
        .map_err(|error| format!("failed to compile {expected_id} schema: {error}"))?
        .validate(value)
        .map_err(|error| format!("document fails {expected_id}: {error}"))?;
    Ok(())
}

fn verify_descriptor(descriptor: &Value) -> Result<Vec<u8>, String> {
    let descriptor = object(descriptor, "file descriptor")?;
    let relative = descriptor["path"]
        .as_str()
        .ok_or_else(|| "file descriptor path must be a string".to_owned())?;
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe repository-relative path {relative:?}"));
    }
    let root = fs::canonicalize(repo_root())
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;
    let resolved = fs::canonicalize(root.join(path))
        .map_err(|error| format!("failed to resolve {relative}: {error}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!(
            "file descriptor path escapes repository: {relative}"
        ));
    }
    let bytes =
        fs::read(resolved).map_err(|error| format!("failed to read {relative}: {error}"))?;
    if descriptor["sha256"] != json!(sha256(&bytes)) {
        return Err(format!("stale digest for {relative}"));
    }
    Ok(bytes)
}

fn verify_sidecar(relative: &str, bytes: &[u8]) -> Result<(), String> {
    let expected = fs::read_to_string(repo_root().join(relative))
        .map_err(|error| format!("failed to read {relative}: {error}"))?;
    if expected != format!("{}\n", sha256(bytes)) {
        return Err(format!("stale digest sidecar {relative}"));
    }
    Ok(())
}

fn verify_approval(approval: &Value, subject: &str) -> Result<(), String> {
    if approval["approval_subject_sha256"] != json!(format!("sha256:{subject}"))
        || approval["approved_by"]
            .as_str()
            .is_none_or(|value| !valid_github_login(value))
        || approval["approved_at"]
            .as_str()
            .is_none_or(|value| !valid_date(value))
        || approval["approval_url"]
            .as_str()
            .and_then(|url| {
                url.strip_prefix(
                "https://github.com/sagikazarmark/excalidraw-render.orig/issues/34#issuecomment-",
            )
            })
            .is_none_or(|id| {
                id.is_empty() || !id.chars().all(|character| character.is_ascii_digit())
            })
    {
        return Err("SVG viewer qualification approval is invalid".to_owned());
    }
    Ok(())
}

fn approval_subject_from_manifest(manifest: &Value) -> Result<String, String> {
    let mut manifest = manifest.clone();
    manifest
        .as_object_mut()
        .ok_or_else(|| "qualification manifest must be an object".to_owned())?
        .remove("approval");
    Ok(sha256(&canonical_bytes(&json!({
        "manifest": manifest,
        "schema": "excalidraw-render/conformance-svg-viewer-approval-subject/v1",
    }))?))
}

fn has_runtime(records: &[Value], name: &str, value: &str) -> bool {
    records
        .iter()
        .any(|record| record["name"] == json!(name) && record["value"] == json!(value))
}

fn array<'a>(value: &'a Value, subject: &str) -> Result<&'a [Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{subject} must be an array"))
}
