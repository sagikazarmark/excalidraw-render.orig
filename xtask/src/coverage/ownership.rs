//! Global obligation/owner boundary. The execution inventory remains unchanged:
//! selecting ownership never silently regenerates approved renderer snapshots.
use super::*;
use crate::conformance::verify_approval_for_issue;
mod authority_migration;
mod png_profiles;
mod reconciliation;
mod representative;
pub use authority_migration::ownership_authority_migration;
pub use reconciliation::reconcile_stage4_bindings;

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
    Representative {
        binding: representative::Binding,
    },
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
    catalog_with_spec(&read("docs/renderer-spec-v1.md")?)
}
fn catalog_with_spec(spec_bytes: &[u8]) -> Result<Catalog, String> {
    catalog_from_inventory(spec_bytes, load_inventory()?, descriptor(INVENTORY)?)
}
fn catalog_from_inventory(
    spec_bytes: &[u8],
    inventory: Inventory,
    inventory_descriptor: Descriptor,
) -> Result<Catalog, String> {
    let spec_descriptor = Descriptor {
        path: "docs/renderer-spec-v1.md".into(),
        sha256: sha256(spec_bytes),
    };
    let mut policy: Policy = parse(&read(POLICY)?)?;
    if policy.schema != "excalidraw-render/coverage-ownership-policy/v1" {
        return Err("invalid ownership policy schema".into());
    }
    policy
        .rows
        .extend(png_profiles::rows(&read(png_profiles::PATH)?)?);
    let surface: Value = parse(&read(SURFACE)?)?;
    let surface_descriptor = descriptor(SURFACE)?;
    let mut obligations = Vec::new();
    for row in &inventory.coverage {
        if row.id.starts_with("case-label:") {
            obligations.push(Obligation{id:row.id.clone(),category:"case-label".into(),
                criterion:format!("Primary evidence for the existing local label {}. Format/profile sub-obligations are separate.",row.id),
                authority:inventory_descriptor.clone(),pointer:"/coverage".into(),candidates:row.candidates.clone()});
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
        inventory_descriptor,
        surface_descriptor,
        descriptor(POLICY)?,
        descriptor(png_profiles::PATH)?,
        spec_descriptor.clone(),
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
    let spec = std::str::from_utf8(spec_bytes).map_err(|e| e.to_string())?;
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
            authority: spec_descriptor.clone(),
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
    executable_test(source, main, module, function, false)
}

fn executable_test(
    source: &str,
    main: &str,
    module: &str,
    function: &str,
    library: bool,
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
    let test_cfg = |attr: &syn::Attribute| {
        matches!(attr.style, syn::AttrStyle::Outer)
            && attr.path().is_ident("cfg")
            && attr.parse_args::<syn::Ident>().is_ok_and(|id| id == "test")
    };
    if library
        && modules[0]
            .attrs
            .iter()
            .filter(|attr| test_cfg(attr))
            .count()
            != 1
    {
        return Err("library test owner module must use exactly cfg(test)".into());
    }
    if modules[0]
        .attrs
        .iter()
        .any(|attr| !attr.path().is_ident("doc") && !(library && test_cfg(attr)))
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

#[cfg(test)]
fn test_command(module: &str, function: &str) -> Vec<String> {
    owner_test_command("fast", module, function)
}
fn owner_test_command(binary: &str, module: &str, function: &str) -> Vec<String> {
    let mut args: Vec<_> = [
        "cargo",
        "test",
        "--locked",
        "--release",
        "-p",
        "excalidraw-render",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    if binary == "lib" {
        args.push("--lib".into());
    } else {
        args.extend(["--test".into(), "fast".into()]);
    }
    args.extend([
        format!("{module}::{function}"),
        "--".into(),
        "--exact".into(),
    ]);
    args
}

/// This adapter deliberately supports the repository's default library target
/// and explicit workspace membership, not arbitrary Cargo target redirection.
fn library_layout(package: &str, workspace: &str) -> Result<(), String> {
    let package: toml::Value =
        toml::from_str(package).map_err(|e| format!("invalid renderer manifest: {e}"))?;
    let workspace: toml::Value =
        toml::from_str(workspace).map_err(|e| format!("invalid workspace manifest: {e}"))?;
    let lib = package.get("lib");
    if package
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        != Some("excalidraw-render")
        || package
            .get("package")
            .and_then(|p| p.get("autolib"))
            .and_then(toml::Value::as_bool)
            == Some(false)
        || lib.is_some_and(|l| l.as_table().is_none())
        || lib
            .and_then(|l| l.get("path"))
            .is_some_and(|p| p.as_str() != Some("src/lib.rs"))
        || ["test", "harness"].iter().any(|key| {
            lib.and_then(|l| l.get(key))
                .is_some_and(|v| v.as_bool() != Some(true))
        })
        || lib
            .and_then(|l| l.get("proc-macro"))
            .is_some_and(|v| v.as_bool() != Some(false))
    {
        return Err("unsupported library test target layout".into());
    }
    let workspace = workspace
        .get("workspace")
        .ok_or("missing workspace target layout")?;
    if workspace.get("exclude").is_some()
        || !workspace
            .get("members")
            .and_then(toml::Value::as_array)
            .is_some_and(|members| {
                members
                    .iter()
                    .any(|m| m.as_str() == Some("crates/excalidraw-render"))
            })
    {
        return Err(
            "library test target requires explicit, unexcluded workspace membership".into(),
        );
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
                    Owner::Representative { binding } => {
                        if row.category != "interaction" {
                            return Err(
                                "representative adapter requires the interaction obligation".into(),
                            );
                        }
                        representative::verify(&row.id, binding)?;
                    }
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
                        let (prefix, entry, library) = match binary.as_str() {
                            "fast" => (
                                "crates/excalidraw-render/tests/fast/",
                                "crates/excalidraw-render/tests/fast/main.rs",
                                false,
                            ),
                            "lib" => {
                                library_layout(
                                    &String::from_utf8(read(
                                        "crates/excalidraw-render/Cargo.toml",
                                    )?)
                                    .map_err(|e| e.to_string())?,
                                    &String::from_utf8(read("Cargo.toml")?)
                                        .map_err(|e| e.to_string())?,
                                )?;
                                (
                                    "crates/excalidraw-render/src/",
                                    "crates/excalidraw-render/src/lib.rs",
                                    true,
                                )
                            }
                            _ => return Err("unsupported executable owner path/binary".into()),
                        };
                        if !source.path.starts_with(prefix) || !source.path.ends_with(".rs") {
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
                        let main = String::from_utf8(read(entry)?).map_err(|e| e.to_string())?;
                        if descriptor(&source.path)? != *source {
                            return Err("stale ownership evidence".into());
                        }
                        let source_text =
                            String::from_utf8(read(&source.path)?).map_err(|e| e.to_string())?;
                        if library {
                            executable_test(&source_text, &main, module, function, true)?;
                        } else {
                            unconditional_test(&source_text, &main, module, function)?;
                        }
                        let args = owner_test_command(binary, module, function);
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

/// An append-only inventory expansion: the prior catalog is authenticated by
/// the existing ledger, every prior obligation/disposition is preserved, and
/// new obligations enter only as explicit gaps. Assignment remains a separate
/// reviewed operation.
fn extended_ledger(
    before: &Catalog,
    after: &Catalog,
    ledger: &Ledger,
    issue: u64,
    reason: &str,
) -> Result<Ledger, String> {
    if !ledger.approval.is_null() {
        return Err(
            "approved ownership requires a new reviewed decision, not an inventory extension"
                .into(),
        );
    }
    if issue == 0 || !nonempty(reason) {
        return Err("inventory extension requires an issue and concrete gap reason".into());
    }
    if before.schema != "excalidraw-render/coverage-obligations/v1" || after.schema != before.schema
    {
        return Err("invalid extension catalog schema".into());
    }
    validate(before, ledger, false)?;
    let previous: BTreeMap<_, _> = before
        .obligations
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    let current: BTreeMap<_, _> = after
        .obligations
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    for (id, row) in &previous {
        if current.get(id) != Some(row) {
            return Err(format!(
                "inventory extension changed or removed existing obligation: {id}"
            ));
        }
    }
    let additions: Vec<_> = after
        .obligations
        .iter()
        .filter(|r| !previous.contains_key(r.id.as_str()))
        .collect();
    if additions.is_empty() {
        return Err("inventory extension has no new obligations".into());
    }
    let mut extended = ledger.clone();
    for row in additions {
        extended.assignments.push(Assignment {
            id: row.id.clone(),
            disposition: Disposition::Gap {
                issue,
                reason: reason.into(),
            },
        });
    }
    extended.assignments.sort_by(|a, b| a.id.cmp(&b.id));
    extended.catalog_sha256 = digest(after)?;
    validate(after, &extended, false)?;
    Ok(extended)
}

pub fn extend_ownership(previous_catalog: &str, issue: u64, reason: &str) -> Result<(), String> {
    let (current, ledger) = load()?;
    let previous: Catalog = parse(&fs::read(previous_catalog).map_err(|e| e.to_string())?)?;
    let extended = extended_ledger(&previous, &current, &ledger, issue, reason)?;
    let mut bytes = serde_json::to_vec_pretty(&extended).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    fs::write(repo_root().join(LEDGER), bytes).map_err(|e| e.to_string())?;
    println!(
        "extended ownership inventory: preserved {} dispositions, added {} explicit gaps",
        ledger.assignments.len(),
        extended.assignments.len() - ledger.assignments.len()
    );
    Ok(())
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
    record_owner("fast", module, function, rationale, ids)
}
pub fn record_library_test_owner(
    module: &str,
    function: &str,
    rationale: &str,
    ids: &[String],
) -> Result<(), String> {
    record_owner("lib", module, function, rationale, ids)
}
pub fn verify_representative_owner(case: &str) -> Result<(), String> {
    representative::approved_owner(case).map(|_| ())
}
pub fn record_representative_owner(case: &str, rationale: &str) -> Result<(), String> {
    if !nonempty(rationale) {
        return Err("ownership rationale is empty".into());
    }
    // Refuse an unknown, dead, stale or unapproved candidate before any ledger
    // mutation, including when unrelated catalog bindings are currently stale.
    let owner = representative::approved_owner(case)?;
    let (catalog, mut ledger) = load()?;
    if !ledger.approval.is_null() {
        return Err("approved ownership requires a new reviewed decision".into());
    }
    validate(&catalog, &ledger, false)?;
    let assignment = ledger
        .assignments
        .iter_mut()
        .find(|r| r.id == representative::OBLIGATION)
        .ok_or("missing representative interaction obligation")?;
    if !matches!(assignment.disposition, Disposition::Gap { .. }) {
        return Err("representative interaction already assigned".into());
    }
    assignment.disposition = Disposition::Owned {
        owner,
        rationale: rationale.into(),
    };
    validate(&catalog, &ledger, false)?;
    let mut bytes = serde_json::to_vec_pretty(&ledger).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    fs::write(repo_root().join(LEDGER), bytes).map_err(|e| e.to_string())
}
fn record_owner(
    binary: &str,
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
        source: descriptor(&format!(
            "crates/excalidraw-render/{}/{module}.rs",
            if binary == "lib" { "src" } else { "tests/fast" }
        ))?,
        function: function.into(),
        binary: binary.into(),
        command: owner_test_command(binary, module, function),
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
    refresh_owner_source("fast", module)
}
pub fn refresh_library_test_owner_source(module: &str) -> Result<(), String> {
    refresh_owner_source("lib", module)
}
fn refresh_owner_source(binary: &str, module: &str) -> Result<(), String> {
    let (catalog, mut ledger) = load()?;
    if !ledger.approval.is_null() {
        return Err(
            "approved ownership requires explicit review before refreshing test sources".into(),
        );
    }
    let source = descriptor(&format!(
        "crates/excalidraw-render/{}/{module}.rs",
        if binary == "lib" { "src" } else { "tests/fast" }
    ))?;
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

    #[test]
    fn inventory_extension_preserves_dispositions_and_only_adds_gaps() {
        let (before, ledger) = fixture();
        let mut after = before.clone();
        let mut row = before.obligations[0].clone();
        row.id = "new-profile".into();
        after.obligations.push(row);
        let extended = extended_ledger(
            &before,
            &after,
            &ledger,
            85,
            "independent profile vector missing",
        )
        .unwrap();
        assert_eq!(extended.assignments.len(), 2);
        let original = extended
            .assignments
            .iter()
            .find(|r| r.id == ledger.assignments[0].id)
            .unwrap();
        assert_eq!(
            serde_json::to_value(original).unwrap(),
            serde_json::to_value(&ledger.assignments[0]).unwrap()
        );
        assert_eq!(validate(&after, &extended, false).unwrap(), 2);
        assert!(extended.approval.is_null());
        assert!(extended_ledger(&before, &before, &ledger, 85, "gap").is_err());
        let mut changed = after.clone();
        changed.obligations[0].criterion.push_str(" widened");
        assert!(extended_ledger(&before, &changed, &ledger, 85, "gap")
            .unwrap_err()
            .contains("changed or removed"));
        let mut removed = after.clone();
        removed.obligations.remove(0);
        assert!(extended_ledger(&before, &removed, &ledger, 85, "gap").is_err());
        let mut stale = before.clone();
        stale.obligations[0].criterion.push_str(" forged");
        assert!(extended_ledger(&stale, &after, &ledger, 85, "gap").is_err());
        let mut approved = ledger.clone();
        approved.approval = json!({"existing":"approval"});
        assert!(extended_ledger(&before, &after, &approved, 85, "gap").is_err());
    }

    #[test]
    fn inventory_extension_preserves_live_owners_and_approved_exclusions() {
        let (before, ledger) = load().unwrap();
        let mut after = before.clone();
        let mut row = before
            .obligations
            .iter()
            .find(|r| r.id == "codec:png:malformed")
            .unwrap()
            .clone();
        row.id = "codec:png:test-only-extension".into();
        after.obligations.push(row);
        let extended = extended_ledger(
            &before,
            &after,
            &ledger,
            85,
            "test-only missing profile evidence",
        )
        .unwrap();
        let indexed: BTreeMap<_, _> = extended
            .assignments
            .iter()
            .map(|row| (&row.id, row))
            .collect();
        assert!(ledger
            .assignments
            .iter()
            .any(|r| matches!(r.disposition, Disposition::Excluded { .. })));
        assert!(ledger
            .assignments
            .iter()
            .any(|r| matches!(r.disposition, Disposition::Owned { .. })));
        for original in &ledger.assignments {
            assert_eq!(
                serde_json::to_value(original).unwrap(),
                serde_json::to_value(indexed[&original.id]).unwrap()
            );
        }
        assert_eq!(extended.assignments.len(), ledger.assignments.len() + 1);
    }

    #[test]
    fn library_adapter_requires_a_real_unconditional_library_test() {
        let main = "#[cfg(test)] mod sample;";
        let source = "#[test] fn example() {}";
        executable_test(source, main, "sample", "example", true).unwrap();
        for main in [
            "mod sample;",
            "#[cfg(feature=\"hidden\")] mod sample;",
            "#[cfg(all(test, unix))] mod sample;",
            "#[cfg(test)] #[path=\"elsewhere.rs\"] mod sample;",
            "#[cfg(test)] mod sample {}",
            "#[cfg(test)] #[cfg(test)] mod sample;",
        ] {
            assert!(
                executable_test(source, main, "sample", "example", true).is_err(),
                "{main}"
            );
        }
        for source in [
            "// #[test] fn example() {}",
            "#[ignore] #[test] fn example() {}",
            "#[cfg(test)] #[test] fn example() {}",
            "use custom::*; #[test] fn example() {}",
            "#[test] fn example() {} #[test] fn example() {}",
        ] {
            assert!(
                executable_test(source, main, "sample", "example", true).is_err(),
                "{source}"
            );
        }
        assert_ne!(
            owner_test_command("lib", "sample", "example"),
            test_command("sample", "example")
        );
        assert_eq!(
            owner_test_command("lib", "sample", "example"),
            [
                "cargo",
                "test",
                "--locked",
                "--release",
                "-p",
                "excalidraw-render",
                "--lib",
                "sample::example",
                "--",
                "--exact"
            ]
        );
    }

    #[test]
    fn library_adapter_rejects_disabled_redirected_and_custom_harness_targets() {
        let package = "[package]\nname='excalidraw-render'\n";
        let workspace = "[workspace]\nmembers=['crates/excalidraw-render']\n";
        library_layout(package, workspace).unwrap();
        library_layout(
            &format!("{package}[lib]\npath='src/lib.rs'\ntest=true\nharness=true\n"),
            workspace,
        )
        .unwrap();
        for suffix in [
            "autolib=false\n",
            "[lib]\npath='src/other.rs'\n",
            "[lib]\ntest=false\n",
            "[lib]\nharness=false\n",
            "[lib]\nproc-macro=true\n",
            "[[lib]]\npath='src/lib.rs'\n",
        ] {
            assert!(
                library_layout(&format!("{package}{suffix}"), workspace).is_err(),
                "{suffix}"
            );
        }
        for workspace in ["[workspace]\nmembers=['elsewhere']","[workspace]\nmembers=['crates/*']","[workspace]\nmembers=['crates/excalidraw-render']\nexclude=['crates/excalidraw-render']"] {
            assert!(library_layout(package,workspace).is_err());
        }
    }

    #[test]
    fn library_owner_binds_source_digest_and_exact_executable_command() {
        let (catalog, mut ledger) = fixture();
        let module = "metadata_failure_contract";
        let function = "metadata_failures_have_exact_recovery_in_every_format";
        ledger.assignments[0].disposition = Disposition::Owned {
            owner: Owner::Test {
                source: descriptor("crates/excalidraw-render/src/metadata_failure_contract.rs")
                    .unwrap(),
                function: function.into(),
                binary: "lib".into(),
                command: owner_test_command("lib", module, function),
            },
            rationale: "reviewed fault contract".into(),
        };
        assert_eq!(validate(&catalog, &ledger, false).unwrap(), 0);
        let mut stale = ledger.clone();
        if let Disposition::Owned {
            owner: Owner::Test { source, .. },
            ..
        } = &mut stale.assignments[0].disposition
        {
            source.sha256 = "0".repeat(64);
        }
        assert!(validate(&catalog, &stale, false)
            .unwrap_err()
            .contains("stale ownership evidence"));
        let mut redirected = ledger.clone();
        if let Disposition::Owned {
            owner: Owner::Test { command, .. },
            ..
        } = &mut redirected.assignments[0].disposition
        {
            *command = test_command(module, function);
        }
        assert!(validate(&catalog, &redirected, false)
            .unwrap_err()
            .contains("exact executable test"));
    }

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
