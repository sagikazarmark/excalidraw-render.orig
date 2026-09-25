//! Discovery and completeness rules for optional five-layer coverage reports.
//! The historical `layers.json` still binds the old rules for explicit audits;
//! current reports recompute observations without requiring record refreshes.
//!
//! Every item is discovered by a rule below from the repository and the
//! ownership catalog, never from ledger dispositions, so recording an owner
//! does not change the record. Completeness then reads the ledger: each item
//! must map to owned rows whose owners pass the ledger's own checks.
use super::*;
use crate::coverage::ownership::PrimaryOwner;

pub(super) const IMPLEMENTATION: &str = "xtask/src/coverage/layers/derive.rs";
pub(super) const SPEC: &str = "docs/renderer-spec-v1.md";
const CANDIDATE_REGISTRY: &str = "tests/conformance/candidates/registry.json";
const REPRESENTATIVE_ROW: &str = "interaction:frames-text-assets";
const CASE_REGISTRY: &str = "tests/conformance/cases/representative/registry.json";
const ORACLE_ROOT: &str = "tests/oracle";
const COMPARATOR_ROOT: &str = "tests/conformance/comparator-vectors";
const FAMILY_SENTENCE: &str = "Exact vectors cover ";
/// `tools/representative-intake/case.py`: comparison id to format, in order.
const COMPARISON_FORMATS: [(&str, &str); 5] = [
    ("svg-chromium", "svg"),
    ("svg-rust", "svg"),
    ("pdf-pdfium", "pdf"),
    ("pdf-pdfjs", "pdf"),
    ("png-canvas", "png"),
];
const ISSUE_84: &str = "https://github.com/sagikazarmark/excalidraw-render.orig/issues/84";

/// One rule of the record, anchored in the renderer specification.
pub(super) struct Rule {
    pub id: &'static str,
    pub layer: &'static str,
    pub statement: &'static str,
    pub spec_anchor: &'static str,
}

pub(super) const RULES: &[Rule] = &[
    Rule {
        id: "vector-families",
        layer: "vector",
        statement: "The exact-vector families are the comma-separated list of the spec sentence beginning \"Exact vectors cover \", in order; the record binds that sentence's text, not the whole spec. Each family is declared once, in that order, with row selectors (an exact catalog id, or a prefix ending in \":*\"), expanded against the current catalog at verification: every selector must match a row, never a closure row, and only layer:vector:* layer rows.",
        spec_anchor: "Exact vectors cover ",
    },
    Rule {
        id: "vector-domains",
        layer: "vector",
        statement: "Further exact-vector domains are declared with their own spec anchor and row selectors.",
        spec_anchor: "1. Exact compatibility-critical vectors.",
    },
    Rule {
        id: "vector-corpora",
        layer: "vector",
        statement: "Stored vector corpora are every tests/conformance/comparator-vectors/*/ directory (which must hold manifest.json), every crates/*/tests/fixtures/*-vectors.json file and every tools/*-vectors/ derivation directory. Each is declared exactly once with row selectors; its bytes, the vectors themselves, are bound by tree digest.",
        spec_anchor: "1. Exact compatibility-critical vectors.",
    },
    Rule {
        id: "oracle-classification",
        layer: "vector",
        statement: "Every tests/oracle/*/ directory is declared exactly once: a vector oracle maps to the codec/profile rows its pinned reference values decide; an authority or profile input maps to no row. Oracle bytes are the evidence of those rows and are bound by their owners, not here.",
        spec_anchor: "1. Exact compatibility-critical vectors.",
    },
    Rule {
        id: "registered-cases",
        layer: "focused",
        statement: "The record binds the ids of the cases in tests/conformance/cases/registry.json. Each maps to the current case-label and layer:focused:* rows that name it as a candidate, and must be the case owner of at least one of them.",
        spec_anchor: "2. Focused scene documents with official SVG/PNG evidence.",
    },
    Rule {
        id: "representative-candidates",
        layer: "representative",
        statement: "Checked by rule, never bound: tests/conformance/candidates/registry.json holds exactly one entry; it covers interaction:frames-text-assets, and each of its coverage rows is owned by the representative owner bound to that entry, which validates. Every other candidates/* directory is a superseded or unregistered collection, excluded by rule.",
        spec_anchor: "3. Representative licensed or project-authored scene documents.",
    },
    Rule {
        id: "representative-cases",
        layer: "representative",
        statement: "Every #84 case of tests/conformance/cases/representative/registry.json is checked here as tools/representative-intake/case.py checks it: its case.json is approved on a #85 comment binding its subject (case.py's subject computation), its retained-file inventory and intake manifest digest are current, the intake's sanitization retained_sha256 is the retained scene's digest, a license is present, and it has comparisons and at least one anchored crop besides whole. Unreferenced intake bundles and tests/public-scenes entries are excluded by rule.",
        spec_anchor: "External scenes require consent or redistribution-compatible licensing and are",
    },
    Rule {
        id: "adversarial-targets",
        layer: "adversarial",
        statement: "Fuzz and property targets are crates/*/src/**/*_fuzz* (a module file or directory, outermost match), crates/*/tests/*_fuzz* and crates/*/tests/limits_* (a test binary with its module directory), and crates/*/{src,tests}/**/frame_batch_property. Each maps to exactly the row layer:adversarial:<name with - for _>. The record binds each target and the REQUIRED_RISKS it declares, not its source bytes.",
        spec_anchor: "4. Adversarial and resource-boundary scene documents.",
    },
    Rule {
        id: "regression-corpora",
        layer: "regression",
        statement: "Regression corpora are crates/*/tests/qualification/*-regressions.json. Each maps to exactly the row layer:regression:<name>, recording its digest, its case ids and public-facade case ids (each unique within its list) and its risks.",
        spec_anchor: "5. Permanent minimized regressions.",
    },
    Rule {
        id: "no-orphan-layer-rows",
        layer: "all",
        statement: "At verification, every current catalog row layer:<layer>:* is referenced by an item of its own layer, every mapped row exists, and closure rows are never mapped.",
        spec_anchor: "The conformance corpus has five layers:",
    },
    Rule {
        id: "completeness",
        layer: "all",
        statement: "Each vector item maps to at least one owned row; every mapped layer row is owned; every owned mapped row's owner passes the ledger's checks (a case owner also passes its conformance verification). Registered cases, candidates, fuzz targets and regression corpora are owned as their rules state.",
        spec_anchor: "exactly one primary record or approved exclusion.",
    },
];

/// Catalog rows by id: (category, candidates).
type CatalogRows = BTreeMap<String, (String, Vec<String>)>;

fn text(repo: &Repo, path: &str) -> Result<String, String> {
    String::from_utf8(repo.read(path)?).map_err(|_| format!("{path} is not UTF-8"))
}

fn source(repo: &Repo, path: &str) -> Result<Value, String> {
    Ok(json!({"path":path,"sha256":sha256(&repo.read(path)?)}))
}

/// The spec's exact-vector sentence (whitespace-normalized) and its families.
pub(super) fn spec_families(spec: &str) -> Result<(String, Vec<String>), String> {
    let start = spec
        .find(FAMILY_SENTENCE)
        .ok_or("the spec has no exact-vector sentence")?
        + FAMILY_SENTENCE.len();
    let rest = &spec[start..];
    let bytes = rest.as_bytes();
    let end = (0..bytes.len())
        .find(|&i| bytes[i] == b'.' && bytes.get(i + 1).is_none_or(|b| b.is_ascii_whitespace()))
        .ok_or("the exact-vector sentence does not end")?;
    let sentence = rest[..end].split_whitespace().collect::<Vec<_>>().join(" ");
    let families: Vec<String> = sentence
        .split(", ")
        .map(|f| f.strip_prefix("and ").unwrap_or(f).to_owned())
        .collect();
    if families.iter().any(|f| f.is_empty())
        || families.iter().collect::<BTreeSet<_>>().len() != families.len()
    {
        return Err("the exact-vector sentence has an empty or repeated family".into());
    }
    Ok((format!("{FAMILY_SENTENCE}{sentence}."), families))
}

/// Expand row selectors against the catalog: every row exists, no closure
/// row, no duplicate, and a vector item names only `layer:vector:*` layer rows.
fn expand(selectors: &Value, catalog: &CatalogRows, item: &str) -> Result<Vec<String>, String> {
    let selectors = selectors
        .as_array()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{item}: rows must be a non-empty array"))?;
    let mut rows = Vec::new();
    for selector in selectors {
        let selector = selector
            .as_str()
            .ok_or_else(|| format!("{item}: row selectors are strings"))?;
        let matched: Vec<String> = match selector.strip_suffix(":*") {
            Some(prefix) => catalog
                .keys()
                .filter(|id| id.starts_with(&format!("{prefix}:")))
                .cloned()
                .collect(),
            None if catalog.contains_key(selector) => vec![selector.to_owned()],
            None => Vec::new(),
        };
        if matched.is_empty() {
            return Err(format!(
                "{item}: selector {selector} matches no catalog row"
            ));
        }
        rows.extend(matched);
    }
    if rows.iter().collect::<BTreeSet<_>>().len() != rows.len() {
        return Err(format!("{item}: selectors overlap"));
    }
    if let Some(bad) = rows.iter().find(|r| {
        r.starts_with("closure:") || (r.starts_with("layer:") && !r.starts_with("layer:vector:"))
    }) {
        return Err(format!("{item}: row {bad} cannot answer a vector item"));
    }
    Ok(rows)
}

/// Digest of a file set: canonical `[{"path","sha256"}]` sorted by path.
fn tree(repo: &Repo, files: &[String]) -> Result<Value, String> {
    let mut entries = Vec::new();
    for path in files.iter().collect::<BTreeSet<_>>() {
        entries.push(json!({"path":path,"sha256":sha256(&repo.read(path)?)}));
    }
    Ok(json!({"sha256":sha256(&canonical_bytes(&Value::Array(entries))?),"files":files.len()}))
}

fn dirs(repo: &Repo, dir: &str) -> Result<Vec<String>, String> {
    Ok(repo
        .children(dir)?
        .into_iter()
        .filter(|(_, is_dir)| *is_dir)
        .map(|(name, _)| name)
        .collect())
}

fn files_below(repo: &Repo, dir: &str) -> Result<Vec<String>, String> {
    let (parent, name) = dir.rsplit_once('/').ok_or("invalid directory")?;
    if repo.children(parent)?.contains(&(name.to_owned(), true)) {
        repo.list(dir)
    } else {
        Ok(Vec::new())
    }
}

/// Every stored vector corpus: (path, kind, files).
/// A discovered vector corpus: path, kind and files.
type Corpus = (String, &'static str, Vec<String>);

fn corpora(repo: &Repo) -> Result<Vec<Corpus>, String> {
    let mut found = Vec::new();
    for name in dirs(repo, COMPARATOR_ROOT)? {
        let dir = format!("{COMPARATOR_ROOT}/{name}");
        let files = repo.list(&dir)?;
        if !files.contains(&format!("{dir}/manifest.json")) {
            return Err(format!(
                "comparator vector directory {dir} has no manifest.json"
            ));
        }
        found.push((dir, "comparator-vectors", files));
    }
    for krate in dirs(repo, "crates")? {
        for (name, is_dir) in repo.children(&format!("crates/{krate}/tests/fixtures"))? {
            if !is_dir && name.ends_with("-vectors.json") {
                let path = format!("crates/{krate}/tests/fixtures/{name}");
                found.push((path.clone(), "fixture-vectors", vec![path]));
            }
        }
    }
    for name in dirs(repo, "tools")? {
        if name.ends_with("-vectors") {
            let dir = format!("tools/{name}");
            let files = repo.list(&dir)?;
            found.push((dir, "derivation-tool", files));
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

/// Discovered fuzz/property targets: module path stem to its files.
fn targets(repo: &Repo) -> Result<BTreeMap<String, Vec<String>>, String> {
    let fuzz = |name: &str| name.contains("_fuzz");
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for krate in dirs(repo, "crates")? {
        for sub in ["src", "tests"] {
            let base = format!("crates/{krate}/{sub}");
            for path in files_below(repo, &base)? {
                let rest = &path[base.len() + 1..];
                let segments: Vec<&str> = rest.split('/').collect();
                let stem = |segment: &str| segment.strip_suffix(".rs").map(str::to_owned);
                // The outermost matching segment names the target; a file
                // segment counts only as a `.rs` module.
                let hit = segments.iter().enumerate().find_map(|(i, segment)| {
                    let last = i + 1 == segments.len();
                    let name = if last {
                        stem(segment)?
                    } else {
                        (*segment).to_owned()
                    };
                    let matches = name == "frame_batch_property"
                        || (sub == "src" && fuzz(&name))
                        || (sub == "tests"
                            && i == 0
                            && (fuzz(&name) || name.starts_with("limits_")));
                    matches.then(|| {
                        format!(
                            "{base}/{}",
                            [&segments[..i], &[name.as_str()]].concat().join("/")
                        )
                    })
                });
                if let Some(target) = hit {
                    found.entry(target).or_default().push(path);
                }
            }
        }
    }
    Ok(found)
}

/// The `REQUIRED_RISKS` a target declares: the last path segment of each
/// element of one `const REQUIRED_RISKS: [T; N] = [..]`, N checked.
pub(super) fn required_risks(sources: &[(String, String)]) -> Result<Vec<String>, String> {
    fn visit(items: &[syn::Item], out: &mut Vec<Result<Vec<String>, String>>) {
        for item in items {
            match item {
                syn::Item::Const(c) if c.ident == "REQUIRED_RISKS" => out.push(risks(c)),
                syn::Item::Mod(m) => {
                    if let Some((_, items)) = &m.content {
                        visit(items, out);
                    }
                }
                _ => {}
            }
        }
    }
    fn risks(item: &syn::ItemConst) -> Result<Vec<String>, String> {
        let syn::Type::Array(array) = &*item.ty else {
            return Err("REQUIRED_RISKS must be a fixed-size array".into());
        };
        let declared = match &array.len {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(n),
                ..
            }) => n.base10_parse::<usize>().map_err(|e| e.to_string())?,
            _ => return Err("REQUIRED_RISKS length must be a literal".into()),
        };
        let syn::Expr::Array(elements) = &*item.expr else {
            return Err("REQUIRED_RISKS must be an array literal".into());
        };
        let names = elements
            .elems
            .iter()
            .map(|e| match e {
                syn::Expr::Path(p) => p
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .ok_or_else(|| "empty REQUIRED_RISKS path".to_owned()),
                _ => Err("REQUIRED_RISKS elements must be paths".into()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if names.len() != declared || names.iter().collect::<BTreeSet<_>>().len() != names.len() {
            return Err("REQUIRED_RISKS length or uniqueness is inconsistent".into());
        }
        Ok(names)
    }
    let mut found = Vec::new();
    for (path, text) in sources {
        let file = syn::parse_file(text).map_err(|e| format!("{path}: {e}"))?;
        visit(&file.items, &mut found);
    }
    match found.len() {
        0 => Ok(Vec::new()),
        1 => found.pop().unwrap(),
        _ => Err("a target declares REQUIRED_RISKS more than once".into()),
    }
}

// Approval subjects use the same Python-compatible serialization as intake.
pub(super) use crate::representative::json::compact_into as python_json;
#[cfg(test)]
pub(super) use crate::representative::json::python_float;

/// `case.py subject`: SHA-256 of the canonical case record without its approval.
pub(super) fn case_subject(case: &Value) -> Result<String, String> {
    let mut body = case.clone();
    body.as_object_mut()
        .ok_or("case.json must be an object")?
        .remove("approval");
    let mut out = String::new();
    python_json(&body, &mut out)?;
    Ok(sha256(out.as_bytes()))
}

fn byte_descriptor(bytes: &[u8]) -> Value {
    json!({"bytes":bytes.len(),"sha256":sha256(bytes)})
}

/// One #84 case, checked as case.py checks it offline, plus the five-layer
/// requirements. Returns its record entry.
fn representative_case(repo: &Repo, entry: &Value) -> Result<Value, String> {
    let id = string(entry, "case_id")?;
    let dir = string(entry, "path")?;
    let fail = |problem: &str| format!("#84 case {id}: {problem}");
    let case_path = format!("{dir}/case.json");
    let case_bytes = repo.read(&case_path)?;
    let case: Value = parse(&case_bytes)?;
    if case["case_id"] != id
        || case["schema"] != "excalidraw-render/representative-case/v1"
        || case["layer"] != "representative"
        || case["originating_issue"] != ISSUE_84
    {
        return Err(fail("identity, schema, layer or originating issue"));
    }
    let mut inventory = serde_json::Map::new();
    for path in repo.list(dir)? {
        if path != case_path {
            inventory.insert(
                path[dir.len() + 1..].to_owned(),
                byte_descriptor(&repo.read(&path)?),
            );
        }
    }
    if case["files"] != Value::Object(inventory) {
        return Err(fail("retained case files changed"));
    }
    let bundle = string(&case["intake"], "path")?;
    let manifest_bytes = repo.read(&format!("{bundle}/manifest.json"))?;
    if case["intake"]["bytes"] != manifest_bytes.len()
        || case["intake"]["sha256"] != sha256(&manifest_bytes)
    {
        return Err(fail("intake bundle changed"));
    }
    let manifest: Value = parse(&manifest_bytes)?;
    let files = manifest["files"]
        .as_object()
        .ok_or_else(|| fail("intake files"))?;
    let mut bundle_files: BTreeSet<String> = repo
        .list(bundle)?
        .into_iter()
        .map(|p| p[bundle.len() + 1..].to_owned())
        .collect();
    bundle_files.remove("manifest.json");
    if manifest["case_id"] != id || bundle_files != files.keys().cloned().collect() {
        return Err(fail("intake identity or file inventory"));
    }
    for (name, descriptor) in files {
        if *descriptor != byte_descriptor(&repo.read(&format!("{bundle}/{name}"))?) {
            return Err(fail("intake file binding changed"));
        }
    }
    let scene = files
        .get("scene.excalidraw")
        .and_then(|d| d["sha256"].as_str())
        .ok_or_else(|| fail("intake has no retained scene"))?;
    if manifest["sanitization"]["retained_sha256"] != scene {
        return Err(fail(
            "sanitization retained_sha256 is not the retained scene digest",
        ));
    }
    let license = manifest["license"]
        .as_str()
        .filter(|l| !l.trim().is_empty())
        .ok_or_else(|| fail("no license"))?;
    if files
        .get("LICENSE")
        .and_then(|d| d["bytes"].as_u64())
        .unwrap_or(0)
        == 0
    {
        return Err(fail("no license text"));
    }
    let formats = case["formats"].as_array().ok_or_else(|| fail("formats"))?;
    let comparisons: Vec<Value> = COMPARISON_FORMATS
        .iter()
        .filter(|(_, f)| formats.contains(&json!(f)))
        .map(|(c, _)| json!(c))
        .collect();
    if comparisons.is_empty() || case["comparisons"] != Value::Array(comparisons.clone()) {
        return Err(fail(
            "comparisons are empty or differ from the registered formats",
        ));
    }
    let plan: Value = parse(&repo.read(&format!("{bundle}/plan.json"))?)?;
    let regions = plan["regions"]
        .as_array()
        .ok_or_else(|| fail("plan regions"))?;
    let crops: Vec<&str> = regions
        .iter()
        .filter(|r| {
            r["id"] != "whole" && r["element_ids"].as_array().is_some_and(|e| !e.is_empty())
        })
        .filter_map(|r| r["id"].as_str())
        .collect();
    if !regions.iter().any(|r| r["id"] == "whole") || crops.is_empty() {
        return Err(fail(
            "needs the whole judgment and at least one anchored crop",
        ));
    }
    let subject = case_subject(&case)?;
    let approval = case
        .get("approval")
        .ok_or_else(|| fail("missing historical approval field"))?;
    if !approval.is_null() {
        verify_approval_for_issue(approval, &subject, "85").map_err(|e| fail(&e))?;
    }
    Ok(
        json!({"case":id,"path":dir,"case_sha256":sha256(&case_bytes),
            "subject_sha256":format!("sha256:{subject}"),"approval_url":approval["approval_url"],
            "intake":{"path":bundle,"manifest_sha256":sha256(&manifest_bytes),"scene_sha256":scene,"license":license},
            "comparisons":comparisons,"crops":crops}),
    )
}

/// Validate actual representative case evidence independently of coverage maps.
pub(super) fn verify_representative_cases(repo: &Repo) -> Result<(), String> {
    representative_cases(repo).map(|_| ())
}

fn representative_cases(repo: &Repo) -> Result<Vec<Value>, String> {
    let registry: Value = parse(&repo.read(CASE_REGISTRY)?)?;
    if registry["schema"] != "excalidraw-render/representative-case-registry/v1" {
        return Err("invalid representative case registry schema".into());
    }
    array(&registry, "cases")?
        .iter()
        .map(|entry| representative_case(repo, entry))
        .collect()
}

fn exact_keys(value: &Value, keys: &[&str], item: &str) -> Result<(), String> {
    let actual: BTreeSet<&str> = value
        .as_object()
        .ok_or_else(|| format!("{item} must be an object"))?
        .keys()
        .map(String::as_str)
        .collect();
    if actual != keys.iter().copied().collect() {
        return Err(format!("{item} must have exactly the members {keys:?}"));
    }
    Ok(())
}

fn rationale(value: &Value, item: &str) -> Result<(), String> {
    if value["rationale"]
        .as_str()
        .is_none_or(|r| r.trim().is_empty())
    {
        return Err(format!("{item}: empty rationale"));
    }
    Ok(())
}

/// Declared items keyed by `key`, which must equal `discovered` in order.
fn declared<'a>(
    declarations: &'a Value,
    section: &str,
    key: &str,
    discovered: &[String],
) -> Result<Vec<&'a Value>, String> {
    let items: Vec<&Value> = declarations[section]
        .as_array()
        .ok_or_else(|| format!("declarations.{section} must be an array"))?
        .iter()
        .collect();
    let names: Vec<String> = items
        .iter()
        .map(|i| i[key].as_str().unwrap_or_default().to_owned())
        .collect();
    if names != discovered {
        let missing: Vec<_> = discovered.iter().filter(|d| !names.contains(d)).collect();
        let stale: Vec<_> = names.iter().filter(|n| !discovered.contains(n)).collect();
        return Err(format!(
            "declarations.{section} must declare every discovered item once, in order; undeclared: {missing:?}; not discovered: {stale:?}"
        ));
    }
    Ok(items)
}

/// Build the record from the repository and the authored declarations alone.
/// It reads neither the ownership catalog nor the ledger, so catalog, spec and
/// representative-successor churn never makes it stale. Structural failures
/// (an undeclared corpus or oracle, a changed family sentence, a failing #84
/// case) refuse.
pub(super) fn build_record(repo: &Repo, declarations: &Value) -> Result<Value, String> {
    exact_keys(
        declarations,
        &[
            "vector_families",
            "vector_domains",
            "vector_corpora",
            "oracles",
        ],
        "declarations",
    )?;
    let spec = text(repo, SPEC)?;
    for rule in RULES {
        if !spec.contains(rule.spec_anchor) {
            return Err(format!("stale spec anchor: {}", rule.spec_anchor));
        }
    }

    // Layer 1: exact vectors. Rows are selectors, expanded at verification.
    let (sentence, families) = spec_families(&spec)?;
    for (item, family) in declared(declarations, "vector_families", "family", &families)?
        .into_iter()
        .zip(&families)
    {
        exact_keys(item, &["family", "rows", "rationale"], family)?;
        rationale(item, family)?;
    }
    let domain_names: Vec<String> = declarations["vector_domains"]
        .as_array()
        .ok_or("declarations.vector_domains must be an array")?
        .iter()
        .map(|d| d["domain"].as_str().unwrap_or_default().to_owned())
        .collect();
    if domain_names.windows(2).any(|w| w[0] >= w[1]) || domain_names.iter().any(|d| d.is_empty()) {
        return Err("declarations.vector_domains must be named and sorted".into());
    }
    for item in declared(declarations, "vector_domains", "domain", &domain_names)? {
        let domain = string(item, "domain")?;
        exact_keys(
            item,
            &["domain", "spec_anchor", "rows", "rationale"],
            domain,
        )?;
        rationale(item, domain)?;
        let anchor = string(item, "spec_anchor")?;
        if anchor.trim().is_empty() || !spec.contains(anchor) {
            return Err(format!("stale spec anchor: {anchor}"));
        }
    }
    let discovered = corpora(repo)?;
    let paths: Vec<String> = discovered.iter().map(|c| c.0.clone()).collect();
    let mut corpus_items = Vec::new();
    for (item, (path, kind, files)) in declared(declarations, "vector_corpora", "path", &paths)?
        .into_iter()
        .zip(&discovered)
    {
        exact_keys(item, &["path", "rows", "rationale"], path)?;
        rationale(item, path)?;
        corpus_items.push(json!({"path":path,"kind":kind,"tree":tree(repo, files)?}));
    }
    let oracle_dirs: Vec<String> = dirs(repo, ORACLE_ROOT)?
        .into_iter()
        .map(|d| format!("{ORACLE_ROOT}/{d}"))
        .collect();
    for (item, dir) in declared(declarations, "oracles", "directory", &oracle_dirs)?
        .into_iter()
        .zip(&oracle_dirs)
    {
        rationale(item, dir)?;
        match string(item, "class")? {
            "vector" => exact_keys(item, &["directory", "class", "rows", "rationale"], dir)?,
            "authority" | "profile" => exact_keys(item, &["directory", "class", "rationale"], dir)?,
            other => return Err(format!("{dir}: unknown oracle class {other}")),
        }
    }

    // Layer 2: registered cases, by id.
    let registry: Value = parse(&repo.read(REGISTRY)?)?;
    let cases = array(&registry, "cases")?
        .iter()
        .map(|case| string(case, "id").map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;

    // Layer 3: the #84 cases. Candidates are checked by rule at verification.
    let representative_items = representative_cases(repo)?;

    // Layer 4: adversarial targets and their declared risks.
    let mut target_items = Vec::new();
    for (target, files) in targets(repo)? {
        let name = target
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .replace('_', "-");
        let sources = files
            .iter()
            .filter(|f| f.ends_with(".rs"))
            .map(|f| Ok((f.clone(), text(repo, f)?)))
            .collect::<Result<Vec<_>, String>>()?;
        let risks = required_risks(&sources).map_err(|e| format!("{target}: {e}"))?;
        target_items.push(json!({"target":target,"row":format!("layer:adversarial:{name}"),"required_risks":risks}));
    }

    // Layer 5: regression corpora.
    let mut regression_items = Vec::new();
    for krate in dirs(repo, "crates")? {
        let dir = format!("crates/{krate}/tests/qualification");
        for (name, is_dir) in repo.children(&dir)? {
            let Some(stem) = name.strip_suffix("-regressions.json").filter(|_| !is_dir) else {
                continue;
            };
            let path = format!("{dir}/{name}");
            let bytes = repo.read(&path)?;
            let corpus: Value = parse(&bytes)?;
            let ids = |member: &str| -> Result<Vec<String>, String> {
                corpus[member]
                    .as_array()
                    .map(|cases| {
                        cases
                            .iter()
                            .map(|c| string(c, "id").map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or(Ok(Vec::new()))
            };
            let case_ids = ids("cases")?;
            let public = ids("publicFacadeCases")?;
            if [&case_ids, &public]
                .iter()
                .any(|ids| ids.iter().collect::<BTreeSet<_>>().len() != ids.len())
            {
                return Err(format!("{path}: duplicate regression case id"));
            }
            let risks: BTreeSet<&str> = array(&corpus, "cases")?
                .iter()
                .filter_map(|c| c["risk"].as_str())
                .collect();
            regression_items.push(
                json!({"path":path,"sha256":sha256(&bytes),"byte_length":bytes.len(),
                "row":format!("layer:regression:{stem}"),"case_ids":case_ids,"public_facade_case_ids":public,"risks":risks}),
            );
        }
    }

    Ok(json!({
        "schema": SCHEMA,
        "compatibility_revision": 1,
        "obligation": "closure:all-five-layers",
        "approving_issue": 85,
        "implementation": source(repo, IMPLEMENTATION)?,
        "spec": {"path": SPEC, "family_sentence": sentence},
        "rules": RULES.iter().map(|r| json!({"id":r.id,"layer":r.layer,"statement":r.statement,"spec_anchor":r.spec_anchor})).collect::<Vec<_>>(),
        "declarations": declarations,
        "vector": {"corpora": corpus_items},
        "focused": {"cases": cases},
        "representative": {"cases": representative_items},
        "adversarial": {"targets": target_items},
        "regression": {"corpora": regression_items},
        "approval": Value::Null,
    }))
}

fn catalog_rows(view: &LedgerView) -> BTreeMap<String, (String, Vec<String>)> {
    view.rows()
        .map(|r| {
            (
                r.id.to_owned(),
                (r.category.to_owned(), r.candidates.to_vec()),
            )
        })
        .collect()
}

/// Every completeness problem of a structurally current record against the
/// current catalog, ledger and candidate registry, in record order. An empty
/// list is complete.
pub(super) fn completeness(record: &Value, repo: &Repo, view: &LedgerView) -> Vec<String> {
    let mut problems = Vec::new();
    if let Err(e) = view.catalog_sha256() {
        problems.push(format!("ledger: {e}"));
    }
    let catalog = catalog_rows(view);
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    let owner = |item: &str, row: &str, problems: &mut Vec<String>| match view.owner(row) {
        Ok(owner) => owner,
        Err(e) => {
            problems.push(format!("{item}: {row}: {e}"));
            None
        }
    };

    // Layer 1: every declared vector item's selectors expand to existing rows,
    // at least one owned, every mapped layer row owned.
    let declarations = &record["declarations"];
    for (section, key) in [
        ("vector_families", "family"),
        ("vector_domains", "domain"),
        ("vector_corpora", "path"),
        ("oracles", "directory"),
    ] {
        for item in declarations[section].as_array().into_iter().flatten() {
            let name = format!("vector {}", item[key].as_str().unwrap_or_default());
            if item.get("rows").is_none() {
                continue; // an authority or profile input
            }
            let rows = match expand(&item["rows"], &catalog, &name) {
                Ok(rows) => rows,
                Err(e) => {
                    problems.push(e);
                    continue;
                }
            };
            referenced.extend(rows.iter().cloned());
            let mut owned = 0;
            for row in &rows {
                match owner(&name, row, &mut problems) {
                    Some(_) => owned += 1,
                    None if row.starts_with("layer:") => {
                        problems.push(format!("{name}: layer row {row} has no primary owner"))
                    }
                    None => {}
                }
            }
            if owned == 0 {
                problems.push(format!("{name}: no mapped row is owned"));
            }
        }
    }

    // Layer 2: each registered case owns one of the rows naming it.
    for case in record["focused"]["cases"].as_array().into_iter().flatten() {
        let case = case.as_str().unwrap_or_default();
        let name = format!("registered case {case}");
        let rows: Vec<&String> = catalog
            .iter()
            .filter(|(_, (category, candidates))| {
                matches!(category.as_str(), "case-label" | "layer")
                    && candidates.iter().any(|c| c == case)
            })
            .map(|(row, _)| row)
            .collect();
        let mut owns = false;
        for row in &rows {
            if row.starts_with("layer:") && !row.starts_with("layer:focused:") {
                problems.push(format!(
                    "{name}: layer row {row} is not a focused-layer row"
                ));
            }
            referenced.insert((*row).clone());
            owns |= owner(&name, row, &mut problems) == Some(PrimaryOwner::Case(case.into()));
        }
        if !owns {
            problems.push(format!(
                "{name} is the case owner of none of its rows {rows:?}"
            ));
        }
    }

    // Layer 3: exactly one registered candidate, the one the representative
    // owner binds; every other candidates/* directory is excluded by rule.
    match representative_candidate(repo) {
        Ok((case, rows)) => {
            let name = format!("representative candidate {case}");
            if !rows.iter().any(|r| r == REPRESENTATIVE_ROW) {
                problems.push(format!("{name} does not cover {REPRESENTATIVE_ROW}"));
            }
            for row in &rows {
                if !catalog.contains_key(row)
                    || row.starts_with("closure:")
                    || row.starts_with("layer:")
                {
                    problems.push(format!("{name}: {row} is not a catalog interaction row"));
                } else if owner(&name, row, &mut problems)
                    != Some(PrimaryOwner::Representative(case.clone()))
                {
                    problems.push(format!(
                        "{name}: {row} is not owned by the representative binding of this entry"
                    ));
                }
            }
        }
        Err(e) => problems.push(format!("representative candidates: {e}")),
    }

    // Layers 4 and 5: each target and corpus row exists, once, and is owned.
    let mut rows_seen = BTreeSet::new();
    for (section, list, key) in [
        ("adversarial", "targets", "target"),
        ("regression", "corpora", "path"),
    ] {
        for item in record[section][list].as_array().into_iter().flatten() {
            let name = format!("{section} {}", item[key].as_str().unwrap_or_default());
            let row = item["row"].as_str().unwrap_or_default();
            if !catalog.contains_key(row) {
                problems.push(format!("{name} has no catalog row {row}"));
                continue;
            }
            if !rows_seen.insert(row) {
                problems.push(format!("{name}: more than one item maps to {row}"));
            }
            referenced.insert(row.to_owned());
            if owner(&name, row, &mut problems).is_none() {
                problems.push(format!("{name}: {row} has no primary owner"));
            }
        }
    }

    // No orphan layer rows.
    let orphans: Vec<&String> = catalog
        .keys()
        .filter(|id| id.starts_with("layer:") && !referenced.contains(*id))
        .collect();
    if !orphans.is_empty() {
        problems.push(format!(
            "orphan layer row(s) no item references: {orphans:?}"
        ));
    }
    problems.dedup();
    problems
}

/// The candidate registry's single representative entry: (case, coverage).
fn representative_candidate(repo: &Repo) -> Result<(String, Vec<String>), String> {
    let registry: Value = parse(&repo.read(CANDIDATE_REGISTRY)?)?;
    if registry["schema"] != "excalidraw-render/representative-corpus-adapter/v1" {
        return Err("invalid candidate registry schema".into());
    }
    let [entry] = array(&registry, "cases")?.as_slice() else {
        return Err("the candidate registry must hold exactly one representative entry".into());
    };
    let rows = array(entry, "coverage")?
        .iter()
        .map(|r| {
            r.as_str()
                .map(str::to_owned)
                .ok_or("coverage rows are strings")
        })
        .collect::<Result<_, _>>()?;
    Ok((string(entry, "id")?.to_owned(), rows))
}
