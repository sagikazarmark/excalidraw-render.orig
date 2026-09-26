//! Explicit, one-layout migration, not an alternate execution-key contract.
//! Authenticates the previous inventory from Git, proves that only physical font
//! locations moved, then requires exact fresh-process public-facade equality.
use super::*;
use std::path::Path;
use std::process::Command;

const OLD_CATALOG: &str = "crates/excalidraw-render-assets-base/catalog/revision-1.json";
const NEW_CATALOG: &str = "crates/excalidraw-fonts/catalog/revision-1.json";
const BASELINE: &str = "tests/conformance/coverage/native-baseline/revision-1.json";
const SIDECAR: &str = "tests/conformance/coverage/native-baseline/revision-1.sha256";
pub(super) const RECEIPT: &str = "tests/conformance/coverage/font-packaging-reconciliation.json";

#[derive(Debug, Deserialize, Serialize)]
enum ReceiptSchema {
    #[serde(rename = "excalidraw-render/font-packaging-reconciliation/v1")]
    V1,
}

/// Historical migration receipt, read only by explicit migration commands.
/// Audit/replay details are retained verbatim; they are not a live baseline gate.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Receipt {
    schema: ReceiptSchema,
    base_commit: String,
    old_catalog: Descriptor,
    new_catalog: Descriptor,
    audit: Value,
    old_inventory_sha256: String,
    new_inventory_sha256: String,
    baseline_before_sha256: String,
    baseline_after_sha256: String,
    pdf_judgments_before_sha256: String,
    pdf_judgments_after_sha256: String,
    replay: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) ownership: Option<OwnershipReceipt>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnershipReceipt {
    pub(super) original_ledger_sha256: String,
    pub(super) ledger_after_sha256: String,
    pub(super) original_spec_sha256: String,
    pub(super) spec_sha256: String,
    pub(super) criteria_candidates_selectors_dispositions_unchanged: bool,
    pub(super) pdf_record_digest_rebound: bool,
}

impl Receipt {
    pub(super) fn load() -> Result<(Vec<u8>, Self), String> {
        let bytes = read(RECEIPT)?;
        let receipt = parse(&bytes)?;
        Ok((bytes, receipt))
    }

    pub(super) fn bytes(&self) -> Result<Vec<u8>, String> {
        let value = serde_json::to_value(self).map_err(|e| e.to_string())?;
        Ok([&canonical_bytes(&value)?[..], b"\n"].concat())
    }
}

pub(super) fn previous(base: &str, path: &str) -> Result<Vec<u8>, String> {
    if !hex(base, 40) {
        return Err("font packaging migration requires a full previous commit hash".into());
    }
    let output = Command::new("git")
        .current_dir(repo_root())
        .args(["show", &format!("{base}:{path}")])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(output.stdout)
}

fn storage_only(old: &Value, new: &Value) -> Result<usize, String> {
    let mut normalized = new.clone();
    let assets = normalized["assets"]
        .as_array_mut()
        .ok_or("missing assets")?;
    let old_assets = array(old, "assets")?;
    if assets.len() != old_assets.len() || assets.is_empty() {
        return Err("font packaging changed asset count".into());
    }
    let mut moved = 0;
    for (asset, before) in assets.iter_mut().zip(old_assets) {
        let segments = asset["segments"].as_array_mut().ok_or("missing segments")?;
        let previous = array(before, "segments")?;
        if segments.len() != previous.len() {
            return Err("font packaging changed segment count".into());
        }
        for (segment, old) in segments.iter_mut().zip(previous) {
            if segment["offset"].as_u64().is_none() || !segment["shard"].is_string() {
                return Err("invalid physical segment location".into());
            }
            if segment["offset"] != old["offset"] || segment["shard"] != old["shard"] {
                moved += 1;
            }
            segment["offset"] = old["offset"].clone();
            segment["shard"] = old["shard"].clone();
        }
    }
    if &normalized != old || moved == 0 {
        return Err("font packaging changed catalog semantics beyond offsets/shards".into());
    }
    Ok(moved)
}

fn audit(base: &str) -> Result<(Inventory, Inventory, usize), String> {
    let old_bytes = previous(base, OLD_CATALOG)?;
    let moved = storage_only(&parse(&old_bytes)?, &parse(&read(NEW_CATALOG)?)?)?;
    let old: Inventory = parse(&previous(base, INVENTORY)?)?;
    let reconstructed = derive_inventory_with_catalog(
        parse(&read(REQUIREMENTS)?)?,
        descriptor(REQUIREMENTS)?,
        Descriptor {
            path: OLD_CATALOG.into(),
            sha256: sha256(&old_bytes),
        },
    )?;
    if reconstructed != old {
        return Err("font packaging changed non-catalog execution inputs or coverage".into());
    }
    Ok((old, load_inventory()?, moved))
}

fn rekey(record: &mut Value, field: &str, old: &Inventory, new: &Inventory) -> Result<(), String> {
    for row in record[field]
        .as_array_mut()
        .ok_or("missing execution rows")?
    {
        let key: CaseKey = serde_json::from_value(row["key"].clone()).map_err(|e| e.to_string())?;
        let index = old
            .executions
            .iter()
            .position(|e| e.key == key)
            .ok_or("record contains a non-previous execution key")?;
        row["key"] = serde_json::to_value(&new.executions[index].key).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn rebind_font_packaging(base: &str, candidate: &str) -> Result<(), String> {
    let (old, current, moved) = audit(base)?;
    crate::catalog::verify()?;
    let before_baseline = read(BASELINE)?;
    let before_sidecar = read(SIDECAR)?;
    let before_pdf = read(pdf_judgments::RECORD)?;
    let inventory_bytes = read(INVENTORY)?;
    if before_baseline != previous(base, BASELINE)?
        || before_pdf != previous(base, pdf_judgments::RECORD)?
    {
        return Err("font packaging requires unchanged accepted baseline and PDF judgments".into());
    }
    // Independently rederive all old reports before moving any keys.
    let mut pdf = pdf_judgments::verify_record(&pdf_judgments::Repo::real(), &old)?;
    rekey(&mut pdf, "executions", &old, &current)?;
    let pdf_bytes = [&canonical_bytes(&pdf)?[..], b"\n"].concat();
    let mut staged = pdf_judgments::Repo::real();
    staged
        .overrides
        .insert(pdf_judgments::RECORD.into(), Some(pdf_bytes.clone()));
    pdf_judgments::verify_record(&staged, &current)?;
    let (baseline, sidecar, replay) =
        snapshots::storage_rebind(&old, &current, Path::new(candidate))?;
    let catalog: Value = parse(&read(NEW_CATALOG)?)?;
    let receipt = Receipt {
        schema: ReceiptSchema::V1,
        base_commit: base.into(),
        old_catalog: Descriptor {
            path: OLD_CATALOG.into(),
            sha256: sha256(&previous(base, OLD_CATALOG)?),
        },
        new_catalog: descriptor(NEW_CATALOG)?,
        audit: json!({"assets": array(&catalog, "assets")?.len(), "moved_segments": moved,
            "only_segment_offsets_and_shards_changed": true,
            "all_non_catalog_inventory_inputs_unchanged": true}),
        old_inventory_sha256: sha256(&previous(base, INVENTORY)?),
        new_inventory_sha256: sha256(&read(INVENTORY)?),
        baseline_before_sha256: sha256(&before_baseline),
        baseline_after_sha256: sha256(&baseline),
        pdf_judgments_before_sha256: sha256(&before_pdf),
        pdf_judgments_after_sha256: sha256(&pdf_bytes),
        replay,
        ownership: None,
    };
    if repo_root().join(RECEIPT).exists() {
        return Err("font packaging reconciliation already exists".into());
    }
    if read(BASELINE)? != before_baseline
        || read(SIDECAR)? != before_sidecar
        || read(pdf_judgments::RECORD)? != before_pdf
        || read(INVENTORY)? != inventory_bytes
    {
        return Err("evidence changed while preparing packaging migration".into());
    }
    // No artifact blobs, report bytes, tolerances, provenance or approvals change.
    for (path, bytes) in [
        (BASELINE, baseline),
        (SIDECAR, sidecar),
        (pdf_judgments::RECORD, pdf_bytes),
    ] {
        fs::write(repo_root().join(path), bytes).map_err(|e| e.to_string())?;
    }
    fs::write(repo_root().join(RECEIPT), receipt.bytes()?).map_err(|e| e.to_string())?;
    snapshots::replay_snapshots(candidate)?;
    println!(
        "rebound {} executions after exact output equality; {moved} physical segments moved",
        current.executions.len()
    );
    Ok(())
}

pub fn rebind_font_packaging_ownership(base: &str, spec: &str) -> Result<(), String> {
    let (old, current, _) = audit(base)?;
    let (_, receipt) = Receipt::load()?;
    if receipt.base_commit != base
        || receipt.new_inventory_sha256 != sha256(&read(INVENTORY)?)
        || receipt.new_catalog != descriptor(NEW_CATALOG)?
        || receipt.baseline_after_sha256 != sha256(&read(BASELINE)?)
        || receipt.pdf_judgments_after_sha256 != sha256(&read(pdf_judgments::RECORD)?)
    {
        return Err("font packaging receipt does not bind current evidence".into());
    }
    snapshots::verify_baseline()?;
    ownership::storage_rebind(base, spec, old, current)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deliberately self-contained: routine tests neither invoke Git nor read
    // migration receipts/current baselines, which may legitimately evolve.
    fn inventory(input_hash: char) -> Inventory {
        Inventory {
            schema: INVENTORY_SCHEMA.into(),
            compatibility_revision: 1,
            scope: "fixture".into(),
            sources: vec![],
            coverage: vec![],
            requirements: vec![],
            executions: vec![Execution {
                key: CaseKey {
                    case: "fixture".into(),
                    variant: "default".into(),
                    export: ExportTarget::Scene {},
                    format: Format::Pdf,
                    inputs_sha256: input_hash.to_string().repeat(64),
                },
                options: json!({}),
                reference_artifact: None,
            }],
        }
    }

    #[test]
    fn receipt_schema_and_ownership_fields_are_checked_without_live_evidence() {
        let hash = "a".repeat(64);
        let mut value = json!({
            "schema":"excalidraw-render/font-packaging-reconciliation/v1",
            "base_commit":"b".repeat(40),
            "old_catalog":{"path":OLD_CATALOG,"sha256":hash},
            "new_catalog":{"path":NEW_CATALOG,"sha256":hash},
            "audit":{"fixture":"preserved"}, "replay":{"fixture":"preserved"},
            "old_inventory_sha256":hash, "new_inventory_sha256":hash,
            "baseline_before_sha256":hash, "baseline_after_sha256":hash,
            "pdf_judgments_before_sha256":hash, "pdf_judgments_after_sha256":hash
        });
        for owner in [
            None,
            Some(json!({
                "original_ledger_sha256":hash, "ledger_after_sha256":hash,
                "original_spec_sha256":hash, "spec_sha256":hash,
                "criteria_candidates_selectors_dispositions_unchanged":true,
                "pdf_record_digest_rebound":true
            })),
        ] {
            if let Some(owner) = owner {
                value["ownership"] = owner;
            }
            let receipt: Receipt = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(parse::<Value>(&receipt.bytes().unwrap()).unwrap(), value);
        }
        for pointer in [
            "/schema",
            "/base_commit",
            "/ownership/ledger_after_sha256",
            "/ownership/pdf_record_digest_rebound",
        ] {
            let mut malformed = value.clone();
            *malformed.pointer_mut(pointer).unwrap() = json!(42);
            assert!(
                serde_json::from_value::<Receipt>(malformed).is_err(),
                "{pointer}"
            );
        }
        value["schema"] = json!("excalidraw-render/font-packaging-reconciliation/v2");
        assert!(serde_json::from_value::<Receipt>(value).is_err());
    }

    #[test]
    fn rebinding_preserves_sample_judgment_and_provenance_fields() {
        let old = inventory('a');
        let current = inventory('b');
        for rows in ["expectations", "executions"] {
            let mut record = json!({
                "provenance": {"commit":"historical", "inventory":"historical"},
                "approval": null,
                rows: [{"key":old.executions[0].key,
                    "sample":{"outcome":{"sha256":"artifact"},"report":"report"},
                    "expected_report":{"diagnostics":[],"sha256":"report"}}]
            });
            let mut expected = record.clone();
            expected[rows][0]["key"] = serde_json::to_value(&current.executions[0].key).unwrap();
            rekey(&mut record, rows, &old, &current).unwrap();
            assert_eq!(record, expected);
        }
    }
    #[test]
    fn only_physical_locations_can_move() {
        let old = json!({"profile":"fixed", "assets":[{"identity":"font", "completeSha256":"digest",
            "segments":[{"offset":0,"shard":"old","length":12,"sha256":"bytes","order":0}]}]});
        let mut new = old.clone();
        new["assets"][0]["segments"][0]["offset"] = json!(42);
        new["assets"][0]["segments"][0]["shard"] = json!("new");
        assert_eq!(storage_only(&old, &new).unwrap(), 1);
        for pointer in [
            "/profile",
            "/assets/0/identity",
            "/assets/0/completeSha256",
            "/assets/0/segments/0/length",
            "/assets/0/segments/0/sha256",
            "/assets/0/segments/0/order",
        ] {
            let mut changed = new.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("changed");
            assert!(storage_only(&old, &changed).is_err(), "{pointer}");
        }
    }

    #[test]
    fn execution_rebinding_rejects_unknown_keys_and_preserves_samples() {
        let current = inventory('b');
        let mut old = current.clone();
        old.executions[0].key.inputs_sha256 = "0".repeat(64);
        let mut record = json!({"expectations":[{"key": old.executions[0].key,
            "sample":{"outcome":{"sha256":"unchanged"},"report":"unchanged"}}]});
        let sample = record["expectations"][0]["sample"].clone();
        rekey(&mut record, "expectations", &old, &current).unwrap();
        assert_eq!(record["expectations"][0]["sample"], sample);
        assert_eq!(
            record["expectations"][0]["key"],
            serde_json::to_value(&current.executions[0].key).unwrap()
        );
        assert!(rekey(&mut record, "expectations", &old, &current).is_err());
    }
}
