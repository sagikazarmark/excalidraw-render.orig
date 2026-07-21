use std::fs;
use std::path::{Component, Path};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::conformance::repo_root;

const MANIFEST_PATH: &str =
    "tests/conformance/pdf-viewer-qualification/rectangle-outline/manifest.json";
const MANIFEST_DIGEST_PATH: &str =
    "tests/conformance/pdf-viewer-qualification/rectangle-outline/manifest.sha256";
const QUALIFICATION_SCHEMA_ID: &str = "excalidraw-render/conformance-pdf-viewer-qualification/v1";
const VIEWER_PROFILE_SCHEMA_ID: &str = "excalidraw-render/conformance-pdf-viewer-profile/v1";
const STRUCTURE_PROFILE_SCHEMA_ID: &str = "excalidraw-render/conformance-pdf-structure-profile/v1";
const REFERENCE_PROFILE_DIGEST: &str =
    "sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
const COMPARATOR_DIGEST: &str = "1aa92237359c66d2ff8ed95e133134108d95cb96bfd6dae29fa17bcb9c8035a8";
const REFERENCE_RASTER_DIGEST: &str =
    "5ab7b28c0d733b93198a0c456e1683f5d130c310831f1593ccc3e6c0c2ba8d8a";
const VISUAL_FAIL_RASTER_DIGEST: &str =
    "67aee623b8731045330343370de0dfd30fdbb7a343bf2043d1f36473e4b9b88f";

pub fn verify(require_approval: bool) -> Result<(), String> {
    let manifest_bytes = fs::read(repo_root().join(MANIFEST_PATH))
        .map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("invalid PDF qualification manifest: {error}"))?;
    let canonical_manifest = canonical_bytes(&manifest)?;
    if manifest_bytes != [&canonical_manifest[..], b"\n"].concat() {
        return Err("PDF qualification manifest is not canonical JSON plus LF".to_owned());
    }
    verify_sidecar(MANIFEST_DIGEST_PATH, &canonical_manifest)?;
    verify_declared_schema(&manifest, QUALIFICATION_SCHEMA_ID)?;
    if manifest["id"] != json!("pdf-viewers.rectangle-outline.v1")
        || manifest["originating_issue"]
            != json!("https://github.com/sagikazarmark/excalidraw-render.orig/issues/35")
        || manifest["case"]["id"] != json!("focused.rectangle-outline.v1")
        || manifest["case"]["control"]["renderer_evidence"] != json!(false)
        || manifest["case"]["control"]["vector_only"] != json!(true)
    {
        return Err("PDF qualification identity is invalid".to_owned());
    }

    verify_visual_authority(&manifest["case"]["visual_authority"])?;
    verify_fixture(
        &manifest["case"]["fixtures"]["valid"],
        "candidate-control.pdf",
    )?;
    verify_fixture(
        &manifest["case"]["fixtures"]["visual_fail"],
        "visual-fail-structure-pass.pdf",
    )?;
    verify_fixture(
        &manifest["case"]["fixtures"]["structure_fail"],
        "visual-pass-structure-fail.pdf",
    )?;
    verify_fixture(
        &manifest["case"]["fixtures"]["parser_warning"],
        "parser-warning.pdf",
    )?;
    verify_fixture(
        &manifest["case"]["fixtures"]["coverage"],
        "structure-coverage.pdf",
    )?;
    verify_fixture(
        &manifest["case"]["fixtures"]["active_action"],
        "active-action.pdf",
    )?;
    verify_expectation(&manifest["case"]["expected_structures"]["calibration"])?;
    verify_expectation(&manifest["case"]["expected_structures"]["visual_fail"])?;
    verify_expectation(&manifest["case"]["expected_structures"]["coverage"])?;

    verify_descriptor(&manifest["comparator"])?;
    if manifest["comparator"]["sha256"] != json!(COMPARATOR_DIGEST) {
        return Err("PDF qualification does not use the approved comparator".to_owned());
    }
    for source in array(&manifest["implementation"], "qualification implementation")? {
        verify_descriptor(source)?;
    }
    verify_viewer_profile(&manifest["profiles"]["pdfium"], "pdfium")?;
    verify_viewer_profile(&manifest["profiles"]["pdfjs"], "pdfjs")?;
    verify_structure_profile(&manifest["profiles"]["structure"])?;

    verify_viewer_output(&manifest["outputs"]["pdfium"], "pdfium")?;
    verify_viewer_output(&manifest["outputs"]["pdfjs"], "pdfjs")?;
    verify_structure_output(&manifest["structures"]["valid"], true, &[])?;
    verify_structure_output(&manifest["structures"]["visual_fail"], true, &[])?;
    verify_structure_output(
        &manifest["structures"]["structure_fail"],
        false,
        &[
            "form-count",
            "image-count",
            "image-inventory",
            "structural-inventory-sha256",
        ],
    )?;
    verify_structure_output(
        &manifest["structures"]["parser_warning"],
        false,
        &["parser-warnings"],
    )?;
    verify_structure_output(&manifest["structures"]["coverage"], true, &[])?;
    verify_structure_output(
        &manifest["structures"]["active_action"],
        false,
        &[
            "structural-inventory-sha256",
            "action-kinds",
            "prohibited-actions",
        ],
    )?;
    if manifest["determinism"]
        != json!({
            "fresh_processes": true,
            "resolution_dpi": 96,
            "samples_per_role": 2,
            "shared_parser_state": false,
            "shared_viewer_state": false,
        })
    {
        return Err("PDF harness determinism record is invalid".to_owned());
    }

    let subject = approval_subject_from_manifest(&manifest)?;
    match &manifest["approval"] {
        Value::Null if require_approval => {
            return Err("PDF harness qualification has no approval record".to_owned())
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
        return Err("PDF harness qualification is already approved".to_owned());
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
    fs::write(&path, [&canonical[..], b"\n"].concat())
        .map_err(|error| format!("failed to write {MANIFEST_PATH}: {error}"))?;
    fs::write(
        repo_root().join(MANIFEST_DIGEST_PATH),
        format!("{}\n", sha256(&canonical)),
    )
    .map_err(|error| format!("failed to write {MANIFEST_DIGEST_PATH}: {error}"))?;
    verify(true)
}

fn verify_visual_authority(descriptor: &Value) -> Result<(), String> {
    let bytes = verify_raster_descriptor(descriptor)?;
    if descriptor["path"]
        != json!("tests/conformance/svg-viewer-qualification/rectangle-outline/evidence/chromium-reference.rgba")
        || descriptor["sha256"] != json!(REFERENCE_RASTER_DIGEST)
        || sha256(&bytes) != REFERENCE_RASTER_DIGEST
    {
        return Err("PDF visual authority is not canonical Chromium official SVG".to_owned());
    }
    Ok(())
}

fn verify_fixture(descriptor: &Value, filename: &str) -> Result<(), String> {
    let bytes = verify_descriptor(descriptor)?;
    let expected_path =
        format!("tests/conformance/pdf-viewer-qualification/rectangle-outline/{filename}");
    if descriptor["path"] != json!(expected_path)
        || descriptor["byte_length"] != json!(bytes.len())
        || !bytes.starts_with(b"%PDF-1.7\n")
    {
        return Err(format!("invalid PDF qualification fixture {filename}"));
    }
    Ok(())
}

fn verify_expectation(descriptor: &Value) -> Result<(), String> {
    let bytes = verify_descriptor(descriptor)?;
    let expectation: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid PDF structure expectation: {error}"))?;
    validate_against(
        "tests/conformance/pdf-structure-expectation-v1.schema.json",
        &expectation,
    )?;
    if expectation["schema"] != json!("excalidraw-render/pdf-structure-expectation/v1")
        || expectation["pdf_version"] != json!("1.7")
        || expectation["media_box"] != json!([0, 0, 150, 90])
        || expectation["crop_box"] != json!([0, 0, 150, 90])
        || expectation["output_intent_count"] != json!(1)
    {
        return Err("PDF structure expectation is invalid".to_owned());
    }
    Ok(())
}

fn verify_viewer_profile(descriptor: &Value, viewer: &str) -> Result<(), String> {
    let bytes = verify_descriptor(descriptor)?;
    let path = string(&descriptor["path"], "viewer profile path")?;
    verify_sidecar(&path.replace(".json", ".sha256"), &bytes)?;
    let profile: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {viewer} PDF viewer profile: {error}"))?;
    verify_declared_schema(&profile, VIEWER_PROFILE_SCHEMA_ID)?;
    if profile["identity"]["reference_profile_digest"] != json!(REFERENCE_PROFILE_DIGEST)
        || profile["qualification"]["case_id"] != json!("focused.rectangle-outline.v1")
        || profile["qualification"]["dpi"] != json!(96)
        || profile["settings"]["dimensions"]["dpi"] != json!("96")
        || profile["settings"]["resources"]["network"]
            .as_str()
            .is_none_or(|value| !value.contains("none"))
    {
        return Err(format!("{viewer} PDF viewer profile identity is invalid"));
    }
    verify_descriptor(&profile["implementation"]["lockfile"])?;
    for source in array(&profile["implementation"]["sources"], "viewer sources")? {
        verify_descriptor(source)?;
    }
    match viewer {
        "pdfium" => {
            if profile["id"] != json!("canonical-pdf.pdfium.v1")
                || profile["identity"]["viewer"] != json!("pdfium")
                || profile["identity"]["upstream_version"] != json!("152.0.7947.0")
                || !has_runtime(
                    &profile["runtime"],
                    "pypdfium2",
                    "5.12.1 at b3e7e67a1e35c9436b52cb043d476b89ec8c38cb",
                )?
                || !has_runtime(
                    &profile["runtime"],
                    "pdfium",
                    "152.0.7947.0 build 7947, pdfium-binaries, default flags",
                )?
                || !has_runtime(&profile["runtime"], "python", "3.12.3")?
                || !has_runtime(
                    &profile["runtime"],
                    "platform",
                    "Linux x86_64, manylinux_2_17 glibc ABI",
                )?
            {
                return Err("canonical PDFium profile settings are invalid".to_owned());
            }
            verify_exact_directory("tools/pdf-harness/pdfium/empty-fonts", &[".keep"])?;
        }
        "pdfjs" => {
            if profile["id"] != json!("canonical-pdf.pdfjs.v1")
                || profile["identity"]["viewer"] != json!("pdfjs")
                || profile["identity"]["upstream_version"] != json!("6.1.200")
                || profile["identity"]["upstream_commit"]
                    != json!("6353acefe5007cd4899247a8c4e83cb7c9435a54")
                || !has_runtime(&profile["runtime"], "node", "24.16.0")?
                || !has_runtime(
                    &profile["runtime"],
                    "pdfjs-dist",
                    "6.1.200 at 6353acefe5007cd4899247a8c4e83cb7c9435a54",
                )?
                || !has_runtime(&profile["runtime"], "npm", "11.13.0")?
                || !has_runtime(
                    &profile["runtime"],
                    "@napi-rs/canvas",
                    "1.0.2 Linux x64 GNU Skia backend",
                )?
                || !has_runtime(
                    &profile["runtime"],
                    "container",
                    "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9, linux/amd64",
                )?
            {
                return Err("canonical PDF.js profile settings are invalid".to_owned());
            }
        }
        _ => return Err(format!("unknown PDF viewer {viewer}")),
    }
    Ok(())
}

fn verify_structure_profile(descriptor: &Value) -> Result<(), String> {
    let bytes = verify_descriptor(descriptor)?;
    let path = string(&descriptor["path"], "structure profile path")?;
    verify_sidecar(&path.replace(".json", ".sha256"), &bytes)?;
    let profile: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid PDF structure profile: {error}"))?;
    verify_declared_schema(&profile, STRUCTURE_PROFILE_SCHEMA_ID)?;
    if profile["id"] != json!("canonical-pdf-structure.qpdf.v1")
        || profile["identity"]["parser"] != json!("qpdf")
        || profile["identity"]["upstream_version"] != json!("12.3.2")
        || profile["identity"]["upstream_commit"]
            != json!("a898bb3a7289d1d05789d6d3f0d5dd534943a8da")
        || profile["identity"]["reference_profile_digest"] != json!(REFERENCE_PROFILE_DIGEST)
        || profile["qualification"]["case_id"] != json!("focused.rectangle-outline.v1")
        || !has_runtime(
            &profile["runtime"],
            "qpdf",
            "12.3.2 at a898bb3a7289d1d05789d6d3f0d5dd534943a8da",
        )?
        || !has_runtime(
            &profile["runtime"],
            "python",
            "3.12.3 standard library only",
        )?
    {
        return Err("canonical PDF structure profile identity is invalid".to_owned());
    }
    verify_descriptor(&profile["implementation"]["lockfile"])?;
    for source in array(&profile["implementation"]["sources"], "structure sources")? {
        verify_descriptor(source)?;
    }
    verify_descriptor(&profile["qualification"]["report_schema"])?;
    verify_descriptor(&profile["qualification"]["expectation_schema"])?;
    Ok(())
}

fn verify_viewer_output(output: &Value, viewer: &str) -> Result<(), String> {
    verify_visual_control(
        &output["valid"],
        viewer,
        "valid",
        true,
        REFERENCE_RASTER_DIGEST,
    )?;
    verify_visual_control(
        &output["visual_fail"],
        viewer,
        "visual-fail",
        false,
        VISUAL_FAIL_RASTER_DIGEST,
    )?;
    verify_visual_control(
        &output["structure_fail"],
        viewer,
        "structure-fail",
        true,
        REFERENCE_RASTER_DIGEST,
    )?;
    Ok(())
}

fn verify_visual_control(
    control: &Value,
    viewer: &str,
    role: &str,
    expected_pass: bool,
    expected_hash: &str,
) -> Result<(), String> {
    verify_raster_descriptor(&control["raster"])?;
    if control["expected_pass"] != json!(expected_pass)
        || control["raster"]["sha256"] != json!(expected_hash)
        || control["run_sha256"] != json!([expected_hash, expected_hash])
    {
        return Err(format!("invalid {viewer} {role} raster outcome"));
    }
    let report = verify_descriptor(&control["comparison"])?;
    let expected_report = match role {
        "valid" | "structure-fail" => format!(
            "{{\"changed_pixels\":0,\"height\":120,\"mode\":\"{viewer}\",\"passed\":true,\"ssim_bits\":\"3ff0000000000000\",\"width\":200}}\n"
        ),
        "visual-fail" => format!(
            "{{\"changed_pixels\":1600,\"height\":120,\"mode\":\"{viewer}\",\"passed\":false,\"ssim_bits\":\"3fed276f3bda86aa\",\"width\":200}}\n"
        ),
        _ => return Err(format!("unknown visual role {role}")),
    };
    if report != expected_report.as_bytes() {
        return Err(format!("invalid {viewer} {role} comparator report"));
    }
    Ok(())
}

fn verify_structure_output(
    output: &Value,
    expected_pass: bool,
    expected_failures: &[&str],
) -> Result<(), String> {
    let bytes = verify_descriptor(&output["report"])?;
    let report: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid PDF structure report: {error}"))?;
    validate_against(
        "tests/conformance/pdf-structure-report-v1.schema.json",
        &report,
    )?;
    let failures: Vec<_> = array(&report["checks"], "structure checks")?
        .iter()
        .filter(|check| check["passed"] == json!(false))
        .map(|check| check["id"].clone())
        .collect();
    let expected: Vec<_> = expected_failures
        .iter()
        .map(|failure| json!(failure))
        .collect();
    let hash = output["report"]["sha256"].clone();
    if output["expected_pass"] != json!(expected_pass)
        || report["passed"] != json!(expected_pass)
        || report["expected_pass"] != json!(expected_pass)
        || output["failure_checks"] != Value::Array(expected.clone())
        || failures != expected
        || output["run_sha256"] != json!([hash, hash])
    {
        return Err("PDF structural gate outcome is invalid".to_owned());
    }
    if report["document"]["output_intents"]
        .as_array()
        .is_none_or(|values| values.len() != 1)
        || report["pages"]
            .as_array()
            .is_none_or(|values| values.len() != 1)
    {
        return Err("PDF structural report omits required document data".to_owned());
    }
    let page = &report["pages"][0];
    for field in [
        "annotations",
        "clips",
        "fonts",
        "forms",
        "images",
        "mappings",
        "masks",
        "operations",
        "text_operations",
        "transparency",
        "vector_operations",
    ] {
        if !page[field].is_array() {
            return Err(format!("PDF structural report omits {field}"));
        }
    }
    if expected_failures
        == [
            "form-count",
            "image-count",
            "image-inventory",
            "structural-inventory-sha256",
        ]
        && page["images"][0]["classification"] != json!("undeclared")
    {
        return Err("structural failure does not identify undeclared image".to_owned());
    }
    if expected_failures == ["parser-warnings"]
        && report["parser"]["warnings"]
            .as_array()
            .is_none_or(Vec::is_empty)
    {
        return Err("parser warning control does not retain warnings".to_owned());
    }
    if expected_failures.contains(&"prohibited-actions")
        && report["document"]["actions"]
            .as_array()
            .is_none_or(|actions| {
                actions.len() != 1
                    || actions[0]["classification"] != json!("prohibited")
                    || actions[0]["kind"] != json!("/JavaScript")
            })
    {
        return Err("active-action control does not retain the prohibited action".to_owned());
    }
    if output["report"]["path"]
        == json!("tests/conformance/pdf-viewer-qualification/rectangle-outline/evidence/coverage-structure.json")
    {
        if page["fonts"].as_array().is_none_or(|values| values.len() != 1)
            || page["fonts"][0]["embedded"] != json!(true)
            || page["fonts"][0]["to_unicode_sha256"].is_null()
            || page["forms"].as_array().is_none_or(|values| values.len() != 2)
            || page["mappings"]
                .as_array()
                .is_none_or(|values| values.len() != 2)
            || page["annotations"][0]["action"]["kind"] != json!("/URI")
            || page["clips"].as_array().is_none_or(|values| values.len() != 1)
            || page["masks"].as_array().is_none_or(|values| values.len() != 2)
            || page["transparency"]
                .as_array()
                .is_none_or(|values| values.len() != 1)
            || page["images"].as_array().is_none_or(|values| values.len() != 2)
            || page["images"][0]["classification"] != json!("source")
            || page["images"][1]["classification"] != json!("source")
            || page["images"][1]["resource"]
                != json!("page/CoverageForm/SourceImage/SMask")
            || page["images"][1]["data_sha256"]
                != json!("76be8b528d0075f7aae98d6fa57a6d3c83ae480a8469e668d7b0af968995ac71")
            || report["document"]["xmp"].is_null()
            || report["document"]["attachments"]["scene.excalidraw"].is_null()
        {
            return Err("PDF structural coverage report is incomplete".to_owned());
        }
        let mapping_kinds: Vec<_> = array(&page["mappings"], "font mappings")?
            .iter()
            .map(|mapping| mapping["kind"].clone())
            .collect();
        if Value::Array(mapping_kinds) != json!(["to-unicode", "actual-text"]) {
            return Err("PDF structural coverage mappings are incomplete".to_owned());
        }
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
        return Err("PDF viewer raster descriptor is invalid".to_owned());
    }
    if bytes
        .chunks_exact(4)
        .any(|pixel| pixel[3] == 0 && pixel[..3] != [0, 0, 0])
    {
        return Err("PDF viewer raster has nonzero RGB below zero alpha".to_owned());
    }
    Ok(bytes)
}

fn verify_declared_schema(value: &Value, expected_id: &str) -> Result<(), String> {
    let schema = object(&value["schema"], "schema descriptor")?;
    if schema["id"] != json!(expected_id) {
        return Err(format!("unexpected schema identity for {expected_id}"));
    }
    let bytes = verify_descriptor(&value["schema"])?;
    let definition: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {expected_id} schema: {error}"))?;
    validate_schema(&definition, value, expected_id)
}

fn validate_against(relative: &str, value: &Value) -> Result<(), String> {
    let bytes = fs::read(repo_root().join(relative))
        .map_err(|error| format!("failed to read {relative}: {error}"))?;
    let definition: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid schema {relative}: {error}"))?;
    validate_schema(&definition, value, relative)
}

fn validate_schema(definition: &Value, value: &Value, subject: &str) -> Result<(), String> {
    jsonschema::draft202012::meta::validate(definition)
        .map_err(|error| format!("{subject} schema fails its meta-schema: {error}"))?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(definition)
        .map_err(|error| format!("failed to compile {subject} schema: {error}"))?
        .validate(value)
        .map_err(|error| format!("document fails {subject}: {error}"))
}

fn verify_descriptor(descriptor: &Value) -> Result<Vec<u8>, String> {
    let descriptor = object(descriptor, "file descriptor")?;
    let relative = string(&descriptor["path"], "file descriptor path")?;
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

fn verify_exact_directory(relative: &str, expected: &[&str]) -> Result<(), String> {
    let path = repo_root().join(relative);
    if path.is_symlink() {
        return Err(format!(
            "profile directory must not be a symlink: {relative}"
        ));
    }
    let mut entries = fs::read_dir(&path)
        .map_err(|error| format!("failed to read profile directory {relative}: {error}"))?
        .map(|entry| {
            let entry =
                entry.map_err(|error| format!("failed to read entry under {relative}: {error}"))?;
            if entry.path().is_symlink() {
                return Err(format!("profile directory contains a symlink: {relative}"));
            }
            entry
                .file_name()
                .into_string()
                .map_err(|_| format!("profile directory contains a non-UTF-8 entry: {relative}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    if entries != expected {
        return Err(format!(
            "profile directory has undeclared entries: {relative}"
        ));
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
                    "https://github.com/sagikazarmark/excalidraw-render.orig/issues/35#issuecomment-",
                )
            })
            .is_none_or(|id| {
                id.is_empty() || !id.chars().all(|character| character.is_ascii_digit())
            })
    {
        return Err("PDF harness qualification approval is invalid".to_owned());
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
        "schema": "excalidraw-render/conformance-pdf-viewer-approval-subject/v1",
    }))?))
}

fn has_runtime(records: &Value, name: &str, expected_value: &str) -> Result<bool, String> {
    Ok(array(records, "runtime records")?.iter().any(|record| {
        record["name"] == json!(name) && record["value"].as_str() == Some(expected_value)
    }))
}

fn object<'a>(value: &'a Value, subject: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{subject} must be an object"))
}

fn array<'a>(value: &'a Value, subject: &str) -> Result<&'a [Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{subject} must be an array"))
}

fn string<'a>(value: &'a Value, subject: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{subject} must be a string"))
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
            let mut keys: Vec<_> = values.keys().collect();
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

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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
