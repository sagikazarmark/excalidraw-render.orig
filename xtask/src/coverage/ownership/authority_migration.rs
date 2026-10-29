//! Read-only derivation of the catalog rebind caused by the PNG text-placement
//! amendment (#169). It does not update a ledger.
use super::*;
const SPEC: &str = "docs/renderer-spec-v1.md";
fn amendment_only(before: &[u8], after: &[u8]) -> Result<(), String> {
    let before = std::str::from_utf8(before).map_err(|e| e.to_string())?;
    let after = std::str::from_utf8(after).map_err(|e| e.to_string())?;
    const START: &str = "Text placement follows the canvas's text rasterization as well (#169).";
    const END: &str = "rendering of it: the reference has no bundled Helvetica";
    let split = |s: &str| -> Result<(String, String), String> {
        let (prefix, body) = s
            .split_once(START)
            .ok_or("missing PNG text-placement anchor")?;
        let (_, suffix) = body
            .split_once(END)
            .ok_or("missing PNG text-placement boundary")?;
        Ok((prefix.into(), suffix.into()))
    };
    if before == after || split(before)? != split(after)? {
        return Err("authority migration must be confined to the reviewed draft PNG text-placement amendment".into());
    }
    Ok(())
}
pub(super) fn authority_only(
    before: &Catalog,
    after: &Catalog,
    ledger: &Ledger,
) -> Result<Vec<String>, String> {
    if ledger.catalog_sha256 != digest(before)?
        || ledger.schema != SCHEMA
        || ledger.compatibility_revision != 1
    {
        return Err("previous catalog is not authenticated by the ownership ledger".into());
    }
    let select = |catalog: &Catalog| -> Result<Descriptor, String> {
        let found: Vec<_> = catalog.sources.iter().filter(|s| s.path == SPEC).collect();
        if found.len() != 1 {
            return Err("catalog must bind one spec authority".into());
        }
        Ok(found[0].clone())
    };
    let old = select(before)?;
    let new = select(after)?;
    if old == new {
        return Err("no spec authority change to review".into());
    }
    let mut normalized = after.clone();
    let mut ids = Vec::new();
    for source in &mut normalized.sources {
        if source.path == SPEC {
            *source = old.clone();
        }
    }
    for row in &mut normalized.obligations {
        if row.authority.path == SPEC {
            if row.authority != new {
                return Err("inconsistent new spec authority binding".into());
            }
            row.authority = old.clone();
            ids.push(row.id.clone());
        }
    }
    if canonical_bytes(&serde_json::to_value(&normalized).map_err(|e| e.to_string())?)?
        != canonical_bytes(&serde_json::to_value(before).map_err(|e| e.to_string())?)?
    {
        return Err("authority migration changed criteria, identities, pointers, candidates or non-spec sources".into());
    }
    let expected: BTreeSet<_> = before.obligations.iter().map(|r| r.id.as_str()).collect();
    let assigned: BTreeSet<_> = ledger.assignments.iter().map(|r| r.id.as_str()).collect();
    if assigned != expected || assigned.len() != ledger.assignments.len() {
        return Err("authority migration requires the exact unchanged assignment set".into());
    }
    Ok(ids)
}
pub fn ownership_authority_migration(
    base: &str,
    expected_new_spec: &str,
    review_note: &str,
) -> Result<Value, String> {
    if !(7..=40).contains(&base.len()) || !base.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("previous spec must name an explicit commit hash".into());
    }
    let current_spec = read(SPEC)?;
    if sha256(&current_spec) != expected_new_spec {
        return Err(
            "reviewed new spec digest does not match current bytes; no authority rebind prepared"
                .into(),
        );
    }
    let note = descriptor(review_note)?;
    let git = |args: &[&str]| -> Result<Vec<u8>, String> {
        let out = std::process::Command::new("git")
            .current_dir(repo_root())
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        Ok(out.stdout)
    };
    let commit = String::from_utf8(git(&[
        "rev-parse",
        "--verify",
        &format!("{base}^{{commit}}"),
    ])?)
    .map_err(|e| e.to_string())?
    .trim()
    .to_owned();
    let previous_spec = git(&["show", &format!("{commit}:{SPEC}")])?;
    amendment_only(&previous_spec, &current_spec)?;
    // Planning is allowed against the ledger-authenticated stored inventory
    // while live evidence is stale. This is not a verification bypass: normal
    // catalog/assignment/closure paths still call load_inventory(), and this
    // command cannot apply a migration or assign an owner. Surface every live
    // inventory failure in the proposal rather than silently refreshing it.
    let inventory: Inventory = parse(&read(INVENTORY)?)?;
    if inventory.schema != INVENTORY_SCHEMA || inventory.compatibility_revision != 1 {
        return Err("authority migration requires the revision-1 stored inventory".into());
    }
    let live_inventory = match load_inventory() {
        Ok(_) => json!({"passed":true}),
        Err(error) => json!({"passed":false,"error":error}),
    };
    let before = catalog_from_inventory(&previous_spec, inventory.clone(), descriptor(INVENTORY)?)?;
    let after = catalog_from_inventory(&current_spec, inventory, descriptor(INVENTORY)?)?;
    let ledger_bytes = read(LEDGER)?;
    let ledger: Ledger = parse(&ledger_bytes)?;
    let ids = authority_only(&before, &after, &ledger)?;
    let mut exclusions = Vec::new();
    for assignment in &ledger.assignments {
        if let Disposition::Excluded { reason, proof, .. } = &assignment.disposition {
            let old = before
                .obligations
                .iter()
                .find(|r| r.id == assignment.id)
                .ok_or("exclusion obligation")?;
            let new = after
                .obligations
                .iter()
                .find(|r| r.id == assignment.id)
                .ok_or("exclusion obligation")?;
            if old != new {
                return Err("authority migration changes an approved exclusion subject".into());
            }
            check_evidence(proof)?;
            let subject = digest(
                &json!({"schema":"excalidraw-render/coverage-exclusion/v1","obligation":old,"reason":reason,"proof":proof}),
            )?;
            exclusions.push(json!({"id":assignment.id,"approval_subject_sha256":format!("sha256:{subject}"),"disposition":assignment.disposition}));
        }
    }
    if descriptor(SPEC)?.sha256 != expected_new_spec {
        return Err("spec changed while preparing authority migration".into());
    }
    if descriptor(review_note)? != note || read(LEDGER)? != ledger_bytes {
        return Err("review note or ledger changed while preparing authority migration".into());
    }
    for source in &after.sources {
        if descriptor(&source.path)? != *source {
            return Err("catalog source changed while preparing authority migration".into());
        }
    }
    Ok(
        json!({"schema":"excalidraw-render/ownership-authority-migration-proposal/v1","status":"review-required-not-applied","basis":"ledger-authenticated stored inventory","live_inventory_validation":live_inventory,"base_commit":commit,"review_note":note,"ledger":descriptor(LEDGER)?,"old_spec":{"path":SPEC,"sha256":sha256(&previous_spec)},"new_spec":{"path":SPEC,"sha256":expected_new_spec},"old_catalog_sha256":digest(&before)?,"new_catalog_sha256":digest(&after)?,"affected_authority_ids":ids,"assignments_sha256":digest(&ledger.assignments)?,"preserved_exclusions":exclusions,"criteria_dispositions_and_non_spec_sources_unchanged":true,"live_owner_validation":"separate required check; this proposal does not refresh stale owners or establish closure","approval":null}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Catalog, Catalog, Ledger) {
        let old = Descriptor {
            path: SPEC.into(),
            sha256: "a".repeat(64),
        };
        let new = Descriptor {
            path: SPEC.into(),
            sha256: "b".repeat(64),
        };
        let before = Catalog {
            schema: "excalidraw-render/coverage-obligations/v1".into(),
            sources: vec![old.clone()],
            obligations: vec![Obligation {
                id: "fixture".into(),
                category: "interaction".into(),
                criterion: "unchanged criterion".into(),
                authority: old,
                pointer: "same anchor".into(),
                candidates: vec![],
            }],
        };
        let mut after = before.clone();
        after.sources[0] = new.clone();
        after.obligations[0].authority = new;
        let ledger = Ledger {
            schema: SCHEMA.into(),
            compatibility_revision: 1,
            catalog_sha256: digest(&before).unwrap(),
            approval: Value::Null,
            assignments: vec![Assignment {
                id: "fixture".into(),
                disposition: Disposition::Gap {
                    issue: 85,
                    reason: "unchanged gap".into(),
                },
            }],
        };
        (before, after, ledger)
    }
    #[test]
    fn authority_migration_is_not_a_digest_override() {
        let (before, after, ledger) = fixture();
        assert_eq!(
            authority_only(&before, &after, &ledger).unwrap(),
            vec!["fixture"]
        );
        for mutation in 0..5 {
            let mut bad = after.clone();
            match mutation {
                0 => bad.obligations[0].criterion.push_str(" widened"),
                1 => bad.obligations[0].pointer.push_str(" changed"),
                2 => bad.obligations[0].candidates.push("new owner".into()),
                3 => bad.obligations.clear(),
                _ => bad.sources.push(Descriptor {
                    path: "other".into(),
                    sha256: "c".repeat(64),
                }),
            }
            assert!(authority_only(&before, &bad, &ledger).is_err());
        }
        let mut stale = ledger.clone();
        stale.catalog_sha256 = "0".repeat(64);
        assert!(authority_only(&before, &after, &stale).is_err());
        let mut approved = ledger;
        approved.approval = json!({"existing":"approval"});
        authority_only(&before, &after, &approved).unwrap();
    }
    #[test]
    fn authority_migration_rejects_changes_outside_the_reviewed_paragraph() {
        let before="prefix Text placement follows the canvas's text rasterization as well (#169). old rendering of it: the reference has no bundled Helvetica suffix";
        let after = before.replace(" old ", " new ");
        amendment_only(before.as_bytes(), after.as_bytes()).unwrap();
        assert!(amendment_only(
            before.as_bytes(),
            after.replace("prefix", "changed").as_bytes()
        )
        .is_err());
        assert!(amendment_only(
            before.as_bytes(),
            after.replace("suffix", "changed").as_bytes()
        )
        .is_err());
        assert!(amendment_only(before.as_bytes(), before.as_bytes()).is_err());
    }
}
