//! Evidence-producing projection and approval preimage. Validators/replay are
//! consumers and remain outside this approval-bearing file.
use super::*;

pub(super) fn subject(baseline: &Baseline) -> Result<String, String> {
    let mut value = serde_json::to_value(baseline).map_err(|e| e.to_string())?;
    value
        .as_object_mut()
        .ok_or("baseline must be an object")?
        .remove("approval");
    Ok(sha256(&json_bytes(
        &json!({"schema":SUBJECT,"baseline":value}),
    )?))
}

pub(super) fn build_baseline(bundle: Snapshots, raw: &[u8]) -> Result<Baseline, String> {
    let implementation = read(IMPLEMENTATION)?;
    Ok(Baseline {
        schema: SCHEMA.into(),
        compatibility_revision: 1,
        target: bundle.target,
        inventory: bundle.inventory,
        implementation: FileRecord {
            path: IMPLEMENTATION.into(),
            sha256: sha256(&implementation),
            byte_length: implementation.len() as u64,
        },
        provenance: Provenance {
            commit: bundle.commit,
            rustc: bundle.rustc,
            candidate_sha256: sha256(raw),
            tool_manifest: bundle.tool_manifest,
            dagger_lock: bundle.dagger_lock,
            consumer_lock: bundle.consumer_lock,
            consumer_manifest: bundle.consumer_manifest,
            consumer_config: bundle.consumer_config,
            collector: bundle.collector,
            packages: bundle.packages,
        },
        expectations: bundle.runs[0]
            .results
            .iter()
            .map(|r| Expectation {
                key: r.key.clone(),
                sample: r.samples[0].clone(),
            })
            .collect(),
        approval: Value::Null,
    })
}
