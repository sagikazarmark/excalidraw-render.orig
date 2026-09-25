use super::*;
use crate::representative::comparison::{specification, ssim, Variant};

pub(super) fn judge(case: &Value, collection: &Path) -> Result<Value> {
    let record = decode(&read(collection, "viewers.json")?)?;
    let variants = strings(case, "variants")?;
    let comparisons = strings(case, "comparisons")?;
    let expected: BTreeSet<_> = variants
        .iter()
        .flat_map(|v| comparisons.iter().map(move |c| (*v, *c)))
        .collect();
    let mut seen = BTreeSet::new();
    // Validate presence independently of cached results. An empty collection
    // cannot be turned into a passing judgment by omitting failing comparisons.
    for judgment in array(&record, "judgments")? {
        let variant = text(judgment, "variant")?;
        let comparison = text(judgment, "comparison")?;
        specification(comparison)?;
        require(variants.contains(&variant), "unknown judgment variant")?;
        require(
            seen.insert((variant, comparison)),
            "duplicate judgment comparison",
        )?;
    }
    require(
        expected.is_subset(&seen),
        "missing registered judgment comparison",
    )?;
    let exceptions = array(case, "exceptions")?;
    let mut used = BTreeSet::new();
    let mut controlled = 0;
    let mut failures = Vec::new();
    for variant_id in variants {
        // Variant names have already been checked against the verified intake.
        let mut variant = Variant::load(&collection.join(variant_id), None)?;
        for &comparison in &comparisons {
            let spec = specification(comparison)?;
            for region in variant.classify(spec)? {
                if region.within {
                    controlled += 1;
                    continue;
                }
                let exception = exceptions.iter().enumerate().find(|(_, e)| {
                    e["variant"] == variant_id
                        && e["comparison_id"] == comparison
                        && e["region"] == region.id
                });
                if let Some((index, e)) = exception {
                    let maximum = e["max_changed_pixels"]
                        .as_u64()
                        .ok_or("invalid exception ceiling")?;
                    if region.changed <= maximum
                        && region.score >= ssim(text(e, "minimum_ssim_bits")?)?
                    {
                        used.insert(index);
                        continue;
                    }
                }
                failures.push(json!({"variant":variant_id,"comparison":comparison,"region":region.id,"changed_pixels":region.changed,"ssim":region.score,"control_changed_pixels":region.control_changed,"control_ssim":region.control_score}));
            }
        }
    }
    require(
        failures.is_empty(),
        &format!("comparisons fail: {}", json!(failures)),
    )?;
    let dead: Vec<_> = exceptions
        .iter()
        .enumerate()
        .filter(|(i, _)| !used.contains(i))
        .map(|(_, e)| json!([e["variant"], e["comparison_id"], e["region"]]))
        .collect();
    require(
        dead.is_empty(),
        &format!("dead exceptions: {}", json!(dead)),
    )?;
    Ok(
        json!({"case_id":case["case_id"],"within_controls":controlled,"exceptions_applied":used.len()}),
    )
}
