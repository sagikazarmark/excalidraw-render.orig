//! Evidence-producing derivation for `pdf-judgments.json`. This file is bound by
//! digest in the record (like the native baseline producer), so a change to any
//! derivation rule, report serialization, or judgment classification requires a
//! fresh #85 approval. Verification and replay consumers live in the parent.
//!
//! Expected PDF reports are derived only from approved declarations: the case
//! registry's `expected_diagnostics` and the manifests' expected reports, plus
//! the record's authored per-case PDF declarations. Renderer output is never an
//! input; the native baseline is consulted only by the separate cross-check.
use super::*;

pub(super) const IMPLEMENTATION: &str = "xtask/src/coverage/pdf_judgments/derive.rs";

/// One spec-anchored transformation rule. Anchors are exact substrings of the
/// renderer specification, checked on every verification.
pub(super) struct Rule {
    pub id: &'static str,
    pub statement: &'static str,
    pub spec_anchor: &'static str,
}

pub(super) const RULES: &[Rule] = &[
    Rule {
        id: "svg-authority",
        statement: "A PDF execution's diagnostics are the case's approved SVG declaration in order, each with format rewritten to \"pdf\"; official PNG is not authoritative where formats differ.",
        spec_anchor: "Official PNG is not authoritative",
    },
    Rule {
        id: "closed-code-table",
        statement: "Only codes in the format table transfer; any other declared code fails derivation until a spec-anchored format rule is added.",
        spec_anchor: "Only animation flattening and the two declared font substitutions are",
    },
    Rule {
        id: "png-consistency",
        statement: "The PNG declaration must equal the SVG declaration minus PNG-silent codes, format rewritten to \"png\"; otherwise the two approved declarations disagree beyond the spec's format rules.",
        spec_anchor: "| `link.unsafe-omitted` | Warning in SVG/PDF; PNG silent |",
    },
    Rule {
        id: "format-neutral-manifest-report",
        statement: "Typography and frame manifests carry one expected report for every format; it is transferred only when it is the canonical empty report, which has no format member.",
        spec_anchor: "A standalone empty report is exactly this JSON, with no trailing LF:",
    },
    Rule {
        id: "pdf-only-declarations",
        statement: "pdf.* diagnostics exist only in PDF and are never inferred: each case carries an explicit declaration (possibly empty) inserted at insert_at. A scene whose nested SVG payload contains a pixel-domain filter must declare its pdf.raster-fallback warnings.",
        spec_anchor: "`AllowIsolated` emits exactly one `pdf.raster-fallback` warning per emitted",
    },
    Rule {
        id: "report-serialization",
        statement: "Report bytes are minified schema-1 JSON in the declared member order: schema_version, diagnostics; diagnostic members code, severity, format, effects, element_id, json_pointer, message, details; details members in the order of their closed kind.",
        spec_anchor: "Diagnostics have this member order:",
    },
];

/// Diagnostic codes whose PDF presence the derivation may infer. `png` is
/// whether PNG reports the same occurrence.
pub(super) struct FormatRule {
    pub code: &'static str,
    pub png: bool,
    pub spec_anchor: &'static str,
}

pub(super) const FORMAT_RULES: &[FormatRule] = &[
    FormatRule {
        code: "font.liberation-sans-substituted",
        png: true,
        spec_anchor: "| `font.liberation-sans-substituted` | Info; local-font substitution |",
    },
    FormatRule {
        code: "font.missing-glyph",
        png: true,
        spec_anchor: "| `font.missing-glyph` | Warning; replacement glyph |",
    },
    FormatRule {
        code: "font.noto-emoji-substituted",
        png: true,
        spec_anchor: "| `font.noto-emoji-substituted` | Info; emoji-font substitution |",
    },
    FormatRule {
        code: "link.unsafe-omitted",
        png: false,
        spec_anchor: "| `link.unsafe-omitted` | Warning in SVG/PDF; PNG silent |",
    },
];

/// Where PDF judgment files are searched: every file under `SCOPE_ROOT` whose
/// repository-relative path contains `pdf` (ASCII case-insensitive), except in
/// these subtrees, which carry their own records (candidate/proposal evidence,
/// the ownership ledger, native baseline, application transactions and this
/// record itself).
pub(super) const SCOPE_ROOT: &str = "tests/conformance";
pub(super) const EXCLUDED_SUBTREES: &[&str] = &[
    "tests/conformance/candidates",
    "tests/conformance/coverage",
    "tests/conformance/proposals",
];

/// First matching pattern classifies a discovered file. Unmatched files fail
/// verification as unbound PDF judgment files.
pub(super) const JUDGMENT_PATTERNS: &[(&str, &str)] = &[
    (
        "tests/conformance/pdf-backend-qualification/*-structure.json",
        "backend-structure-expectation",
    ),
    (
        "tests/conformance/pdf-viewer-qualification/*/manifest.json",
        "viewer-qualification-manifest",
    ),
    (
        "tests/conformance/pdf-viewer-qualification/*/expected-*.json",
        "viewer-structure-expectation",
    ),
    (
        "tests/conformance/structure-profiles/*pdf*.json",
        "structure-profile",
    ),
    (
        "tests/conformance/viewer-profiles/*pdf*.json",
        "viewer-profile",
    ),
    (
        "tests/conformance/cases/focused/*/expected-pdf-structure.json",
        "case-structure-expectation",
    ),
    (
        "tests/conformance/cases/focused/*/pdf-visual-exception.json",
        "case-visual-exception",
    ),
    (
        "tests/conformance/cases/focused/*/pdf-tolerance-exceptions.json",
        "case-visual-exception",
    ),
    (
        "tests/conformance/cases/focused/*/pdf-renderer-expectations.json",
        "case-renderer-expectations",
    ),
    (
        "tests/conformance/cases/representative/*/expected-report-pdf.json",
        "representative-expected-report",
    ),
];

pub(super) const NON_JUDGMENT_PATTERNS: &[(&str, &str, &str)] = &[
    (
        "tests/conformance/*.schema.json",
        "schema",
        "Format definition validated by its consumers; carries no case judgment.",
    ),
    (
        "tests/conformance/pdf-viewer-qualification/*/evidence/*.json",
        "measured-evidence",
        "Measurement output bound by digest in its approved qualification manifest.",
    ),
    (
        "**/*.sha256",
        "digest-sidecar",
        "Digest of a sibling record; the record itself is bound here.",
    ),
    (
        "**/README.md",
        "documentation",
        "Prose; not an approval-bearing judgment.",
    ),
    (
        "**/*.pdf",
        "artifact",
        "Input/control artifact bound by digest by its manifest or test.",
    ),
    (
        "**/*.rgba",
        "artifact",
        "Raster evidence bound by digest by its manifest or test.",
    ),
    (
        "**/*.svg",
        "artifact",
        "Reference evidence bound by digest by its manifest or test.",
    ),
    (
        "**/*.excalidraw",
        "artifact",
        "Scene input bound by digest by its structure expectation or test.",
    ),
];

pub(super) fn glob(pattern: &str, path: &str) -> bool {
    fn segment(pattern: &[u8], text: &[u8]) -> bool {
        match pattern.split_first() {
            None => text.is_empty(),
            Some((b'*', rest)) => (0..=text.len()).any(|i| segment(rest, &text[i..])),
            Some((c, rest)) => text.first() == Some(c) && segment(rest, &text[1..]),
        }
    }
    fn segments(pattern: &[&str], path: &[&str]) -> bool {
        match pattern.split_first() {
            None => path.is_empty(),
            Some((&"**", rest)) => (0..=path.len()).any(|i| segments(rest, &path[i..])),
            Some((first, rest)) => {
                !path.is_empty()
                    && segment(first.as_bytes(), path[0].as_bytes())
                    && segments(rest, &path[1..])
            }
        }
    }
    let pattern: Vec<_> = pattern.split('/').collect();
    let path: Vec<_> = path.split('/').collect();
    segments(&pattern, &path)
}

/// Details member order per closed kind (spec Section 11.3).
const DETAIL_ORDERS: &[(&str, &[&str])] = &[
    (
        "parse_limit",
        &["kind", "field", "configured", "value_kind", "measurement"],
    ),
    ("request_value", &["kind", "field", "asset_id", "option"]),
    ("revision", &["kind", "requested"]),
    ("compatibility", &["kind", "recovery", "recipe_id"]),
    (
        "asset",
        &[
            "kind",
            "asset_id",
            "claimed_media_type",
            "validated_media_type",
            "recovery",
            "recipe_id",
        ],
    ),
    (
        "resource_limit",
        &[
            "kind",
            "field",
            "configured",
            "value_kind",
            "measurement",
            "recovery",
        ],
    ),
    ("memory_unavailable", &["kind", "subject_kind", "subject"]),
    ("missing_glyph", &["kind", "codepoint", "recovery"]),
    (
        "svg_font_subset_failure",
        &["kind", "stage", "resolved_family", "asset_identity"],
    ),
    ("metadata", &["kind", "component", "operation", "recovery"]),
    (
        "nested_source",
        &["kind", "asset_chain", "paths", "recovery"],
    ),
    (
        "nested_resource_limit",
        &[
            "kind",
            "field",
            "configured",
            "value_kind",
            "measurement",
            "asset_chain",
            "paths",
            "recovery",
            "recipe_id",
        ],
    ),
    (
        "nested_asset_claim",
        &[
            "kind",
            "asset_chain",
            "paths",
            "claimed_media_type",
            "validated_media_type",
            "recovery",
        ],
    ),
    (
        "pdf_raster_fallback",
        &[
            "kind",
            "recovery",
            "logical_bounds",
            "pixel_width",
            "pixel_height",
            "causes",
        ],
    ),
];

/// Nested closed shapes, selected by the member that holds them.
const NESTED_ORDERS: &[(&str, &[&str])] = &[
    ("measurement", &["kind", "value"]),
    ("asset_chain", &["asset_id", "occurrence"]),
    ("paths", &["segments"]),
    ("causes", &["segments"]),
    ("segments", &["kind", "ordinal", "name"]),
    ("logical_bounds", &["x", "y", "width", "height"]),
];

const DIAGNOSTIC_ORDER: &[&str] = &[
    "code",
    "severity",
    "format",
    "effects",
    "element_id",
    "json_pointer",
    "message",
    "details",
];

fn write_scalar(value: &Value, out: &mut String) -> Result<(), String> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::String(s) => out.push_str(&serde_json::to_string(s).map_err(|e| e.to_string())?),
        Value::Number(n) if n.is_i64() || n.is_u64() => out.push_str(&n.to_string()),
        Value::Number(n) => {
            let f = n.as_f64().ok_or("non-finite report number")?;
            if !f.is_finite() || (f != 0.0 && !(1e-6..1e21).contains(&f.abs())) {
                return Err(
                    "report number needs an exponent; unsupported by this derivation".into(),
                );
            }
            let f = if f == 0.0 { 0.0 } else { f };
            out.push_str(&format!("{f}"));
        }
        _ => return Err("expected a scalar report value".into()),
    }
    Ok(())
}

fn write_ordered(value: &Value, member: &str, out: &mut String) -> Result<(), String> {
    match value {
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_ordered(item, member, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let order = if member == "details" {
                let kind = map.get("kind").and_then(Value::as_str).unwrap_or_default();
                DETAIL_ORDERS
                    .iter()
                    .find(|(k, _)| *k == kind)
                    .map(|(_, o)| *o)
                    .ok_or_else(|| format!("unknown diagnostic details kind {kind:?}"))?
            } else {
                NESTED_ORDERS
                    .iter()
                    .find(|(k, _)| *k == member)
                    .map(|(_, o)| *o)
                    .ok_or_else(|| format!("no declared member order for {member:?}"))?
            };
            write_members(map, order, out)?;
        }
        scalar => write_scalar(scalar, out)?,
    }
    Ok(())
}

fn write_members(
    map: &serde_json::Map<String, Value>,
    order: &[&str],
    out: &mut String,
) -> Result<(), String> {
    if map.len() != order.len() || order.iter().any(|k| !map.contains_key(*k)) {
        return Err(format!(
            "report object members differ from declared order {order:?}"
        ));
    }
    out.push('{');
    for (i, key) in order.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&serde_json::to_string(key).unwrap());
        out.push(':');
        write_ordered(&map[*key], key, out)?;
    }
    out.push('}');
    Ok(())
}

/// The spec-ordered schema-1 report bytes for `diagnostics`.
pub(super) fn report_bytes(diagnostics: &[Value]) -> Result<Vec<u8>, String> {
    let mut out = String::from(r#"{"schema_version":1,"diagnostics":["#);
    for (i, diagnostic) in diagnostics.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_members(
            diagnostic
                .as_object()
                .ok_or("diagnostic must be an object")?,
            DIAGNOSTIC_ORDER,
            &mut out,
        )?;
    }
    out.push_str("]}");
    Ok(out.into_bytes())
}

/// The approved SVG/PNG declarations for one inventory key and the files and
/// registry members they come from.
pub(super) struct Declared {
    pub svg: Vec<Value>,
    pub png: Vec<Value>,
    pub sources: Vec<Value>,
    pub scene: String,
}

fn file_source(repo: &Repo, path: &str) -> Result<(Value, Vec<u8>), String> {
    let bytes = repo.read(path)?;
    Ok((json!({"path":path,"sha256":sha256(&bytes)}), bytes))
}

fn manifest_report(repo: &Repo, dir: &str, descriptor: &Value) -> Result<(Value, Vec<u8>), String> {
    let path = format!("{dir}/{}", string(descriptor, "path")?);
    let (source, bytes) = file_source(repo, &path)?;
    if descriptor["sha256"] != source["sha256"] {
        return Err(format!("manifest descriptor is stale for {path}"));
    }
    Ok((source, bytes))
}

pub(super) fn declared(repo: &Repo, registry: &Value, key: &CaseKey) -> Result<Declared, String> {
    let case = registry["cases"]
        .as_array()
        .ok_or("registry cases must be an array")?
        .iter()
        .find(|c| c["id"] == key.case)
        .ok_or_else(|| format!("inventory case {} is not registered", key.case))?;
    let dir = string(case, "path")?.to_owned();
    let manifest_path = format!("{dir}/manifest.json");
    let manifest: Value = parse(&repo.read(&manifest_path)?)?;
    let scene = format!("{dir}/{}", string(&manifest["scene"], "path")?);
    match string(case, "kind")? {
        "scene" => {
            if key.export != (ExportTarget::Scene {}) {
                return Err(format!("{}: scene case with frame export", key.case));
            }
            let declared = &case["expected_diagnostics"];
            let variant = manifest["variants"]
                .as_array()
                .ok_or("manifest variants must be an array")?
                .iter()
                .find(|v| v["id"] == key.variant)
                .ok_or_else(|| format!("{}: unknown variant {}", key.case, key.variant))?;
            let mut sources = vec![
                json!({"path":REGISTRY,"case":key.case,"member":"expected_diagnostics",
                "canonical_sha256":sha256(&canonical_bytes(declared)?)}),
            ];
            let mut lists = Vec::new();
            for format in ["svg", "png"] {
                let list = declared[format]
                    .as_array()
                    .ok_or_else(|| format!("{}: no {format} expected_diagnostics", key.case))?;
                let (source, bytes) =
                    manifest_report(repo, &dir, &variant["formats"][format]["expected_report"])?;
                let report: Value = parse(&bytes)?;
                if report != json!({"schema_version":1,"diagnostics":list}) {
                    return Err(format!(
                        "{}: manifest {format} report disagrees with the registry declaration",
                        key.case
                    ));
                }
                sources.push(source);
                lists.push(list.clone());
            }
            let png = lists.pop().unwrap();
            let svg = lists.pop().unwrap();
            Ok(Declared {
                svg,
                png,
                sources,
                scene,
            })
        }
        kind @ ("typography" | "frame") => {
            match (&key.export, kind) {
                (ExportTarget::Scene {}, "typography") => {
                    if !manifest["variants"]
                        .as_array()
                        .is_some_and(|v| v.iter().any(|v| v["id"] == key.variant))
                    {
                        return Err(format!("{}: unknown variant {}", key.case, key.variant));
                    }
                }
                (ExportTarget::Frame { ordinal }, "frame") => {
                    if key.variant != "default"
                        || !manifest["targets"]
                            .as_array()
                            .is_some_and(|t| t.iter().any(|t| t["frame_ordinal"] == json!(ordinal)))
                    {
                        return Err(format!("{}: unknown frame target", key.case));
                    }
                }
                _ => return Err(format!("{}: export disagrees with case kind", key.case)),
            }
            let (source, bytes) = manifest_report(repo, &dir, &manifest["expected_report"])?;
            if bytes != report_bytes(&[])? {
                return Err(format!(
                    "{}: a format-neutral manifest report must be the canonical empty report",
                    key.case
                ));
            }
            Ok(Declared {
                svg: vec![],
                png: vec![],
                sources: vec![source],
                scene,
            })
        }
        kind => Err(format!("unsupported case kind {kind}")),
    }
}

fn with_format(diagnostic: &Value, format: &str) -> Value {
    let mut d = diagnostic.clone();
    d["format"] = json!(format);
    d
}

/// Pixel-domain nested-SVG content that §8.6 admits only as raster fallback.
fn fallback_effects(repo: &Repo, scene: &str) -> Result<bool, String> {
    use base64::Engine;
    let scene: Value = parse(&repo.read(scene)?)?;
    let Some(files) = scene["files"].as_object() else {
        return Ok(false);
    };
    for file in files.values() {
        let url = file["dataURL"].as_str().unwrap_or_default();
        let Some((head, data)) = url.split_once(',') else {
            continue;
        };
        if !head.starts_with("data:image/svg") && file["mimeType"] != "image/svg+xml" {
            continue;
        }
        let text = if head.ends_with(";base64") {
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|e| format!("invalid nested SVG payload: {e}"))?
        } else {
            data.as_bytes().to_vec()
        };
        let text = String::from_utf8_lossy(&text).to_ascii_lowercase();
        if text.contains("<filter") || text.contains("%3cfilter") || text.contains("<fe") {
            return Ok(true);
        }
    }
    Ok(false)
}

fn declaration<'a>(declarations: &'a [Value], case: &str) -> Result<&'a Value, String> {
    let found: Vec<_> = declarations.iter().filter(|d| d["case"] == case).collect();
    match found.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("missing PDF declaration for case {case}")),
        _ => Err(format!("duplicate PDF declaration for case {case}")),
    }
}

/// The expected PDF report entry for one inventory execution.
pub(super) fn derive_execution(
    repo: &Repo,
    registry: &Value,
    key: &CaseKey,
    declarations: &[Value],
) -> Result<Value, String> {
    if key.format != Format::Pdf {
        return Err("only PDF executions are derived".into());
    }
    let declared = declared(repo, registry, key)?;
    let mut transferred = Vec::new();
    let mut png_view = Vec::new();
    for diagnostic in &declared.svg {
        let code = string(diagnostic, "code")?;
        let rule = FORMAT_RULES
            .iter()
            .find(|r| r.code == code)
            .ok_or_else(|| format!("{}: no PDF format rule for code {code}", key.case))?;
        if diagnostic["format"] != "svg" {
            return Err(format!(
                "{}: SVG declaration has a non-SVG diagnostic",
                key.case
            ));
        }
        transferred.push(with_format(diagnostic, "pdf"));
        if rule.png {
            png_view.push(with_format(diagnostic, "png"));
        }
    }
    if png_view != declared.png {
        return Err(format!(
            "{}: PNG and SVG declarations disagree beyond the spec's format rules",
            key.case
        ));
    }
    let declaration = declaration(declarations, &key.case)?;
    let extra = declaration["pdf_only_diagnostics"]
        .as_array()
        .ok_or("pdf_only_diagnostics must be an array")?;
    let at = declaration["insert_at"]
        .as_u64()
        .ok_or("insert_at must be a nonnegative integer")? as usize;
    if at > transferred.len() {
        return Err(format!("{}: insert_at is out of range", key.case));
    }
    for diagnostic in extra {
        if diagnostic["format"] != "pdf" || !string(diagnostic, "code")?.starts_with("pdf.") {
            return Err(format!(
                "{}: PDF-only declarations must be pdf.* PDF diagnostics",
                key.case
            ));
        }
    }
    if fallback_effects(repo, &declared.scene)?
        && !extra.iter().any(|d| d["code"] == "pdf.raster-fallback")
    {
        return Err(format!(
            "{}: nested SVG has a pixel-domain filter but declares no pdf.raster-fallback",
            key.case
        ));
    }
    let mut diagnostics = transferred;
    diagnostics.splice(at..at, extra.iter().cloned());
    let bytes = report_bytes(&diagnostics)?;
    Ok(json!({
        "key": key,
        "sources": declared.sources,
        "expected_report": {"byte_length":bytes.len(),"sha256":sha256(&bytes),"diagnostics":diagnostics},
    }))
}

fn classify(path: &str) -> Option<Result<&'static str, (&'static str, &'static str)>> {
    if let Some((_, role)) = JUDGMENT_PATTERNS.iter().find(|(p, _)| glob(p, path)) {
        return Some(Ok(role));
    }
    NON_JUDGMENT_PATTERNS
        .iter()
        .find(|(p, _, _)| glob(p, path))
        .map(|(_, class, reason)| Err((*class, *reason)))
}

/// Discovered PDF files split into bound judgments and classified non-judgment
/// files; any unclassified file is an unbound judgment and fails.
/// Bound (path, role) judgments and classified non-judgment files.
pub(super) type Classified = (Vec<(String, String)>, Vec<Value>);

pub(super) fn classify_files(repo: &Repo) -> Result<Classified, String> {
    let mut judgments = Vec::new();
    let mut others = Vec::new();
    let mut unbound = Vec::new();
    for path in repo.list(SCOPE_ROOT)? {
        if EXCLUDED_SUBTREES
            .iter()
            .any(|s| path.starts_with(&format!("{s}/")))
            || !path.to_ascii_lowercase().contains("pdf")
        {
            continue;
        }
        match classify(&path) {
            Some(Ok(role)) => judgments.push((path, role.to_owned())),
            Some(Err((class, _))) => others.push(json!({"path":path,"class":class})),
            None => unbound.push(path),
        }
    }
    if !unbound.is_empty() {
        return Err(format!(
            "unbound PDF judgment file(s): {}",
            unbound.join(", ")
        ));
    }
    Ok((judgments, others))
}

fn approval_of(value: &Value) -> Option<Value> {
    value["approval"].as_object().map(|a| {
        json!({"approval_url":a.get("approval_url"),"approval_subject_sha256":a.get("approval_subject_sha256")})
    })
}

/// Existing approval references: an embedded approval, issue numbers the file
/// records itself, and approved records (other bound judgments, or the case
/// record in the same directory) that name this file and pin its digest.
fn judgment_entries(repo: &Repo, files: &[(String, String)]) -> Result<Vec<Value>, String> {
    let mut loaded = Vec::new();
    for (path, role) in files {
        let bytes = repo.read(path)?;
        let value: Value = parse(&bytes)?;
        loaded.push((path, role, bytes, value));
    }
    let mut carriers: Vec<(String, Vec<u8>, Value)> = Vec::new();
    for (path, _, bytes, value) in &loaded {
        if let Some(approval) = approval_of(value) {
            carriers.push(((*path).clone(), bytes.clone(), approval));
        }
    }
    let mut entries = Vec::new();
    for (path, role, bytes, value) in &loaded {
        let digest = sha256(bytes);
        let name = path.rsplit('/').next().unwrap_or(path.as_str());
        let mut issues = BTreeSet::new();
        let items: Vec<&Value> = match value {
            Value::Array(items) => items.iter().collect(),
            other => vec![other],
        };
        for item in items {
            if let Some(issue) = item["approved_issue"].as_u64() {
                issues.insert(issue);
            }
        }
        let mut local = carriers.clone();
        if let Some((dir, _)) = path.rsplit_once('/') {
            let case = format!("{dir}/case.json");
            if let Ok(bytes) = repo.read(&case) {
                let value: Value = parse(&bytes)?;
                if let Some(approval) = approval_of(&value) {
                    local.push((case, bytes, approval));
                }
            }
        }
        let bound_by: Vec<_> = local
            .iter()
            .filter(|(carrier, bytes, _)| {
                let text = String::from_utf8_lossy(bytes);
                // Identical content shares a digest, so the carrier must also
                // name this file.
                carrier != *path && text.contains(&digest) && text.contains(name)
            })
            .map(|(carrier, _, approval)| json!({"path":carrier,"approval":approval}))
            .collect();
        entries.push(json!({"path":path,"role":role,"sha256":digest,"byte_length":bytes.len(),
            "approval_reference":{"embedded":approval_of(value),"approved_issues":issues,"bound_by":bound_by}}));
    }
    Ok(entries)
}

/// The complete record, preserving the authored `pdf_declarations`.
pub(super) fn build_record(
    repo: &Repo,
    inventory: &Inventory,
    declarations: &[Value],
) -> Result<Value, String> {
    let registry: Value = parse(&repo.read(REGISTRY)?)?;
    let pdf_cases: BTreeSet<_> = inventory
        .executions
        .iter()
        .filter(|e| e.key.format == Format::Pdf)
        .map(|e| e.key.case.clone())
        .collect();
    let declared_cases: Vec<_> = declarations
        .iter()
        .map(|d| string(d, "case").map(str::to_owned))
        .collect::<Result<_, _>>()?;
    if declared_cases.windows(2).any(|w| w[0] >= w[1])
        || declared_cases.iter().cloned().collect::<BTreeSet<_>>() != pdf_cases
    {
        return Err("pdf_declarations must name every PDF inventory case once, sorted".into());
    }
    for d in declarations {
        let fields: BTreeSet<_> = d
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        let expected: BTreeSet<String> = [
            "case",
            "insert_at",
            "justification",
            "pdf_only_diagnostics",
            "spec_anchor",
        ]
        .map(str::to_owned)
        .into_iter()
        .collect();
        if fields != expected
            || string(d, "justification")?.trim().is_empty()
            || string(d, "spec_anchor")?.trim().is_empty()
        {
            return Err(format!("invalid PDF declaration for {}", d["case"]));
        }
    }
    let executions = inventory
        .executions
        .iter()
        .filter(|e| e.key.format == Format::Pdf)
        .map(|e| derive_execution(repo, &registry, &e.key, declarations))
        .collect::<Result<Vec<_>, _>>()?;
    let (files, others) = classify_files(repo)?;
    let (inventory_source, _) = file_source(repo, INVENTORY)?;
    let (implementation, _) = file_source(repo, IMPLEMENTATION)?;
    Ok(json!({
        "schema": SCHEMA,
        "compatibility_revision": 1,
        "obligation": "closure:pdf-judgments",
        "approving_issue": 85,
        "inventory": inventory_source,
        "implementation": implementation,
        "derivation_rules": RULES.iter().map(|r| json!({"id":r.id,"statement":r.statement,"spec_anchor":r.spec_anchor})).collect::<Vec<_>>(),
        "format_rules": FORMAT_RULES.iter().map(|r| json!({"code":r.code,"svg":true,"pdf":true,"png":r.png,"spec_anchor":r.spec_anchor})).collect::<Vec<_>>(),
        "pdf_declarations": declarations,
        "executions": executions,
        "judgment_scope": {
            "root": SCOPE_ROOT,
            "path_contains": "pdf",
            "excluded_subtrees": EXCLUDED_SUBTREES,
            "judgment_patterns": JUDGMENT_PATTERNS.iter().map(|(p, r)| json!({"pattern":p,"role":r})).collect::<Vec<_>>(),
            "non_judgment_patterns": NON_JUDGMENT_PATTERNS.iter().map(|(p, c, r)| json!({"pattern":p,"class":c,"reason":r})).collect::<Vec<_>>(),
        },
        "judgments": judgment_entries(repo, &files)?,
        "non_judgment_files": others,
        "approval": Value::Null,
    }))
}
