//! Global obligation/owner boundary. The execution inventory remains unchanged:
//! selecting ownership never silently regenerates approved renderer snapshots.
use super::*;
use crate::conformance::verify_approval_for_issue;

const POLICY: &str = "tests/conformance/coverage/ownership-policy.json";
const LEDGER: &str = "tests/conformance/coverage/ownership.json";
const SCHEMA: &str = "excalidraw-render/coverage-ownership/v1";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Obligation {
    id: String,
    category: String,
    criterion: String,
    authority: Descriptor,
    pointer: String,
    candidates: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PolicyRow {
    id: String,
    category: String,
    criterion: String,
    spec_anchor: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema: String,
    rows: Vec<PolicyRow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema: String,
    sources: Vec<Descriptor>,
    obligations: Vec<Obligation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Owner {
    Case {
        case: String,
        manifest: Descriptor,
    },
    Test {
        source: Descriptor,
        function: String,
        binary: String,
        command: Vec<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum Disposition {
    Owned {
        owner: Owner,
        rationale: String,
    },
    Gap {
        issue: u64,
        reason: String,
    },
    Excluded {
        reason: String,
        proof: Evidence,
        approval: Value,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Assignment {
    id: String,
    disposition: Disposition,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema: String,
    compatibility_revision: u64,
    catalog_sha256: String,
    approval: Value,
    assignments: Vec<Assignment>,
}

fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}

fn catalog() -> Result<Catalog, String> {
    let inventory = load_inventory()?;
    let policy: Policy = parse(&read(POLICY)?)?;
    if policy.schema != "excalidraw-render/coverage-ownership-policy/v1" {
        return Err("invalid ownership policy schema".into());
    }
    let surface: Value = parse(&read(SURFACE)?)?;
    let surface_descriptor = descriptor(SURFACE)?;
    let mut obligations = Vec::new();
    for row in &inventory.coverage {
        if row.id.starts_with("case-label:") {
            obligations.push(Obligation{id:row.id.clone(),category:"case-label".into(),
                criterion:format!("Primary evidence for the existing local label {}. Format/profile sub-obligations are separate.",row.id),
                authority:descriptor(INVENTORY)?,pointer:"/coverage".into(),candidates:row.candidates.clone()});
        }
    }
    for (index, field) in array(&surface, "fieldDispositions")?.iter().enumerate() {
        let base = format!(
            "surface:{}:{}",
            string(field, "recordKind")?,
            string(field, "path")?
        );
        for format in array(&field["scope"], "formats")? {
            let format = format.as_str().ok_or("invalid surface format")?;
            obligations.push(Obligation{id:format!("{base}:{format}"),category:"surface-field".into(),
                criterion:format!("{} {} in {format}: disposition, missing/invalid and value-rule recovery under the declared participation rules",string(field,"recordKind")?,string(field,"path")?),
                authority:surface_descriptor.clone(),pointer:format!("/fieldDispositions/{index}"),candidates:vec![]});
        }
    }
    for (index, row) in array(&surface, "registryMappings")?.iter().enumerate() {
        let code = string(row, "code")?;
        for format in row["formatRules"]
            .as_object()
            .ok_or("diagnostic format rules missing")?
            .keys()
        {
            obligations.push(Obligation{id:format!("diagnostic:{code}:{format}"),category:"diagnostic".into(),
                criterion:format!("{code} in {format}: exact severity, effects, details, recovery, occurrence scope and order (including silence where specified)"),
                authority:surface_descriptor.clone(),pointer:format!("/registryMappings/{index}/formatRules/{format}"),candidates:vec![]});
        }
    }
    let registry: Value = parse(&read(REGISTRY)?)?;
    let mut sources = vec![
        descriptor(INVENTORY)?,
        surface_descriptor,
        descriptor(POLICY)?,
        descriptor("docs/renderer-spec-v1.md")?,
    ];
    for case in array(&registry, "cases")? {
        let id = string(case, "id")?;
        let path = format!("{}/manifest.json", string(case, "path")?);
        let manifest: Value = parse(&read(&path)?)?;
        if let Some(exceptions) = manifest["exceptions"].as_array() {
            for (index, exception) in exceptions.iter().enumerate() {
                obligations.push(Obligation{id:format!("exception:{id}:{}:{}:{}",string(exception,"variant")?,string(exception,"format")?,string(exception,"region")?),
                    category:"tolerance-exception".into(),criterion:"Approved ceiling and non-defect cause; reject stale exceptions when canonical budget passes".into(),
                    authority:descriptor(&path)?,pointer:format!("/exceptions/{index}"),candidates:vec![id.into()]});
            }
        }
        sources.push(descriptor(&path)?);
    }
    let spec = String::from_utf8(read("docs/renderer-spec-v1.md")?).map_err(|e| e.to_string())?;
    for row in policy.rows {
        if !matches!(
            row.category.as_str(),
            "codec-profile" | "interaction" | "closure"
        ) || !nonempty(&row.criterion)
            || !nonempty(&row.spec_anchor)
            || !spec.contains(&row.spec_anchor)
        {
            return Err(format!(
                "invalid ownership policy row or stale spec anchor: {}",
                row.id
            ));
        }
        obligations.push(Obligation {
            id: row.id,
            category: row.category,
            criterion: row.criterion,
            authority: descriptor("docs/renderer-spec-v1.md")?,
            pointer: row.spec_anchor,
            candidates: vec![],
        });
    }
    obligations.sort_by(|a, b| a.id.cmp(&b.id));
    if obligations.iter().any(|r| !nonempty(&r.id))
        || obligations.windows(2).any(|p| p[0].id == p[1].id)
    {
        return Err("duplicate or empty ownership obligation".into());
    }
    for required in [
        "closure:atomic-profile-inventory",
        "closure:all-five-layers",
        "closure:pdf-judgments",
        "closure:process-target-equality",
    ] {
        if !obligations.iter().any(|r| r.id == required) {
            return Err(format!("required closure obligation removed: {required}"));
        }
    }
    for codec in ["png", "jpeg", "gif", "webp", "bmp", "ico"] {
        for facet in [
            "passthrough",
            "default-frame",
            "orientation",
            "color-profile",
            "alpha",
            "malformed",
            "limits",
        ] {
            let id = format!("codec:{codec}:{facet}");
            if !obligations.iter().any(|r| r.id == id) {
                return Err(format!("required codec obligation removed: {id}"));
            }
        }
    }
    for id in [
        "codec:png:animation",
        "codec:gif:animation",
        "codec:webp:animation",
        "codec:png:hdr-gain-map",
        "codec:jpeg:hdr-gain-map",
        "codec:avif:rejection",
        "codec:nested-svg:sanitization",
        "codec:nested-svg:references",
        "codec:nested-svg:pdf-fallback",
        "interaction:frames-text-assets",
        "interaction:dark-mode-nested-svg-pdf",
        "interaction:diagnostic-order",
        "interaction:frame-batch-limits",
    ] {
        if !obligations.iter().any(|r| r.id == id) {
            return Err(format!("required policy obligation removed: {id}"));
        }
    }
    Ok(Catalog {
        schema: "excalidraw-render/coverage-obligations/v1".into(),
        sources,
        obligations,
    })
}

pub fn ownership_catalog() -> Result<String, String> {
    serde_json::to_string_pretty(&catalog()?).map_err(|e| e.to_string())
}

pub fn seed_ownership() -> Result<String, String> {
    let catalog = catalog()?;
    let ledger=Ledger{schema:SCHEMA.into(),compatibility_revision:1,catalog_sha256:digest(&catalog)?,approval:Value::Null,
        assignments:catalog.obligations.iter().map(|row|Assignment{id:row.id.clone(),disposition:Disposition::Gap{
            issue:85,reason:"No reviewed primary owner assigned yet; candidate evidence is not complete coverage.".into()}}).collect()};
    serde_json::to_string_pretty(&ledger).map_err(|e| e.to_string())
}

fn check_evidence(evidence: &Evidence) -> Result<(), String> {
    if descriptor(&evidence.source.path)? != evidence.source
        || !nonempty(&evidence.anchor)
        || !String::from_utf8_lossy(&read(&evidence.source.path)?).contains(&evidence.anchor)
    {
        return Err("stale ownership evidence".into());
    }
    Ok(())
}

fn unconditional_test(
    source: &str,
    main: &str,
    module: &str,
    function: &str,
) -> Result<(), String> {
    // Narrow source adapter, not a Rust parser: reject conditional/ignored or
    // macro-attributed modules entirely instead of claiming they execute.
    for line in source.lines().map(str::trim) {
        if line.starts_with("#[") && line != "#[test]" || line.starts_with("#![") {
            return Err("test owner contains unsupported attributes (conditional/ignored tests are not owners)".into());
        }
    }
    if main.contains("#[cfg") || main.contains("#![") || main.contains("#[ignore") {
        return Err("test binary contains conditional attributes".into());
    }
    let lines: Vec<_> = main.lines().collect();
    let at = lines
        .iter()
        .position(|l| *l == format!("mod {module};"))
        .ok_or("test owner module is not compiled")?;
    if at > 0 && lines[at - 1].trim().starts_with("#[") {
        return Err("test owner module has unsupported attributes".into());
    }
    if !source.contains(&format!("#[test]\nfn {function}(")) {
        return Err("test owner function is not an unconditional test".into());
    }
    Ok(())
}

fn validate(catalog: &Catalog, ledger: &Ledger, closed: bool) -> Result<usize, String> {
    if ledger.schema != SCHEMA
        || ledger.compatibility_revision != 1
        || ledger.catalog_sha256 != digest(catalog)?
    {
        return Err("stale ownership catalog binding".into());
    }
    let expected: BTreeMap<_, _> = catalog
        .obligations
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    let mut seen = BTreeSet::new();
    let mut gaps = 0;
    for assignment in &ledger.assignments {
        let row = expected
            .get(assignment.id.as_str())
            .ok_or_else(|| format!("unknown ownership identity {}", assignment.id))?;
        if !seen.insert(assignment.id.as_str()) {
            return Err(format!("duplicate primary ownership {}", assignment.id));
        }
        match &assignment.disposition {
            Disposition::Gap { issue, reason } => {
                if *issue == 0 || !nonempty(reason) {
                    return Err("gap must name an issue and reason".into());
                }
                gaps += 1;
            }
            Disposition::Owned { owner, rationale } => {
                if !nonempty(rationale) {
                    return Err("ownership rationale is empty".into());
                }
                match owner {
                    Owner::Case { case, manifest } => {
                        if row.category != "case-label" || !row.candidates.contains(case) {
                            return Err(format!(
                                "case owner does not establish obligation {}",
                                row.id
                            ));
                        }
                        let registry: Value = parse(&read(REGISTRY)?)?;
                        let entry = array(&registry, "cases")?
                            .iter()
                            .find(|e| e["id"] == case.as_str())
                            .ok_or("owner case is not registered")?;
                        let path = format!("{}/manifest.json", string(entry, "path")?);
                        if *manifest != descriptor(&path)? {
                            return Err(format!("stale owner manifest for {case}"));
                        }
                    }
                    Owner::Test {
                        source,
                        function,
                        binary,
                        command,
                    } => {
                        let valid_name = |s: &str| {
                            !s.is_empty()
                                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        };
                        if !valid_name(function) || !valid_name(binary) {
                            return Err("invalid test owner selector".into());
                        }
                        // Only existing integration-test modules in the renderer's
                        // auto-discovered fast binary are admitted by v1.
                        let prefix = "crates/excalidraw-render/tests/fast/";
                        if !source.path.starts_with(prefix)
                            || !source.path.ends_with(".rs")
                            || binary != "fast"
                        {
                            return Err("unsupported executable owner path/binary".into());
                        }
                        let module = source
                            .path
                            .strip_prefix(prefix)
                            .unwrap()
                            .strip_suffix(".rs")
                            .unwrap();
                        if !valid_name(module) {
                            return Err("nested test module must have an explicit adapter".into());
                        }
                        let main =
                            String::from_utf8(read("crates/excalidraw-render/tests/fast/main.rs")?)
                                .map_err(|e| e.to_string())?;
                        if !main.lines().any(|l| l == format!("mod {module};")) {
                            return Err("test owner module is not compiled".into());
                        }
                        check_evidence(&Evidence {
                            source: source.clone(),
                            anchor: format!("#[test]\nfn {function}("),
                        })?;
                        let source_text =
                            String::from_utf8(read(&source.path)?).map_err(|e| e.to_string())?;
                        unconditional_test(&source_text, &main, module, function)?;
                        let args: Vec<String> = [
                            "cargo",
                            "test",
                            "--locked",
                            "--release",
                            "-p",
                            "excalidraw-render",
                            "--test",
                            "fast",
                        ]
                        .into_iter()
                        .map(String::from)
                        .chain([
                            format!("{module}::{function}"),
                            "--".into(),
                            "--exact".into(),
                        ])
                        .collect();
                        if command != &args {
                            return Err(
                                "test owner command does not select its exact executable test"
                                    .into(),
                            );
                        }
                    }
                }
            }
            Disposition::Excluded {
                reason,
                proof,
                approval,
            } => {
                if !nonempty(reason) {
                    return Err("exclusion reason is empty".into());
                }
                check_evidence(proof)?;
                let subject = digest(
                    &json!({"schema":"excalidraw-render/coverage-exclusion/v1","obligation":row,"reason":reason,"proof":proof}),
                )?;
                verify_approval_for_issue(approval, &subject, "85")?;
            }
        }
    }
    if seen != expected.keys().copied().collect() {
        return Err("missing ownership assignments (use explicit gaps)".into());
    }
    if closed && gaps > 0 {
        return Err(format!("coverage ownership has {gaps} unresolved gaps"));
    }
    if closed || !ledger.approval.is_null() {
        verify_approval_for_issue(&ledger.approval, &subject(catalog, ledger)?, "85")?;
    }
    Ok(gaps)
}

fn subject(catalog: &Catalog, ledger: &Ledger) -> Result<String, String> {
    let mut value = serde_json::to_value(ledger).map_err(|e| e.to_string())?;
    value.as_object_mut().unwrap().remove("approval");
    // Test source hashes are live location checks, not judgment-bearing output.
    // Approve the selected executable and rationale without pinning unrelated
    // consuming tests in the same source module.
    for assignment in value["assignments"].as_array_mut().unwrap() {
        if assignment["disposition"]["owner"]["kind"] == "test" {
            assignment["disposition"]["owner"]["source"]
                .as_object_mut()
                .unwrap()
                .remove("sha256");
        }
    }
    // Subject contains no floating-point option values: authorities are byte descriptors.
    digest(
        &json!({"schema":"excalidraw-render/coverage-ownership-approval/v1","catalog":catalog,"ledger":value}),
    )
}

fn load() -> Result<(Catalog, Ledger), String> {
    Ok((catalog()?, parse(&read(LEDGER)?)?))
}

pub fn verify_ownership() -> Result<(), String> {
    let (catalog, ledger) = load()?;
    let gaps = validate(&catalog, &ledger, false)?;
    for case in crate::conformance::case_ids()? {
        crate::conformance::verify(&case, true)?;
    }
    println!("verified {} ownership assignments: {} assigned/excluded, {gaps} explicit gaps; ledger approval {}",
        ledger.assignments.len(),ledger.assignments.len()-gaps,if ledger.approval.is_null(){"pending"}else{"recorded"});
    Ok(())
}

pub fn ownership_subject() -> Result<String, String> {
    let (catalog, ledger) = load()?;
    let gaps = validate(&catalog, &ledger, false)?;
    if gaps > 0 {
        return Err(format!(
            "cannot approve completed ownership: {gaps} gaps remain"
        ));
    }
    subject(&catalog, &ledger)
}

pub(super) fn verify_closed() -> Result<(), String> {
    let (catalog, ledger) = load()?;
    validate(&catalog, &ledger, true)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Catalog, Ledger) {
        let row = Obligation {
            id: "fixture".into(),
            category: "diagnostic".into(),
            criterion: "test coverage".into(),
            authority: Descriptor {
                path: "fixture".into(),
                sha256: "a".repeat(64),
            },
            pointer: "/rule".into(),
            candidates: vec![],
        };
        let catalog = Catalog {
            schema: "excalidraw-render/coverage-obligations/v1".into(),
            sources: vec![],
            obligations: vec![row],
        };
        let ledger = Ledger {
            schema: SCHEMA.into(),
            compatibility_revision: 1,
            catalog_sha256: digest(&catalog).unwrap(),
            approval: Value::Null,
            assignments: vec![Assignment {
                id: "fixture".into(),
                disposition: Disposition::Gap {
                    issue: 85,
                    reason: "missing exact test".into(),
                },
            }],
        };
        (catalog, ledger)
    }

    #[test]
    fn gaps_are_explicit_not_coverage_and_assignments_are_exactly_once() {
        let (catalog, ledger) = fixture();
        assert_eq!(validate(&catalog, &ledger, false).unwrap(), 1);
        assert!(validate(&catalog, &ledger, true)
            .unwrap_err()
            .contains("unresolved gaps"));
        let mut missing = ledger.clone();
        missing.assignments.clear();
        assert!(validate(&catalog, &missing, false)
            .unwrap_err()
            .contains("missing ownership"));
        let mut duplicate = ledger.clone();
        duplicate.assignments.push(duplicate.assignments[0].clone());
        assert!(validate(&catalog, &duplicate, false)
            .unwrap_err()
            .contains("duplicate primary"));
        let mut unknown = ledger.clone();
        unknown.assignments[0].id = "unknown".into();
        assert!(validate(&catalog, &unknown, false)
            .unwrap_err()
            .contains("unknown ownership"));
        let mut stale = ledger;
        stale.catalog_sha256 = "0".repeat(64);
        assert!(validate(&catalog, &stale, false)
            .unwrap_err()
            .contains("catalog binding"));
    }

    #[test]
    fn ignored_and_conditional_owners_are_rejected() {
        let source = "#[test]\nfn example() {}";
        unconditional_test(source, "mod sample;", "sample", "example").unwrap();
        for attribute in ["#[ignore]", "#[cfg(any())]", "#[cfg_attr(test, ignore)]"] {
            assert!(unconditional_test(
                &format!("{attribute}\n{source}"),
                "mod sample;",
                "sample",
                "example"
            )
            .is_err());
            assert!(unconditional_test(
                source,
                &format!("{attribute}\nmod sample;"),
                "sample",
                "example"
            )
            .is_err());
        }
    }

    #[test]
    fn case_assignment_cannot_claim_unrelated_or_expanded_scope() {
        let (catalog, mut ledger) = fixture();
        ledger.assignments[0].disposition = Disposition::Owned {
            owner: Owner::Case {
                case: "focused.rectangle-outline.v1".into(),
                manifest: descriptor(
                    "tests/conformance/cases/focused/rectangle-outline/manifest.json",
                )
                .unwrap(),
            },
            rationale: "not proof of all diagnostics".into(),
        };
        assert!(validate(&catalog, &ledger, false)
            .unwrap_err()
            .contains("does not establish"));
    }

    #[test]
    fn test_owner_checks_compiled_module_exact_selector_and_source_digest() {
        let (catalog, mut ledger) = load().unwrap();
        validate(&catalog, &ledger, false).unwrap();
        let row = ledger
            .assignments
            .iter_mut()
            .find(|r| r.id == "codec:avif:rejection")
            .unwrap();
        let Disposition::Owned {
            owner: Owner::Test { command, .. },
            ..
        } = &mut row.disposition
        else {
            panic!()
        };
        command[8] = "no_such_test".into();
        assert!(validate(&catalog, &ledger, false)
            .unwrap_err()
            .contains("exact executable test"));
        let (_, mut ledger) = load().unwrap();
        let row = ledger
            .assignments
            .iter_mut()
            .find(|r| r.id == "codec:avif:rejection")
            .unwrap();
        let Disposition::Owned {
            owner: Owner::Test { source, .. },
            ..
        } = &mut row.disposition
        else {
            panic!()
        };
        source.sha256 = "0".repeat(64);
        assert!(validate(&catalog, &ledger, false)
            .unwrap_err()
            .contains("stale ownership evidence"));
    }

    #[test]
    fn exclusion_requires_proof_and_scoped_approval() {
        let (catalog, mut ledger) = fixture();
        let proof = Evidence {
            source: descriptor("docs/renderer-spec-v1.md").unwrap(),
            anchor: "AVIF is not accepted in revision 1".into(),
        };
        ledger.assignments[0].disposition = Disposition::Excluded {
            reason: "fixture exclusion".into(),
            proof,
            approval: Value::Null,
        };
        assert!(validate(&catalog, &ledger, false).is_err());
        let before = subject(&catalog, &ledger).unwrap();
        ledger.approval = json!({"anything":"excluded from subject"});
        assert_eq!(subject(&catalog, &ledger).unwrap(), before);
        ledger.assignments[0].id = "changed".into();
        assert_ne!(subject(&catalog, &ledger).unwrap(), before);
    }
}
