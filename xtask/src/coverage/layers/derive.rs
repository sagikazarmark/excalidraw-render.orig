//! Discovery, derivation and completeness rules for `layers.json`
//! (`closure:all-five-layers`, ADR-0015). This file is bound by digest in the
//! record, so a change to any discovery rule, derived field or completeness
//! rule requires a fresh #85 approval. I/O, approval and commands live in the
//! parent.
//!
//! Every item is discovered by a rule below from the repository and the
//! ownership catalog, never from ledger dispositions, so recording an owner
//! does not change the record. Completeness then reads the ledger: each item
//! must map to owned rows whose owners pass the ledger's own checks.
use super::*;
use crate::coverage::ownership::PrimaryOwner;

pub(super) const IMPLEMENTATION: &str = "xtask/src/coverage/layers/derive.rs";
pub(super) const SPEC: &str = "docs/renderer-spec-v1.md";
const LEDGER: &str = "tests/conformance/coverage/ownership.json";
const CANDIDATE_REGISTRY: &str = "tests/conformance/candidates/registry.json";
const CANDIDATE_ROOT: &str = "tests/conformance/candidates";
const CASE_REGISTRY: &str = "tests/conformance/cases/representative/registry.json";
const INTAKE_ROOT: &str = "tests/conformance/intake";
const PUBLIC_SCENES: &str = "tests/public-scenes";
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
        statement: "The exact-vector families are the comma-separated list of the spec sentence beginning \"Exact vectors cover \", in order. Each is declared once, in that order, with row selectors (an exact catalog id, or a prefix ending in \":*\" that matches at least one row), expanded against the catalog.",
        spec_anchor: "Exact vectors cover Rough.js",
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
        statement: "Stored vector corpora are every tests/conformance/comparator-vectors/*/ directory (which must hold manifest.json), every crates/*/tests/fixtures/*-vectors.json file and every tools/*-vectors/ derivation directory. Each is declared exactly once with row selectors; its bytes are bound by tree digest.",
        spec_anchor: "1. Exact compatibility-critical vectors.",
    },
    Rule {
        id: "oracle-classification",
        layer: "vector",
        statement: "Every tests/oracle/*/ directory is declared exactly once: a vector oracle maps to the codec/profile rows its pinned reference values decide; an authority or profile input maps to no row. Its bytes are bound by tree digest.",
        spec_anchor: "1. Exact compatibility-critical vectors.",
    },
    Rule {
        id: "registered-cases",
        layer: "focused",
        statement: "Every case of tests/conformance/cases/registry.json maps to the case-label and layer rows that name it as a candidate, and must be the case owner of at least one of them.",
        spec_anchor: "2. Focused scene documents with official SVG/PNG evidence.",
    },
    Rule {
        id: "representative-candidates",
        layer: "representative",
        statement: "Every entry of tests/conformance/candidates/registry.json maps to its coverage rows, each owned by the representative owner bound to that entry. Unregistered candidate directories are excluded and listed.",
        spec_anchor: "3. Representative licensed or project-authored scene documents.",
    },
    Rule {
        id: "representative-cases",
        layer: "representative",
        statement: "Every #84 case of tests/conformance/cases/representative/registry.json is checked here as tools/representative-intake/case.py checks it: its case.json is approved on a #85 comment binding its subject (case.py's subject computation), its retained-file inventory and intake manifest digest are current, the intake's sanitization retained_sha256 is the retained scene's digest, a license is present, and it has comparisons and at least one anchored crop besides whole. Unreferenced intake bundles and tests/public-scenes entries are excluded and listed.",
        spec_anchor: "External scenes require consent or redistribution-compatible licensing and are",
    },
    Rule {
        id: "adversarial-targets",
        layer: "adversarial",
        statement: "Fuzz and property targets are crates/*/src/**/*_fuzz* (a module file or directory, outermost match), crates/*/tests/*_fuzz* and crates/*/tests/limits_* (a test binary with its module directory), and crates/*/{src,tests}/**/frame_batch_property. Each maps to exactly the row layer:adversarial:<name with - for _>, and records the REQUIRED_RISKS it declares.",
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
        statement: "Every catalog row layer:<layer>:* is referenced by an item of its own layer; closure rows are never mapped.",
        spec_anchor: "The conformance corpus has five layers:",
    },
    Rule {
        id: "completeness",
        layer: "all",
        statement: "Each vector item maps to at least one owned row; every mapped layer row is owned; every owned mapped row's owner passes the ledger's checks (a case owner also passes its conformance verification). Registered cases, candidates, fuzz targets and regression corpora are owned as their rules state.",
        spec_anchor: "exactly one primary record or approved exclusion.",
    },
];

/// The catalog rows the record maps into, and the catalog digest the ledger binds.
pub(super) struct CatalogRows {
    pub sha256: String,
    pub rows: BTreeMap<String, (String, Vec<String>)>,
}

impl CatalogRows {
    pub(super) fn of(view: &LedgerView) -> Result<Self, String> {
        Ok(Self {
            sha256: view.catalog_sha256()?,
            rows: view
                .rows()
                .map(|r| {
                    (
                        r.id.to_owned(),
                        (r.category.to_owned(), r.candidates.to_vec()),
                    )
                })
                .collect(),
        })
    }
}

fn text(repo: &Repo, path: &str) -> Result<String, String> {
    String::from_utf8(repo.read(path)?).map_err(|_| format!("{path} is not UTF-8"))
}

fn source(repo: &Repo, path: &str) -> Result<Value, String> {
    Ok(json!({"path":path,"sha256":sha256(&repo.read(path)?)}))
}

/// The ten (or however many) families of the spec's exact-vector sentence.
pub(super) fn spec_families(spec: &str) -> Result<Vec<String>, String> {
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
    Ok(families)
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
                .rows
                .keys()
                .filter(|id| id.starts_with(&format!("{prefix}:")))
                .cloned()
                .collect(),
            None if catalog.rows.contains_key(selector) => vec![selector.to_owned()],
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

/// `json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)`.
pub(super) fn python_json(value: &Value, out: &mut String) -> Result<(), String> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => out.push_str(&i.to_string()),
            (_, Some(u), _) => out.push_str(&u.to_string()),
            (_, _, Some(f)) => out.push_str(&python_float(f)?),
            _ => return Err("unsupported JSON number".into()),
        },
        Value::String(s) => out.push_str(&serde_json::to_string(s).map_err(|e| e.to_string())?),
        Value::Array(values) => {
            out.push('[');
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                python_json(v, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).map_err(|e| e.to_string())?);
                out.push(':');
                python_json(&map[k], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Python's `repr(float)`: shortest round-trip digits, positional for
/// decimal exponents -4..16, otherwise `d.ddde±XX`.
pub(super) fn python_float(f: f64) -> Result<String, String> {
    if !f.is_finite() {
        return Err("non-finite JSON number".into());
    }
    if f == 0.0 {
        return Ok(if f.is_sign_negative() { "-0.0" } else { "0.0" }.into());
    }
    let formatted = format!("{:e}", f.abs());
    let (mantissa, exponent) = formatted.split_once('e').ok_or("float format")?;
    let exp: i32 = exponent.parse().map_err(|_| "float exponent")?;
    let digits = mantissa.replace('.', "");
    let n = digits.len() as i32;
    let body = if (-4..16).contains(&exp) {
        if exp < 0 {
            format!("0.{}{digits}", "0".repeat((-exp - 1) as usize))
        } else if n <= exp + 1 {
            format!("{digits}{}.0", "0".repeat((exp + 1 - n) as usize))
        } else {
            let (int, frac) = digits.split_at((exp + 1) as usize);
            format!("{int}.{frac}")
        }
    } else {
        let (first, rest) = digits.split_at(1);
        let mantissa = if rest.is_empty() {
            first.to_owned()
        } else {
            format!("{first}.{rest}")
        };
        format!(
            "{mantissa}e{}{:02}",
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        )
    };
    Ok(format!("{}{body}", if f < 0.0 { "-" } else { "" }))
}

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
/// requirements. Returns its record entry and intake bundle directory.
fn representative_case(repo: &Repo, entry: &Value) -> Result<(Value, String), String> {
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
    let approval = &case["approval"];
    if approval.is_null() {
        return Err(fail("case.json is not approved"));
    }
    verify_approval_for_issue(approval, &subject, "85").map_err(|e| fail(&e))?;
    Ok((
        json!({"case":id,"path":dir,"case_sha256":sha256(&case_bytes),
            "subject_sha256":format!("sha256:{subject}"),"approval_url":approval["approval_url"],
            "intake":{"path":bundle,"manifest_sha256":sha256(&manifest_bytes),"scene_sha256":scene,"license":license},
            "comparisons":comparisons,"crops":crops}),
        bundle.to_owned(),
    ))
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

/// Build the record from the repository, the catalog and the authored
/// declarations. Structural failures (an undeclared corpus or oracle, a target
/// or corpus without its row, an orphan layer row, a failing #84 case) refuse.
pub(super) fn build_record(
    repo: &Repo,
    catalog: &CatalogRows,
    declarations: &Value,
) -> Result<Value, String> {
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
    let mut referenced: BTreeSet<String> = BTreeSet::new();

    // Layer 1: exact vectors.
    let families = spec_families(&spec)?;
    let mut family_items = Vec::new();
    for (item, family) in declared(declarations, "vector_families", "family", &families)?
        .into_iter()
        .zip(&families)
    {
        exact_keys(item, &["family", "rows", "rationale"], family)?;
        rationale(item, family)?;
        let rows = expand(&item["rows"], catalog, family)?;
        referenced.extend(rows.iter().cloned());
        family_items.push(json!({"family":family,"rows":rows}));
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
    let mut domain_items = Vec::new();
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
        let rows = expand(&item["rows"], catalog, domain)?;
        referenced.extend(rows.iter().cloned());
        domain_items.push(json!({"domain":domain,"spec_anchor":anchor,"rows":rows}));
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
        let rows = expand(&item["rows"], catalog, path)?;
        referenced.extend(rows.iter().cloned());
        corpus_items.push(json!({"path":path,"kind":kind,"tree":tree(repo, files)?,"rows":rows}));
    }
    let oracle_dirs: Vec<String> = dirs(repo, ORACLE_ROOT)?
        .into_iter()
        .map(|d| format!("{ORACLE_ROOT}/{d}"))
        .collect();
    let mut oracle_items = Vec::new();
    for (item, dir) in declared(declarations, "oracles", "directory", &oracle_dirs)?
        .into_iter()
        .zip(&oracle_dirs)
    {
        rationale(item, dir)?;
        let class = string(item, "class")?;
        let rows = match class {
            "vector" => {
                exact_keys(item, &["directory", "class", "rows", "rationale"], dir)?;
                expand(&item["rows"], catalog, dir)?
            }
            "authority" | "profile" => {
                exact_keys(item, &["directory", "class", "rationale"], dir)?;
                Vec::new()
            }
            other => return Err(format!("{dir}: unknown oracle class {other}")),
        };
        referenced.extend(rows.iter().cloned());
        oracle_items.push(
            json!({"directory":dir,"class":class,"tree":tree(repo, &repo.list(dir)?)?,"rows":rows}),
        );
    }

    // Layer 2: registered cases.
    let registry: Value = parse(&repo.read(REGISTRY)?)?;
    let mut case_items = Vec::new();
    for case in array(&registry, "cases")? {
        let id = string(case, "id")?;
        let rows: Vec<String> = catalog
            .rows
            .iter()
            .filter(|(_, (category, candidates))| {
                matches!(category.as_str(), "case-label" | "layer")
                    && candidates.iter().any(|c| c == id)
            })
            .map(|(row, _)| row.clone())
            .collect();
        if let Some(bad) = rows
            .iter()
            .find(|r| r.starts_with("layer:") && !r.starts_with("layer:focused:"))
        {
            return Err(format!(
                "case {id}: layer row {bad} is not a focused-layer row"
            ));
        }
        referenced.extend(rows.iter().cloned());
        case_items.push(json!({"case":id,"layer":case["layer"],"rows":rows}));
    }

    // Layer 3: representative candidates and #84 cases.
    let candidates: Value = parse(&repo.read(CANDIDATE_REGISTRY)?)?;
    if candidates["schema"] != "excalidraw-render/representative-corpus-adapter/v1" {
        return Err("invalid candidate registry schema".into());
    }
    let mut candidate_items = Vec::new();
    let mut registered_dirs = BTreeSet::new();
    for entry in array(&candidates, "cases")? {
        let id = string(entry, "id")?;
        let directory = string(entry, "directory")?;
        registered_dirs.insert(directory.to_owned());
        let rows: Vec<String> = array(entry, "coverage")?
            .iter()
            .map(|r| {
                r.as_str()
                    .map(str::to_owned)
                    .ok_or("coverage rows are strings")
            })
            .collect::<Result<_, _>>()?;
        if rows.is_empty()
            || rows.iter().any(|r| {
                !catalog.rows.contains_key(r)
                    || r.starts_with("closure:")
                    || r.starts_with("layer:")
            })
        {
            return Err(format!(
                "candidate {id}: coverage must name catalog interaction rows"
            ));
        }
        candidate_items.push(json!({"case":id,"directory":directory,"rows":rows}));
    }
    let excluded_candidates: Vec<String> = dirs(repo, CANDIDATE_ROOT)?
        .into_iter()
        .map(|d| format!("{CANDIDATE_ROOT}/{d}"))
        .filter(|d| !registered_dirs.contains(d))
        .collect();
    let representative: Value = parse(&repo.read(CASE_REGISTRY)?)?;
    if representative["schema"] != "excalidraw-render/representative-case-registry/v1" {
        return Err("invalid representative case registry schema".into());
    }
    let mut representative_items = Vec::new();
    let mut bundles = BTreeSet::new();
    for entry in array(&representative, "cases")? {
        let (item, bundle) = representative_case(repo, entry)?;
        representative_items.push(item);
        bundles.insert(bundle);
    }
    let excluded_intake: Vec<String> = dirs(repo, INTAKE_ROOT)?
        .into_iter()
        .map(|d| format!("{INTAKE_ROOT}/{d}"))
        .filter(|d| !bundles.contains(d))
        .collect();
    let public_scenes: Vec<String> = repo
        .children(PUBLIC_SCENES)?
        .into_iter()
        .map(|(name, _)| format!("{PUBLIC_SCENES}/{name}"))
        .collect();

    // Layer 4: adversarial targets.
    let mut target_items = Vec::new();
    let mut target_rows = BTreeSet::new();
    for (target, files) in targets(repo)? {
        let name = target
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .replace('_', "-");
        let row = format!("layer:adversarial:{name}");
        if !catalog.rows.contains_key(&row) {
            return Err(format!(
                "adversarial target {target} has no catalog row {row}"
            ));
        }
        if !target_rows.insert(row.clone()) {
            return Err(format!("more than one adversarial target maps to {row}"));
        }
        let sources = files
            .iter()
            .filter(|f| f.ends_with(".rs"))
            .map(|f| Ok((f.clone(), text(repo, f)?)))
            .collect::<Result<Vec<_>, String>>()?;
        let risks = required_risks(&sources).map_err(|e| format!("{target}: {e}"))?;
        referenced.insert(row.clone());
        target_items.push(
            json!({"target":target,"tree":tree(repo, &files)?,"row":row,"required_risks":risks}),
        );
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
            let row = format!("layer:regression:{stem}");
            if !catalog.rows.contains_key(&row) {
                return Err(format!("regression corpus {path} has no catalog row {row}"));
            }
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
            referenced.insert(row.clone());
            regression_items.push(
                json!({"path":path,"sha256":sha256(&bytes),"byte_length":bytes.len(),
                "row":row,"case_ids":case_ids,"public_facade_case_ids":public,"risks":risks}),
            );
        }
    }

    // No orphan layer rows.
    let orphans: Vec<&String> = catalog
        .rows
        .keys()
        .filter(|id| id.starts_with("layer:") && !referenced.contains(*id))
        .collect();
    if !orphans.is_empty() {
        return Err(format!(
            "orphan layer row(s) no item references: {orphans:?}"
        ));
    }

    Ok(json!({
        "schema": SCHEMA,
        "compatibility_revision": 1,
        "obligation": "closure:all-five-layers",
        "approving_issue": 85,
        "implementation": source(repo, IMPLEMENTATION)?,
        "spec": source(repo, SPEC)?,
        "ledger": {"path": LEDGER, "catalog_sha256": catalog.sha256},
        "rules": RULES.iter().map(|r| json!({"id":r.id,"layer":r.layer,"statement":r.statement,"spec_anchor":r.spec_anchor})).collect::<Vec<_>>(),
        "declarations": declarations,
        "vector": {"families": family_items, "domains": domain_items, "corpora": corpus_items, "oracles": oracle_items},
        "focused": {"registry": source(repo, REGISTRY)?, "cases": case_items},
        "representative": {
            "candidate_registry": source(repo, CANDIDATE_REGISTRY)?,
            "candidates": candidate_items,
            "case_registry": source(repo, CASE_REGISTRY)?,
            "cases": representative_items,
            "excluded": {"candidate_directories": excluded_candidates, "intake_bundles": excluded_intake, "public_scenes": public_scenes},
        },
        "adversarial": {"targets": target_items},
        "regression": {"corpora": regression_items},
        "approval": Value::Null,
    }))
}

fn rows_of(item: &Value) -> Vec<&str> {
    item["rows"]
        .as_array()
        .map(|rows| rows.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Every completeness problem of a structurally current record, in record
/// order. An empty list is complete.
pub(super) fn completeness(record: &Value, view: &LedgerView) -> Vec<String> {
    let mut problems = Vec::new();
    let owner = |item: &str, row: &str, problems: &mut Vec<String>| match view.owner(row) {
        Ok(owner) => owner,
        Err(e) => {
            problems.push(format!("{item}: {row}: {e}"));
            None
        }
    };
    let vector = &record["vector"];
    for (section, key) in [
        ("families", "family"),
        ("domains", "domain"),
        ("corpora", "path"),
        ("oracles", "directory"),
    ] {
        for item in vector[section].as_array().into_iter().flatten() {
            let name = format!("vector {}", item[key].as_str().unwrap_or_default());
            let rows = rows_of(item);
            if rows.is_empty() {
                continue; // an authority or profile input
            }
            let mut owned = 0;
            for row in rows {
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
    for item in record["focused"]["cases"].as_array().into_iter().flatten() {
        let case = item["case"].as_str().unwrap_or_default();
        let name = format!("registered case {case}");
        let mut owns = false;
        for row in rows_of(item) {
            owns |= owner(&name, row, &mut problems) == Some(PrimaryOwner::Case(case.into()));
        }
        if !owns {
            problems.push(format!(
                "{name} is the case owner of none of its rows {:?}",
                rows_of(item)
            ));
        }
    }
    for item in record["representative"]["candidates"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let case = item["case"].as_str().unwrap_or_default();
        let name = format!("representative candidate {case}");
        for row in rows_of(item) {
            if owner(&name, row, &mut problems) != Some(PrimaryOwner::Representative(case.into())) {
                problems.push(format!(
                    "{name}: {row} is not owned by its representative binding"
                ));
            }
        }
    }
    for (section, list, key) in [
        ("adversarial", "targets", "target"),
        ("regression", "corpora", "path"),
    ] {
        for item in record[section][list].as_array().into_iter().flatten() {
            let name = format!("{section} {}", item[key].as_str().unwrap_or_default());
            let row = item["row"].as_str().unwrap_or_default();
            if owner(&name, row, &mut problems).is_none() {
                problems.push(format!("{name}: {row} has no primary owner"));
            }
        }
    }
    problems.dedup();
    problems
}
