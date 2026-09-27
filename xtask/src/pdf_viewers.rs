use std::fs;
use std::path::{Component, Path};

use serde_json::{json, Value};

use crate::conformance::{canonical_bytes, object, repo_root, sha256};

mod runtime;
pub use runtime::refresh as refresh_pdfium_runtime;

const MANIFEST_PATH: &str =
    "tests/conformance/pdf-viewer-qualification/rectangle-outline/manifest.json";
const MANIFEST_DIGEST_PATH: &str =
    "tests/conformance/pdf-viewer-qualification/rectangle-outline/manifest.sha256";
const QUALIFICATION_SCHEMA_ID: &str = "excalidraw-render/conformance-pdf-viewer-qualification/v1";
const VIEWER_PROFILE_SCHEMA_ID: &str = "excalidraw-render/conformance-pdf-viewer-profile/v1";
const STRUCTURE_PROFILE_SCHEMA_ID: &str = "excalidraw-render/conformance-pdf-structure-profile/v1";
const REFERENCE_PROFILE_DIGEST: &str =
    "sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
pub(crate) const COMPARATOR_DIGEST: &str =
    "dec398c92938c474413f535e8f1202446e65785b4fa7651a4234f82991227ca4";
const REFERENCE_RASTER_DIGEST: &str =
    "5ab7b28c0d733b93198a0c456e1683f5d130c310831f1593ccc3e6c0c2ba8d8a";
const VISUAL_FAIL_RASTER_DIGEST: &str =
    "67aee623b8731045330343370de0dfd30fdbb7a343bf2043d1f36473e4b9b88f";

// These producer identities describe the historical Python-era capture.
// Rust replay checks the same retained bytes with the current adapter.
// Viewer adapters, parser locks, schemas, comparator and evidence stay live.
const HISTORICAL_QUALIFICATION_SOURCES: &[&str] = &[
    "tools/pdf-viewer-qualification.sh",
    "tools/pdf-harness/generate-fixtures.py",
    "xtask/src/pdf_viewers.rs",
    "tools/pdf-harness/pdfium/test_alpha.py",
];
const HISTORICAL_STRUCTURE_SOURCES: &[&str] = &[
    "tools/pdf-harness/structure/inspect.py",
    "tools/pdf-harness/run-structure.sh",
    "tools/pdf-harness/generate-fixtures.py",
    "tools/pdf-harness/install.sh",
];
const HISTORICAL_PDFIUM_SOURCES: &[&str] = &[
    "tools/pdf-harness/run-pdfium.sh",
    "tools/pdf-harness/install.sh",
    "tools/pdf-harness/pdfium/render.py",
    "tools/pdf-harness/pdfium/test_alpha.py",
];

/// Shared with the Node collector. The declaration itself is not a producer
/// entry: preserve the five-file inventory used by existing Rust collections.
pub(crate) fn rust_pdfium_sources() -> Result<Vec<String>, String> {
    serde_json::from_str(include_str!(
        "../../tools/pdf-harness/pdfium-rust/sources.json"
    ))
    .map_err(|error| format!("invalid Rust PDFium source inventory: {error}"))
}

pub(crate) fn archived_pdfium_lock(lock: &Value) -> Result<Value, String> {
    if lock
        != &json!({
            "path": "tools/pdf-harness/pdfium.lock.json",
            "sha256": "0df950516087c77e43df29b44273dfb414512c9a884dfb196988d7c20232ad90",
        })
    {
        return Err("invalid historical PDFium lock identity".into());
    }
    Ok(json!({
        "path": "tools/pdf-harness/history/pypdfium2.lock.json",
        "sha256": lock["sha256"],
    }))
}

pub fn verify() -> Result<(), String> {
    runtime::verify()?;
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
    verify_producer_sources(
        &manifest["implementation"],
        HISTORICAL_QUALIFICATION_SOURCES,
    )?;
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

    Ok(())
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
    if viewer == "pdfium" {
        // The approved profile describes its original producer. Authenticate the
        // relocated archive without rewriting the approval-bound descriptor.
        verify_descriptor(&archived_pdfium_lock(
            &profile["implementation"]["lockfile"],
        )?)?;
    } else {
        verify_descriptor(&profile["implementation"]["lockfile"])?;
    }
    if viewer == "pdfium" {
        verify_producer_sources(
            &profile["implementation"]["sources"],
            HISTORICAL_PDFIUM_SOURCES,
        )?;
    } else {
        verify_producer_sources(
            &profile["implementation"]["sources"],
            &["tools/pdf-harness/install.sh"],
        )?;
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
    verify_producer_sources(
        &profile["implementation"]["sources"],
        HISTORICAL_STRUCTURE_SOURCES,
    )?;
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
        .as_chunks::<4>()
        .0
        .iter()
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
        .build(&crate::conformance::schema_without_approval_gate(
            definition,
        ))
        .map_err(|error| format!("failed to compile {subject} schema: {error}"))?
        .validate(value)
        .map_err(|error| format!("document fails {subject}: {error}"))
}

fn verify_producer_sources(sources: &Value, historical: &[&str]) -> Result<(), String> {
    crate::conformance_candidate_gate::require_historical_sources(sources, historical)?;
    for source in array(sources, "producer sources")? {
        let path = string(&source["path"], "producer source path")?;
        if !historical.contains(&path) {
            verify_descriptor(source)?;
        }
    }
    Ok(())
}

fn verify_descriptor(descriptor: &Value) -> Result<Vec<u8>, String> {
    let descriptor = object(descriptor, "file descriptor")?;
    let relative = string(&descriptor["path"], "file descriptor path")?;
    let bytes = read_repository_file(&repo_root(), relative)?;
    if descriptor["sha256"] != json!(sha256(&bytes)) {
        return Err(format!("stale digest for {relative}"));
    }
    Ok(bytes)
}

fn read_repository_file(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
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
        return Err(format!(
            "file descriptor path escapes repository: {relative}"
        ));
    }
    fs::read(resolved).map_err(|error| format!("failed to read {relative}: {error}"))
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

fn has_runtime(records: &Value, name: &str, expected_value: &str) -> Result<bool, String> {
    Ok(array(records, "runtime records")?.iter().any(|record| {
        record["name"] == json!(name) && record["value"].as_str() == Some(expected_value)
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_producers_require_valid_membership_without_acceptance_hashes() {
        let manifest: Value =
            serde_json::from_slice(&fs::read(repo_root().join(MANIFEST_PATH)).unwrap()).unwrap();
        let mut changed = manifest.clone();
        let sources = changed["implementation"].as_array_mut().unwrap();
        let source = sources
            .iter_mut()
            .find(|s| s["path"] == "tools/pdf-harness/generate-fixtures.py")
            .unwrap();
        source["sha256"] = json!("a".repeat(64));
        verify_producer_sources(&changed["implementation"], HISTORICAL_QUALIFICATION_SOURCES)
            .unwrap();

        for mutation in 0..5 {
            let mut sources = manifest["implementation"].clone();
            let rows = sources.as_array_mut().unwrap();
            let index = rows
                .iter()
                .position(|s| s["path"] == "tools/pdf-harness/generate-fixtures.py")
                .unwrap();
            match mutation {
                0 => rows[index]["sha256"] = json!("not-a-digest"),
                1 => rows[index]["path"] = json!("../generate-fixtures.py"),
                2 => {
                    rows.remove(index);
                }
                3 => rows.push(rows[index].clone()),
                _ => rows[index]["byte_length"] = json!(-1),
            }
            assert!(
                verify_producer_sources(&sources, HISTORICAL_QUALIFICATION_SOURCES).is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn producer_exceptions_do_not_relax_parser_or_comparator_hashes() {
        for path in [
            "tools/pdf-harness/structure/qpdf.lock.json",
            "crates/excalidraw-render-compare/examples/compare_pdf_rgba.rs",
        ] {
            let sources = json!([{"path": path, "sha256": "a".repeat(64)}]);
            assert!(verify_producer_sources(&sources, &[])
                .unwrap_err()
                .contains("stale digest"));
        }
    }
}
