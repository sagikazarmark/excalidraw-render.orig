//! One explicitly authorized Stage-4 metadata transition, never a general hash
//! override. It preserves execution/criterion/disposition semantics and approvals.
use super::*;
const RECORD: &str = "tests/conformance/coverage/stage4-binding-reconciliation.json";
const SPEC: &str = "docs/renderer-spec-v1.md";
const SPEC_HASH: &str = "628acb6d7cf2e2756b1d0bacbab3aab9ee73b548df5252057747f7ea17270243";
const ASSESSMENT: &str = "docs/research/stage4-png-device-phase-assessment.md";
const ASSESSMENT_HASH: &str = "286a8aaf93d0050608baac9cb86c074fbd513bf20fdc616dc034a5db1cd24e40";
const BEFORE_LEDGER: &str = "6882c37db5a89ebcccce0ee0b0a5bf399cbcc891cb20a10e19f3bfea1e0d7788";
const LOCATOR: &str = "crates/excalidraw-render/tests/fast/static_png_asset_render.rs";
const CANARY: &str = "tests/conformance/candidates/stage4-excalifont-canary";
const SOURCES: [(&str, &str); 5] = [
    (
        LOCATOR,
        "035e15f5cd1783062deba54f60be11fa2037d1fede0988f54a4e1cf9b8bf7265",
    ),
    (
        "crates/excalidraw-render/tests/fast/png_stage4_contract.rs",
        "85c060c4eea57c2ac27a828ca750e624c0ac101af1e3a30bf9ccab646c174666",
    ),
    (
        "crates/excalidraw-render/src/png_projection_contract.rs",
        "063741f25a877ffa800d9dfad2d85ffc982c3400a5aba10eb46117ee770dce84",
    ),
    (
        "crates/excalidraw-render/tests/fast/stage4_interaction_contract.rs",
        "a2a3358bd8ac93fe930ee2e237ae89a93a32ccf5d8fad5f8d60cac15eed70676",
    ),
    (
        "crates/excalidraw-render/tests/fast/image_field_contract.rs",
        "2149d913be045a76c023db2d2b691871e5138680c69f8dc38605a03265fb963e",
    ),
];
fn pretty<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    let mut b = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    b.push(b'\n');
    Ok(b)
}
fn desc(path: &str, bytes: &[u8]) -> Descriptor {
    Descriptor {
        path: path.into(),
        sha256: sha256(bytes),
    }
}
pub fn reconcile_stage4_bindings(canary_qualification: &str) -> Result<Value, String> {
    if canary_qualification == format!("{}/bridge.json", crate::conformance_pdf_bridge::DIRECTORY) {
        return reconcile_image_icc_bindings();
    }
    let old_ledger_bytes = read(LEDGER)?;
    if sha256(&old_ledger_bytes) != BEFORE_LEDGER {
        return Err("Stage-4 reconciliation requires its exact reviewed before-ledger".into());
    }
    if descriptor(SPEC)?.sha256 != SPEC_HASH || descriptor(ASSESSMENT)?.sha256 != ASSESSMENT_HASH {
        return Err("parent-confirmed spec/assessment changed".into());
    }
    let qualification_bytes = read(canary_qualification)?;
    if sha256(&qualification_bytes)
        != "f05f408b7187af1b2ae5edb7c451179afb50d0f1bcb752926e610067ab431e87"
    {
        return Err(
            "canary qualification is not the independently observed reviewed record".into(),
        );
    }
    let qualification: Value = parse(&qualification_bytes)?;
    if qualification["canonical_typography_passed"] != true
        || qualification["new_native_png_sha256"]
            != "9b2da5d13b6a2c672ab80be51edbeeb9491195b57939dea9f48e80e3887f3b95"
    {
        return Err("missing separately qualified native canary".into());
    }
    crate::conformance_candidate_native::check_producers(&qualification["native_producers"])?;
    for (path, hash) in SOURCES {
        if descriptor(path)?.sha256 != hash {
            return Err(format!("focused-check source changed: {path}"));
        }
    }
    let git = |path: &str| -> Result<Vec<u8>, String> {
        let out = std::process::Command::new("git")
            .current_dir(repo_root())
            .args(["show", &format!("629930b:{path}")])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        Ok(out.stdout)
    };
    let old_spec = git(SPEC)?;
    let spec = read(SPEC)?;
    let old_locator = String::from_utf8(git(LOCATOR)?).map_err(|e| e.to_string())?;
    let expected_locator = old_locator.replace(
        "-30 0 0 -20 30 20 cm\\n/Im0 Do",
        "-30 0 0 20 30 0 cm\\n/Im0 Do",
    );
    if expected_locator.as_bytes() != read(LOCATOR)? || expected_locator == old_locator {
        return Err("locator changed beyond the reviewed matrix assertion".into());
    }
    let old_requirements_bytes = read(REQUIREMENTS)?;
    let old_requirements: Requirements = parse(&old_requirements_bytes)?;
    let old_inventory_bytes = read(INVENTORY)?;
    let old_inventory: Inventory = parse(&old_inventory_bytes)?;
    if old_inventory.requirements != old_requirements.requirements {
        return Err("requirements differ from the authenticated stored inventory".into());
    }
    let mut requirements = old_requirements.clone();
    let row = requirements
        .requirements
        .iter_mut()
        .find(|r| r.id == "fuzz.codecs")
        .ok_or("missing fuzz.codecs locator")?;
    if row.evidence.len() != 1
        || row.evidence[0].source.path != LOCATOR
        || row.evidence[0].source.sha256 != sha256(&git(LOCATOR)?)
        || row.evidence[0].anchor
            != "fn apng_is_preserved_in_svg_and_flattened_to_frame_zero_in_png_and_pdf()"
    {
        return Err("unexpected codec locator disposition".into());
    }
    row.evidence[0].source = descriptor(LOCATOR)?;
    let requirements_bytes = pretty(&requirements)?;
    let inventory = derive_inventory_with_requirements(
        requirements.clone(),
        desc(REQUIREMENTS, &requirements_bytes),
    )?;
    let mut normalized_inventory = inventory.clone();
    normalized_inventory.requirements = old_inventory.requirements.clone();
    for source in &mut normalized_inventory.sources {
        if source.path == REQUIREMENTS {
            *source = desc(REQUIREMENTS, &old_requirements_bytes);
        }
    }
    if normalized_inventory != old_inventory {
        return Err("reconciliation changed inventory execution/coverage semantics".into());
    }
    let inventory_bytes = pretty(&inventory)?;
    let before = catalog_from_inventory(
        &old_spec,
        old_inventory.clone(),
        desc(INVENTORY, &old_inventory_bytes),
    )?;
    let spec_only =
        catalog_from_inventory(&spec, old_inventory, desc(INVENTORY, &old_inventory_bytes))?;
    let mut ledger: Ledger = parse(&old_ledger_bytes)?;
    let authority_ids = authority_migration::authority_only(&before, &spec_only, &ledger)?;
    if authority_ids.len() != 1007 {
        return Err("reviewed spec-authority scope changed".into());
    }
    let after =
        catalog_from_inventory(&spec, inventory.clone(), desc(INVENTORY, &inventory_bytes))?;
    let mut normalized_catalog = after.clone();
    let mut inventory_authority_ids = Vec::new();
    for source in &mut normalized_catalog.sources {
        if source.path == INVENTORY {
            *source = desc(INVENTORY, &old_inventory_bytes);
        }
    }
    for row in &mut normalized_catalog.obligations {
        if row.authority.path == INVENTORY {
            inventory_authority_ids.push(row.id.clone());
            row.authority = desc(INVENTORY, &old_inventory_bytes);
        }
    }
    if digest(&normalized_catalog)? != digest(&spec_only)? {
        return Err("inventory locator changed catalog criteria or other authorities".into());
    }
    let original = ledger.clone();
    let mut owner_changes = Vec::new();
    for assignment in &mut ledger.assignments {
        if let Disposition::Owned {
            owner: Owner::Test { source, .. },
            ..
        } = &mut assignment.disposition
        {
            let current = descriptor(&source.path)?;
            if *source != current {
                if !SOURCES
                    .iter()
                    .any(|(p, h)| source.path == *p && current.sha256 == *h)
                {
                    return Err(format!("unreviewed owner source change: {}", source.path));
                }
                owner_changes.push(
                    json!({"obligation":assignment.id,"before":source.clone(),"after":current}),
                );
                *source = current;
            }
        }
    }
    let mut semantic_copy = ledger.clone();
    for (row, old) in semantic_copy
        .assignments
        .iter_mut()
        .zip(&original.assignments)
    {
        if let (
            Disposition::Owned {
                owner: Owner::Test { source, .. },
                ..
            },
            Disposition::Owned {
                owner: Owner::Test {
                    source: old_source, ..
                },
                ..
            },
        ) = (&mut row.disposition, &old.disposition)
        {
            *source = old_source.clone();
        }
    }
    if digest(&semantic_copy.assignments)? != digest(&original.assignments)? {
        return Err(
            "reconciliation changed a disposition, selector, command, rationale or exclusion"
                .into(),
        );
    }
    ledger.catalog_sha256 = digest(&after)?;
    let gaps = validate(&after, &ledger, false)?;
    let ledger_bytes = pretty(&ledger)?;
    let record = json!({"schema":"excalidraw-render/stage4-binding-reconciliation/v1","authorization":"User explicitly authorized the draft PNG rule, locator-only reconciliation, focused-tested owner source refresh and the one native PNG canary change; no maintainer/case/baseline approval is implied.","spec":descriptor(SPEC)?,"assessment":descriptor(ASSESSMENT)?,"before_files":[desc(REQUIREMENTS,&old_requirements_bytes),desc(INVENTORY,&old_inventory_bytes),desc(LEDGER,&old_ledger_bytes)],"after_files":[desc(REQUIREMENTS,&requirements_bytes),desc(INVENTORY,&inventory_bytes),desc(LEDGER,&ledger_bytes)],"old_catalog_sha256":digest(&before)?,"spec_only_catalog_sha256":digest(&spec_only)?,"new_catalog_sha256":digest(&after)?,"spec_authority_ids":authority_ids,"inventory_locator_authority_ids":inventory_authority_ids,"owner_source_changes":owner_changes,"execution_count":inventory.executions.len(),"assignment_count":ledger.assignments.len(),"gaps":gaps,"canary_qualification":desc(&format!("{CANARY}/qualification.json"),&qualification_bytes),"preserved_exclusions":ledger.assignments.iter().filter(|r|matches!(r.disposition,Disposition::Excluded{..})).collect::<Vec<_>>(),"approval":null});
    if repo_root().join(RECORD).exists() {
        return Err("binding reconciliation record already exists".into());
    }
    let files = [
        (REQUIREMENTS, requirements_bytes, old_requirements_bytes),
        (INVENTORY, inventory_bytes, old_inventory_bytes),
        (LEDGER, ledger_bytes, old_ledger_bytes),
    ];
    for (path, _, old) in &files {
        if read(path)? != *old {
            return Err("binding input changed before publication".into());
        }
    }
    let origin = std::path::Path::new(canary_qualification)
        .parent()
        .and_then(|p| p.to_str())
        .ok_or("canary evidence parent")?;
    fs::create_dir(repo_root().join(CANARY)).map_err(|e| e.to_string())?;
    for name in [
        "scene.json",
        "official.svg",
        "official.png",
        "official-png.rgba",
        "provenance.json",
        "native.svg",
        "native.png",
        "native.pdf",
        "qualification.json",
    ] {
        crate::conformance_candidate_native::write_new(
            &repo_root().join(CANARY).join(name),
            &read(&format!("{origin}/{name}"))?,
        )?;
    }
    let result: Result<(), String> = (|| {
        for (path, new, _) in &files {
            fs::write(repo_root().join(path), new).map_err(|e| e.to_string())?;
        }
        crate::conformance_candidate_native::write_new(
            &repo_root().join(RECORD),
            &pretty(&record)?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        for (path, _, old) in &files {
            fs::write(repo_root().join(path), old)
                .map_err(|e| format!("rollback failed: {e}; original: {error}"))?;
        }
        return Err(error);
    }
    Ok(record)
}

fn reconcile_image_icc_bindings() -> Result<Value, String> {
    const FOLLOWUP: &str =
        "tests/conformance/coverage/stage4-image-icc-binding-reconciliation.json";
    let old_ledger = read(LEDGER)?;
    if sha256(&old_ledger) != "fdc86c8ab6e29371d13f95a0190735286aca764ef0c1c1f3e663a889a33402b5"
        || repo_root().join(FOLLOWUP).exists()
    {
        return Err("image ICC binding refresh requires its exact reviewed before-state".into());
    }
    let bridge = crate::conformance_pdf_bridge::validate(&read(&format!(
        "{}/native.pdf",
        crate::conformance_pdf_bridge::DIRECTORY
    ))?)?;
    let spec = read(SPEC)?;
    if sha256(&spec) != SPEC_HASH {
        return Err("spec changed during image-only binding refresh".into());
    }
    let old_requirements_bytes = read(REQUIREMENTS)?;
    let mut requirements: Requirements = parse(&old_requirements_bytes)?;
    let old_inventory_bytes = read(INVENTORY)?;
    let old_inventory: Inventory = parse(&old_inventory_bytes)?;
    if old_inventory.requirements != requirements.requirements {
        return Err("stale requirements before image ICC refresh".into());
    }
    let r = requirements
        .requirements
        .iter_mut()
        .find(|r| r.id == "fuzz.codecs")
        .ok_or("fuzz.codecs")?;
    if r.evidence.len() != 1
        || r.evidence[0].source.path != LOCATOR
        || r.evidence[0].source.sha256 != SOURCES[0].1
    {
        return Err("unexpected image ICC locator before-state".into());
    }
    r.evidence[0].source = descriptor(LOCATOR)?;
    let requirements_bytes = pretty(&requirements)?;
    let inventory =
        derive_inventory_with_requirements(requirements, desc(REQUIREMENTS, &requirements_bytes))?;
    let mut normalized = inventory.clone();
    normalized.requirements = old_inventory.requirements.clone();
    for d in &mut normalized.sources {
        if d.path == REQUIREMENTS {
            *d = desc(REQUIREMENTS, &old_requirements_bytes);
        }
    }
    if normalized != old_inventory {
        return Err("image ICC refresh changed execution/coverage semantics".into());
    }
    let inventory_bytes = pretty(&inventory)?;
    let before =
        catalog_from_inventory(&spec, old_inventory, desc(INVENTORY, &old_inventory_bytes))?;
    let after =
        catalog_from_inventory(&spec, inventory.clone(), desc(INVENTORY, &inventory_bytes))?;
    let mut normalized = after.clone();
    for d in &mut normalized.sources {
        if d.path == INVENTORY {
            *d = desc(INVENTORY, &old_inventory_bytes);
        }
    }
    for r in &mut normalized.obligations {
        if r.authority.path == INVENTORY {
            r.authority = desc(INVENTORY, &old_inventory_bytes);
        }
    }
    if digest(&normalized)? != digest(&before)? {
        return Err("image ICC refresh changed catalog criteria".into());
    }
    let mut ledger: Ledger = parse(&old_ledger)?;
    if ledger.catalog_sha256 != digest(&before)? {
        return Err("image ICC refresh is not authorized by the before-ledger".into());
    }
    let approved_sources = [
        (
            LOCATOR,
            "02377311ef96aea23ba878915cf7d95819c5b0c281bb224857b60014338dc61b",
        ),
        (
            "crates/excalidraw-render/tests/fast/png_profile_contract.rs",
            "44a5b9b4fba24704191b32f53008467605e6afa67f3fad02a3937ea53aba64c5",
        ),
        (
            "crates/excalidraw-render/tests/fast/png_stage4_contract.rs",
            "2676c6dac4e7f7f62f206cb78c71ad3e4c918fd54767751bcf024cb69655ce73",
        ),
    ];
    let mut changes = Vec::new();
    for a in &mut ledger.assignments {
        if let Disposition::Owned {
            owner: Owner::Test { source, .. },
            ..
        } = &mut a.disposition
        {
            let current = descriptor(&source.path)?;
            if *source != current {
                if !approved_sources
                    .iter()
                    .any(|(p, h)| source.path == *p && current.sha256 == *h)
                {
                    return Err(format!("unreviewed image ICC owner source {}", source.path));
                }
                changes.push(json!({"obligation":a.id,"before":source.clone(),"after":current}));
                *source = current;
            }
        }
    }
    ledger.catalog_sha256 = digest(&after)?;
    let gaps = validate(&after, &ledger, false)?;
    let ledger_bytes = pretty(&ledger)?;
    let record = json!({"schema":"excalidraw-render/stage4-image-icc-binding-reconciliation/v1","authorization":"User-authorized direct image ICC binding fix and stricter oracles; selectors, dispositions, criteria, execution records, exclusions and approvals unchanged.","bridge":bridge,"previous_reconciliation":descriptor(RECORD)?,"before_files":[desc(REQUIREMENTS,&old_requirements_bytes),desc(INVENTORY,&old_inventory_bytes),desc(LEDGER,&old_ledger)],"after_files":[desc(REQUIREMENTS,&requirements_bytes),desc(INVENTORY,&inventory_bytes),desc(LEDGER,&ledger_bytes)],"old_catalog_sha256":digest(&before)?,"new_catalog_sha256":digest(&after)?,"owner_source_changes":changes,"execution_count":inventory.executions.len(),"gaps":gaps,"approval":null});
    let files = [
        (REQUIREMENTS, requirements_bytes, old_requirements_bytes),
        (INVENTORY, inventory_bytes, old_inventory_bytes),
        (LEDGER, ledger_bytes, old_ledger),
    ];
    for (path, _, old) in &files {
        if read(path)? != *old {
            return Err("image ICC binding changed before publication".into());
        }
    }
    let result: Result<(), String> = (|| {
        for (path, new, _) in &files {
            fs::write(repo_root().join(path), new).map_err(|e| e.to_string())?;
        }
        crate::conformance_candidate_native::write_new(
            &repo_root().join(FOLLOWUP),
            &pretty(&record)?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        for (path, _, old) in &files {
            fs::write(repo_root().join(path), old).map_err(|e| e.to_string())?;
        }
        return Err(error);
    }
    Ok(record)
}
