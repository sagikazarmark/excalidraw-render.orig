//! Shared draft inventory and execution-result boundary for #85 / #86.
//! Candidate evidence is not an ownership decision or an execution result.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::conformance::{canonical_bytes, read_repository_file, repo_root, sha256};

const INVENTORY: &str = "tests/conformance/coverage/inventory.json";
const REQUIREMENTS: &str = "tests/conformance/coverage/requirements.json";
const REGISTRY: &str = "tests/conformance/cases/registry.json";
const SURFACE: &str = "tests/oracle/scene-surfaces/revision-1.json";
const INVENTORY_SCHEMA: &str = "excalidraw-render/coverage-inventory/v1";
const RESULT_SCHEMA: &str = "excalidraw-render/coverage-results/v1";
const TARGETS: [&str; 2] = ["wasm32-unknown-unknown", "x86_64-unknown-linux-gnu"];

mod snapshots;
pub use snapshots::{
    baseline_subject, record_baseline_approval, replay_snapshots, stage_baseline, verify_baseline,
    verify_snapshots,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Evidence {
    source: Descriptor,
    /// Exact source anchor, not a claim that the test has executed.
    anchor: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Requirement {
    id: String,
    issue: u64,
    remaining: String,
    evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Requirements {
    schema: String,
    requirements: Vec<Requirement>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CoverageRow {
    id: String,
    /// Local coverage labels are candidates, never automatically primary owners.
    candidates: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Format {
    Svg,
    Png,
    Pdf,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ExportTarget {
    Scene {},
    Frame { ordinal: u64 },
}

/// The same key is consumed by snapshot generation and every target collector.
/// Its digest binds the whole manifest, scene, options, profile and asset catalog.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct CaseKey {
    case: String,
    variant: String,
    export: ExportTarget,
    format: Format,
    inputs_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Execution {
    key: CaseKey,
    options: Value,
    /// Reference evidence exists for these formats; PDF absence remains visible.
    reference_artifact: Option<Descriptor>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: String,
    compatibility_revision: u64,
    scope: String,
    sources: Vec<Descriptor>,
    coverage: Vec<CoverageRow>,
    executions: Vec<Execution>,
    requirements: Vec<Requirement>,
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    read_repository_file(&repo_root(), path)
}

fn descriptor(path: &str) -> Result<Descriptor, String> {
    Ok(Descriptor {
        path: path.into(),
        sha256: sha256(&read(path)?),
    })
}

fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|error| format!("invalid coverage contract: {error}"))
}

fn digest<T: Serialize>(value: &T) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    Ok(sha256(&canonical_bytes(&value)?))
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value[field]
        .as_str()
        .ok_or_else(|| format!("missing string {field}"))
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, String> {
    value[field]
        .as_array()
        .ok_or_else(|| format!("missing array {field}"))
}

fn add_row(rows: &mut BTreeMap<String, BTreeSet<String>>, id: String, candidate: Option<&str>) {
    let candidates = rows.entry(id).or_default();
    if let Some(candidate) = candidate {
        candidates.insert(candidate.into());
    }
}

fn derive_inventory() -> Result<Inventory, String> {
    let requirements: Requirements = parse(&read(REQUIREMENTS)?)?;
    if requirements.schema != "excalidraw-render/coverage-requirements/v1" {
        return Err("unexpected coverage requirements schema".into());
    }
    let mut seen = BTreeSet::new();
    for requirement in &requirements.requirements {
        if requirement.id.is_empty()
            || !seen.insert(requirement.id.as_str())
            || requirement.issue == 0
            || requirement.remaining.trim().is_empty()
        {
            return Err("invalid or duplicate coverage requirement".into());
        }
        for evidence in &requirement.evidence {
            if descriptor(&evidence.source.path)? != evidence.source
                || evidence.anchor.is_empty()
                || !String::from_utf8_lossy(&read(&evidence.source.path)?)
                    .contains(&evidence.anchor)
            {
                return Err(format!("stale evidence anchor for {}", requirement.id));
            }
        }
    }
    // Fixed pending milestones cannot disappear merely by removing a policy row.
    let expected = [
        "corpus.five-layers",
        "coverage.global-ownership",
        "fuzz.codecs",
        "fuzz.complete-render",
        "snapshots.public-facade",
        "determinism.process-order",
        "determinism.cross-target",
        "smoke.mixed-scene",
    ];
    if seen != expected.into_iter().collect() {
        return Err("coverage requirements must retain the complete milestone set".into());
    }
    let registry: Value = parse(&read(REGISTRY)?)?;
    let surface: Value = parse(&read(SURFACE)?)?;
    let catalog = descriptor("crates/excalidraw-render-assets-base/catalog/revision-1.json")?;
    let mut sources = vec![
        descriptor(REGISTRY)?,
        descriptor(SURFACE)?,
        descriptor(REQUIREMENTS)?,
        catalog.clone(),
    ];
    let mut rows = BTreeMap::new();
    for field in array(&surface, "fieldDispositions")? {
        add_row(
            &mut rows,
            format!(
                "surface:{}:{}",
                string(field, "recordKind")?,
                string(field, "path")?
            ),
            None,
        );
    }
    let mut executions = Vec::new();
    let mut cases = BTreeSet::new();
    for case in array(&registry, "cases")? {
        let id = string(case, "id")?;
        if !cases.insert(id) {
            return Err(format!("duplicate case {id}"));
        }
        let path = string(case, "path")?;
        let manifest_path = format!("{path}/manifest.json");
        let manifest: Value = parse(&read(&manifest_path)?)?;
        let manifest_descriptor = descriptor(&manifest_path)?;
        sources.push(manifest_descriptor.clone());
        let scene_path = format!("{path}/{}", string(&manifest["scene"], "path")?);
        let scene = descriptor(&scene_path)?;
        if scene.sha256 != string(&manifest["scene"], "sha256")? {
            return Err(format!("stale scene for {id}"));
        }
        let profile = if case["kind"] == "frame" {
            &manifest["profile"]
        } else {
            &manifest["reference"]["profile"]
        };
        let profile = descriptor(string(profile, "path")?)?;
        sources.push(profile.clone());
        if let Some(coverage) = manifest["coverage"].as_array() {
            for row in coverage {
                add_row(
                    &mut rows,
                    format!("case-label:{}", string(row, "identity")?),
                    Some(id),
                );
            }
        }
        let mut append = |variant: &str,
                          export: ExportTarget,
                          options: &Value,
                          formats: &Value|
         -> Result<(), String> {
            let input_digest = digest(&json!({"manifest":manifest_descriptor,"scene":scene,
                "profile":profile,"catalog":catalog,"options":options,"export":export}))?;
            for (format, name) in [
                (Format::Svg, "svg"),
                (Format::Png, "png"),
                (Format::Pdf, "pdf"),
            ] {
                let reference_artifact = if formats[name]["artifact"].is_object() {
                    let artifact = descriptor(&format!(
                        "{path}/{}",
                        string(&formats[name]["artifact"], "path")?
                    ))?;
                    if artifact.sha256 != string(&formats[name]["artifact"], "sha256")? {
                        return Err(format!(
                            "stale reference artifact for {id}/{variant}/{name}"
                        ));
                    }
                    Some(artifact)
                } else {
                    None
                };
                executions.push(Execution {
                    key: CaseKey {
                        case: id.into(),
                        variant: variant.into(),
                        export: export.clone(),
                        format,
                        inputs_sha256: input_digest.clone(),
                    },
                    options: options.clone(),
                    reference_artifact,
                });
            }
            Ok(())
        };
        if case["kind"] == "frame" {
            for target in array(&manifest, "targets")? {
                let ordinal = target["frame_ordinal"]
                    .as_u64()
                    .filter(|n| *n > 0)
                    .ok_or("invalid frame ordinal")?;
                append(
                    "default",
                    ExportTarget::Frame { ordinal },
                    &manifest["options"],
                    &target["outputs"],
                )?;
            }
        } else {
            for variant in array(&manifest, "variants")? {
                append(
                    string(variant, "id")?,
                    ExportTarget::Scene {},
                    &variant["options"],
                    &variant["formats"],
                )?;
            }
        }
    }
    executions.sort_by(|a, b| a.key.cmp(&b.key));
    if executions.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err("duplicate execution key".into());
    }
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    sources.dedup();
    Ok(Inventory {
        schema: INVENTORY_SCHEMA.into(),
        compatibility_revision: 1,
        scope: "draft-registered-cases-and-surface-fields".into(),
        sources,
        coverage: rows
            .into_iter()
            .map(|(id, candidates)| CoverageRow {
                id,
                candidates: candidates.into_iter().collect(),
            })
            .collect(),
        executions,
        requirements: requirements.requirements,
    })
}

pub fn inventory_json() -> Result<String, String> {
    serde_json::to_string_pretty(&derive_inventory()?).map_err(|error| error.to_string())
}

pub fn inventory_digest() -> Result<String, String> {
    digest(&load_inventory()?)
}

fn load_inventory() -> Result<Inventory, String> {
    let inventory: Inventory = parse(&read(INVENTORY)?)?;
    if inventory != derive_inventory()? {
        return Err(
            "stale coverage inventory; regenerate with conformance inventory and review the diff"
                .into(),
        );
    }
    Ok(inventory)
}

pub fn verify_inventory() -> Result<(), String> {
    let inventory = load_inventory()?;
    for id in crate::conformance::case_ids()? {
        crate::conformance::verify(&id, true)?;
    }
    println!("verified draft inventory: {} coverage rows, {} executions, {} unresolved milestones; release closure is pending",
        inventory.coverage.len(), inventory.executions.len(), inventory.requirements.len());
    Ok(())
}

pub fn verify_closure() -> Result<(), String> {
    let inventory = load_inventory()?;
    Err(format!(
        "release coverage is draft and unapproved; unresolved: {}",
        inventory
            .requirements
            .iter()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Bytes {
    sha256: String,
    byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Outcome {
    Artifact { bytes: Bytes },
    Failure { code: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Sample {
    outcome: Outcome,
    report: Bytes,
    trace: Bytes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CaseResult {
    key: CaseKey,
    samples: [Sample; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Order {
    Forward,
    Reverse,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProcessRun {
    target: String,
    order: Order,
    /// Collector-issued identity of a fresh process/instance; never a PID shared across hosts.
    process_id: String,
    results: Vec<CaseResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultBundle {
    schema: String,
    compatibility_revision: u64,
    commit: String,
    inventory_sha256: String,
    tool_manifest_sha256: String,
    dagger_lock_sha256: String,
    runs: Vec<ProcessRun>,
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_results(inventory: &Inventory, bundle: &ResultBundle) -> Result<(), String> {
    if bundle.schema != RESULT_SCHEMA
        || bundle.compatibility_revision != 1
        || !hex(&bundle.commit, 40)
        || bundle.inventory_sha256 != digest(inventory)?
        || !hex(&bundle.tool_manifest_sha256, 64)
        || !hex(&bundle.dagger_lock_sha256, 64)
    {
        return Err("invalid result identity or inventory binding".into());
    }
    let keys: Vec<_> = inventory.executions.iter().map(|e| &e.key).collect();
    if keys.is_empty() {
        return Err("result inventory is empty".into());
    }
    let mut runs = BTreeSet::new();
    let mut processes = BTreeSet::new();
    let mut canonical = BTreeMap::new();
    for run in &bundle.runs {
        if !TARGETS.contains(&run.target.as_str())
            || !runs.insert((run.target.as_str(), run.order.clone()))
        {
            return Err("unsupported target or duplicate process order".into());
        }
        if run.process_id.trim().is_empty() || !processes.insert(&run.process_id) {
            return Err("process identities must be nonempty and distinct".into());
        }
        let expected: Vec<_> = match run.order {
            Order::Forward => keys.clone(),
            Order::Reverse => keys.iter().rev().copied().collect(),
        };
        if run.results.iter().map(|r| &r.key).collect::<Vec<_>>() != expected {
            return Err(
                "result keys are missing, extra, duplicated, stale or out of process order".into(),
            );
        }
        for result in &run.results {
            for sample in &result.samples {
                let mut records = vec![&sample.report, &sample.trace];
                match &sample.outcome {
                    Outcome::Artifact { bytes } => records.push(bytes),
                    Outcome::Failure { code } if code.trim().is_empty() => {
                        return Err("failure code is empty".into())
                    }
                    Outcome::Failure { .. } => {}
                }
                if records
                    .iter()
                    .any(|b| !hex(&b.sha256, 64) || b.byte_length == 0)
                {
                    return Err("invalid artifact/report/trace descriptor".into());
                }
            }
            if result.samples[0] != result.samples[1] {
                return Err(format!("fresh-session mismatch for {:?}", result.key));
            }
            if let Some(first) = canonical.insert(&result.key, &result.samples[0]) {
                if first != &result.samples[0] {
                    return Err(format!(
                        "cross-target/process mismatch for {:?}",
                        result.key
                    ));
                }
            }
        }
    }
    if runs.len() != TARGETS.len() * 2 {
        return Err("missing supported target/process order".into());
    }
    Ok(())
}

pub fn verify_results(path: &str) -> Result<(), String> {
    let inventory = load_inventory()?;
    let bundle: ResultBundle = parse(&fs::read(path).map_err(|e| e.to_string())?)?;
    if bundle.tool_manifest_sha256
        != descriptor("tests/package-consumer/tool-manifest.json")?.sha256
        || bundle.dagger_lock_sha256 != descriptor("dagger.lock")?.sha256
    {
        return Err("result tooling differs from the current pinned tools".into());
    }
    validate_results(&inventory, &bundle)?;
    println!("verified result equality for the draft inventory; canonical snapshot approval and release closure remain pending");
    Ok(())
}
