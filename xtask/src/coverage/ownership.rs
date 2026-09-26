//! Global obligation/owner boundary. The execution inventory remains unchanged:
//! selecting ownership never silently regenerates approved renderer snapshots.
use super::*;
mod authority_migration;
mod executable;
mod png_profiles;
mod reconciliation;
mod record;
mod representative;
mod test_group;
mod view;
pub use authority_migration::ownership_authority_migration;
pub use reconciliation::reconcile_stage4_bindings;
pub use test_group::filter as test_group_filter;
pub(super) use view::{LedgerView, PrimaryOwner};

const POLICY: &str = "tests/conformance/coverage/ownership-policy.json";
const LEDGER: &str = "tests/conformance/coverage/ownership.json";
const SCHEMA: &str = "excalidraw-render/coverage-ownership/v1";
/// The one root test target an owner may select: registered-case direct PNG
/// parity, which holds each whole-artifact PNG tolerance exception at its exact
/// ceilings and fails a stale one that the canonical budget would pass.
const EXCEPTION_BINARY: &str = "parity_registered_case_png";
const EXCEPTION_FUNCTION: &str = "every_registered_scene_case_matches_its_official_direct_png";

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
    /// Registered cases that may own a `layer` row; no other category names any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    candidates: Vec<String>,
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
    /// Membership is a checked manifest of unconditional tests. The normal CI
    /// runner schedules each member once, rather than an umbrella re-running it.
    TestGroup {
        group: Descriptor,
    },
    Representative {
        binding: representative::Binding,
    },
    Case {
        case: String,
        manifest: Descriptor,
    },
    /// An executable test. The compact form (no `package`/`module`) is the
    /// original adapter: `fast` and `lib` flat modules of the renderer and the
    /// direct PNG exception binary, the module derived from the source path.
    /// Every other target names its package and full module path.
    Test {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        package: Option<String>,
        source: Descriptor,
        function: String,
        binary: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        module: Option<String>,
        command: Vec<String>,
    },
    /// ADR-0015: an approved judgment record, for closure obligations only,
    /// checked by a compiled-in verifier registered by the record's schema.
    Record {
        record: Descriptor,
        approval_subject_sha256: String,
        verifier: String,
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
    let registered: BTreeSet<_> = array(&registry, "cases")?
        .iter()
        .filter_map(|case| case["id"].as_str())
        .collect();
    for row in policy.rows {
        if !policy_row_valid(&row, spec, &registered) {
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
            candidates: row.candidates,
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

/// A policy row is one of the explicit categories, anchored in the spec. Only
/// `layer` rows (all `layer:`-prefixed) may name candidate cases, each
/// registered and listed once.
fn policy_row_valid(row: &PolicyRow, spec: &str, registered: &BTreeSet<&str>) -> bool {
    let layer = row.category == "layer";
    matches!(
        row.category.as_str(),
        "codec-profile" | "interaction" | "closure" | "layer"
    ) && layer == row.id.starts_with("layer:")
        && (layer || row.candidates.is_empty())
        && row
            .candidates
            .iter()
            .all(|case| registered.contains(case.as_str()))
        && row.candidates.iter().collect::<BTreeSet<_>>().len() == row.candidates.len()
        && nonempty(&row.criterion)
        && nonempty(&row.spec_anchor)
        && spec.contains(&row.spec_anchor)
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

/// Read a repository text file for the executable adapter; absence is a
/// resolution outcome, never a panic.
fn read_text(path: &str) -> Option<String> {
    read(path)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

/// The effective package and module path of a test owner. The compact form
/// is the original adapter and derives its single flat module from the
/// source path; the explicit form is required for every other target and is
/// refused where the compact form would express the same owner.
fn test_selector(
    package: Option<&str>,
    source: &Descriptor,
    binary: &str,
    module: Option<&str>,
) -> Result<(String, String), String> {
    let flat = |binary: &str, module: &str| match binary {
        "fast" => Some(format!("crates/excalidraw-render/tests/fast/{module}.rs")),
        "lib" => Some(format!("crates/excalidraw-render/src/{module}.rs")),
        _ => None,
    };
    match (package, module) {
        (None, None) => {
            let prefix = match binary {
                EXCEPTION_BINARY => {
                    if source.path != owner_source(EXCEPTION_BINARY, EXCEPTION_BINARY) {
                        return Err("unsupported executable owner path/binary".into());
                    }
                    return Ok((executable::RENDERER.into(), String::new()));
                }
                "fast" => "crates/excalidraw-render/tests/fast/",
                "lib" => "crates/excalidraw-render/src/",
                _ => return Err("unsupported executable owner path/binary".into()),
            };
            let module = source
                .path
                .strip_prefix(prefix)
                .and_then(|m| m.strip_suffix(".rs"))
                .ok_or("unsupported executable owner path/binary")?;
            if !executable::valid_name(module) {
                return Err("nested test module must have an explicit adapter".into());
            }
            Ok((executable::RENDERER.into(), module.into()))
        }
        (Some(package), Some(module)) => {
            let compact = package == executable::RENDERER
                && match binary {
                    EXCEPTION_BINARY => module.is_empty(),
                    _ => {
                        executable::valid_name(module)
                            && flat(binary, module).as_deref() == Some(source.path.as_str())
                    }
                };
            if compact {
                return Err(
                    "an owner the compact adapter expresses must use the compact form".into(),
                );
            }
            Ok((package.into(), module.into()))
        }
        _ => Err("test owner package and module are named together or not at all".into()),
    }
}

/// Resolve an executable owner's selector against the compiled module tree
/// and return the file holding the selected module.
fn resolve_test_owner(
    package: &str,
    binary: &str,
    module: &str,
    function: &str,
) -> Result<String, String> {
    let root = executable::target_root(package, binary, &read_text)?;
    executable::resolve(&root, module, function, binary == "lib", &read_text)
}

/// Build the canonical owner for a reviewed selector.
fn test_owner(package: &str, binary: &str, module: &str, function: &str) -> Result<Owner, String> {
    let path = resolve_test_owner(package, binary, module, function)?;
    let source = descriptor(&path)?;
    let explicit = test_selector(Some(package), &source, binary, Some(module)).is_ok();
    let owner = Owner::Test {
        package: explicit.then(|| package.into()),
        source,
        function: function.into(),
        binary: binary.into(),
        module: explicit.then(|| module.into()),
        command: executable::command(package, binary, module, function),
    };
    Ok(owner)
}

fn check_test_owner(
    package: Option<&str>,
    source: &Descriptor,
    function: &str,
    binary: &str,
    module: Option<&str>,
    command: &[String],
) -> Result<(), String> {
    if !executable::valid_name(function) || !executable::valid_name(binary) {
        return Err("invalid test owner selector".into());
    }
    let (package, module) = test_selector(package, source, binary, module)?;
    let resolved = resolve_test_owner(&package, binary, &module, function)?;
    if resolved != source.path {
        return Err("test owner source is not the selected compiled module".into());
    }
    // The digest records the source when this owner was selected. Resolve the
    // test against today's tree above; unrelated edits need no ledger refresh.
    if !hex(&source.sha256, 64) {
        return Err("invalid historical test source digest".into());
    }
    if command != executable::command(&package, binary, &module, function) {
        return Err("test owner command does not select its exact executable test".into());
    }
    Ok(())
}

/// The original flat-module adapter over an in-memory crate: `main` is the
/// crate root and `source` the selected module's file.
#[cfg(test)]
fn executable_test(
    source: &str,
    main: &str,
    module: &str,
    function: &str,
    library: bool,
) -> Result<(), String> {
    let read = executable::memory(&[
        ("crate/root.rs", main),
        (&format!("crate/{module}.rs"), source),
    ]);
    let resolved = executable::resolve("crate/root.rs", module, function, library, &read)?;
    if resolved != format!("crate/{module}.rs") {
        return Err("test owner module is not compiled".into());
    }
    Ok(())
}

#[cfg(test)]
fn unconditional_test(
    source: &str,
    main: &str,
    module: &str,
    function: &str,
) -> Result<(), String> {
    executable_test(source, main, module, function, false)
}

/// The selected function is an unconditional top-level test of `source`.
#[cfg(test)]
fn crate_root_test(source: &str, function: &str) -> Result<(), String> {
    let read = executable::memory(&[("crate/root.rs", source)]);
    executable::resolve("crate/root.rs", "", function, false, &read).map(|_| ())
}

#[cfg(test)]
fn test_command(module: &str, function: &str) -> Vec<String> {
    owner_test_command("fast", module, function)
}
/// The compact adapter's command: the exception binary selects its root test.
#[cfg(test)]
fn owner_test_command(binary: &str, module: &str, function: &str) -> Vec<String> {
    let module = if binary == EXCEPTION_BINARY {
        ""
    } else {
        module
    };
    executable::command(executable::RENDERER, binary, module, function)
}

/// This adapter deliberately supports the repository's default library target
/// and explicit workspace membership, not arbitrary Cargo target redirection.
#[cfg(test)]
fn library_layout(package: &str, workspace: &str) -> Result<(), String> {
    executable::package_layout(executable::RENDERER, package, workspace, true)
}

/// The exception adapter's scope: the obligation's exception is the only
/// whole-artifact direct PNG exception of an existing variant of a registered
/// scene case, with no viewer profile, which is exactly what the owning test
/// selects and judges.
fn exception_scope(row: &Obligation) -> Result<(), String> {
    let invalid = || format!("exception outside the direct PNG adapter scope: {}", row.id);
    if descriptor(&row.authority.path)? != row.authority {
        return Err(format!("stale exception authority for {}", row.id));
    }
    let manifest: Value = parse(&read(&row.authority.path)?)?;
    let exception = manifest.pointer(&row.pointer).ok_or_else(invalid)?;
    let variant = string(exception, "variant")?;
    let [case] = row.candidates.as_slice() else {
        return Err(invalid());
    };
    let registry: Value = parse(&read(REGISTRY)?)?;
    let scene = array(&registry, "cases")?.iter().any(|entry| {
        entry["id"] == case.as_str()
            && entry["kind"] == "scene"
            && entry["path"]
                .as_str()
                .map(|path| format!("{path}/manifest.json"))
                == Some(row.authority.path.clone())
    });
    let scoped =
        |e: &Value| e["variant"] == variant && e["format"] == "png" && e["region"] == "whole";
    if !scene
        || !scoped(exception)
        || !exception["viewer_profile"].is_null()
        || row.id != format!("exception:{case}:{variant}:png:whole")
        || array(&manifest, "exceptions")?
            .iter()
            .filter(|e| scoped(e))
            .count()
            != 1
        || !array(&manifest, "variants")?
            .iter()
            .any(|v| v["id"] == variant)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Every check `validate` applies to one assignment; `Ok(true)` is a gap.
/// Shared with the five-layer record, which checks the owners of the rows it
/// maps without validating (and so recursing into) the closure rows.
fn check_disposition(row: &Obligation, disposition: &Disposition) -> Result<bool, String> {
    match disposition {
        Disposition::Gap { issue, reason } => {
            if *issue == 0 || !nonempty(reason) {
                return Err("gap must name an issue and reason".into());
            }
        }
        Disposition::Owned { owner, rationale } => {
            if !nonempty(rationale) {
                return Err("ownership rationale is empty".into());
            }
            if row.id.starts_with("closure:") != matches!(owner, Owner::Record { .. }) {
                return Err(format!(
                    "closure obligations are owned only by approved judgment records, and records own only closure obligations: {}",
                    row.id
                ));
            }
            let exception_owner = matches!(owner, Owner::Test { package: None, binary, function, .. }
                if binary == EXCEPTION_BINARY && function == EXCEPTION_FUNCTION);
            if (row.category == "tolerance-exception") != exception_owner {
                return Err(format!(
                    "tolerance exceptions are owned only by {EXCEPTION_BINARY}::{EXCEPTION_FUNCTION}: {}",
                    row.id
                ));
            }
            if exception_owner {
                exception_scope(row)?;
            }
            match owner {
                Owner::TestGroup { group } => test_group::verify(group)?,
                Owner::Representative { binding } => {
                    if row.category != "interaction" {
                        return Err(
                            "representative adapter requires the interaction obligation".into()
                        );
                    }
                    representative::verify(&row.id, binding)?;
                }
                Owner::Case { case, manifest } => {
                    // A case owns its own scoped label, or a layer row whose
                    // single candidate it is.
                    let establishes = match row.category.as_str() {
                        "case-label" => row.candidates.contains(case),
                        "layer" => row.candidates == std::slice::from_ref(case),
                        _ => false,
                    };
                    if !establishes {
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
                    package,
                    source,
                    function,
                    binary,
                    module,
                    command,
                } => check_test_owner(
                    package.as_deref(),
                    source,
                    function,
                    binary,
                    module.as_deref(),
                    command,
                )?,
                Owner::Record {
                    record,
                    approval_subject_sha256,
                    verifier,
                } => record::verify(&row.id, record, approval_subject_sha256, verifier)?,
            }
        }
        Disposition::Excluded { reason, proof, .. } => {
            if !nonempty(reason) {
                return Err("exclusion reason is empty".into());
            }
            check_evidence(proof)?;
        }
    }
    Ok(matches!(disposition, Disposition::Gap { .. }))
}

fn validate(catalog: &Catalog, ledger: &Ledger, closed: bool) -> Result<usize, String> {
    validate_with_layers(catalog, ledger, closed, closed)
}

fn advisory_layer(row: &Obligation) -> bool {
    row.id == "closure:all-five-layers" || row.category == "layer"
}

fn validate_with_layers(
    catalog: &Catalog,
    ledger: &Ledger,
    closed: bool,
    audit_layers: bool,
) -> Result<usize, String> {
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
        // Five-layer mappings are advisory. Keep historical rows in the ledger,
        // but ordinary verification does not demand current mappings or a
        // regenerated/approved layers.json. Explicit closure audits stay strict.
        if !audit_layers && advisory_layer(row) {
            continue;
        }
        if check_disposition(row, &assignment.disposition)? {
            gaps += 1;
        }
    }
    if expected
        .values()
        .any(|row| (audit_layers || !advisory_layer(row)) && !seen.contains(row.id.as_str()))
    {
        return Err("missing ownership assignments (use explicit gaps)".into());
    }
    if closed && gaps > 0 {
        return Err(format!("coverage ownership has {gaps} unresolved gaps"));
    }
    Ok(gaps)
}

#[cfg(test)]
fn subject(catalog: &Catalog, ledger: &Ledger) -> Result<String, String> {
    let mut value = serde_json::to_value(ledger).map_err(|e| e.to_string())?;
    value.as_object_mut().unwrap().remove("approval");
    // Test source hashes are historical provenance, not judgment-bearing output.
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
        crate::conformance::verify(&case)?;
    }
    // These checks previously ran as a side effect of five-layer completeness.
    // Preserve the actual registered representative evidence checks separately.
    layers::verify_representative_cases()?;
    let advisory = ledger
        .assignments
        .iter()
        .filter(|a| {
            catalog
                .obligations
                .iter()
                .any(|row| row.id == a.id && advisory_layer(row))
        })
        .count();
    println!("verified {} non-layer ownership assignments: {} assigned/excluded, {gaps} explicit gaps; {advisory} five-layer mappings advisory; ledger approval {}",
        ledger.assignments.len()-advisory,ledger.assignments.len()-advisory-gaps,if ledger.approval.is_null(){"pending"}else{"recorded"});
    Ok(())
}

/// Record an explicitly reviewed selection. No inference from field names or
/// source anchors is allowed; every requested identity must still be a gap.
/// `module` is the full module path from the target's crate root (empty for
/// the root of a test binary); the recorded owner resolves it exactly.
pub fn record_test_owner(
    package: &str,
    binary: &str,
    module: &str,
    function: &str,
    rationale: &str,
    ids: &[String],
) -> Result<(), String> {
    record_owner(package, binary, module, function, rationale, ids)
}
pub fn record_library_test_owner(
    module: &str,
    function: &str,
    rationale: &str,
    ids: &[String],
) -> Result<(), String> {
    record_owner(
        executable::RENDERER,
        "lib",
        module,
        function,
        rationale,
        ids,
    )
}
/// Assign `layer` rows to the registered case that is their single candidate;
/// the case is checked through the same registration/manifest binding as a
/// case-label owner and executed by `verify-ownership`.
pub fn record_case_owner(case: &str, rationale: &str, ids: &[String]) -> Result<(), String> {
    if let Some(id) = ids.iter().find(|id| !id.starts_with("layer:")) {
        return Err(format!(
            "case owners are recorded here only for layer rows: {id}"
        ));
    }
    let registry: Value = parse(&read(REGISTRY)?)?;
    let entry = array(&registry, "cases")?
        .iter()
        .find(|e| e["id"] == case)
        .ok_or("owner case is not registered")?;
    let owner = Owner::Case {
        case: case.into(),
        manifest: descriptor(&format!("{}/manifest.json", string(entry, "path")?))?,
    };
    assign_owner(owner, rationale, ids)
}
/// Assign tolerance exceptions to the registered-case direct PNG test, which
/// holds each at its exact ceilings and rejects it once stale.
pub fn record_exception_test_owner(rationale: &str, ids: &[String]) -> Result<(), String> {
    record_owner(
        executable::RENDERER,
        EXCEPTION_BINARY,
        "",
        EXCEPTION_FUNCTION,
        rationale,
        ids,
    )
}
pub fn refresh_exception_test_owner_source() -> Result<(), String> {
    refresh_owner_sources(EXCEPTION_BINARY, &[EXCEPTION_BINARY])
}
fn owner_source(binary: &str, module: &str) -> String {
    match binary {
        "lib" => format!("crates/excalidraw-render/src/{module}.rs"),
        EXCEPTION_BINARY => format!("crates/excalidraw-render/tests/{module}.rs"),
        _ => format!("crates/excalidraw-render/tests/fast/{module}.rs"),
    }
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
    package: &str,
    binary: &str,
    module: &str,
    function: &str,
    rationale: &str,
    ids: &[String],
) -> Result<(), String> {
    assign_owner(
        test_owner(package, binary, module, function)?,
        rationale,
        ids,
    )
}
fn assign_owner(owner: Owner, rationale: &str, ids: &[String]) -> Result<(), String> {
    let (catalog, mut ledger) = load()?;
    validate(&catalog, &ledger, false)?;
    assign_gaps(&mut ledger, &owner, rationale, ids)?;
    validate(&catalog, &ledger, false)?;
    write_ledger(&ledger)
}
/// Replace explicit gaps, and only gaps, with one reviewed owner.
fn assign_gaps(
    ledger: &mut Ledger,
    owner: &Owner,
    rationale: &str,
    ids: &[String],
) -> Result<(), String> {
    if ids.is_empty() || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err("select nonempty, unique ownership identities".into());
    }
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
    Ok(())
}

/// Explicitly record newer test-source provenance in an unapproved ledger.
/// Ordinary source edits do not require this legacy maintenance operation.
/// The selected executable and reviewed rationale are preserved and revalidated.
pub fn refresh_test_owner_source(module: &str) -> Result<(), String> {
    refresh_owner_sources("fast", &[module])
}
pub fn refresh_test_owner_sources(modules: &[String]) -> Result<(), String> {
    refresh_owner_sources(
        "fast",
        &modules.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}
pub fn refresh_library_test_owner_source(module: &str) -> Result<(), String> {
    refresh_owner_sources("lib", &[module])
}
fn refresh_owner_sources(binary: &str, modules: &[&str]) -> Result<(), String> {
    if modules.is_empty() || modules.iter().any(|m| !executable::valid_name(m)) {
        return Err("source refresh requires valid module names".into());
    }
    let (catalog, mut ledger) = load()?;
    let sources = modules
        .iter()
        .map(|module| descriptor(&owner_source(binary, module)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut refreshed = BTreeSet::new();
    for assignment in &mut ledger.assignments {
        if let Disposition::Owned {
            owner:
                Owner::Test {
                    package: None,
                    module: None,
                    source: existing,
                    ..
                },
            ..
        } = &mut assignment.disposition
        {
            if let Some(source) = sources.iter().find(|s| s.path == existing.path) {
                *existing = source.clone();
                refreshed.insert(source.path.clone());
            }
        }
    }
    if sources.iter().any(|s| !refreshed.contains(&s.path)) {
        return Err("no existing test owners for one or more sources".into());
    }
    validate(&catalog, &ledger, false)?;
    let mut bytes = serde_json::to_vec_pretty(&ledger).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    fs::write(repo_root().join(LEDGER), bytes).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_ledger(ledger: &Ledger) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(ledger).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    fs::write(repo_root().join(LEDGER), bytes).map_err(|e| e.to_string())
}

/// ADR-0015: assign `owner` (a verified record owner) to its closure row. The
/// row must be a gap or an existing record owner, and the ledger unapproved.
fn assign_record_owner(
    catalog: &Catalog,
    ledger: &mut Ledger,
    obligation: &str,
    owner: Owner,
    rationale: &str,
) -> Result<(), String> {
    if !obligation.starts_with("closure:") || !matches!(owner, Owner::Record { .. }) {
        return Err(format!(
            "record owners are allowed only for closure obligations: {obligation}"
        ));
    }
    if !nonempty(rationale) {
        return Err("ownership rationale is empty".into());
    }
    let assignment = ledger
        .assignments
        .iter_mut()
        .find(|r| r.id == obligation)
        .ok_or_else(|| format!("unknown ownership identity {obligation}"))?;
    if !matches!(
        assignment.disposition,
        Disposition::Gap { .. }
            | Disposition::Owned {
                owner: Owner::Record { .. },
                ..
            }
    ) {
        return Err(format!(
            "closure row is neither a gap nor record-owned: {obligation}"
        ));
    }
    assignment.disposition = Disposition::Owned {
        owner,
        rationale: rationale.into(),
    };
    validate(catalog, ledger, false).map(|_| ())
}

/// `record-closure-owner`: verify the registered verifier's approved record
/// completely, then record it as the owner of `obligation`.
pub fn record_closure_owner(
    obligation: &str,
    verifier: &str,
    rationale: &str,
) -> Result<(), String> {
    if verifier == "layers" {
        return Err(
            "five-layer ownership maintenance is retired; use conformance layers-report".into(),
        );
    }
    if !obligation.starts_with("closure:") {
        return Err(format!(
            "record owners are allowed only for closure obligations: {obligation}"
        ));
    }
    if !nonempty(rationale) {
        return Err("ownership rationale is empty".into());
    }
    // Refuse an unknown verifier or unbound/unapproved record before any
    // ledger mutation.
    let owner = record::current_owner(obligation, verifier)?;
    let (catalog, mut ledger) = load()?;
    validate(&catalog, &ledger, false)?;
    assign_record_owner(&catalog, &mut ledger, obligation, owner, rationale)?;
    write_ledger(&ledger)?;
    println!("recorded {verifier} record owner for {obligation}");
    Ok(())
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
    fn five_layer_mapping_is_optional_but_pdf_judgments_remain_required() {
        let (catalog, mut ledger) = load().unwrap();
        for assignment in &mut ledger.assignments {
            if assignment.id == "closure:all-five-layers" || assignment.id.starts_with("layer:") {
                assignment.disposition = Disposition::Owned {
                    owner: Owner::Record {
                        record: Descriptor {
                            path: "retired/layers.json".into(),
                            sha256: "0".repeat(64),
                        },
                        approval_subject_sha256: "historical".into(),
                        verifier: "layers".into(),
                    },
                    rationale: "historical coverage mapping".into(),
                };
            }
        }
        validate(&catalog, &ledger, false).unwrap();
        ledger
            .assignments
            .retain(|a| a.id != "closure:all-five-layers" && !a.id.starts_with("layer:"));
        validate(&catalog, &ledger, false).unwrap();
        assert!(validate_with_layers(&catalog, &ledger, false, true).is_err());
        let pdf = ledger
            .assignments
            .iter_mut()
            .find(|a| a.id == "closure:pdf-judgments")
            .unwrap();
        if let Disposition::Owned {
            owner: Owner::Record { record, .. },
            ..
        } = &mut pdf.disposition
        {
            record.sha256 = "0".repeat(64);
        } else {
            panic!("PDF judgments must remain independently owned");
        }
        assert!(validate(&catalog, &ledger, false)
            .unwrap_err()
            .contains("record digest mismatch"));
    }

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
        extended_ledger(&before, &after, &approved, 85, "gap").unwrap();
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

    fn exception_owner() -> Owner {
        Owner::Test {
            package: None,
            module: None,
            source: descriptor(&owner_source(EXCEPTION_BINARY, EXCEPTION_BINARY)).unwrap(),
            function: EXCEPTION_FUNCTION.into(),
            binary: EXCEPTION_BINARY.into(),
            command: owner_test_command(EXCEPTION_BINARY, EXCEPTION_BINARY, EXCEPTION_FUNCTION),
        }
    }

    fn assigned(ledger: &Ledger, id: &str, owner: Owner) -> Ledger {
        let mut ledger = ledger.clone();
        let row = ledger.assignments.iter_mut().find(|r| r.id == id).unwrap();
        row.disposition = Disposition::Owned {
            owner,
            rationale: "test".into(),
        };
        ledger
    }

    #[test]
    fn tolerance_exceptions_are_owned_only_by_the_direct_png_exception_test() {
        let (catalog, ledger) = load().unwrap();
        let exception = catalog
            .obligations
            .iter()
            .find(|r| r.category == "tolerance-exception")
            .unwrap();
        validate(
            &catalog,
            &assigned(&ledger, &exception.id, exception_owner()),
            false,
        )
        .unwrap();
        let other = catalog
            .obligations
            .iter()
            .find(|r| r.category != "tolerance-exception")
            .unwrap();
        assert!(validate(
            &catalog,
            &assigned(&ledger, &other.id, exception_owner()),
            false
        )
        .is_err());
        let Owner::Test { source, .. } = exception_owner() else {
            unreachable!()
        };
        for (function, binary) in [
            ("another_test", EXCEPTION_BINARY),
            (EXCEPTION_FUNCTION, "fast"),
        ] {
            let owner = Owner::Test {
                package: None,
                module: None,
                source: source.clone(),
                function: function.into(),
                binary: binary.into(),
                command: owner_test_command(binary, EXCEPTION_BINARY, function),
            };
            assert!(validate(&catalog, &assigned(&ledger, &exception.id, owner), false).is_err());
        }
        assert_eq!(
            owner_test_command(EXCEPTION_BINARY, EXCEPTION_BINARY, EXCEPTION_FUNCTION),
            [
                "cargo",
                "test",
                "--locked",
                "--release",
                "-p",
                "excalidraw-render",
                "--test",
                EXCEPTION_BINARY,
                EXCEPTION_FUNCTION,
                "--",
                "--exact"
            ]
        );
    }

    #[test]
    fn the_exception_adapter_binds_its_exact_direct_png_scope() {
        let (catalog, _) = load().unwrap();
        let row = catalog
            .obligations
            .iter()
            .find(|r| r.category == "tolerance-exception")
            .unwrap();
        exception_scope(row).unwrap();
        let mut renamed = row.clone();
        renamed.id = renamed.id.replace(":png:whole", ":svg:whole");
        let mut moved = row.clone();
        moved.pointer = "/exceptions/99".into();
        let mut foreign = row.clone();
        foreign.candidates = vec!["focused.another.v1".into()];
        let mut stale = row.clone();
        stale.authority.sha256 = "0".repeat(64);
        for row in [renamed, moved, foreign, stale] {
            assert!(exception_scope(&row).is_err(), "{row:?}");
        }
        let source = "//! doc\nmod common;\nuse std::fs;\n#[test]\nfn example() {}";
        crate_root_test(source, "example").unwrap();
        for source in [
            "#![cfg(any())]\n#[test]\nfn example() {}",
            "#[ignore]\n#[test]\nfn example() {}",
            "mod inner {\n#[test]\nfn example() {}\n}",
        ] {
            assert!(crate_root_test(source, "example").is_err(), "{source}");
        }
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
    fn library_owner_accepts_historical_source_but_requires_exact_executable_command() {
        let (catalog, mut ledger) = fixture();
        let module = "metadata_failure_contract";
        let function = "metadata_failures_have_exact_recovery_in_every_format";
        ledger.assignments[0].disposition = Disposition::Owned {
            owner: Owner::Test {
                package: None,
                module: None,
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
        assert_eq!(validate(&catalog, &stale, false).unwrap(), 0);
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

    /// A one-row catalog holding the real `closure:pdf-judgments` row.
    fn closure_fixture() -> (Catalog, Ledger) {
        let (mut catalog, mut ledger) = fixture();
        catalog.obligations[0].id = "closure:pdf-judgments".into();
        catalog.obligations[0].category = "closure".into();
        ledger.assignments[0].id = "closure:pdf-judgments".into();
        ledger.catalog_sha256 = digest(&catalog).unwrap();
        (catalog, ledger)
    }

    fn library_owner() -> Owner {
        let function = "metadata_failures_have_exact_recovery_in_every_format";
        Owner::Test {
            package: None,
            module: None,
            source: descriptor("crates/excalidraw-render/src/metadata_failure_contract.rs")
                .unwrap(),
            function: function.into(),
            binary: "lib".into(),
            command: owner_test_command("lib", "metadata_failure_contract", function),
        }
    }

    #[test]
    fn closure_rows_are_owned_only_by_verified_approved_records() {
        let owner = record::current_owner("closure:pdf-judgments", "pdf-judgments").unwrap();
        let (catalog, mut ledger) = closure_fixture();
        assign_record_owner(
            &catalog,
            &mut ledger,
            "closure:pdf-judgments",
            owner.clone(),
            "reviewed",
        )
        .unwrap();
        assert_eq!(validate(&catalog, &ledger, false).unwrap(), 0);
        // The record descriptor is judgment-bearing: it stays in the subject.
        let before = subject(&catalog, &ledger).unwrap();
        let mut stale = ledger.clone();
        let Disposition::Owned {
            owner: Owner::Record { record, .. },
            ..
        } = &mut stale.assignments[0].disposition
        else {
            panic!()
        };
        record.sha256 = "0".repeat(64);
        assert_ne!(subject(&catalog, &stale).unwrap(), before);
        assert!(validate(&catalog, &stale, false)
            .unwrap_err()
            .contains("stale or unbound record owner"));
        {
            let mut bad = ledger.clone();
            let Disposition::Owned {
                owner: Owner::Record { verifier, .. },
                ..
            } = &mut bad.assignments[0].disposition
            else {
                panic!()
            };
            *verifier = "sh -c true".into();
            let error = validate(&catalog, &bad, false).unwrap_err();
            assert!(error.contains("unknown record verifier"), "{error}");
        }
        // A closure row with a test owner, and a non-closure row with a record owner.
        let tested = assigned(&ledger, "closure:pdf-judgments", library_owner());
        assert!(validate(&catalog, &tested, false)
            .unwrap_err()
            .contains("closure obligations are owned only by approved judgment records"));
        let (catalog, ledger) = fixture();
        let foreign = assigned(&ledger, "fixture", owner.clone());
        assert!(validate(&catalog, &foreign, false)
            .unwrap_err()
            .contains("closure obligations are owned only by approved judgment records"));
        let mut ledger = ledger;
        assert!(
            assign_record_owner(&catalog, &mut ledger, "fixture", owner, "reviewed")
                .unwrap_err()
                .contains("only for closure obligations")
        );
    }

    #[test]
    fn record_closure_owner_rejects_invalid_record_and_non_record_owners() {
        let owner = Owner::Record {
            record: Descriptor {
                path: "unused".into(),
                sha256: "0".repeat(64),
            },
            approval_subject_sha256: "unused".into(),
            verifier: "pdf-judgments".into(),
        };
        let (catalog, ledger) = closure_fixture();
        let mut approved = ledger.clone();
        approved.approval = json!({"existing":"approval"});
        assert!(assign_record_owner(
            &catalog,
            &mut approved,
            "closure:pdf-judgments",
            owner.clone(),
            "r"
        )
        .unwrap_err()
        .contains("verifier pdf-judgments checks"));
        let mut tested = assigned(&ledger, "closure:pdf-judgments", library_owner());
        assert!(assign_record_owner(
            &catalog,
            &mut tested,
            "closure:pdf-judgments",
            owner.clone(),
            "r"
        )
        .unwrap_err()
        .contains("neither a gap nor record-owned"));
        let mut empty = ledger.clone();
        assert!(
            assign_record_owner(&catalog, &mut empty, "closure:pdf-judgments", owner, " ")
                .unwrap_err()
                .contains("rationale is empty")
        );
        for (obligation, verifier, expected) in [
            (
                "codec:png:alpha",
                "pdf-judgments",
                "only for closure obligations",
            ),
            (
                "closure:pdf-judgments",
                "unknown",
                "unknown record verifier",
            ),
            (
                "closure:all-five-layers",
                "pdf-judgments",
                "not closure:all-five-layers",
            ),
        ] {
            let error = record_closure_owner(obligation, verifier, "reviewed").unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn complete_ownership_is_checked_without_approval_metadata() {
        let (catalog, gap) = fixture();
        assert!(validate(&catalog, &gap, true)
            .unwrap_err()
            .contains("unresolved gaps"));
        let complete = assigned(&gap, "fixture", library_owner());
        for approval in [Value::Null, json!({"historical":"unverified"})] {
            let mut ledger = complete.clone();
            ledger.approval = approval;
            assert_eq!(validate(&catalog, &ledger, true).unwrap(), 0);
        }
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
    fn test_owner_checks_compiled_module_and_selector_not_historical_source_digest() {
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
        validate(&catalog, &ledger, false).unwrap();
    }

    #[test]
    fn exclusion_requires_proof_without_an_approval_record() {
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
        validate(&catalog, &ledger, false).unwrap();
        let before = subject(&catalog, &ledger).unwrap();
        ledger.approval = json!({"anything":"excluded from subject"});
        assert_eq!(subject(&catalog, &ledger).unwrap(), before);
        ledger.assignments[0].id = "changed".into();
        assert_ne!(subject(&catalog, &ledger).unwrap(), before);
    }

    fn owned_fixture(owner: Owner) -> (Catalog, Ledger) {
        let (catalog, mut ledger) = fixture();
        ledger.assignments[0].disposition = Disposition::Owned {
            owner,
            rationale: "reviewed executable owner".into(),
        };
        (catalog, ledger)
    }

    fn explicit_owner(package: &str, binary: &str, module: &str, function: &str) -> Owner {
        let owner = test_owner(package, binary, module, function)
            .unwrap_or_else(|e| panic!("{package} {binary} {module}::{function}: {e}"));
        let Owner::Test {
            package: Some(named),
            module: Some(path),
            ..
        } = &owner
        else {
            panic!("{module}::{function} must use the explicit adapter form: {owner:?}");
        };
        assert_eq!((named.as_str(), path.as_str()), (package, module));
        owner
    }

    #[test]
    fn explicit_adapter_accepts_nested_library_root_binary_and_compare_owners() {
        let compare = executable::COMPARE;
        let renderer = executable::RENDERER;
        for (package, binary, module, function, source) in [
            // An inline `#[cfg(test)] mod tests` two file modules deep.
            (
                renderer,
                "lib",
                "render::number_format::tests",
                "ecmascript_spelling_matches_pinned_chromium_vectors",
                "crates/excalidraw-render/src/render/number_format.rs",
            ),
            // `render/mod.rs` then `tests.rs`, importing its parent with `use super::*`.
            (
                renderer,
                "lib",
                "render::tests",
                "outline_operations_match_the_pinned_roughjs_vectors",
                "crates/excalidraw-render/src/render/tests.rs",
            ),
            // A non-`mod.rs` parent file (`normalization.rs`) owns its directory.
            (
                renderer,
                "lib",
                "normalization::fractional_index::tests",
                "repair_matches_the_pinned_source_order_vectors",
                "crates/excalidraw-render/src/normalization/fractional_index.rs",
            ),
            // A `mod.rs` directory module directly under the crate root.
            (
                renderer,
                "lib",
                "scene_fuzz",
                "fixed_seed_scene_fuzz_smoke_is_bounded_and_deterministic",
                "crates/excalidraw-render/src/scene_fuzz/mod.rs",
            ),
            // A root test binary with a `#[path]` module and its own submodule.
            (
                renderer,
                "render_fuzz",
                "render_fuzz::regressions",
                "confirmed_failures_are_manifested_and_replayed",
                "crates/excalidraw-render/tests/render_fuzz/regressions.rs",
            ),
            (
                renderer,
                "asset_fuzz",
                "asset_fuzz",
                "fixed_seed_asset_fuzz_smoke",
                "crates/excalidraw-render/tests/asset_fuzz/mod.rs",
            ),
            // The crate root of a test binary.
            (
                renderer,
                "limits_scene_admission",
                "",
                "fixed_structure_and_input_byte_boundaries_are_enforced",
                "crates/excalidraw-render/tests/limits_scene_admission.rs",
            ),
            (
                compare,
                "pixelmatch_upstream",
                "",
                "pixelmatch_matches_the_pinned_upstream_counts_and_diff_bytes",
                "crates/excalidraw-render-compare/tests/pixelmatch_upstream.rs",
            ),
            (
                compare,
                "ssim_vectors",
                "",
                "ssim_matches_every_normative_kernel_position_vector",
                "crates/excalidraw-render-compare/tests/ssim_vectors.rs",
            ),
            (
                compare,
                "profile",
                "",
                "canonical_comparator_profile_schema_and_all_bound_bytes_are_current",
                "crates/excalidraw-render-compare/tests/profile.rs",
            ),
        ] {
            let owner = explicit_owner(package, binary, module, function);
            let Owner::Test {
                source: bound,
                command,
                ..
            } = &owner
            else {
                unreachable!()
            };
            assert_eq!(bound.path, source);
            let selector = if module.is_empty() {
                function.to_string()
            } else {
                format!("{module}::{function}")
            };
            let target = if binary == "lib" {
                vec!["--lib".to_string()]
            } else {
                vec!["--test".to_string(), binary.to_string()]
            };
            let expected: Vec<String> = ["cargo", "test", "--locked", "--release", "-p", package]
                .into_iter()
                .map(String::from)
                .chain(target)
                .chain([selector, "--".into(), "--exact".into()])
                .collect();
            assert_eq!(command, &expected);
            let (catalog, ledger) = owned_fixture(owner);
            assert_eq!(
                validate(&catalog, &ledger, false).unwrap(),
                0,
                "{module}::{function}"
            );
        }
        // The original adapters keep their compact form and stored bytes.
        assert_eq!(
            serde_json::to_value(
                test_owner(
                    renderer,
                    "lib",
                    "metadata_failure_contract",
                    "metadata_failures_have_exact_recovery_in_every_format"
                )
                .unwrap()
            )
            .unwrap(),
            serde_json::to_value(library_owner()).unwrap()
        );
        assert_eq!(
            serde_json::to_value(
                test_owner(renderer, EXCEPTION_BINARY, "", EXCEPTION_FUNCTION).unwrap()
            )
            .unwrap(),
            serde_json::to_value(exception_owner()).unwrap()
        );
    }

    #[test]
    fn explicit_adapter_rejects_missing_ignored_misplaced_and_stale_owners() {
        let renderer = executable::RENDERER;
        for (package, binary, module, function) in [
            // Missing functions.
            (
                renderer,
                "lib",
                "render::number_format::tests",
                "no_such_test",
            ),
            (executable::COMPARE, "ssim_vectors", "", "no_such_test"),
            // Ignored, or ignored unless a feature is enabled.
            (
                renderer,
                "lib",
                "scene_fuzz",
                "nightly_scene_fuzz_is_bounded_and_deterministic",
            ),
            (
                renderer,
                "lib",
                "render::frame_batch_property",
                "frame_batch_rendering_matches_standalone_around_resource_limits",
            ),
            (
                renderer,
                "render_fuzz",
                "render_fuzz",
                "nightly_complete_render_fuzz",
            ),
            (
                executable::COMPARE,
                "profile",
                "",
                "materialize_canonical_comparator_closure",
            ),
            // Wrong module paths, targets or packages.
            (
                renderer,
                "lib",
                "number_format::tests",
                "ecmascript_spelling_matches_pinned_chromium_vectors",
            ),
            (
                renderer,
                "lib",
                "render::number_format",
                "ecmascript_spelling_matches_pinned_chromium_vectors",
            ),
            (renderer, "lib", "render::no_such_module", "example"),
            (
                renderer,
                "render_fuzz",
                "",
                "confirmed_failures_are_manifested_and_replayed",
            ),
            (
                renderer,
                "render_fuzz",
                "render_fuzz::generator",
                "confirmed_failures_are_manifested_and_replayed",
            ),
            (
                renderer,
                "fast",
                "render_fuzz::regressions",
                "confirmed_failures_are_manifested_and_replayed",
            ),
            (renderer, "no_such_binary", "", "example"),
            (
                executable::COMPARE,
                "pixelmatch_upstream",
                "pixelmatch",
                "pixelmatch_matches_the_pinned_upstream_counts_and_diff_bytes",
            ),
            ("excalidraw-render-cli", "lib", "", "example"),
            (renderer, "lib", "render::tests::", "example"),
            // The wasm-only module is not compiled into a native test build.
            (renderer, "lib", "wasm_tests", "example"),
        ] {
            assert!(
                test_owner(package, binary, module, function).is_err(),
                "accepted {package} {binary} {module}::{function}"
            );
        }
        let valid = explicit_owner(
            renderer,
            "lib",
            "render::number_format::tests",
            "ecmascript_spelling_matches_pinned_chromium_vectors",
        );
        let mutate = |edit: &dyn Fn(&mut Owner)| {
            let mut owner = valid.clone();
            edit(&mut owner);
            let (catalog, ledger) = owned_fixture(owner);
            validate(&catalog, &ledger, false).unwrap_err()
        };
        let invalid = mutate(&|o| {
            if let Owner::Test { source, .. } = o {
                source.sha256 = "not-a-digest".into();
            }
        });
        assert!(
            invalid.contains("invalid historical test source digest"),
            "{invalid}"
        );
        let mut historical = valid.clone();
        if let Owner::Test { source, .. } = &mut historical {
            source.sha256 = "0".repeat(64);
        }
        let (catalog, ledger) = owned_fixture(historical);
        validate(&catalog, &ledger, false).unwrap();
        let moved = mutate(&|o| {
            if let Owner::Test { module, .. } = o {
                *module = Some("render::tests".into());
            }
        });
        assert!(
            moved.contains("not an unconditional test") || moved.contains("not the selected"),
            "{moved}"
        );
        let missing = mutate(&|o| {
            if let Owner::Test {
                function, command, ..
            } = o
            {
                *function = "no_such_test".into();
                *command = executable::command(
                    renderer,
                    "lib",
                    "render::number_format::tests",
                    "no_such_test",
                );
            }
        });
        assert!(missing.contains("not an unconditional test"), "{missing}");
        let redirected = mutate(&|o| {
            if let Owner::Test { command, .. } = o {
                command[7] =
                    "render::number_format::ecmascript_spelling_matches_pinned_chromium_vectors"
                        .into();
            }
        });
        assert!(redirected.contains("exact executable test"), "{redirected}");
        let half = mutate(&|o| {
            if let Owner::Test { package, .. } = o {
                *package = None;
            }
        });
        assert!(half.contains("named together"), "{half}");
        // A nested owner cannot hide behind the compact form, and a compact
        // owner cannot be respelled explicitly.
        let compact_nested = mutate(&|o| {
            if let Owner::Test {
                package, module, ..
            } = o
            {
                *package = None;
                *module = None;
            }
        });
        assert!(
            compact_nested.contains("explicit adapter"),
            "{compact_nested}"
        );
        let Owner::Test {
            source,
            function,
            binary,
            command,
            ..
        } = library_owner()
        else {
            unreachable!()
        };
        let respelled = Owner::Test {
            package: Some(renderer.into()),
            source,
            function,
            binary,
            module: Some("metadata_failure_contract".into()),
            command,
        };
        let (catalog, ledger) = owned_fixture(respelled);
        assert!(validate(&catalog, &ledger, false)
            .unwrap_err()
            .contains("compact form"));
    }

    #[test]
    fn module_resolution_follows_rust_file_rules_and_fails_closed() {
        let test = "#[test] fn example() {}";
        let resolve = |files: &[(&str, &str)], module: &str, library: bool| {
            executable::resolve(
                "c/src/lib.rs",
                module,
                "example",
                library,
                &executable::memory(files),
            )
        };
        // `name.rs`, `name/mod.rs`, inline modules and `#[path]` files.
        assert_eq!(
            resolve(
                &[
                    ("c/src/lib.rs", "mod a;"),
                    ("c/src/a.rs", "#[cfg(test)] mod b;"),
                    ("c/src/a/b.rs", test)
                ],
                "a::b",
                true
            )
            .unwrap(),
            "c/src/a/b.rs"
        );
        assert_eq!(
            resolve(
                &[
                    ("c/src/lib.rs", "mod a;"),
                    (
                        "c/src/a/mod.rs",
                        "mod b { #[cfg(test)] mod t { #[test] fn example() {} } }"
                    )
                ],
                "a::b::t",
                true
            )
            .unwrap(),
            "c/src/a/mod.rs"
        );
        assert_eq!(
            resolve(
                &[
                    ("c/src/lib.rs", "#[path = \"../x/y.rs\"] mod a;"),
                    ("c/x/y.rs", "mod b;"),
                    ("c/x/b.rs", test)
                ],
                "a::b",
                false
            )
            .unwrap(),
            "c/x/b.rs"
        );
        assert_eq!(
            resolve(
                &[
                    ("c/src/lib.rs", "mod a;"),
                    ("c/src/a.rs", "mod b { mod c; }"),
                    ("c/src/a/b/c.rs", test)
                ],
                "a::b::c",
                false
            )
            .unwrap(),
            "c/src/a/b/c.rs"
        );
        // `use super::*` is followed; the parent must not be able to shadow `test`.
        let glob = "use super::*; #[test] fn example() {}";
        assert!(resolve(
            &[
                ("c/src/lib.rs", "mod a; use std::fmt;"),
                ("c/src/a.rs", glob)
            ],
            "a",
            false
        )
        .is_ok());
        for parent in [
            "mod a; use custom::test;",
            "mod a; use custom::*;",
            "mod a; inject!();",
            "mod a; pub(crate) fn test() {}",
            "mod a; use super::*;",
        ] {
            assert!(
                resolve(
                    &[("c/src/lib.rs", parent), ("c/src/a.rs", glob)],
                    "a",
                    false
                )
                .is_err(),
                "{parent}"
            );
        }
        assert!(
            resolve(
                &[
                    ("c/src/lib.rs", "mod a;"),
                    ("c/src/a.rs", "mod b; use custom::*;"),
                    ("c/src/a/b.rs", "mod c;"),
                    ("c/src/a/b/c.rs", glob)
                ],
                "a::b::c",
                false
            )
            .is_ok(),
            "a grandparent glob is not imported"
        );
        for files in [
            // Ambiguous, missing or duplicated module files and declarations.
            vec![
                ("c/src/lib.rs", "mod a;"),
                ("c/src/a.rs", test),
                ("c/src/a/mod.rs", test),
            ],
            vec![("c/src/lib.rs", "mod b;"), ("c/src/a.rs", test)],
            vec![("c/src/lib.rs", "mod a; mod a;"), ("c/src/a.rs", test)],
            // Conditional, redirected-inline or macro-imported chains.
            vec![
                ("c/src/lib.rs", "#[cfg(all(test, unix))] mod a;"),
                ("c/src/a.rs", test),
            ],
            vec![
                ("c/src/lib.rs", "#[cfg(not(test))] mod a;"),
                ("c/src/a.rs", test),
            ],
            vec![
                ("c/src/lib.rs", "#[allow(dead_code)] mod a;"),
                ("c/src/a.rs", test),
            ],
            vec![
                ("c/src/lib.rs", "#[macro_use] mod a;"),
                ("c/src/a.rs", test),
            ],
            vec![
                ("c/src/lib.rs", "mod a { #[path = \"x.rs\"] mod b; }"),
                ("c/src/x.rs", test),
            ],
            vec![
                ("c/src/lib.rs", "mod a;"),
                ("c/src/a.rs", "#![cfg(unix)] #[test] fn example() {}"),
            ],
            vec![
                ("c/src/lib.rs", "#![cfg(unix)] mod a;"),
                ("c/src/a.rs", test),
            ],
            vec![
                ("c/src/lib.rs", "mod a;"),
                ("c/src/a.rs", "#[test] #[ignore] fn example() {}"),
            ],
            vec![
                ("c/src/lib.rs", "#[path = \"../../../x.rs\"] mod a;"),
                ("x.rs", test),
            ],
        ] {
            assert!(resolve(&files, "a", false).is_err(), "{files:?}");
        }
        // A library owner needs a test-only module on its chain.
        let files = [
            ("c/src/lib.rs", "mod a;"),
            ("c/src/a.rs", "mod b;"),
            ("c/src/a/b.rs", test),
        ];
        assert!(resolve(&files, "a::b", false).is_ok());
        assert!(resolve(&files, "a::b", true)
            .unwrap_err()
            .contains("cfg(test)"));
        // Test binaries are auto-discovered harness targets only.
        let workspace = "[workspace]\nmembers=['crates/excalidraw-render-compare']\n";
        let package = "[package]\nname='excalidraw-render-compare'\n";
        executable::package_layout(executable::COMPARE, package, workspace, false).unwrap();
        for suffix in [
            "autotests=false\n",
            "[[test]]\nname='ssim_vectors'\nharness=false\n",
        ] {
            assert!(
                executable::package_layout(
                    executable::COMPARE,
                    &format!("{package}{suffix}"),
                    workspace,
                    false
                )
                .is_err(),
                "{suffix}"
            );
        }
    }

    #[test]
    fn layer_rows_take_a_single_candidate_case_owner_or_an_executable_test() {
        let (mut catalog, mut ledger) = fixture();
        catalog.obligations[0].id = "layer:focused:frame-export".into();
        catalog.obligations[0].category = "layer".into();
        catalog.obligations[0].candidates = vec!["focused.frame-export.v1".into()];
        ledger.assignments[0].id = "layer:focused:frame-export".into();
        ledger.catalog_sha256 = digest(&catalog).unwrap();
        let case = |id: &str, path: &str| Owner::Case {
            case: id.into(),
            manifest: descriptor(&format!(
                "tests/conformance/cases/focused/{path}/manifest.json"
            ))
            .unwrap(),
        };
        let with = |owner: Owner, catalog: &Catalog| {
            let mut ledger = ledger.clone();
            assign_gaps(
                &mut ledger,
                &owner,
                "reviewed",
                &["layer:focused:frame-export".into()],
            )
            .unwrap();
            check_disposition(&catalog.obligations[0], &ledger.assignments[0].disposition)
                .map(usize::from)
        };
        assert_eq!(
            with(case("focused.frame-export.v1", "frame-export"), &catalog).unwrap(),
            0
        );
        assert!(with(
            case("focused.rectangle-outline.v1", "rectangle-outline"),
            &catalog
        )
        .is_err());
        let mut stale = case("focused.frame-export.v1", "frame-export");
        if let Owner::Case { manifest, .. } = &mut stale {
            manifest.sha256 = "0".repeat(64);
        }
        assert!(with(stale, &catalog)
            .unwrap_err()
            .contains("stale owner manifest"));
        let mut two = catalog.clone();
        two.obligations[0]
            .candidates
            .push("focused.rectangle-outline.v1".into());
        let mut rebound = ledger.clone();
        rebound.catalog_sha256 = digest(&two).unwrap();
        let mut assigned = rebound.clone();
        assign_gaps(
            &mut assigned,
            &case("focused.frame-export.v1", "frame-export"),
            "reviewed",
            &["layer:focused:frame-export".into()],
        )
        .unwrap();
        assert!(
            check_disposition(&two.obligations[0], &assigned.assignments[0].disposition)
                .unwrap_err()
                .contains("does not establish")
        );
        // An executable test may own a layer row; a record may not.
        assert_eq!(with(library_owner(), &catalog).unwrap(), 0);
        // Only gaps are assigned.
        let mut owned = ledger.clone();
        assign_gaps(
            &mut owned,
            &library_owner(),
            "reviewed",
            &["layer:focused:frame-export".into()],
        )
        .unwrap();
        assert!(assign_gaps(
            &mut owned,
            &library_owner(),
            "again",
            &["layer:focused:frame-export".into()]
        )
        .is_err());
    }

    #[test]
    fn only_spec_anchored_layer_rows_name_registered_candidate_cases() {
        let registered: BTreeSet<&str> = ["focused.frame-export.v1"].into();
        let spec = "The conformance corpus has five layers:";
        let row = |id: &str, category: &str, candidates: &[&str]| PolicyRow {
            id: id.into(),
            category: category.into(),
            criterion: "criterion".into(),
            spec_anchor: spec.into(),
            candidates: candidates.iter().map(|c| c.to_string()).collect(),
        };
        assert!(policy_row_valid(
            &row("layer:vector:ssim", "layer", &[]),
            spec,
            &registered
        ));
        assert!(policy_row_valid(
            &row(
                "layer:focused:frame-export",
                "layer",
                &["focused.frame-export.v1"]
            ),
            spec,
            &registered
        ));
        for bad in [
            row("interaction:x", "interaction", &["focused.frame-export.v1"]),
            row("layer:focused:x", "layer", &["focused.unregistered.v1"]),
            row(
                "layer:focused:x",
                "layer",
                &["focused.frame-export.v1", "focused.frame-export.v1"],
            ),
            row("vector:ssim", "layer", &[]),
            row("layer:vector:ssim", "codec-profile", &[]),
            row("layer:vector:ssim", "vector", &[]),
        ] {
            assert!(!policy_row_valid(&bad, spec, &registered), "{}", bad.id);
        }
        assert!(!policy_row_valid(
            &row("layer:vector:ssim", "layer", &[]),
            "other spec",
            &registered
        ));
    }
}
