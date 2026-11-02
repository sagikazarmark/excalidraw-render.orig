use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use serde_json::{json, Value};

use crate::conformance::{
    canonical_bytes, object, read_repository_file as read_repository_file_in, repo_root, sha256,
};
use crate::revision::sealed_digest;
use crate::viewer_identity::{runtime_record, svg_viewer_profile, ReferenceRuntime};

const MANIFEST_PATH: &str =
    "tests/conformance/svg-font-subset-viewer-qualification/revision-1.json";
const MANIFEST_DIGEST_PATH: &str =
    "tests/conformance/svg-font-subset-viewer-qualification/revision-1.sha256";
const SCHEMA_ID: &str = "excalidraw-render/conformance-svg-font-subset-viewer-qualification/v1";
const CORPUS_PATH: &str = concat!(
    "tests/oracle/compatibility-artifacts/",
    "revision-1-svg-font-subset-qualification-corpus.json"
);
/// The corpus cells and semantic executions the qualification covers: the 9
/// classes over 230 official chunks and the Noto Emoji fallback (#150), less
/// the not-selected and not-applicable cells, deduplicated by semantic subset
/// identity into executions. `verify` holds the corpus and the manifest to
/// these counts; the schema states the execution count as well.
pub(crate) const SUCCESSFUL_CELLS: usize = 780;
pub(crate) const EXECUTIONS: usize = 662;
const SUBSET_PROFILE_PATH: &str =
    "tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-profile.json";
const CHROMIUM_PROFILE_PATH: &str =
    "tests/conformance/viewer-profiles/canonical-svg-chromium-v1.json";
const PURE_RUST_PROFILE_PATH: &str =
    "tests/conformance/viewer-profiles/canonical-svg-pure-rust-v1.json";
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
    materialize_schema()?;
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
        let id = string(&case["glyph_set"], "glyph set")?;
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
            "glyph_set": id,
            "pure_rust_decision": rust_decision,
            "semantic_subset_identity": case["semantic_subset_identity"],
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

fn materialize_schema() -> Result<(), String> {
    let path =
        repo_root().join("tests/conformance/svg-font-subset-viewer-qualification-v1.schema.json");
    let mut schema: Value = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("invalid SVG font subset viewer schema: {error}"))?;
    schema["properties"]["executions"]["minItems"] = json!(EXECUTIONS);
    schema["properties"]["executions"]["maxItems"] = json!(EXECUTIONS);
    let bytes = canonical_bytes(&schema)?;
    fs::write(&path, [&bytes[..], b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn report_decisions(report: &Value, viewer: &str) -> Result<BTreeMap<String, String>, String> {
    if report["viewer"] != json!(viewer) {
        return Err(format!("unexpected {viewer} report identity"));
    }
    let mut decisions = BTreeMap::new();
    for result in array(&report["results"], "viewer report results")? {
        let id = string(&result["glyph_set"], "viewer result glyph set")?;
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
    read_repository_file_in(&repo_root(), relative)
}

pub fn verify() -> Result<(), String> {
    verify_internal()
}

fn verify_internal() -> Result<(), String> {
    let bytes = fs::read(repo_root().join(MANIFEST_PATH))
        .map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?;
    let manifest: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid SVG font subset viewer manifest: {error}"))?;
    let canonical = canonical_bytes(&manifest)?;
    if bytes != [&canonical[..], b"\n"].concat() {
        return Err("SVG font subset viewer manifest is not canonical JSON plus LF".to_owned());
    }
    verify_sidecar(MANIFEST_DIGEST_PATH, &canonical)?;
    verify_manifest(&manifest)
}

fn verify_manifest(manifest: &Value) -> Result<(), String> {
    verify_schema(manifest)?;
    verify_identity_and_inputs(manifest)?;

    let expected = expected_cells()?;
    let executions = verify_execution_coverage(manifest, &expected)?;
    let input_sha256 = string(
        &manifest["input_manifest_sha256"],
        "viewer input manifest digest",
    )?;
    verify_sha256(input_sha256, "viewer input manifest digest")?;
    let chromium = verify_viewer_report(&manifest["viewer_reports"]["chromium"], "chromium")?;
    let pure_rust = verify_viewer_report(&manifest["viewer_reports"]["pure_rust"], "pure_rust")?;
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
    Ok(())
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
        || manifest["corpus"]["sha256"] != json!(sealed_digest(CORPUS_PATH)?)
        || manifest["subset_profile"]["path"] != json!(SUBSET_PROFILE_PATH)
        || manifest["subset_profile"]["sha256"] != json!(sealed_digest(SUBSET_PROFILE_PATH)?)
        || manifest["viewer_profiles"]["chromium"]["path"] != json!(CHROMIUM_PROFILE_PATH)
        || manifest["viewer_profiles"]["chromium"]["sha256"]
            != json!(sealed_digest(CHROMIUM_PROFILE_PATH)?)
        || manifest["viewer_profiles"]["pure_rust"]["path"] != json!(PURE_RUST_PROFILE_PATH)
        || manifest["viewer_profiles"]["pure_rust"]["sha256"]
            != json!(sealed_digest(PURE_RUST_PROFILE_PATH)?)
    {
        return Err("qualification dependency identity is stale".to_owned());
    }
    // Implementation identities describe the run that produced the accepted
    // evidence. They describe that run, but need not match the
    // current source tree. The corpus test checks today's generated subsets;
    // viewer reproduction checks today's adapters against the retained reports.
    let mut sources = BTreeSet::new();
    for source in array(&manifest["implementation"], "qualification implementation")? {
        let (path, _) = descriptor_identity(source)?;
        if !sources.insert(path) {
            return Err(format!("duplicate qualification implementation {path}"));
        }
    }
    if sources.is_empty() {
        return Err("qualification implementation provenance is empty".to_owned());
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
    /// The WOFF2 the viewers load as the source face: the chunk itself, or
    /// the encoding of the Noto Emoji sfnt the corpus pins as
    /// `viewerSourceWoff2Sha256`.
    source_woff2_sha256: String,
    /// The manifested source sfnt, when the source is one (the Noto Emoji
    /// fallback); the execution's `source_sfnt_sha256` must be it.
    source_sfnt_sha256: Option<String>,
    subset_woff2_sha256: String,
}

fn expected_cells() -> Result<BTreeMap<(String, String), ExpectedCell>, String> {
    let bytes = fs::read(repo_root().join(CORPUS_PATH))
        .map_err(|error| format!("failed to read {CORPUS_PATH}: {error}"))?;
    if sha256(&bytes) != sealed_digest(CORPUS_PATH)? {
        return Err(format!(
            "{CORPUS_PATH} is not the bound qualification corpus"
        ));
    }
    let corpus: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid font subset qualification corpus: {error}"))?;
    let mut expected = BTreeMap::new();
    for source in array(&corpus["sources"], "font subset corpus sources")? {
        let source_id = string(&source["sourceIdentity"], "source identity")?;
        let source_sha256 = string(&source["sourceSha256"], "source digest")?;
        let (source_woff2_sha256, source_sfnt_sha256) =
            match string(&source["sourceContainer"], "source container")? {
                "woff2" => (source_sha256.to_owned(), None),
                "sfnt" => (
                    string(
                        &source["viewerSourceWoff2Sha256"],
                        "viewer source WOFF2 digest",
                    )?
                    .to_owned(),
                    Some(source_sha256.to_owned()),
                ),
                container => {
                    return Err(format!(
                        "{source_id} has an unknown source container {container:?}"
                    ))
                }
            };
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
                        source_woff2_sha256: source_woff2_sha256.clone(),
                        source_sfnt_sha256: source_sfnt_sha256.clone(),
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
    if expected.len() != SUCCESSFUL_CELLS {
        return Err(format!(
            "font subset corpus has {} successful cells, expected {SUCCESSFUL_CELLS}",
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
        let id = string(&execution["glyph_set"], "execution glyph set")?;
        if executions.insert(id.to_owned(), execution).is_some() {
            return Err(format!("duplicate qualification execution {id}"));
        }
        let semantic_identity = string(
            &execution["semantic_subset_identity"],
            "execution semantic subset identity",
        )?;
        verify_sha256(semantic_identity, "execution semantic subset identity")?;
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
            if expected_cell.semantic_identity != semantic_identity
                || glyph_set(source, &coverage["ordered_scalars"])? != id
                || coverage["ordered_scalars"] != expected_cell.scalars
                || execution["source_woff2_sha256"] != json!(expected_cell.source_woff2_sha256)
                || expected_cell
                    .source_sfnt_sha256
                    .as_ref()
                    .is_some_and(|sfnt| execution["source_sfnt_sha256"] != json!(sfnt))
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
    if executions.len() != EXECUTIONS {
        return Err(format!(
            "qualification has {} semantic executions, expected {EXECUTIONS}",
            executions.len()
        ));
    }
    Ok(executions)
}

/// The host both viewers qualified the subsets on (ADR-0006).
pub(crate) const QUALIFICATION_HOST: &str = "linux-amd64";
/// The pure-Rust viewer's crates whose versions its font-subset report records.
pub(crate) const PURE_RUST_REPORT_CRATES: [&str; 6] =
    ["fontdb", "harfrust", "resvg", "skrifa", "tiny-skia", "usvg"];

/// The Chromium report's environment: the reference container's runtime and
/// launch arguments, as the reference profile records them.
fn chromium_environment(reference: &ReferenceRuntime) -> Value {
    json!({
        "browser_arguments": reference.browser_arguments.join(" "),
        "chromium": reference.chromium,
        "container": reference.container,
        "host": QUALIFICATION_HOST,
        "locale": "en-US",
        "node": format!("v{}", reference.node),
        "playwright": reference.playwright,
        "role_processes": "fresh Chromium process per execution, role, and repeat",
        "timezone": "UTC",
    })
}

/// The pure-Rust report's environment: the canonical pure-Rust SVG viewer's
/// runtime, as its profile records it (and `verify-svg-viewers` holds that
/// profile to the viewer's lockfile and toolchain).
fn pure_rust_environment(profile: &Value) -> Result<Value, String> {
    let mut environment = json!({
        "host": QUALIFICATION_HOST,
        "rustc": format!("rustc {}", runtime_record(profile, "rustc")?),
        "target": runtime_record(profile, "target")?,
    });
    for name in PURE_RUST_REPORT_CRATES {
        environment[name] = json!(runtime_record(profile, name)?);
    }
    Ok(environment)
}

fn verify_viewer_report(
    descriptor: &Value,
    viewer: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let bytes = verify_descriptor(descriptor)?;
    let report: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {viewer} font subset viewer report: {error}"))?;
    if report["schema"] != json!("excalidraw-render/svg-font-subset-viewer-report/v2")
        || report["viewer"] != json!(viewer)
        || report.get("inputs_sha256").is_some()
        || report["samples_per_role"] != json!(2)
        || report["fresh_state_per_role"] != json!(true)
    {
        return Err(format!("{viewer} viewer report identity is invalid"));
    }
    let environment = &report["environment"];
    let expected_environment = match viewer {
        "chromium" => chromium_environment(&ReferenceRuntime::load()?),
        "pure_rust" => pure_rust_environment(&svg_viewer_profile("pure-rust")?)?,
        _ => return Err(format!("unknown viewer report {viewer}")),
    };
    if environment != &expected_environment {
        return Err(format!("{viewer} viewer environment is not pinned"));
    }
    let mut results = BTreeMap::new();
    for result in array(&report["results"], "viewer results")? {
        let id = string(&result["glyph_set"], "viewer result glyph set")?;
        if results.insert(id.to_owned(), result.clone()).is_some() {
            return Err(format!("duplicate {viewer} viewer result {id}"));
        }
        verify_result(result, viewer)?;
    }
    Ok(results)
}

fn verify_result(result: &Value, viewer: &str) -> Result<(), String> {
    let id = string(&result["glyph_set"], "viewer result glyph set")?;
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
        .build(&crate::conformance::schema_without_approval_gate(&schema))
        .map_err(|error| format!("failed to compile SVG font subset viewer schema: {error}"))?
        .validate(value)
        .map_err(|error| format!("SVG font subset viewer manifest fails its schema: {error}"))
}

fn descriptor_identity(descriptor: &Value) -> Result<(&str, &str), String> {
    let descriptor = object(descriptor, "file descriptor")?;
    let relative = string(
        descriptor
            .get("path")
            .ok_or_else(|| "file descriptor path is required".to_owned())?,
        "file descriptor path",
    )?;
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe repository-relative path {relative:?}"));
    }
    let expected = string(
        descriptor
            .get("sha256")
            .ok_or_else(|| "file descriptor digest is required".to_owned())?,
        "file descriptor digest",
    )?;
    verify_sha256(expected, "file descriptor digest")?;
    Ok((relative, expected))
}

fn verify_descriptor(descriptor: &Value) -> Result<Vec<u8>, String> {
    let (relative, expected) = descriptor_identity(descriptor)?;
    let root = fs::canonicalize(repo_root())
        .map_err(|error| format!("failed to resolve repository root: {error}"))?;
    let resolved = fs::canonicalize(root.join(relative))
        .map_err(|error| format!("failed to resolve {relative}: {error}"))?;
    if !resolved.starts_with(&root) {
        return Err(format!(
            "file descriptor path escapes repository: {relative}"
        ));
    }
    let bytes =
        fs::read(resolved).map_err(|error| format!("failed to read {relative}: {error}"))?;
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

/// The key of an execution: what the viewers render, the source font's
/// identity (which names its bytes' digest) and the ordered scalars set in it.
/// It is the semantic subset identity's preimage without the subset profile,
/// so the executions and both viewer reports keep their keys when the profile
/// moves; the renderer's viewer-input materializer computes the same.
fn glyph_set(source_identity: &str, ordered_scalars: &Value) -> Result<String, String> {
    let scalars = array(ordered_scalars, "ordered scalars")?
        .iter()
        .map(|scalar| {
            scalar
                .as_u64()
                .map(|scalar| scalar.to_string())
                .ok_or_else(|| "ordered scalars must be integers".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    Ok(sha256(
        format!("excalidraw-render:svg-font-subset-glyph-set:v1\n{source_identity}\n{scalars}")
            .as_bytes(),
    ))
}

/// The fields of an execution that name the subset rather than what it
/// renders. A subset profile change moves only these, and
/// [`rebind`] carries them from fresh viewer inputs.
const SUBSET_FIELDS: [&str; 5] = [
    "semantic_subset_identity",
    "subset_assets_sha256",
    "subset_sfnt_sha256",
    "subset_svg_sha256",
    "subset_woff2_sha256",
];

/// Rebinds the qualification to the subsets of a moved subset profile without
/// re-running the viewers: from freshly materialized viewer inputs it rewrites
/// each execution's subset fields, the input manifest digest and the subset
/// profile descriptor, and nothing else. The glyph sets must be the record's
/// exactly, and the source fields must not move, so the viewer reports, keyed
/// by glyph set, stay as they are. The viewers' rasters are compared again
/// only when the qualification is regenerated.
pub fn rebind(inputs: &str) -> Result<(), String> {
    let input_bytes = read_repository_file(inputs)?;
    let input: Value = serde_json::from_slice(&input_bytes)
        .map_err(|error| format!("invalid viewer input manifest: {error}"))?;
    if input["schema"] != json!("excalidraw-render/svg-font-subset-viewer-inputs/v2") {
        return Err("unsupported SVG font subset viewer input manifest".to_owned());
    }
    let bytes = fs::read(repo_root().join(MANIFEST_PATH))
        .map_err(|error| format!("failed to read {MANIFEST_PATH}: {error}"))?;
    let mut manifest: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid SVG font subset viewer manifest: {error}"))?;
    let rebound = rebind_executions(&mut manifest, &input)?;
    manifest["input_manifest_sha256"] = json!(sha256(&input_bytes));
    manifest["subset_profile"] = descriptor(SUBSET_PROFILE_PATH)?;
    manifest["corpus"] = descriptor(CORPUS_PATH)?;
    let canonical = canonical_bytes(&manifest)?;
    fs::write(
        repo_root().join(MANIFEST_PATH),
        [&canonical[..], b"\n"].concat(),
    )
    .map_err(|error| format!("failed to write {MANIFEST_PATH}: {error}"))?;
    fs::write(
        repo_root().join(MANIFEST_DIGEST_PATH),
        format!("{}\n", sha256(&canonical)),
    )
    .map_err(|error| format!("failed to write {MANIFEST_DIGEST_PATH}: {error}"))?;
    println!("rebound {rebound} of {EXECUTIONS} executions to the current subsets");
    Ok(())
}

fn rebind_executions(manifest: &mut Value, input: &Value) -> Result<usize, String> {
    let cases = array(&input["cases"], "viewer input cases")?
        .iter()
        .map(|case| Ok((string(&case["glyph_set"], "glyph set")?.to_owned(), case)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let executions = manifest["executions"]
        .as_array_mut()
        .ok_or_else(|| "qualification executions must be an array".to_owned())?;
    if executions.len() != cases.len() {
        return Err("the viewer inputs do not cover the record's glyph sets".to_owned());
    }
    let mut rebound = 0;
    for execution in executions {
        let key = string(&execution["glyph_set"], "execution glyph set")?.to_owned();
        let case = cases
            .get(&key)
            .ok_or_else(|| format!("the viewer inputs lack glyph set {key}"))?;
        if case["coverage"] != execution["coverage"]
            || [
                "source_assets_sha256",
                "source_sfnt_sha256",
                "source_svg_sha256",
                "source_woff2_sha256",
            ]
            .iter()
            .any(|field| case[*field] != execution[*field])
        {
            return Err(format!(
                "glyph set {key} renders another source than the record; regenerate the qualification"
            ));
        }
        let mut moved = false;
        for field in SUBSET_FIELDS {
            verify_sha256(string(&case[field], field)?, field)?;
            if execution[field] != case[field] {
                execution[field] = case[field].clone();
                moved = true;
            }
        }
        rebound += usize::from(moved);
    }
    Ok(rebound)
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

    fn accepted_manifest() -> Value {
        serde_json::from_slice(&read_repository_file(MANIFEST_PATH).unwrap()).unwrap()
    }

    #[test]
    fn glyph_sets_name_the_source_and_scalars_only() {
        let key = glyph_set("fonts/A.woff2@sha256:00", &json!([65, 66])).unwrap();
        assert_eq!(
            key,
            sha256(
                b"excalidraw-render:svg-font-subset-glyph-set:v1\nfonts/A.woff2@sha256:00\n65,66"
            )
        );
        assert_ne!(
            key,
            glyph_set("fonts/A.woff2@sha256:00", &json!([66, 65])).unwrap()
        );
        assert_ne!(
            key,
            glyph_set("fonts/B.woff2@sha256:00", &json!([65, 66])).unwrap()
        );
        assert!(glyph_set("fonts/A.woff2@sha256:00", &json!(["A"])).is_err());
    }

    #[test]
    fn rebinding_moves_only_the_subset_fields_of_unchanged_glyph_sets() {
        let digest = |byte: char| byte.to_string().repeat(64);
        let execution = json!({
            "coverage": [{"class": "mapped-scalar", "ordered_scalars": [65], "source_identity": "s"}],
            "decision": "pass",
            "glyph_set": digest('a'),
            "semantic_subset_identity": digest('b'),
            "source_assets_sha256": digest('c'),
            "source_sfnt_sha256": digest('c'),
            "source_svg_sha256": digest('c'),
            "source_woff2_sha256": digest('c'),
            "subset_assets_sha256": digest('d'),
            "subset_sfnt_sha256": digest('d'),
            "subset_svg_sha256": digest('d'),
            "subset_woff2_sha256": digest('d'),
        });
        let mut case = execution.clone();
        for field in SUBSET_FIELDS {
            case[field] = json!(digest('e'));
        }
        let input = json!({"cases": [case.clone()]});
        let mut manifest = json!({"executions": [execution.clone()]});
        assert_eq!(rebind_executions(&mut manifest, &input).unwrap(), 1);
        let rebound = &manifest["executions"][0];
        for field in SUBSET_FIELDS {
            assert_eq!(rebound[field], json!(digest('e')), "{field}");
        }
        for field in ["glyph_set", "coverage", "decision", "source_woff2_sha256"] {
            assert_eq!(rebound[field], execution[field], "{field}");
        }
        // Idempotent: nothing moves the second time.
        assert_eq!(rebind_executions(&mut manifest, &input).unwrap(), 0);

        let mut moved_source = case.clone();
        moved_source["source_woff2_sha256"] = json!(digest('f'));
        let mut manifest = json!({"executions": [execution.clone()]});
        assert!(
            rebind_executions(&mut manifest, &json!({"cases": [moved_source]}))
                .unwrap_err()
                .contains("regenerate the qualification")
        );
        let mut other = case;
        other["glyph_set"] = json!(digest('9'));
        assert!(rebind_executions(&mut manifest, &json!({"cases": [other]}))
            .unwrap_err()
            .contains("lack glyph set"));
    }

    #[test]
    fn accepted_qualification_survives_implementation_edits() {
        // This verifier is itself a recorded producer. Adding this test changes
        // its source bytes without changing any subset, input or viewer result.
        verify().unwrap();
    }

    #[test]
    fn historical_implementation_need_not_exist_in_the_current_tree() {
        let mut manifest = accepted_manifest();
        // In-memory proposal only: changing the historical record itself still
        // requires acceptance, tested separately below.
        manifest["approval"] = Value::Null;
        manifest["implementation"][0] = json!({
            "path": "retired/font-subsetter.rs",
            "sha256": "0".repeat(64),
        });
        verify_manifest(&manifest).unwrap();
    }

    #[test]
    fn historical_implementation_changes_do_not_require_acceptance_hashes() {
        for field in ["path", "sha256"] {
            let mut manifest = accepted_manifest();
            manifest["implementation"][0][field] = json!(if field == "path" {
                "retired/font-subsetter.rs".to_owned()
            } else {
                "0".repeat(64)
            });
            verify_manifest(&manifest).unwrap();
        }
        let mut manifest = accepted_manifest();
        manifest["implementation"].as_array_mut().unwrap().remove(0);
        verify_manifest(&manifest).unwrap();
    }

    #[test]
    fn historical_implementation_rejects_malformed_or_duplicate_descriptors() {
        let original = accepted_manifest();
        let source = original["implementation"][0].clone();
        for implementation in [
            json!([]),
            json!([source, source]),
            json!([{"path": "../outside.rs", "sha256": "0".repeat(64)}]),
            json!([{"path": "/absolute.rs", "sha256": "0".repeat(64)}]),
            json!([{"path": "", "sha256": "0".repeat(64)}]),
            json!([{"path": "retired/font-subsetter.rs", "sha256": "not-a-digest"}]),
        ] {
            let mut manifest = original.clone();
            manifest["approval"] = Value::Null;
            manifest["implementation"] = implementation;
            assert!(verify_manifest(&manifest).is_err());
        }
    }

    #[test]
    fn corpus_profiles_and_viewer_reports_still_bind_current_bytes() {
        let mut original = accepted_manifest();
        original["approval"] = Value::Null;
        verify_manifest(&original).unwrap();
        for pointer in [
            "/corpus/sha256",
            "/subset_profile/sha256",
            "/viewer_profiles/chromium/sha256",
            "/viewer_profiles/pure_rust/sha256",
            "/viewer_reports/chromium/sha256",
            "/viewer_reports/pure_rust/sha256",
        ] {
            let mut changed = original.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("0".repeat(64));
            assert!(
                verify_manifest(&changed)
                    .unwrap_err()
                    .contains("stale digest"),
                "accepted changed {pointer}"
            );
        }
    }

    #[test]
    fn changed_font_bytes_or_missing_executions_still_fail_qualification() {
        let mut original = accepted_manifest();
        original["approval"] = Value::Null;
        for field in ["source_woff2_sha256", "subset_woff2_sha256"] {
            let mut changed = original.clone();
            changed["executions"][0][field] = json!("0".repeat(64));
            assert!(verify_manifest(&changed)
                .unwrap_err()
                .contains("stale or duplicate coverage"));
        }
        let mut changed = original;
        changed["executions"].as_array_mut().unwrap().pop();
        assert!(verify_manifest(&changed).is_err());
    }
}
