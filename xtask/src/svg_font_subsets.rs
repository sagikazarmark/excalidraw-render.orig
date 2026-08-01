use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::conformance::repo_root;

const MANIFEST_PATH: &str =
    "tests/conformance/svg-font-subset-viewer-qualification/revision-1.json";
const MANIFEST_DIGEST_PATH: &str =
    "tests/conformance/svg-font-subset-viewer-qualification/revision-1.sha256";
const SCHEMA_ID: &str = "excalidraw-render/conformance-svg-font-subset-viewer-qualification/v1";
const CORPUS_PATH: &str = concat!(
    "tests/oracle/compatibility-artifacts/",
    "revision-1-svg-font-subset-qualification-corpus.json"
);
const REQUIRED_CHECKS: [&str; 7] = [
    "shaping",
    "glyph_choice",
    "advances_offsets",
    "metrics",
    "line_layout",
    "selectable_text",
    "rasterized_typography",
];

pub fn materialize(inputs: &str, chromium: &str, pure_rust: &str) -> Result<(), String> {
    let destination = repo_root().join("tests/conformance/svg-font-subset-viewer-qualification");
    fs::create_dir_all(&destination)
        .map_err(|error| format!("failed to create {}: {error}", destination.display()))?;
    let input_bytes = read_repository_file(inputs)?;
    let input: Value = serde_json::from_slice(&input_bytes)
        .map_err(|error| format!("invalid viewer input manifest: {error}"))?;
    let chromium_bytes = read_repository_file(chromium)?;
    let rust_bytes = read_repository_file(pure_rust)?;
    let chromium_report: Value = serde_json::from_slice(&chromium_bytes)
        .map_err(|error| format!("invalid Chromium viewer report: {error}"))?;
    let rust_report: Value = serde_json::from_slice(&rust_bytes)
        .map_err(|error| format!("invalid pure-Rust viewer report: {error}"))?;
    let chromium_results = report_decisions(&chromium_report, "chromium")?;
    let rust_results = report_decisions(&rust_report, "pure_rust")?;

    fs::write(destination.join("chromium-report.json"), &chromium_bytes)
        .map_err(|error| format!("failed to write Chromium report: {error}"))?;
    fs::write(destination.join("pure-rust-report.json"), &rust_bytes)
        .map_err(|error| format!("failed to write pure-Rust report: {error}"))?;

    let mut executions = Vec::new();
    for case in array(&input["cases"], "viewer input cases")? {
        let id = string(
            &case["semantic_subset_identity"],
            "semantic subset identity",
        )?;
        let chromium_decision = chromium_results
            .get(id)
            .ok_or_else(|| format!("Chromium report lacks {id}"))?;
        let rust_decision = rust_results
            .get(id)
            .ok_or_else(|| format!("pure-Rust report lacks {id}"))?;
        let pass = chromium_decision == "pass" && rust_decision == "pass";
        executions.push(json!({
            "chromium_decision": chromium_decision,
            "coverage": case["coverage"],
            "decision": if pass { "pass" } else { "fail" },
            "pure_rust_decision": rust_decision,
            "semantic_subset_identity": id,
            "source_assets_sha256": case["source_assets_sha256"],
            "source_sfnt_sha256": case["source_sfnt_sha256"],
            "source_svg_sha256": case["source_svg_sha256"],
            "source_woff2_sha256": case["source_woff2_sha256"],
            "subset_assets_sha256": case["subset_assets_sha256"],
            "subset_sfnt_sha256": case["subset_sfnt_sha256"],
            "subset_svg_sha256": case["subset_svg_sha256"],
            "subset_woff2_sha256": case["subset_woff2_sha256"],
        }));
    }
    let schema_path = "tests/conformance/svg-font-subset-viewer-qualification-v1.schema.json";
    let manifest = json!({
        "approval": Value::Null,
        "compatibility_revision": 1,
        "corpus": descriptor(CORPUS_PATH)?,
        "decision": if executions.iter().all(|execution| execution["decision"] == json!("pass")) { "pass" } else { "fail" },
        "executions": executions,
        "id": "svg-font-subsets.dual-viewer.revision-1.v1",
        "input_manifest_sha256": sha256(&input_bytes),
        "implementation": [
            descriptor("crates/excalidraw-render/src/render/font_subset.rs")?,
            descriptor("crates/excalidraw-render/src/render/font_subset_qualification.rs")?,
            descriptor("tools/reference-oracle/scripts/svg-font-subset-viewer.mjs")?,
            descriptor("tools/svg-viewer-rust/src/main.rs")?,
            descriptor("xtask/src/svg_font_subset_runner.rs")?,
            descriptor("xtask/src/svg_font_subsets.rs")?,
            descriptor("tools/svg-font-subset-qualification.sh")?,
        ],
        "originating_issue": "https://github.com/sagikazarmark/excalidraw-render.orig/issues/99",
        "reproduction_commands": [
            "bash tools/svg-font-subset-qualification.sh generate",
            "bash tools/svg-font-subset-qualification.sh reproduce",
        ],
        "schema": {
            "id": SCHEMA_ID,
            "path": schema_path,
            "sha256": sha256(&fs::read(repo_root().join(schema_path)).map_err(|error| format!("failed to read schema: {error}"))?),
        },
        "subset_profile": descriptor("tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-profile.json")?,
        "viewer_profiles": {
            "chromium": descriptor("tests/conformance/viewer-profiles/canonical-svg-chromium-v1.json")?,
            "pure_rust": descriptor("tests/conformance/viewer-profiles/canonical-svg-pure-rust-v1.json")?,
        },
        "viewer_reports": {
            "chromium": {
                "path": "tests/conformance/svg-font-subset-viewer-qualification/chromium-report.json",
                "sha256": sha256(&chromium_bytes),
            },
            "pure_rust": {
                "path": "tests/conformance/svg-font-subset-viewer-qualification/pure-rust-report.json",
                "sha256": sha256(&rust_bytes),
            },
        },
    });
    let canonical = canonical_bytes(&manifest)?;
    fs::write(
        destination.join("revision-1.json"),
        [&canonical[..], b"\n"].concat(),
    )
    .map_err(|error| format!("failed to write qualification manifest: {error}"))?;
    fs::write(
        destination.join("revision-1.sha256"),
        format!("{}\n", sha256(&canonical)),
    )
    .map_err(|error| format!("failed to write qualification digest: {error}"))?;
    Ok(())
}

fn report_decisions(report: &Value, viewer: &str) -> Result<BTreeMap<String, String>, String> {
    if report["viewer"] != json!(viewer) {
        return Err(format!("unexpected {viewer} report identity"));
    }
    let mut decisions = BTreeMap::new();
    for result in array(&report["results"], "viewer report results")? {
        let id = string(
            &result["semantic_subset_identity"],
            "viewer result identity",
        )?;
        let decision = string(&result["decision"], "viewer decision")?;
        if decisions
            .insert(id.to_owned(), decision.to_owned())
            .is_some()
        {
            return Err(format!("duplicate {viewer} result {id}"));
        }
    }
    Ok(decisions)
}

fn descriptor(path: &str) -> Result<Value, String> {
    Ok(json!({
        "path": path,
        "sha256": sha256(&fs::read(repo_root().join(path)).map_err(|error| format!("failed to read {path}: {error}"))?),
    }))
}

fn read_repository_file(relative: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe repository-relative path {relative:?}"));
    }
    fs::read(repo_root().join(path)).map_err(|error| format!("failed to read {relative}: {error}"))
}

pub fn verify() -> Result<(), String> {
    verify_internal(true)
}

fn verify_internal(require_approval: bool) -> Result<(), String> {
    let bytes = fs::read(repo_root().join(MANIFEST_PATH))
        .map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?;
    let manifest: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid SVG font subset viewer manifest: {error}"))?;
    let canonical = canonical_bytes(&manifest)?;
    if bytes != [&canonical[..], b"\n"].concat() {
        return Err("SVG font subset viewer manifest is not canonical JSON plus LF".to_owned());
    }
    verify_sidecar(MANIFEST_DIGEST_PATH, &canonical)?;
    verify_schema(&manifest)?;
    verify_identity_and_inputs(&manifest)?;

    let expected = expected_cells()?;
    let executions = verify_execution_coverage(&manifest, &expected)?;
    let input_sha256 = string(
        &manifest["input_manifest_sha256"],
        "viewer input manifest digest",
    )?;
    verify_sha256(input_sha256, "viewer input manifest digest")?;
    let chromium = verify_viewer_report(
        &manifest["viewer_reports"]["chromium"],
        "chromium",
        input_sha256,
    )?;
    let pure_rust = verify_viewer_report(
        &manifest["viewer_reports"]["pure_rust"],
        "pure_rust",
        input_sha256,
    )?;
    let expected_ids = executions.keys().cloned().collect::<BTreeSet<_>>();
    if chromium.keys().cloned().collect::<BTreeSet<_>>() != expected_ids
        || pure_rust.keys().cloned().collect::<BTreeSet<_>>() != expected_ids
    {
        return Err("viewer reports do not cover the exact execution inventory".to_owned());
    }

    for (id, execution) in executions {
        let chromium = &chromium[&id];
        let pure_rust = &pure_rust[&id];
        let independently_passed = result_passed(chromium) && result_passed(pure_rust);
        if execution["chromium_decision"] != json!(decision(chromium))
            || execution["pure_rust_decision"] != json!(decision(pure_rust))
            || execution["decision"] != json!(if independently_passed { "pass" } else { "fail" })
            || !independently_passed
        {
            return Err(format!(
                "execution {id} does not pass both viewers independently"
            ));
        }
    }

    if manifest["decision"] != json!("pass") {
        return Err("SVG font subset viewer qualification is not passing".to_owned());
    }
    let subject = approval_subject_from_manifest(&manifest)?;
    match &manifest["approval"] {
        Value::Null if require_approval => {
            return Err("SVG font subset viewer qualification has no approval record".to_owned())
        }
        Value::Null => {}
        approval => verify_approval(approval, &subject)?,
    }
    Ok(())
}

pub fn approval_subject() -> Result<String, String> {
    verify_internal(false)?;
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
    verify_internal(false)?;
    let path = repo_root().join(MANIFEST_PATH);
    let mut manifest: Value = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?,
    )
    .map_err(|error| format!("invalid qualification manifest: {error}"))?;
    if !manifest["approval"].is_null() {
        return Err("SVG font subset viewer qualification is already approved".to_owned());
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
    verify()
}

fn verify_identity_and_inputs(manifest: &Value) -> Result<(), String> {
    if manifest["id"] != json!("svg-font-subsets.dual-viewer.revision-1.v1")
        || manifest["originating_issue"]
            != json!("https://github.com/sagikazarmark/excalidraw-render.orig/issues/99")
        || manifest["compatibility_revision"] != json!(1)
    {
        return Err("SVG font subset viewer qualification identity is invalid".to_owned());
    }
    for descriptor in [
        &manifest["corpus"],
        &manifest["subset_profile"],
        &manifest["viewer_profiles"]["chromium"],
        &manifest["viewer_profiles"]["pure_rust"],
    ] {
        verify_descriptor(descriptor)?;
    }
    if manifest["corpus"]["path"] != json!(CORPUS_PATH)
        || manifest["corpus"]["sha256"]
            != json!("5b96d68e542a24f9bbc75e95473ce009cf7a108ae52fa896d75ee5edcd8a07c8")
        || manifest["subset_profile"]["sha256"]
            != json!("8efda16840ffa56b89d039fe6288fd4a4f793310e5b75d7496fe3bebb876f1a8")
        || manifest["viewer_profiles"]["chromium"]["sha256"]
            != json!("be1664b3ad79c09b99680ff5e8ca0e56a98fce703c94a13e19e889097c90426b")
        || manifest["viewer_profiles"]["pure_rust"]["sha256"]
            != json!("8be13868942970480bb083b454ed003f5809b2b7f1725adbaaa7d5450b69316d")
    {
        return Err("qualification dependency identity is stale".to_owned());
    }
    for source in array(&manifest["implementation"], "qualification implementation")? {
        verify_descriptor(source)?;
    }
    let commands = array(&manifest["reproduction_commands"], "reproduction commands")?;
    if commands.len() != 2
        || commands.iter().any(|command| {
            command.as_str().is_none_or(|command| {
                !command.starts_with("bash tools/svg-font-subset-qualification.sh ")
            })
        })
        || commands[0] != json!("bash tools/svg-font-subset-qualification.sh generate")
        || commands[1] != json!("bash tools/svg-font-subset-qualification.sh reproduce")
    {
        return Err("qualification reproduction commands are invalid".to_owned());
    }
    Ok(())
}

#[derive(Clone)]
struct ExpectedCell {
    semantic_identity: String,
    scalars: Value,
    source_woff2_sha256: String,
    subset_woff2_sha256: String,
}

fn expected_cells() -> Result<BTreeMap<(String, String), ExpectedCell>, String> {
    let bytes = fs::read(repo_root().join(CORPUS_PATH))
        .map_err(|error| format!("failed to read {CORPUS_PATH}: {error}"))?;
    let corpus: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid font subset qualification corpus: {error}"))?;
    let mut expected = BTreeMap::new();
    for source in array(&corpus["sources"], "font subset corpus sources")? {
        let source_id = string(&source["sourceIdentity"], "source identity")?;
        for cell in array(&source["cells"], "font subset corpus cells")? {
            if cell["status"] != json!("success") {
                continue;
            }
            let class = string(&cell["class"], "qualification class")?;
            let semantic_identity =
                string(&cell["semanticSubsetIdentity"], "semantic subset identity")?;
            let key = (source_id.to_owned(), class.to_owned());
            if expected
                .insert(
                    key,
                    ExpectedCell {
                        semantic_identity: semantic_identity.to_owned(),
                        scalars: cell["orderedScalars"].clone(),
                        source_woff2_sha256: string(
                            &source["sourceSha256"],
                            "source WOFF2 digest",
                        )?
                        .to_owned(),
                        subset_woff2_sha256: string(
                            &cell["generatedWoff2Sha256"],
                            "generated WOFF2 digest",
                        )?
                        .to_owned(),
                    },
                )
                .is_some()
            {
                return Err("font subset corpus has duplicate successful cells".to_owned());
            }
        }
    }
    if expected.len() != 748 {
        return Err(format!(
            "font subset corpus has {} successful cells, expected 748",
            expected.len()
        ));
    }
    Ok(expected)
}

fn verify_execution_coverage<'a>(
    manifest: &'a Value,
    expected: &BTreeMap<(String, String), ExpectedCell>,
) -> Result<BTreeMap<String, &'a Value>, String> {
    let mut executions = BTreeMap::new();
    let mut actual = BTreeSet::new();
    for execution in array(&manifest["executions"], "qualification executions")? {
        let id = string(&execution["semantic_subset_identity"], "execution identity")?;
        if executions.insert(id.to_owned(), execution).is_some() {
            return Err(format!("duplicate qualification execution {id}"));
        }
        verify_sha256(id, "execution semantic subset identity")?;
        verify_sha256(
            string(&execution["source_woff2_sha256"], "source WOFF2 digest")?,
            "source WOFF2 digest",
        )?;
        verify_sha256(
            string(&execution["subset_woff2_sha256"], "subset WOFF2 digest")?,
            "subset WOFF2 digest",
        )?;
        for field in [
            "source_assets_sha256",
            "source_sfnt_sha256",
            "source_svg_sha256",
            "subset_assets_sha256",
            "subset_sfnt_sha256",
            "subset_svg_sha256",
        ] {
            verify_sha256(string(&execution[field], field)?, field)?;
        }
        for coverage in array(&execution["coverage"], "execution coverage")? {
            let source = string(&coverage["source_identity"], "coverage source identity")?;
            let class = string(&coverage["class"], "coverage class")?;
            let key = (source.to_owned(), class.to_owned());
            let expected_cell = expected
                .get(&key)
                .ok_or_else(|| format!("unexpected qualification coverage {source}/{class}"))?;
            if expected_cell.semantic_identity != id
                || coverage["ordered_scalars"] != expected_cell.scalars
                || execution["source_woff2_sha256"] != json!(expected_cell.source_woff2_sha256)
                || execution["subset_woff2_sha256"] != json!(expected_cell.subset_woff2_sha256)
                || !actual.insert(key)
            {
                return Err(format!("stale or duplicate coverage {source}/{class}"));
            }
        }
    }
    if actual != expected.keys().cloned().collect::<BTreeSet<_>>() {
        return Err("qualification does not cover every successful corpus cell".to_owned());
    }
    if executions.len() != 634 {
        return Err(format!(
            "qualification has {} semantic executions, expected 634",
            executions.len()
        ));
    }
    Ok(executions)
}

fn verify_viewer_report(
    descriptor: &Value,
    viewer: &str,
    input_sha256: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let bytes = verify_descriptor(descriptor)?;
    let report: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {viewer} font subset viewer report: {error}"))?;
    if report["schema"] != json!("excalidraw-render/svg-font-subset-viewer-report/v1")
        || report["viewer"] != json!(viewer)
        || report["inputs_sha256"] != json!(input_sha256)
        || report["samples_per_role"] != json!(2)
        || report["fresh_state_per_role"] != json!(true)
    {
        return Err(format!("{viewer} viewer report identity is invalid"));
    }
    let environment = &report["environment"];
    let expected_environment = match viewer {
        "chromium" => json!({
            "browser_arguments": "--disable-gpu --font-render-hinting=none --force-color-profile=srgb",
            "chromium": "149.0.7827.55",
            "container": "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9",
            "host": "linux-amd64",
            "locale": "en-US",
            "node": "v24.16.0",
            "playwright": "1.61.0",
            "role_processes": "fresh Chromium process per execution, role, and repeat",
            "timezone": "UTC",
        }),
        "pure_rust" => json!({
            "fontdb": "0.23.0",
            "host": "linux-amd64",
            "resvg": "0.47.0",
            "rustc": "rustc 1.97.1 (8bab26f4f 2026-07-14)",
            "rustybuzz": "0.20.1",
            "target": "x86_64-unknown-linux-gnu",
            "tiny-skia": "0.12.0",
            "usvg": "0.47.0",
        }),
        _ => return Err(format!("unknown viewer report {viewer}")),
    };
    if environment != &expected_environment {
        return Err(format!("{viewer} viewer environment is not pinned"));
    }
    let mut results = BTreeMap::new();
    for result in array(&report["results"], "viewer results")? {
        let id = string(
            &result["semantic_subset_identity"],
            "viewer result identity",
        )?;
        if results.insert(id.to_owned(), result.clone()).is_some() {
            return Err(format!("duplicate {viewer} viewer result {id}"));
        }
        verify_result(result, viewer)?;
    }
    Ok(results)
}

fn verify_result(result: &Value, viewer: &str) -> Result<(), String> {
    let id = string(
        &result["semantic_subset_identity"],
        "viewer result identity",
    )?;
    let width = result["dimensions"]["width"]
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{viewer} result {id} has invalid width"))?;
    let height = result["dimensions"]["height"]
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{viewer} result {id} has invalid height"))?;
    let byte_length = width
        .checked_mul(height)
        .and_then(|area| area.checked_mul(4))
        .ok_or_else(|| format!("{viewer} result {id} dimensions overflow"))?;
    if result["dimensions"]["byte_length"] != json!(byte_length)
        || result["dimensions"]["layout"] != json!("top-left-row-major")
        || result["dimensions"]["pixel_format"] != json!("straight-alpha-srgb-rgba8")
    {
        return Err(format!("{viewer} result {id} raster layout is invalid"));
    }
    let source_raster = string(&result["source_raster_sha256"], "source raster digest")?;
    let subset_raster = string(&result["subset_raster_sha256"], "subset raster digest")?;
    verify_sha256(source_raster, "source raster digest")?;
    verify_sha256(subset_raster, "subset raster digest")?;
    verify_repeats(
        &result["source_run_sha256"],
        source_raster,
        viewer,
        id,
        "source",
    )?;
    verify_repeats(
        &result["subset_run_sha256"],
        subset_raster,
        viewer,
        id,
        "subset",
    )?;
    verify_sha256(
        string(&result["source_semantic_sha256"], "source semantic digest")?,
        "source semantic digest",
    )?;
    verify_sha256(
        string(&result["subset_semantic_sha256"], "subset semantic digest")?,
        "subset semantic digest",
    )?;
    if result["source_semantic_sha256"] != result["subset_semantic_sha256"] {
        return Err(format!(
            "{viewer} result {id} semantic source/subset reports differ"
        ));
    }
    let checks = object(&result["checks"], "semantic checks")?;
    if checks.len() != REQUIRED_CHECKS.len()
        || REQUIRED_CHECKS
            .iter()
            .any(|check| checks.get(*check) != Some(&json!("pass")))
    {
        return Err(format!(
            "{viewer} result {id} has incomplete semantic parity"
        ));
    }
    if result["comparison"]["passed"] != json!(true)
        || result["comparison"]["visual_class"] != json!("typography")
        || result["comparison"]["width"] != json!(width)
        || result["comparison"]["height"] != json!(height)
        || result["decision"] != json!("pass")
    {
        return Err(format!("{viewer} result {id} did not pass independently"));
    }
    verify_compatibility_record(result, viewer, id)?;
    Ok(())
}

fn verify_repeats(
    value: &Value,
    expected: &str,
    viewer: &str,
    id: &str,
    role: &str,
) -> Result<(), String> {
    if value != &json!([expected, expected]) {
        return Err(format!(
            "{viewer} result {id} {role} runs are not deterministic"
        ));
    }
    Ok(())
}

fn verify_compatibility_record(result: &Value, viewer: &str, id: &str) -> Result<(), String> {
    let changed = result["comparison"]["changed_pixels"]
        .as_u64()
        .ok_or_else(|| format!("{viewer} result {id} has invalid changed-pixel count"))?;
    let exact = changed == 0
        && result["comparison"]["ssim_bits"] == json!("3ff0000000000000")
        && result["source_raster_sha256"] == result["subset_raster_sha256"];
    if !result["compatibility_record"].is_null() {
        return Err(format!(
            "{viewer} result {id} unexpectedly carries a compatibility record"
        ));
    }
    if !exact {
        return Err(format!(
            "{viewer} result {id} contains an unapproved source/subset difference"
        ));
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
        "schema": "excalidraw-render/svg-font-subset-viewer-approval-subject/v1",
    }))?))
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
                    "https://github.com/sagikazarmark/excalidraw-render.orig/issues/99#issuecomment-",
                )
            })
            .is_none_or(|id| {
                id.is_empty() || !id.chars().all(|character| character.is_ascii_digit())
            })
    {
        return Err("SVG font subset viewer qualification approval is invalid".to_owned());
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
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
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

fn result_passed(result: &Value) -> bool {
    result["decision"] == json!("pass")
        && REQUIRED_CHECKS
            .iter()
            .all(|check| result["checks"][*check] == json!("pass"))
        && result["comparison"]["passed"] == json!(true)
}

fn decision(result: &Value) -> &'static str {
    if result_passed(result) {
        "pass"
    } else {
        "fail"
    }
}

fn verify_schema(value: &Value) -> Result<(), String> {
    if value["schema"]["id"] != json!(SCHEMA_ID) {
        return Err("unexpected SVG font subset viewer schema identity".to_owned());
    }
    let bytes = verify_descriptor(&value["schema"])?;
    let schema: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid SVG font subset viewer schema: {error}"))?;
    jsonschema::draft202012::meta::validate(&schema)
        .map_err(|error| format!("SVG font subset viewer schema fails its meta-schema: {error}"))?;
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| format!("failed to compile SVG font subset viewer schema: {error}"))?
        .validate(value)
        .map_err(|error| format!("SVG font subset viewer manifest fails its schema: {error}"))
}

fn verify_descriptor(descriptor: &Value) -> Result<Vec<u8>, String> {
    let descriptor = object(descriptor, "file descriptor")?;
    let relative = string(
        descriptor
            .get("path")
            .ok_or_else(|| "file descriptor path is required".to_owned())?,
        "file descriptor path",
    )?;
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
    let expected = string(
        descriptor
            .get("sha256")
            .ok_or_else(|| "file descriptor digest is required".to_owned())?,
        "file descriptor digest",
    )?;
    if sha256(&bytes) != expected {
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

fn verify_sha256(value: &str, subject: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{subject} must be a lowercase SHA-256 digest"));
    }
    Ok(())
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
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key).expect("object key serializes"));
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
