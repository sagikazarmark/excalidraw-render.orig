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

fn may_import_test_attribute(tree: &syn::UseTree) -> bool {
    use syn::ext::IdentExt;
    match tree {
        syn::UseTree::Path(path) => may_import_test_attribute(&path.tree),
        syn::UseTree::Name(name) => name.ident.unraw() == "test",
        syn::UseTree::Rename(rename) => rename.rename.unraw() == "test",
        syn::UseTree::Glob(_) => true,
        syn::UseTree::Group(group) => group.items.iter().any(may_import_test_attribute),
    }
}

fn check_test_attribute_scope(file: &syn::File) -> Result<(), String> {
    // This adapter does not resolve imports or expand macros. Reject scopes
    // that could replace the built-in test attribute instead of guessing.
    for item in &file.items {
        let ambiguous = match item {
            syn::Item::Use(item) => may_import_test_attribute(&item.tree),
            syn::Item::Macro(item) => item.ident.is_none(),
            syn::Item::ExternCrate(item) => {
                item.attrs.iter().any(|a| a.path().is_ident("macro_use"))
            }
            syn::Item::Mod(item) => item.attrs.iter().any(|a| a.path().is_ident("macro_use")),
            _ => false,
        };
        if ambiguous {
            return Err("test owner scope may shadow the built-in test attribute".into());
        }
    }
    Ok(())
}

fn unconditional_test(
    source: &str,
    main: &str,
    module: &str,
    function: &str,
) -> Result<(), String> {
    // Resolve the actual top-level syntax, not textual anchors in comments,
    // strings or nested scopes. Unknown attributes fail closed at the selected
    // module/test boundary; unrelated consuming tests need not share its policy.
    let main = syn::parse_file(main).map_err(|e| format!("invalid test binary source: {e}"))?;
    check_test_attribute_scope(&main)?;
    if main.attrs.iter().any(|attr| !attr.path().is_ident("doc")) {
        return Err("test binary contains conditional attributes".into());
    }
    let modules: Vec<_> = main
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(item) if item.ident == module => Some(item),
            _ => None,
        })
        .collect();
    if modules.len() != 1 || modules[0].content.is_some() {
        return Err("test owner module is not compiled".into());
    }
    if modules[0]
        .attrs
        .iter()
        .any(|attr| !attr.path().is_ident("doc"))
    {
        return Err("test owner module has unsupported attributes".into());
    }
    let source = syn::parse_file(source).map_err(|e| format!("invalid test owner source: {e}"))?;
    check_test_attribute_scope(&source)?;
    if source.attrs.iter().any(|attr| !attr.path().is_ident("doc")) {
        return Err("test owner source contains unsupported attributes".into());
    }
    let functions: Vec<_> = source
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(item) if item.sig.ident == function => Some(item),
            _ => None,
        })
        .collect();
    if functions.len() != 1 {
        return Err("test owner function is not an unconditional test".into());
    }
    let attrs = &functions[0].attrs;
    if attrs
        .iter()
        .filter(|attr| {
            matches!(attr.style, syn::AttrStyle::Outer)
                && matches!(&attr.meta, syn::Meta::Path(path) if path.is_ident("test"))
        })
        .count()
        != 1
        || attrs.iter().any(|attr| {
            !attr.path().is_ident("doc")
                && !(matches!(attr.style, syn::AttrStyle::Outer)
                    && matches!(&attr.meta, syn::Meta::Path(path) if path.is_ident("test")))
        })
    {
        return Err("test owner function is not an unconditional test".into());
    }
    Ok(())
}

fn test_command(module: &str, function: &str) -> Vec<String> {
    [
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
    .collect()
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
                        if descriptor(&source.path)? != *source {
                            return Err("stale ownership evidence".into());
                        }
                        let source_text =
                            String::from_utf8(read(&source.path)?).map_err(|e| e.to_string())?;
                        unconditional_test(&source_text, &main, module, function)?;
                        let args = test_command(module, function);
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

/// Record an explicitly reviewed selection. No inference from field names or
/// source anchors is allowed; every requested identity must still be a gap.
pub fn record_test_owner(
    module: &str,
    function: &str,
    rationale: &str,
    ids: &[String],
) -> Result<(), String> {
    let (catalog, mut ledger) = load()?;
    validate(&catalog, &ledger, false)?;
    if ids.is_empty() || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err("select nonempty, unique ownership identities".into());
    }
    let owner = Owner::Test {
        source: descriptor(&format!("crates/excalidraw-render/tests/fast/{module}.rs"))?,
        function: function.into(),
        binary: "fast".into(),
        command: test_command(module, function),
    };
    for id in ids {
        let row = ledger
            .assignments
            .iter_mut()
            .find(|row| &row.id == id)
            .ok_or_else(|| format!("unknown ownership identity {id}"))?;
        if !matches!(row.disposition, Disposition::Gap { .. }) {
            return Err(format!("ownership already assigned: {id}"));
        }
        row.disposition = Disposition::Owned {
            owner: owner.clone(),
            rationale: rationale.into(),
        };
    }
    validate(&catalog, &ledger, false)?;
    let mut bytes = serde_json::to_vec_pretty(&ledger).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    fs::write(repo_root().join(LEDGER), bytes).map_err(|e| e.to_string())?;
    Ok(())
}

/// Refresh a consuming test's live location binding after editing its source.
/// The selected executable and reviewed rationale are preserved and revalidated.
pub fn refresh_test_owner_source(module: &str) -> Result<(), String> {
    let (catalog, mut ledger) = load()?;
    if !ledger.approval.is_null() {
        return Err(
            "approved ownership requires explicit review before refreshing test sources".into(),
        );
    }
    let source = descriptor(&format!("crates/excalidraw-render/tests/fast/{module}.rs"))?;
    let mut count = 0;
    for assignment in &mut ledger.assignments {
        if let Disposition::Owned {
            owner: Owner::Test {
                source: existing, ..
            },
            ..
        } = &mut assignment.disposition
        {
            if existing.path == source.path {
                *existing = source.clone();
                count += 1;
            }
        }
    }
    if count == 0 {
        return Err("no existing test owners for source".into());
    }
    validate(&catalog, &ledger, false)?;
    let mut bytes = serde_json::to_vec_pretty(&ledger).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    fs::write(repo_root().join(LEDGER), bytes).map_err(|e| e.to_string())?;
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
    fn commented_out_test_is_not_an_executable_owner() {
        for source in [
            "/*\n#[test]\nfn example() {}\n*/",
            "const TEXT: &str = r#\"\n#[test]\nfn example() {}\n\"#;",
            "mod nested {\n#[test]\nfn example() {}\n}",
        ] {
            assert!(unconditional_test(source, "mod sample;", "sample", "example").is_err());
        }
    }

    #[test]
    fn non_compiled_or_redirected_module_is_not_an_executable_owner() {
        let source = "#[test]\nfn example() {}";
        for main in [
            "/*\nmod sample;\n*/",
            "const TEXT: &str = r#\"\nmod sample;\n\"#;",
            "mod other {\nmod sample;\n}",
            "#[cfg(any())]\n\nmod sample;",
            "#[path = \"other.rs\"]\n// a redirect is not the hashed file\nmod sample;",
            "#[path = \"other.rs\"]\n\nmod sample;",
        ] {
            assert!(
                unconditional_test(source, main, "sample", "example").is_err(),
                "accepted noncompiled module: {main}"
            );
        }
    }

    #[test]
    fn inline_conditional_or_ignored_test_is_not_an_executable_owner() {
        for source in [
            "const _: () = (); #[cfg(any())]\n#[test]\nfn example() {}",
            "const _: () = (); #[ignore]\n#[test]\nfn example() {}",
            "const _: () = (); #[cfg_attr(test, ignore)]\n#[test]\nfn example() {}",
        ] {
            assert!(
                unconditional_test(source, "mod sample;", "sample", "example").is_err(),
                "accepted nonexecuting test: {source}"
            );
        }
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
    fn shadowable_or_inner_test_attributes_do_not_establish_ownership() {
        for source in [
            "use custom::discard as test; #[test]\nfn example() {}",
            "use custom::discard as r#test; #[test]\nfn example() {}",
            "use custom::r#test; #[test]\nfn example() {}",
            "use custom::{self, test}; #[test]\nfn example() {}",
            "use custom::*; #[test]\nfn example() {}",
            "inject_test_attribute!(); #[test]\nfn example() {}",
            "fn example() { #![test] }",
            "#[test] fn example() { #![test] }",
        ] {
            assert!(
                unconditional_test(source, "mod sample;", "sample", "example").is_err(),
                "accepted {source}"
            );
        }
    }

    #[test]
    fn owner_selection_uses_syntax_without_restricting_unrelated_tests() {
        unconditional_test(
            "//! Module documentation\n#[ignore]\n#[test]\nfn unrelated() {}\n/// Owner documentation\n#[test] pub fn example () {}",
            "//! Binary documentation\n#[path = \"../common/mod.rs\"] mod common;\n/// Owner module\nmod sample ;",
            "sample",
            "example",
        ).unwrap();
        for source in [
            "#![cfg(any())]\n#[test]\nfn example() {}",
            "#![cfg_attr(test, ignore)]\n#[test]\nfn example() {}",
            "#[test]\nfn example() {}\n#[test]\nfn example() {}",
            "#[test = \"invalid\"]\nfn example() {}",
        ] {
            assert!(unconditional_test(source, "mod sample;", "sample", "example").is_err());
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
