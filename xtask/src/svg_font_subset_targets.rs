use crate::conformance::{read_repository_file, repo_root, sha256};
use serde_json::{json, Value};
use std::env;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

const REPORT_SCHEMA: &str = "excalidraw-render/svg-font-subset-target-evidence/v2";
const AGGREGATE_SCHEMA: &str = "excalidraw-render/svg-font-subset-target-aggregate/v2";
const CROSS_TARGET_SCHEMA: &str = "excalidraw-render/svg-font-subset-cross-target-measurement/v1";
/// Committed cross-target qualification measurement. Canonical resource traces
/// are held stable by cross-target equality rather than by per-cell pins
/// (ADR-0002); this file keeps that equality visible as a reviewable diff and
/// is re-asserted by every aggregation.
const CROSS_TARGET_PATH: &str =
    "tests/conformance/svg-font-subset-cross-target-measurement/revision-1.json";
const RELEASE_ID: &str = "svg-font-subsets.release-evidence.revision-1.v1";
const RELEASE_PATH: &str = "tests/conformance/svg-font-subset-release-evidence/revision-1.json";
/// The verbatim copy of the five-target aggregate that CI uploaded, once the
/// maintainer has recorded it into the release evidence.
const AGGREGATE_PATH: &str = "tests/conformance/svg-font-subset-target-aggregate/revision-1.json";
/// Only a run of this repository's workflow can have produced the aggregate.
const RUN_URL_PREFIX: &str =
    "https://github.com/sagikazarmark/excalidraw-render.orig/actions/runs/";
const MEASUREMENT_PREFIX: &str = "SVG_FONT_SUBSET_TARGET_MEASUREMENT=";
const TARGETS: [&str; 5] = [
    "aarch64-apple-darwin",
    "wasm32-unknown-unknown",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
];
/// The fields every target report, and the aggregate built from them, must
/// agree on.
const AGREED_FIELDS: [&[&str]; 8] = [
    &["commit"],
    &["corpus_sha256"],
    &["subset_profile_sha256"],
    &["viewer_approval_subject"],
    &["measurement", "generatedBytesSha256"],
    &["measurement", "resourceTracesSha256"],
    &["measurement", "successfulCells"],
    &["measurement", "tracedCells"],
];

/// Where the release evidence stands on supported-target evidence. The manifest
/// carries the state as its `targetEvidence.status` / `decision` pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetEvidenceState {
    /// CI has not yet produced the five-target aggregate for this evidence.
    PendingCi,
    /// The five-target aggregate is recorded and bound to this evidence.
    Aggregated,
}

impl TargetEvidenceState {
    const ALL: [Self; 2] = [Self::PendingCi, Self::Aggregated];

    fn status(self) -> &'static str {
        match self {
            Self::PendingCi => "pending_ci",
            Self::Aggregated => "aggregated",
        }
    }

    fn decision(self) -> &'static str {
        match self {
            Self::PendingCi => "pending_target_aggregation",
            Self::Aggregated => "pass",
        }
    }

    /// The state a manifest declares, if its status and decision agree.
    fn declared(status: &str, decision: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|state| state.status() == status && state.decision() == decision)
    }
}

impl fmt::Display for TargetEvidenceState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.status())
    }
}

pub(crate) fn materialize(target: &str, test_log: &str, output: &str) -> Result<(), String> {
    if !TARGETS.contains(&target) {
        return Err(format!("unsupported target {target:?}"));
    }
    let commit = tested_commit()?;
    let measurement = read_measurement(test_log)?;

    let root = repo_root();
    let corpus = fs::read(root.join(
        "tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-qualification-corpus.json",
    ))
    .map_err(|error| format!("failed to read qualification corpus: {error}"))?;
    let profile = read_json(
        &root.join("tests/oracle/compatibility-artifacts/revision-1-svg-font-subset-profile.json"),
    )?;
    let viewers = read_json(
        &root.join("tests/conformance/svg-font-subset-viewer-qualification/revision-1.json"),
    )?;
    let report = json!({
        "schema": REPORT_SCHEMA,
        "commit": commit,
        "compatibility_revision": 1,
        "target": target,
        "corpus_sha256": sha256(&corpus),
        "subset_profile_sha256": profile_digest(&profile)?,
        "viewer_approval_subject": required_path_string(
            &viewers,
            &["approval", "approval_subject_sha256"],
        )?,
        "measurement": measurement,
        "status": "pass",
    });
    write_json(Path::new(output), &report)
}

fn read_measurement(test_log: &str) -> Result<Value, String> {
    let log = fs::read_to_string(test_log)
        .map_err(|error| format!("failed to read qualification log {test_log}: {error}"))?;
    let measurements = log
        .lines()
        .filter_map(|line| line.split_once(MEASUREMENT_PREFIX).map(|(_, value)| value))
        .collect::<Vec<_>>();
    let [measurement] = measurements.as_slice() else {
        return Err(format!(
            "expected exactly one qualification measurement in {test_log}, got {}",
            measurements.len()
        ));
    };
    let measurement: Value = serde_json::from_str(measurement)
        .map_err(|error| format!("qualification measurement is invalid JSON: {error}"))?;
    validate_measurement(&measurement)?;
    Ok(measurement)
}

fn tested_commit() -> Result<String, String> {
    if let Ok(commit) = env::var("GITHUB_SHA") {
        require_digest(&commit, 40, "GITHUB_SHA")?;
        return Ok(commit);
    }
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root())
        .output()
        .map_err(|error| format!("failed to resolve tested commit: {error}"))?;
    if !output.status.success() {
        return Err(format!("git rev-parse HEAD exited with {}", output.status));
    }
    let commit = String::from_utf8(output.stdout)
        .map_err(|error| format!("tested commit is not UTF-8: {error}"))?
        .trim()
        .to_owned();
    require_digest(&commit, 40, "tested commit")?;
    Ok(commit)
}

/// An aggregate may only be recorded for a revision the current branch
/// contains; a run of a since-rewritten or foreign branch is not evidence for
/// `main`.
fn require_ancestor_of_head(root: &Path, commit: &str) -> Result<(), String> {
    let status = Command::new("git")
        .args(["merge-base", "--is-ancestor", commit, "HEAD"])
        .current_dir(root)
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to check aggregate commit ancestry: {error}"))?;
    if !status.success() {
        return Err(format!(
            "aggregate commit {commit} is not an ancestor of HEAD"
        ));
    }
    Ok(())
}

pub(crate) fn aggregate(input: &str, output: &str) -> Result<(), String> {
    let mut paths = fs::read_dir(input)
        .map_err(|error| format!("failed to read target report directory {input}: {error}"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to enumerate target reports: {error}"))?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    paths.sort();
    if paths.len() != TARGETS.len() {
        return Err(format!(
            "expected {} target reports, got {}",
            TARGETS.len(),
            paths.len()
        ));
    }
    let mut reports = paths
        .iter()
        .map(|path| read_json(path))
        .collect::<Result<Vec<_>, _>>()?;
    reports.sort_by_key(|report| {
        required_string(report, "target")
            .unwrap_or_default()
            .to_owned()
    });
    verify_report_set(&reports)?;
    let measurement = required_path(&reports[0], &["measurement"])?.clone();
    verify_cross_target_measurement(&repo_root(), &measurement)?;
    let aggregate = json!({
        "schema": AGGREGATE_SCHEMA,
        "commit": required_string(&reports[0], "commit")?,
        "compatibility_revision": 1,
        "corpus_sha256": required_string(&reports[0], "corpus_sha256")?,
        "subset_profile_sha256": required_string(&reports[0], "subset_profile_sha256")?,
        "viewer_approval_subject": required_string(&reports[0], "viewer_approval_subject")?,
        "measurement": measurement,
        "targets": reports,
        "decision": TargetEvidenceState::Aggregated.decision(),
    });
    write_json(Path::new(output), &aggregate)
}

/// Every report in the set is valid, the set names every supported target
/// exactly once in canonical order, and all five agree on the fields the
/// aggregate carries.
fn verify_report_set(reports: &[Value]) -> Result<(), String> {
    for report in reports {
        validate_report(report)?;
    }
    let actual_targets = reports
        .iter()
        .map(|report| required_string(report, "target"))
        .collect::<Result<Vec<_>, _>>()?;
    if actual_targets != TARGETS {
        return Err(format!(
            "target report set is incomplete: {actual_targets:?}"
        ));
    }
    for path in AGREED_FIELDS {
        let first = required_path(&reports[0], path)?;
        if reports.iter().skip(1).any(|report| {
            required_path(report, path).is_err() || required_path(report, path).ok() != Some(first)
        }) {
            return Err(format!("target reports disagree at {}", path.join(".")));
        }
    }
    Ok(())
}

/// A CI-produced aggregate is internally consistent: identity, a passing
/// decision, five agreeing target reports, and top-level fields that restate
/// what the reports agreed on.
fn validate_aggregate(aggregate: &Value) -> Result<(), String> {
    if required_string(aggregate, "schema")? != AGGREGATE_SCHEMA
        || required_path(aggregate, &["compatibility_revision"]) != Ok(&json!(1))
        || required_string(aggregate, "decision")? != TargetEvidenceState::Aggregated.decision()
    {
        return Err("aggregate identity or decision is invalid".to_owned());
    }
    let reports = required_path(aggregate, &["targets"])?
        .as_array()
        .ok_or_else(|| "aggregate targets are not an array".to_owned())?;
    verify_report_set(reports)?;
    for path in AGREED_FIELDS {
        if required_path(aggregate, path)? != required_path(&reports[0], path)? {
            return Err(format!(
                "aggregate disagrees with its target reports at {}",
                path.join(".")
            ));
        }
    }
    Ok(())
}

/// `<prefix><run id>` or `<prefix><run id>/attempts/<attempt>`: the URL GitHub
/// shows for one workflow run, and nothing that merely starts with it.
fn valid_run_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix(RUN_URL_PREFIX) else {
        return false;
    };
    let digits = |value: &str| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    match rest.split_once("/attempts/") {
        None => digits(rest),
        Some((run, attempt)) => digits(run) && digits(attempt),
    }
}

/// The release evidence bundle together with every committed file whose
/// content the manifest binds: the viewer qualification (approval subject), the
/// font subset profile (semantic digest), the committed cross-target
/// measurement, and the recorded five-target aggregate once CI has produced
/// one. Digests of pinned files are the I/O layer's business
/// (`verify_descriptors`); this type owns every content rule.
struct ReleaseEvidence {
    manifest: Value,
    viewer: Value,
    profile: Value,
    cross_target: Value,
    aggregate: Option<Value>,
}

impl ReleaseEvidence {
    fn load(root: &Path) -> Result<Self, String> {
        let manifest = read_json(&root.join(RELEASE_PATH))?;
        let viewer = repository_json(
            root,
            required_path_string(&manifest, &["viewerEvidence", "path"])?,
        )?;
        let profile = repository_json(
            root,
            required_path_string(&manifest, &["subsetProfile", "path"])?,
        )?;
        let cross_target = read_json(&root.join(CROSS_TARGET_PATH))?;
        let aggregate = match manifest.pointer("/targetEvidence/aggregate") {
            Some(recorded) => Some(repository_json(root, required_string(recorded, "path")?)?),
            None if root.join(AGGREGATE_PATH).exists() => {
                return Err(format!(
                    "an aggregate copy exists at {AGGREGATE_PATH} but the release evidence does not record it"
                ));
            }
            None => None,
        };
        Ok(Self {
            manifest,
            viewer,
            profile,
            cross_target,
            aggregate,
        })
    }

    /// Verifies every content rule and returns the target-evidence state:
    /// pending until CI has produced the five-target aggregate, or aggregated
    /// once the aggregate is recorded and still bound to the current corpus,
    /// profile, viewer approval, and committed measurement.
    fn verify(&self) -> Result<TargetEvidenceState, String> {
        let manifest = &self.manifest;
        if required_string(manifest, "id")? != RELEASE_ID
            || required_path(manifest, &["compatibilityRevision"]) != Ok(&json!(1))
            || required_path(manifest, &["corpus", "successfulCells"]) != Ok(&json!(971))
            || required_path(manifest, &["licensing", "qualifiedSources"]) != Ok(&json!(230))
            || required_path(manifest, &["viewerEvidence", "executions"]) != Ok(&json!(851))
            || required_path_string(manifest, &["viewerEvidence", "status"])? != "approved"
        {
            return Err("SVG font subset release evidence state is stale".to_owned());
        }
        if required_path_string(manifest, &["viewerEvidence", "approvalSubject"])?
            != required_path_string(&self.viewer, &["approval", "approval_subject_sha256"])?
        {
            return Err("release evidence and viewer approval subjects disagree".to_owned());
        }
        let status = required_path_string(manifest, &["targetEvidence", "status"])?;
        let decision = required_string(manifest, "decision")?;
        let state = TargetEvidenceState::declared(status, decision).ok_or_else(|| {
            format!(
                "SVG font subset release evidence state is inconsistent: \
                 target evidence {status:?} with decision {decision:?}"
            )
        })?;
        let recorded = required_path(manifest, &["targetEvidence"])?.get("aggregate");
        match (state, recorded, self.aggregate.as_ref()) {
            (TargetEvidenceState::PendingCi, None, None) => {}
            (TargetEvidenceState::Aggregated, Some(recorded), Some(aggregate)) => {
                self.verify_recorded_aggregate(recorded, aggregate)?;
            }
            _ => {
                return Err(format!(
                    "SVG font subset release evidence state is inconsistent: \
                     target evidence {status:?} with a recorded aggregate {}",
                    if recorded.is_some() {
                        "present"
                    } else {
                        "absent"
                    }
                ));
            }
        }
        Ok(state)
    }

    fn verify_recorded_aggregate(&self, recorded: &Value, aggregate: &Value) -> Result<(), String> {
        if required_string(recorded, "path")? != AGGREGATE_PATH {
            return Err(format!("recorded aggregate must live at {AGGREGATE_PATH}"));
        }
        require_digest(
            required_string(recorded, "sha256")?,
            64,
            "recorded aggregate digest",
        )?;
        if !valid_run_url(required_string(recorded, "runUrl")?) {
            return Err(format!(
                "recorded aggregate run URL is not a {RUN_URL_PREFIX}<run> URL"
            ));
        }
        validate_aggregate(aggregate)?;
        if required_string(recorded, "commit")? != required_string(aggregate, "commit")? {
            return Err("recorded aggregate commit disagrees with the aggregate".to_owned());
        }
        if required_string(aggregate, "corpus_sha256")?
            != required_path_string(&self.manifest, &["corpus", "sha256"])?
        {
            return Err(
                "recorded aggregate was produced for a different qualification corpus".to_owned(),
            );
        }
        if required_string(aggregate, "subset_profile_sha256")? != profile_digest(&self.profile)? {
            return Err(
                "recorded aggregate was produced for a different font subset profile".to_owned(),
            );
        }
        if required_string(aggregate, "viewer_approval_subject")?
            != required_path_string(&self.manifest, &["viewerEvidence", "approvalSubject"])?
        {
            return Err(
                "recorded aggregate was produced for a different viewer approval".to_owned(),
            );
        }
        verify_cross_target_identity(&self.cross_target)?;
        if required_path(&self.cross_target, &["measurement"])?
            != required_path(aggregate, &["measurement"])?
        {
            return Err(
                "recorded aggregate disagrees with the committed cross-target measurement"
                    .to_owned(),
            );
        }
        Ok(())
    }

    /// Records a CI-produced aggregate: the target evidence moves from pending
    /// to aggregated and the release decision to `pass`. The result is
    /// guaranteed to verify against this evidence.
    fn bind_aggregate(
        self,
        aggregate: Value,
        artifact_sha256: &str,
        run_url: &str,
    ) -> Result<Self, String> {
        if self.verify()? != TargetEvidenceState::PendingCi {
            return Err(
                "an aggregate is already recorded; the release evidence is not pending".to_owned(),
            );
        }
        let commit = required_string(&aggregate, "commit")?.to_owned();
        let mut manifest = self.manifest;
        manifest["decision"] = json!(TargetEvidenceState::Aggregated.decision());
        let target_evidence = manifest
            .get_mut("targetEvidence")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "targetEvidence is not an object".to_owned())?;
        target_evidence.insert(
            "status".to_owned(),
            json!(TargetEvidenceState::Aggregated.status()),
        );
        target_evidence.insert(
            "aggregate".to_owned(),
            json!({
                "commit": commit,
                "path": AGGREGATE_PATH,
                "runUrl": run_url,
                "sha256": artifact_sha256,
            }),
        );
        let bound = Self {
            manifest,
            aggregate: Some(aggregate),
            ..self
        };
        bound.verify()?;
        Ok(bound)
    }
}

fn profile_digest(profile: &Value) -> Result<&str, String> {
    required_string(profile, "semanticDigest")?
        .strip_prefix("sha256:")
        .ok_or_else(|| "subset profile semantic digest lacks sha256 prefix".to_owned())
}

/// Verifies the release evidence bundle and returns the target-evidence state
/// it verified.
pub(crate) fn verify_release() -> Result<TargetEvidenceState, String> {
    verify_release_in(&repo_root())
}

fn verify_release_in(root: &Path) -> Result<TargetEvidenceState, String> {
    let evidence = ReleaseEvidence::load(root)?;
    let state = evidence.verify()?;
    verify_descriptors(root, &evidence.manifest)?;
    Ok(state)
}

/// Records the five-target aggregate that the packaged-public-facade workflow
/// uploaded as `svg-font-subset-target-aggregate`: the downloaded artifact is
/// copied verbatim to `AGGREGATE_PATH` and the release evidence moves to
/// aggregated with decision `pass`. Both writes are reviewable diffs; nothing
/// is written unless the pending bundle verifies today and the bound bundle
/// verifies afterwards. The aggregate's commit must be reachable from `HEAD`;
/// beyond that it is provenance, because the content bindings — corpus,
/// profile, viewer approval, measurement — are what keep the record honest
/// across later commits.
pub(crate) fn record_aggregate(aggregate_path: &str, run_url: &str) -> Result<(), String> {
    let root = repo_root();
    let bytes = fs::read(aggregate_path)
        .map_err(|error| format!("failed to read aggregate {aggregate_path}: {error}"))?;
    let aggregate: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse aggregate {aggregate_path}: {error}"))?;
    require_ancestor_of_head(&root, required_string(&aggregate, "commit")?)?;
    record_aggregate_in(&root, &bytes, aggregate, run_url)
}

fn record_aggregate_in(
    root: &Path,
    bytes: &[u8],
    aggregate: Value,
    run_url: &str,
) -> Result<(), String> {
    let evidence = ReleaseEvidence::load(root)?;
    verify_descriptors(root, &evidence.manifest)?;
    let bound = evidence.bind_aggregate(aggregate, &sha256(bytes), run_url)?;
    write_bytes(&root.join(AGGREGATE_PATH), bytes)?;
    write_json(&root.join(RELEASE_PATH), &bound.manifest)
}

/// Writes the committed cross-target measurement from one target's
/// qualification log. Any supported target may produce it: the measurement is
/// only evidence because every target agrees on it.
pub(crate) fn materialize_cross_target(test_log: &str) -> Result<(), String> {
    let measurement = read_measurement(test_log)?;
    write_cross_target(&repo_root().join(CROSS_TARGET_PATH), &measurement)
}

/// Compares the measurement all five targets agreed on against the committed
/// evidence. On disagreement the committed file is rewritten in place before the
/// error is returned, so refreshing it is a mechanical step — re-run, read the
/// diff, commit it — rather than a hand-transcribed digest.
fn verify_cross_target_measurement(root: &Path, measurement: &Value) -> Result<(), String> {
    let path = root.join(CROSS_TARGET_PATH);
    let committed = read_json(&path)?;
    verify_cross_target_identity(&committed)?;
    if required_path(&committed, &["measurement"]) != Ok(measurement) {
        write_cross_target(&path, measurement)?;
        return Err(format!(
            "aggregated qualification measurement disagreed with {CROSS_TARGET_PATH}; \
             it has been rewritten — review the diff and commit it"
        ));
    }
    Ok(())
}

fn verify_cross_target_identity(committed: &Value) -> Result<(), String> {
    if required_string(committed, "schema")? != CROSS_TARGET_SCHEMA
        || required_path(committed, &["compatibilityRevision"]) != Ok(&json!(1))
    {
        return Err("committed cross-target measurement identity is invalid".to_owned());
    }
    validate_measurement(required_path(committed, &["measurement"])?)
}

fn write_cross_target(path: &Path, measurement: &Value) -> Result<(), String> {
    write_json(
        path,
        &json!({
            "schema": CROSS_TARGET_SCHEMA,
            "compatibilityRevision": 1,
            "measurement": measurement,
        }),
    )
}

fn validate_report(report: &Value) -> Result<(), String> {
    if required_string(report, "schema")? != REPORT_SCHEMA
        || required_path(report, &["compatibility_revision"]) != Ok(&json!(1))
        || required_string(report, "status")? != "pass"
    {
        return Err("target report identity or status is invalid".to_owned());
    }
    let target = required_string(report, "target")?;
    if !TARGETS.contains(&target) {
        return Err(format!("unsupported target report {target:?}"));
    }
    require_digest(required_string(report, "commit")?, 40, "commit")?;
    require_digest(
        required_string(report, "corpus_sha256")?,
        64,
        "corpus digest",
    )?;
    require_digest(
        required_string(report, "subset_profile_sha256")?,
        64,
        "profile digest",
    )?;
    let viewer = required_string(report, "viewer_approval_subject")?;
    require_digest(
        viewer.strip_prefix("sha256:").unwrap_or_default(),
        64,
        "viewer approval subject",
    )?;
    validate_measurement(required_path(report, &["measurement"])?)
}

fn validate_measurement(measurement: &Value) -> Result<(), String> {
    let object = measurement
        .as_object()
        .ok_or_else(|| "qualification measurement is not an object".to_owned())?;
    let expected = [
        "generatedBytesSha256",
        "resourceTracesSha256",
        "successfulCells",
        "tracedCells",
    ];
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("qualification measurement fields are incomplete".to_owned());
    }
    require_digest(
        required_string(measurement, "generatedBytesSha256")?,
        64,
        "generated-byte measurement",
    )?;
    require_digest(
        required_string(measurement, "resourceTracesSha256")?,
        64,
        "resource-trace measurement",
    )?;
    if required_path(measurement, &["successfulCells"]) != Ok(&json!(971))
        || required_path(measurement, &["tracedCells"]) != Ok(&json!(1198))
    {
        return Err("qualification measurement cell counts are stale".to_owned());
    }
    Ok(())
}

fn verify_descriptors(root: &Path, value: &Value) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            if let (Some(Value::String(path)), Some(Value::String(expected))) =
                (object.get("path"), object.get("sha256"))
            {
                if sha256(&read_repository_file(root, path)?) != *expected {
                    return Err(format!("stale SVG font subset release descriptor: {path}"));
                }
            }
            for child in object.values() {
                verify_descriptors(root, child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                verify_descriptors(root, child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    required_path_string(value, &[key])
}

fn required_path_string<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    required_path(value, path)?
        .as_str()
        .ok_or_else(|| format!("{} is not a string", path.join(".")))
}

fn required_path<'a>(mut value: &'a Value, path: &[&str]) -> Result<&'a Value, String> {
    for key in path {
        value = value
            .get(*key)
            .ok_or_else(|| format!("missing {}", path.join(".")))?;
    }
    Ok(value)
}

fn require_digest(value: &str, length: usize, name: &str) -> Result<(), String> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{name} is not lowercase hexadecimal"));
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn repository_json(root: &Path, relative: &str) -> Result<Value, String> {
    serde_json::from_slice(&read_repository_file(root, relative)?)
        .map_err(|error| format!("failed to parse {relative}: {error}"))
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    bytes.push(b'\n');
    write_bytes(path, &bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{
        record_aggregate_in, valid_run_url, verify_release_in, ReleaseEvidence,
        TargetEvidenceState, AGGREGATE_PATH, AGGREGATE_SCHEMA, CROSS_TARGET_PATH,
        CROSS_TARGET_SCHEMA, RELEASE_PATH, REPORT_SCHEMA, RUN_URL_PREFIX, TARGETS,
    };
    use crate::conformance::{repo_root, sha256};
    use serde_json::{json, Value};
    use std::fs;
    use std::path::{Path, PathBuf};

    const CORPUS: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const PROFILE: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const VIEWER: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
    const GENERATED: &str = "4444444444444444444444444444444444444444444444444444444444444444";
    const TRACES: &str = "5555555555555555555555555555555555555555555555555555555555555555";
    const COMMIT: &str = "6666666666666666666666666666666666666666";
    const ARTIFACT: &str = "7777777777777777777777777777777777777777777777777777777777777777";
    const RUN_URL: &str =
        "https://github.com/sagikazarmark/excalidraw-render.orig/actions/runs/123456789";

    fn measurement() -> Value {
        json!({
            "generatedBytesSha256": GENERATED,
            "resourceTracesSha256": TRACES,
            "successfulCells": 971,
            "tracedCells": 1198,
        })
    }

    /// The committed manifest's shape today, reduced to the fields the state
    /// machine reads.
    fn pending_manifest() -> Value {
        json!({
            "approval": null,
            "compatibilityRevision": 1,
            "corpus": {"path": "corpus.json", "sha256": CORPUS, "successfulCells": 971},
            "decision": "pending_target_aggregation",
            "id": "svg-font-subsets.release-evidence.revision-1.v1",
            "licensing": {"qualifiedSources": 230},
            "subsetProfile": {"path": "profile.json", "sha256": PROFILE},
            "targetEvidence": {
                "aggregateArtifact": "svg-font-subset-target-aggregate",
                "status": "pending_ci",
            },
            "viewerEvidence": {
                "approvalSubject": VIEWER,
                "executions": 851,
                "path": "viewer.json",
                "status": "approved",
            },
        })
    }

    /// The agreed shape once the aggregate is recorded: the pending manifest
    /// with the state pair flipped and the binding added, nothing else.
    fn aggregated_manifest() -> Value {
        let mut manifest = pending_manifest();
        manifest["decision"] = json!("pass");
        manifest["targetEvidence"]["status"] = json!("aggregated");
        manifest["targetEvidence"]["aggregate"] = json!({
            "commit": COMMIT,
            "path": AGGREGATE_PATH,
            "runUrl": RUN_URL,
            "sha256": ARTIFACT,
        });
        manifest
    }

    fn report(target: &str) -> Value {
        json!({
            "schema": REPORT_SCHEMA,
            "commit": COMMIT,
            "compatibility_revision": 1,
            "target": target,
            "corpus_sha256": CORPUS,
            "subset_profile_sha256": PROFILE,
            "viewer_approval_subject": VIEWER,
            "measurement": measurement(),
            "status": "pass",
        })
    }

    /// What `aggregate-svg-font-subset-targets` uploads when all five targets
    /// agree.
    fn aggregate() -> Value {
        json!({
            "schema": AGGREGATE_SCHEMA,
            "commit": COMMIT,
            "compatibility_revision": 1,
            "corpus_sha256": CORPUS,
            "subset_profile_sha256": PROFILE,
            "viewer_approval_subject": VIEWER,
            "measurement": measurement(),
            "targets": TARGETS.iter().map(|target| report(target)).collect::<Vec<_>>(),
            "decision": "pass",
        })
    }

    fn evidence(manifest: Value, aggregate: Option<Value>) -> ReleaseEvidence {
        ReleaseEvidence {
            manifest,
            viewer: json!({"approval": {"approval_subject_sha256": VIEWER}}),
            profile: json!({"semanticDigest": format!("sha256:{PROFILE}")}),
            cross_target: json!({
                "schema": CROSS_TARGET_SCHEMA,
                "compatibilityRevision": 1,
                "measurement": measurement(),
            }),
            aggregate,
        }
    }

    fn bind(manifest: Value, aggregate: Value, run_url: &str) -> Result<ReleaseEvidence, String> {
        evidence(manifest, None).bind_aggregate(aggregate, ARTIFACT, run_url)
    }

    /// The rejection must come from the rule under test, not from an earlier
    /// check tripping over the same mutation.
    fn assert_rejected<T>(result: Result<T, String>, rule: &str) {
        let Err(error) = result else {
            panic!("stale or foreign evidence must fail closed (expected {rule:?})");
        };
        assert!(
            error.contains(rule),
            "expected a rejection mentioning {rule:?}, got {error:?}"
        );
    }

    #[test]
    fn release_state_pending_verifies_without_a_recorded_aggregate() {
        assert_eq!(
            evidence(pending_manifest(), None).verify(),
            Ok(TargetEvidenceState::PendingCi)
        );
    }

    #[test]
    fn release_state_aggregated_verifies_a_consistent_recorded_aggregate() {
        assert_eq!(
            evidence(aggregated_manifest(), Some(aggregate())).verify(),
            Ok(TargetEvidenceState::Aggregated)
        );
    }

    /// The ADR-0003 refresh regenerates the corpus; a recorded aggregate from
    /// before it must stop verifying.
    #[test]
    fn release_state_rejects_a_recorded_aggregate_once_the_corpus_moves() {
        let mut manifest = aggregated_manifest();
        manifest["corpus"]["sha256"] = json!(PROFILE);
        assert_rejected(
            evidence(manifest, Some(aggregate())).verify(),
            "different qualification corpus",
        );
    }

    #[test]
    fn release_state_rejects_a_recorded_aggregate_once_the_committed_measurement_moves() {
        let mut evidence = evidence(aggregated_manifest(), Some(aggregate()));
        evidence.cross_target["measurement"]["resourceTracesSha256"] = json!(GENERATED);
        assert_rejected(evidence.verify(), "committed cross-target measurement");
    }

    #[test]
    fn release_state_rejects_a_status_and_decision_that_disagree() {
        let mut manifest = aggregated_manifest();
        manifest["decision"] = json!("pending_target_aggregation");
        assert_rejected(
            evidence(manifest, Some(aggregate())).verify(),
            "inconsistent",
        );

        let mut manifest = pending_manifest();
        manifest["targetEvidence"]["aggregate"] =
            aggregated_manifest()["targetEvidence"]["aggregate"].clone();
        assert_rejected(
            evidence(manifest, Some(aggregate())).verify(),
            "inconsistent",
        );
    }

    #[test]
    fn binding_an_aggregate_records_it_and_completes_the_release_decision() {
        let bound =
            bind(pending_manifest(), aggregate(), RUN_URL).expect("a matching aggregate binds");
        assert_eq!(bound.manifest, aggregated_manifest());
        assert_eq!(bound.verify(), Ok(TargetEvidenceState::Aggregated));
    }

    #[test]
    fn binding_rejects_an_aggregate_produced_for_a_different_corpus() {
        let mut aggregate = aggregate();
        aggregate["corpus_sha256"] = json!(PROFILE);
        for report in aggregate["targets"].as_array_mut().unwrap() {
            report["corpus_sha256"] = json!(PROFILE);
        }
        assert_rejected(
            bind(pending_manifest(), aggregate, RUN_URL),
            "different qualification corpus",
        );
    }

    #[test]
    fn binding_rejects_an_aggregate_produced_for_a_different_font_subset_profile() {
        let mut aggregate = aggregate();
        aggregate["subset_profile_sha256"] = json!(CORPUS);
        for report in aggregate["targets"].as_array_mut().unwrap() {
            report["subset_profile_sha256"] = json!(CORPUS);
        }
        assert_rejected(
            bind(pending_manifest(), aggregate, RUN_URL),
            "different font subset profile",
        );
    }

    #[test]
    fn binding_rejects_an_aggregate_produced_for_a_different_viewer_approval() {
        let other = format!("sha256:{CORPUS}");
        let mut aggregate = aggregate();
        aggregate["viewer_approval_subject"] = json!(other);
        for report in aggregate["targets"].as_array_mut().unwrap() {
            report["viewer_approval_subject"] = json!(other);
        }
        assert_rejected(
            bind(pending_manifest(), aggregate, RUN_URL),
            "different viewer approval",
        );
    }

    #[test]
    fn binding_rejects_an_aggregate_whose_reports_do_not_back_its_summary() {
        let mut aggregate = aggregate();
        aggregate["targets"][2]["commit"] = json!("fedcba9876543210fedcba9876543210fedcba98");
        assert_rejected(
            bind(pending_manifest(), aggregate, RUN_URL),
            "target reports disagree at commit",
        );
    }

    #[test]
    fn binding_rejects_an_aggregate_that_did_not_pass() {
        let mut aggregate = aggregate();
        aggregate["decision"] = json!("fail");
        assert_rejected(
            bind(pending_manifest(), aggregate, RUN_URL),
            "identity or decision",
        );
    }

    #[test]
    fn binding_rejects_a_run_outside_this_repository_workflow() {
        assert_rejected(
            bind(
                pending_manifest(),
                aggregate(),
                "https://github.com/someone-else/fork/actions/runs/1",
            ),
            "run URL",
        );
    }

    #[test]
    fn binding_rejects_a_second_aggregate_once_one_is_recorded() {
        assert_rejected(
            evidence(aggregated_manifest(), Some(aggregate())).bind_aggregate(
                aggregate(),
                ARTIFACT,
                RUN_URL,
            ),
            "already recorded",
        );
    }

    /// The URL GitHub shows for a run of this repository's workflow, with or
    /// without an attempt; nothing that merely shares the prefix.
    #[test]
    fn run_urls_are_exactly_one_workflow_run_of_this_repository() {
        assert!(valid_run_url(&format!("{RUN_URL_PREFIX}123456789")));
        assert!(valid_run_url(&format!(
            "{RUN_URL_PREFIX}123456789/attempts/2"
        )));
        for rejected in [
            RUN_URL_PREFIX.to_owned(),
            format!("{RUN_URL_PREFIX}abc"),
            format!("{RUN_URL_PREFIX}1/../../../other/repo/actions/runs/1"),
            format!("{RUN_URL_PREFIX}1?check_suite_focus=true"),
            format!("{RUN_URL_PREFIX}1/attempts/"),
            "https://github.com/someone-else/fork/actions/runs/1".to_owned(),
        ] {
            assert!(!valid_run_url(&rejected), "{rejected} must be rejected");
        }
    }

    /// A scratch repository holding the committed release evidence and every
    /// file it pins, with the pins re-derived from the copies so the test does
    /// not depend on the working tree's digests being fresh.
    fn scratch_repository(name: &str) -> PathBuf {
        let source = repo_root();
        let root = std::env::temp_dir().join(format!(
            "excalidraw-render-xtask-scratch-{}-{name}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove stale scratch repository");
        }
        let copy = |relative: &str| {
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).expect("create scratch directory");
            fs::copy(source.join(relative), &destination).expect("copy pinned evidence file");
        };
        copy(RELEASE_PATH);
        copy(CROSS_TARGET_PATH);
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(root.join(RELEASE_PATH)).unwrap()).unwrap();
        fn refresh(value: &mut Value, root: &Path, copy: &dyn Fn(&str)) {
            match value {
                Value::Object(object) => {
                    if let Some(path) = object
                        .get("path")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                    {
                        if object.contains_key("sha256") {
                            copy(&path);
                            let digest = sha256(&fs::read(root.join(&path)).unwrap());
                            object.insert("sha256".to_owned(), json!(digest));
                        }
                    }
                    for child in object.values_mut() {
                        refresh(child, root, copy);
                    }
                }
                Value::Array(values) => {
                    for child in values {
                        refresh(child, root, copy);
                    }
                }
                _ => {}
            }
        }
        refresh(&mut manifest, &root, &copy);
        super::write_json(&root.join(RELEASE_PATH), &manifest).unwrap();
        root
    }

    /// An aggregate the scratch repository's evidence would accept: real
    /// corpus, profile, viewer, and measurement bindings, synthetic commit.
    fn scratch_aggregate(root: &Path) -> Vec<u8> {
        let evidence = ReleaseEvidence::load(root).expect("scratch evidence loads");
        let mut aggregate = aggregate();
        let corpus = evidence.manifest["corpus"]["sha256"].clone();
        let profile = json!(super::profile_digest(&evidence.profile).unwrap());
        let viewer = evidence.manifest["viewerEvidence"]["approvalSubject"].clone();
        let measurement = evidence.cross_target["measurement"].clone();
        let rebind = |value: &mut Value| {
            value["corpus_sha256"] = corpus.clone();
            value["subset_profile_sha256"] = profile.clone();
            value["viewer_approval_subject"] = viewer.clone();
            value["measurement"] = measurement.clone();
        };
        for report in aggregate["targets"].as_array_mut().unwrap() {
            rebind(report);
        }
        rebind(&mut aggregate);
        serde_json::to_vec_pretty(&aggregate).unwrap()
    }

    #[test]
    fn recording_an_aggregate_moves_the_committed_evidence_to_aggregated() {
        let root = scratch_repository("record");
        let bytes = scratch_aggregate(&root);
        let aggregate: Value = serde_json::from_slice(&bytes).unwrap();

        record_aggregate_in(&root, &bytes, aggregate, RUN_URL).expect("aggregate records");

        assert_eq!(
            verify_release_in(&root),
            Ok(TargetEvidenceState::Aggregated)
        );
        assert_eq!(
            fs::read(root.join(AGGREGATE_PATH)).unwrap(),
            bytes,
            "copied verbatim"
        );
        let manifest: Value =
            serde_json::from_slice(&fs::read(root.join(RELEASE_PATH)).unwrap()).unwrap();
        assert_eq!(
            manifest["targetEvidence"]["aggregate"]["sha256"],
            json!(sha256(&bytes))
        );
        assert_eq!(
            manifest["targetEvidence"]["aggregate"]["commit"],
            json!(COMMIT)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_swapped_aggregate_copy_fails_the_recorded_evidence_closed() {
        let root = scratch_repository("swap");
        let bytes = scratch_aggregate(&root);
        let aggregate: Value = serde_json::from_slice(&bytes).unwrap();
        record_aggregate_in(&root, &bytes, aggregate, RUN_URL).expect("aggregate records");

        let mut swapped = bytes.clone();
        swapped.push(b'\n');
        fs::write(root.join(AGGREGATE_PATH), swapped).unwrap();

        assert_rejected(
            verify_release_in(&root),
            "stale SVG font subset release descriptor",
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_aggregate_copy_the_evidence_does_not_record_fails_closed() {
        let root = scratch_repository("orphan");
        let bytes = scratch_aggregate(&root);
        super::write_bytes(&root.join(AGGREGATE_PATH), &bytes).unwrap();

        assert_rejected(verify_release_in(&root), "does not record it");
        fs::remove_dir_all(root).unwrap();
    }
}
