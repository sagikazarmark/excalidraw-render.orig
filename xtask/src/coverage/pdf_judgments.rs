//! `closure:pdf-judgments`: one #85-approved record that binds an independently
//! derived expected report for every PDF inventory execution and, by digest,
//! every specialized PDF structural/visual judgment file under
//! `tests/conformance`. Derivation source identity is historical provenance;
//! verification still rebuilds every report, rule and judgment classification.
use super::*;
use std::path::{Component, Path, PathBuf};

mod derive;
use derive::{build_record, classify_files, declared, report_bytes, FORMAT_RULES, RULES};

pub(super) const RECORD: &str = "tests/conformance/coverage/pdf-judgments.json";
pub(super) const SCHEMA: &str = "excalidraw-render/pdf-judgments/v1";
#[cfg(test)]
const SUBJECT: &str = "excalidraw-render/pdf-judgments-approval-subject/v1";
const SPEC: &str = "docs/renderer-spec-v1.md";
const BASELINE: &str = "tests/conformance/coverage/native-baseline";

/// Repository view with test overrides: `Some` replaces or adds a file, `None`
/// removes one. Production uses no overrides. Shared with the five-layer
/// record.
pub(super) struct Repo {
    root: PathBuf,
    pub(super) overrides: BTreeMap<String, Option<Vec<u8>>>,
}

impl Repo {
    /// An override-only test repository rooted at a nonexistent path.
    #[cfg(test)]
    pub(super) fn empty(root: PathBuf) -> Self {
        assert!(!root.exists(), "test repository root must not exist");
        Self {
            root,
            overrides: BTreeMap::new(),
        }
    }

    pub(super) fn real() -> Self {
        Self {
            root: repo_root(),
            overrides: BTreeMap::new(),
        }
    }

    pub(super) fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        match self.overrides.get(path) {
            Some(Some(bytes)) => Ok(bytes.clone()),
            Some(None) => Err(format!("failed to read {path}: removed")),
            None => read_repository_file(&self.root, path),
        }
    }

    /// Every regular file below `dir`, repository-relative, sorted.
    pub(super) fn list(&self, dir: &str) -> Result<Vec<String>, String> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
            for entry in fs::read_dir(root.join(dir)).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let relative = dir.join(entry.file_name());
                let kind = entry.file_type().map_err(|e| e.to_string())?;
                if kind.is_dir() {
                    walk(root, &relative, out)?;
                } else if kind.is_file() {
                    out.push(
                        relative
                            .to_str()
                            .ok_or("non-UTF8 conformance path")?
                            .to_owned(),
                    );
                }
            }
            Ok(())
        }
        let mut files = Vec::new();
        // A directory may exist only in the overrides.
        if self.root.join(dir).is_dir() || self.overrides.is_empty() {
            walk(&self.root, Path::new(dir), &mut files)?;
        }
        let mut files: BTreeSet<_> = files.into_iter().collect();
        for (path, bytes) in &self.overrides {
            if path.starts_with(&format!("{dir}/")) {
                if bytes.is_some() {
                    files.insert(path.clone());
                } else {
                    files.remove(path);
                }
            }
        }
        Ok(files.into_iter().collect())
    }

    /// The immediate entries of `dir` as `(name, is_directory)`, sorted, with
    /// overrides applied: an override below a child makes it a directory. A
    /// missing directory has no entries.
    pub(super) fn children(&self, dir: &str) -> Result<Vec<(String, bool)>, String> {
        let mut entries = BTreeMap::new();
        match fs::read_dir(self.root.join(dir)) {
            Ok(read) => {
                for entry in read {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let name = entry
                        .file_name()
                        .into_string()
                        .map_err(|_| "non-UTF8 repository path")?;
                    let kind = entry.file_type().map_err(|e| e.to_string())?;
                    if kind.is_dir() || kind.is_file() {
                        entries.insert(name, kind.is_dir());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("failed to list {dir}: {e}")),
        }
        for (path, bytes) in &self.overrides {
            let Some(rest) = path.strip_prefix(&format!("{dir}/")) else {
                continue;
            };
            match (rest.split_once('/'), bytes) {
                (Some((child, _)), Some(_)) => {
                    entries.insert(child.to_owned(), true);
                }
                (None, Some(_)) => {
                    entries.insert(rest.to_owned(), false);
                }
                (None, None) => {
                    entries.remove(rest);
                }
                (Some(_), None) => {}
            }
        }
        Ok(entries.into_iter().collect())
    }
}

fn record_bytes(record: &Value) -> Result<Vec<u8>, String> {
    Ok([&canonical_bytes(record)?[..], b"\n"].concat())
}

#[cfg(test)]
fn subject_of(record: &Value) -> Result<String, String> {
    let mut record = record.clone();
    record
        .as_object_mut()
        .ok_or("pdf-judgments record must be an object")?
        .remove("approval");
    Ok(sha256(&canonical_bytes(
        &json!({"schema":SUBJECT,"record":record}),
    )?))
}

fn key_value(key: &CaseKey) -> Value {
    serde_json::to_value(key).expect("case keys serialize")
}

fn spec_anchors(repo: &Repo, record: &Value) -> Result<(), String> {
    let spec = String::from_utf8(repo.read(SPEC)?).map_err(|e| e.to_string())?;
    let declared = record["pdf_declarations"]
        .as_array()
        .ok_or("pdf_declarations must be an array")?;
    let anchors = RULES
        .iter()
        .map(|r| r.spec_anchor)
        .chain(FORMAT_RULES.iter().map(|r| r.spec_anchor))
        .chain(declared.iter().filter_map(|d| d["spec_anchor"].as_str()));
    for anchor in anchors {
        if !spec.contains(anchor) {
            return Err(format!("stale spec anchor: {anchor}"));
        }
    }
    Ok(())
}

/// Every consistency check except the baseline cross-check. Returns the record.
pub(super) fn verify_record(repo: &Repo, inventory: &Inventory) -> Result<Value, String> {
    let bytes = repo.read(RECORD)?;
    let record: Value = parse(&bytes)?;
    if bytes != record_bytes(&record)? {
        return Err("pdf-judgments.json is not canonical JSON plus LF".into());
    }
    if record["schema"] != SCHEMA || record["compatibility_revision"] != 1 {
        return Err("invalid pdf-judgments identity".into());
    }
    let executions = record["executions"]
        .as_array()
        .ok_or("executions must be an array")?;
    let pdf: Vec<_> = inventory
        .executions
        .iter()
        .filter(|e| e.key.format == Format::Pdf)
        .map(|e| key_value(&e.key))
        .collect();
    for key in &pdf {
        match executions.iter().filter(|e| &e["key"] == key).count() {
            0 => return Err(format!("PDF execution {key} has no expected report")),
            1 => {}
            _ => {
                return Err(format!(
                    "PDF execution {key} has more than one expected report"
                ))
            }
        }
    }
    if let Some(extra) = executions.iter().find(|e| !pdf.contains(&e["key"])) {
        return Err(format!(
            "expected report for a non-inventory PDF execution {}",
            extra["key"]
        ));
    }
    for execution in executions {
        let report = &execution["expected_report"];
        let bytes = report_bytes(
            report["diagnostics"]
                .as_array()
                .ok_or("expected report diagnostics must be an array")?,
        )?;
        if report["sha256"] != sha256(&bytes) || report["byte_length"] != bytes.len() {
            return Err(format!(
                "stale expected report digest for {}",
                execution["key"]
            ));
        }
    }
    let judgments = record["judgments"]
        .as_array()
        .ok_or("judgments must be an array")?;
    for judgment in judgments {
        let path = string(judgment, "path")?;
        let bytes = repo.read(path)?;
        if judgment["sha256"] != sha256(&bytes) || judgment["byte_length"] != bytes.len() {
            return Err(format!("stale digest for PDF judgment {path}"));
        }
    }
    let (discovered, _) = classify_files(repo)?;
    let bound: BTreeSet<_> = judgments
        .iter()
        .filter_map(|j| j["path"].as_str())
        .collect();
    let unbound: Vec<_> = discovered
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| !bound.contains(p))
        .collect();
    if !unbound.is_empty() {
        return Err(format!(
            "unbound PDF judgment file(s): {}",
            unbound.join(", ")
        ));
    }
    spec_anchors(repo, &record)?;
    // Keep the generating source identity inside the accepted record, without
    // requiring its historical file to exist in today's checkout. Only this
    // descriptor is carried forward; all derivation results are rebuilt below.
    let implementation = &record["implementation"];
    let path = string(implementation, "path")?;
    if path.is_empty()
        || Path::new(path)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || !implementation["sha256"]
            .as_str()
            .is_some_and(|h| hex(h, 64))
        || implementation.as_object().is_none_or(|o| o.len() != 2)
    {
        return Err("invalid historical PDF judgment implementation descriptor".into());
    }
    let mut rebuilt = build_record(
        repo,
        inventory,
        record["pdf_declarations"]
            .as_array()
            .ok_or("pdf_declarations must be an array")?,
        implementation,
    )?;
    rebuilt["approval"] = record["approval"].clone();
    // Approved records keep the generating inventory as provenance. Its
    // incidental test-source links may move while the exact PDF keys, reports,
    // declarations, judgment files and derivation rules above remain identical.
    // Newly generated, unapproved records must still bind today's inventory.
    {
        let historical = &record["inventory"];
        if historical["path"] != INVENTORY
            || !historical["sha256"].as_str().is_some_and(|h| hex(h, 64))
            || historical.as_object().is_none_or(|o| o.len() != 2)
        {
            return Err("invalid historical PDF judgment inventory descriptor".into());
        }
        rebuilt["inventory"] = historical.clone();
    }
    if rebuilt != record {
        return Err(
            "derivation does not reproduce pdf-judgments.json; run pdf-judgments-generate and review the diff"
                .into(),
        );
    }
    Ok(record)
}

fn expected_pdf_reports(record: &Value) -> Result<BTreeMap<String, Vec<u8>>, String> {
    record["executions"]
        .as_array()
        .ok_or("executions must be an array")?
        .iter()
        .map(|e| {
            let diagnostics = e["expected_report"]["diagnostics"]
                .as_array()
                .ok_or("expected report diagnostics must be an array")?;
            Ok((canonical_key(&e["key"])?, report_bytes(diagnostics)?))
        })
        .collect()
}

fn canonical_key(key: &Value) -> Result<String, String> {
    String::from_utf8(canonical_bytes(key)?).map_err(|e| e.to_string())
}

/// Cross-check: every derived PDF report equals the approved native baseline's
/// actual report byte for byte. Returns the number of equal reports.
fn cross_check_baseline(repo: &Repo, record: &Value) -> Result<usize, String> {
    let baseline: Value = parse(&repo.read(&format!("{BASELINE}/revision-1.json"))?)?;
    let expected = expected_pdf_reports(record)?;
    let mut equal = 0;
    let mut mismatches = Vec::new();
    for entry in baseline["expectations"]
        .as_array()
        .ok_or("baseline expectations must be an array")?
    {
        if entry["key"]["format"] != "pdf" {
            continue;
        }
        let key = canonical_key(&entry["key"])?;
        let sha = string(&entry["sample"]["report"], "sha256")?;
        let actual = repo.read(&format!("{BASELINE}/blobs/{sha}"))?;
        match expected.get(&key) {
            Some(bytes) if *bytes == actual => equal += 1,
            Some(_) => mismatches.push(key),
            None => mismatches.push(format!("{key} (no expected report)")),
        }
    }
    if equal != expected.len() || !mismatches.is_empty() {
        return Err(format!(
            "{} derived PDF reports differ from the approved native baseline: {}",
            expected.len() - equal,
            mismatches.join("; ")
        ));
    }
    Ok(equal)
}

/// Snapshot/baseline integration: every actual report must equal its expected
/// report. PDF compares exact bytes with this record; SVG/PNG compare complete
/// JSON values with the manifest-owned declarations. Returns (total, pdf).
pub(super) fn check_actual_reports(
    reports: &[(CaseKey, Vec<u8>)],
) -> Result<(usize, usize), String> {
    let repo = Repo::real();
    let inventory = load_inventory()?;
    let record = verify_record(&repo, &inventory)?;
    let pdf = expected_pdf_reports(&record)?;
    let registry: Value = parse(&repo.read(REGISTRY)?)?;
    let mut pdf_count = 0;
    for (key, actual) in reports {
        if key.format == Format::Pdf {
            let expected = pdf
                .get(&canonical_key(&key_value(key))?)
                .ok_or_else(|| format!("{key:?} has no expected PDF report"))?;
            if expected != actual {
                return Err(format!(
                    "actual PDF report for {key:?} differs from pdf-judgments.json"
                ));
            }
            pdf_count += 1;
        } else {
            let declared = declared(&repo, &registry, key)?;
            let list = if key.format == Format::Svg {
                declared.svg
            } else {
                declared.png
            };
            let actual: Value = parse(actual)?;
            if actual != json!({"schema_version":1,"diagnostics":list}) {
                return Err(format!(
                    "actual report for {key:?} differs from its manifest-owned expected report"
                ));
            }
        }
    }
    Ok((reports.len(), pdf_count))
}

fn existing_declarations(repo: &Repo) -> Result<Vec<Value>, String> {
    let record: Value = parse(
        &repo
            .read(RECORD)
            .map_err(|e| format!("{e}; author pdf_declarations in {RECORD} before generating"))?,
    )?;
    record["pdf_declarations"]
        .as_array()
        .cloned()
        .ok_or_else(|| "pdf_declarations must be an array".into())
}

fn generate_in(repo: &Repo, inventory: &Inventory) -> Result<Value, String> {
    let implementation = json!({
        "path": derive::IMPLEMENTATION,
        "sha256": sha256(&repo.read(derive::IMPLEMENTATION)?),
    });
    build_record(
        repo,
        inventory,
        &existing_declarations(repo)?,
        &implementation,
    )
}

fn write_record(record: &Value) -> Result<(), String> {
    fs::write(repo_root().join(RECORD), record_bytes(record)?).map_err(|e| e.to_string())
}

pub fn generate_pdf_judgments() -> Result<(), String> {
    let repo = Repo::real();
    let record = generate_in(&repo, &load_inventory()?)?;
    write_record(&record)?;
    println!(
        "wrote {RECORD}: {} expected PDF reports, {} bound judgment files; review the diff",
        record["executions"].as_array().map_or(0, Vec::len),
        record["judgments"].as_array().map_or(0, Vec::len)
    );
    Ok(())
}

/// The compiled-in record-owner verifier: `verify-pdf-judgments` in-process.
pub(super) fn verify_owned_record() -> Result<(), String> {
    let repo = Repo::real();
    let record = verify_record(&repo, &load_inventory()?)?;
    cross_check_baseline(&repo, &record).map(|_| ())
}

/// The record's own approval subject computation, without the `sha256:` prefix.
#[cfg(test)]
pub(super) fn record_subject(record: &Value) -> Result<String, String> {
    subject_of(record)
}

pub fn verify_pdf_judgments() -> Result<(), String> {
    let repo = Repo::real();
    let record = verify_record(&repo, &load_inventory()?)?;
    let equal = cross_check_baseline(&repo, &record)?;
    println!(
        "verified pdf-judgments.json: {} expected PDF reports ({} equal the native baseline byte for byte), {} bound judgment files, {} classified non-judgment files",
        record["executions"].as_array().map_or(0, Vec::len),
        equal,
        record["judgments"].as_array().map_or(0, Vec::len),
        record["non_judgment_files"].as_array().map_or(0, Vec::len)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        repo: Repo,
        inventory: Inventory,
        record: Value,
    }

    impl Fixture {
        fn new() -> Self {
            let mut repo = Repo::real();
            let inventory = load_inventory().unwrap();
            let record = generate_in(&repo, &inventory).unwrap();
            repo.overrides
                .insert(RECORD.into(), Some(record_bytes(&record).unwrap()));
            Self {
                repo,
                inventory,
                record,
            }
        }
        fn with_record(&mut self, record: &Value) {
            self.repo
                .overrides
                .insert(RECORD.into(), Some(record_bytes(record).unwrap()));
        }
        fn verify(&self) -> Result<Value, String> {
            verify_record(&self.repo, &self.inventory)
        }
    }

    fn approval(subject: &str, url: &str) -> Value {
        json!({"approved_by":"sagikazarmark","approved_at":"2026-09-24","approval_url":url,
            "approval_subject_sha256":format!("sha256:{subject}")})
    }
    const URL: &str =
        "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1";

    #[test]
    fn accepted_judgments_survive_derivation_source_edits_and_moves() {
        let inventory = load_inventory().unwrap();
        let repo = Repo::real();
        let accepted: Value = parse(&repo.read(RECORD).unwrap()).unwrap();
        let mut edited = repo.read(derive::IMPLEMENTATION).unwrap();
        edited.extend_from_slice(b"\n// A source-only refactor.\n");
        for source in [Some(edited), None] {
            let mut repo = Repo::real();
            repo.overrides.insert(derive::IMPLEMENTATION.into(), source);
            let verified = verify_record(&repo, &inventory).unwrap();
            assert_eq!(verified, accepted);
            assert_eq!(cross_check_baseline(&repo, &verified).unwrap(), 56);
        }
    }

    #[test]
    fn generation_records_current_source_without_rebinding_accepted_judgments() {
        let mut fixture = Fixture::new();
        let mut edited = fixture.repo.read(derive::IMPLEMENTATION).unwrap();
        edited.extend_from_slice(b"\n// Current generation source.\n");
        fixture
            .repo
            .overrides
            .insert(derive::IMPLEMENTATION.into(), Some(edited.clone()));
        // Verification returns the original record even after the source edit.
        assert_eq!(fixture.verify().unwrap(), fixture.record);
        let generated = generate_in(&fixture.repo, &fixture.inventory).unwrap();
        assert_eq!(generated["implementation"]["sha256"], sha256(&edited));
        assert_ne!(
            generated["implementation"],
            fixture.record["implementation"]
        );
        assert_eq!(generated["executions"], fixture.record["executions"]);
        assert!(generated["approval"].is_null());
    }

    #[test]
    fn historical_implementation_requires_well_formed_provenance_only() {
        let mut fixture = Fixture::new();
        let original = fixture.record.clone();
        for implementation in [
            json!({"path": "", "sha256": "0".repeat(64)}),
            json!({"path": "../outside.rs", "sha256": "0".repeat(64)}),
            json!({"path": "/absolute.rs", "sha256": "0".repeat(64)}),
            json!({"path": "retired/derive.rs", "sha256": "invalid"}),
            json!({"path": "retired/derive.rs", "sha256": "0".repeat(64), "extra": true}),
        ] {
            let mut malformed = original.clone();
            malformed["implementation"] = implementation;
            fixture.with_record(&malformed);
            assert!(fixture
                .verify()
                .unwrap_err()
                .contains("implementation descriptor"));
        }
        // Use the actual accepted record, not Fixture's unapproved generation.
        let mut changed: Value = parse(&Repo::real().read(RECORD).unwrap()).unwrap();
        changed["implementation"]["sha256"] = json!("0".repeat(64));
        fixture.with_record(&changed);
        fixture.verify().unwrap();
    }

    #[test]
    fn historical_source_does_not_exempt_rules_scope_or_inputs_from_rebuilding() {
        let mut fixture = Fixture::new();
        for pointer in [
            "/derivation_rules/0/statement",
            "/format_rules/0/png",
            "/judgment_scope/root",
            "/executions/0/key/inputs_sha256",
        ] {
            let mut changed = fixture.record.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("changed");
            fixture.with_record(&changed);
            assert!(fixture.verify().is_err(), "accepted changed {pointer}");
        }
    }

    #[test]
    fn generate_is_deterministic_and_verifies_against_the_baseline() {
        let fixture = Fixture::new();
        assert_eq!(
            generate_in(&fixture.repo, &fixture.inventory).unwrap(),
            fixture.record
        );
        let record = fixture.verify().unwrap();
        let pdf = fixture
            .inventory
            .executions
            .iter()
            .filter(|e| e.key.format == Format::Pdf)
            .count();
        assert_eq!(record["executions"].as_array().unwrap().len(), pdf);
        assert_eq!(cross_check_baseline(&fixture.repo, &record).unwrap(), pdf);
        fixture.verify().unwrap();
    }

    #[test]
    fn serializes_reports_in_the_spec_member_order() {
        assert_eq!(
            report_bytes(&[]).unwrap(),
            br#"{"schema_version":1,"diagnostics":[]}"#
        );
        let diagnostic = json!({"details":{"recipe_id":null,"recovery":"omitted","kind":"compatibility"},
            "severity":"warning","message":"m","json_pointer":"/elements/0/link","format":"pdf",
            "element_id":"a","effects":["semantic"],"code":"link.unsafe-omitted"});
        assert_eq!(
            String::from_utf8(report_bytes(std::slice::from_ref(&diagnostic)).unwrap()).unwrap(),
            r#"{"schema_version":1,"diagnostics":[{"code":"link.unsafe-omitted","severity":"warning","format":"pdf","effects":["semantic"],"element_id":"a","json_pointer":"/elements/0/link","message":"m","details":{"kind":"compatibility","recovery":"omitted","recipe_id":null}}]}"#
        );
        let mut unknown = diagnostic;
        unknown["details"]["extra"] = json!(1);
        assert!(report_bytes(&[unknown]).is_err());
    }

    #[test]
    fn rejects_missing_duplicate_and_stale_expected_reports() {
        let mut fixture = Fixture::new();
        let mut missing = fixture.record.clone();
        missing["executions"].as_array_mut().unwrap().pop();
        fixture.with_record(&missing);
        assert!(fixture
            .verify()
            .unwrap_err()
            .contains("has no expected report"));

        let mut duplicate = fixture.record.clone();
        let first = duplicate["executions"][0].clone();
        duplicate["executions"].as_array_mut().unwrap().push(first);
        fixture.with_record(&duplicate);
        assert!(fixture.verify().unwrap_err().contains("more than one"));

        let mut stale = fixture.record.clone();
        stale["executions"][0]["expected_report"]["sha256"] = json!("0".repeat(64));
        fixture.with_record(&stale);
        assert!(fixture
            .verify()
            .unwrap_err()
            .contains("stale expected report digest"));

        // A consistent but altered expected report is not what derivation yields.
        let mut altered = fixture.record.clone();
        let diagnostic = json!({"code":"pdf.raster-fallback","severity":"warning","format":"pdf",
            "effects":["visual"],"element_id":null,"json_pointer":"","message":"m","details":null});
        let bytes = report_bytes(std::slice::from_ref(&diagnostic)).unwrap();
        altered["executions"][0]["expected_report"] = json!({"byte_length":bytes.len(),
            "sha256":sha256(&bytes),"diagnostics":[diagnostic]});
        fixture.with_record(&altered);
        assert!(fixture.verify().unwrap_err().contains("does not reproduce"));
    }

    #[test]
    fn rejects_stale_judgment_digests_and_unbound_judgment_files() {
        let mut fixture = Fixture::new();
        let path = fixture.record["judgments"][0]["path"]
            .as_str()
            .unwrap()
            .to_owned();
        fixture
            .repo
            .overrides
            .insert(path, Some(b"changed".to_vec()));
        assert!(fixture
            .verify()
            .unwrap_err()
            .contains("stale digest for PDF judgment"));

        let mut fixture = Fixture::new();
        let new = "tests/conformance/cases/focused/freedraw/pdf-visual-exception.json";
        fixture
            .repo
            .overrides
            .insert(new.into(), Some(b"{}".to_vec()));
        let error = fixture.verify().unwrap_err();
        assert!(
            error.contains("unbound PDF judgment file") && error.contains(new),
            "{error}"
        );

        let mut fixture = Fixture::new();
        let odd = "tests/conformance/cases/focused/freedraw/pdf-notes.json";
        fixture
            .repo
            .overrides
            .insert(odd.into(), Some(b"{}".to_vec()));
        assert!(fixture.verify().unwrap_err().contains(odd));

        let mut fixture = Fixture::new();
        let mut dropped = fixture.record.clone();
        dropped["judgments"].as_array_mut().unwrap().remove(0);
        fixture.with_record(&dropped);
        assert!(fixture
            .verify()
            .unwrap_err()
            .contains("unbound PDF judgment file"));
    }

    #[test]
    fn requires_declarations_and_png_svg_consistency() {
        let mut fixture = Fixture::new();
        let mut undeclared = fixture.record.clone();
        undeclared["pdf_declarations"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        fixture.with_record(&undeclared);
        assert!(fixture.verify().unwrap_err().contains("pdf_declarations"));

        let mut bad_anchor = fixture.record.clone();
        bad_anchor["pdf_declarations"][0]["spec_anchor"] = json!("not in the spec");
        fixture.with_record(&bad_anchor);
        assert!(fixture.verify().unwrap_err().contains("stale spec anchor"));

        // A registry whose PNG declaration drops a format-neutral diagnostic.
        let fixture = Fixture::new();
        let mut registry: Value = parse(&fixture.repo.read(REGISTRY).unwrap()).unwrap();
        for case in registry["cases"].as_array_mut().unwrap() {
            if case["id"] == "focused.emoji-fallback.v1" {
                case["expected_diagnostics"]["png"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
        }
        let key = fixture
            .inventory
            .executions
            .iter()
            .find(|e| e.key.case == "focused.emoji-fallback.v1" && e.key.format == Format::Pdf)
            .unwrap()
            .key
            .clone();
        let declarations = fixture.record["pdf_declarations"].as_array().unwrap();
        let error =
            derive::derive_execution(&fixture.repo, &registry, &key, declarations).unwrap_err();
        assert!(error.contains("disagree"), "{error}");
    }

    #[test]
    fn historical_approval_does_not_gate_rederived_reports() {
        let mut fixture = Fixture::new();
        let subject = subject_of(&fixture.record).unwrap();
        let mut record = fixture.record.clone();
        fixture.verify().unwrap();
        let wrong = URL.replace("/issues/85#", "/issues/44#");
        record["approval"] = approval(&subject, URL);
        assert_eq!(subject_of(&record).unwrap(), subject);
        fixture.with_record(&record);
        fixture.verify().unwrap();

        let mut wrong_issue = record.clone();
        wrong_issue["approval"] = approval(&subject, &wrong);
        fixture.with_record(&wrong_issue);
        fixture.verify().unwrap();

        let mut changed = record;
        changed["approval"] = approval(&"f".repeat(64), URL);
        fixture.with_record(&changed);
        fixture.verify().unwrap();
    }

    #[test]
    fn nested_svg_filter_requires_a_declared_raster_fallback() {
        use base64::Engine;
        let mut fixture = Fixture::new();
        let case = "focused.dark-mode-images.v1";
        let scene = "tests/conformance/cases/focused/dark-mode-images/scene.excalidraw";
        let mut value: Value = parse(&fixture.repo.read(scene).unwrap()).unwrap();
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><filter id="f"><feGaussianBlur stdDeviation="2"/></filter></svg>"#;
        value["files"]["file-svg"]["dataURL"] = json!(format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(svg)
        ));
        fixture
            .repo
            .overrides
            .insert(scene.into(), Some(serde_json::to_vec(&value).unwrap()));
        let registry: Value = parse(&fixture.repo.read(REGISTRY).unwrap()).unwrap();
        let key = &fixture
            .inventory
            .executions
            .iter()
            .find(|e| e.key.case == case && e.key.format == Format::Pdf)
            .unwrap()
            .key;
        let mut declarations = fixture.record["pdf_declarations"]
            .as_array()
            .unwrap()
            .clone();
        let error =
            derive::derive_execution(&fixture.repo, &registry, key, &declarations).unwrap_err();
        assert!(error.contains("declares no pdf.raster-fallback"), "{error}");
        let fallback = json!({"code":"pdf.raster-fallback","severity":"warning","format":"pdf",
            "effects":["visual"],"element_id":"image","json_pointer":"/elements/0","message":"m",
            "details":{"kind":"pdf_raster_fallback","recovery":"rasterized",
                "logical_bounds":{"x":1,"y":2.5,"width":3,"height":4},"pixel_width":3,"pixel_height":4,
                "causes":[{"segments":[{"kind":"node","ordinal":1,"name":"filter"}]}]}});
        for declaration in &mut declarations {
            if declaration["case"] == case {
                declaration["pdf_only_diagnostics"] = json!([fallback]);
            }
        }
        let execution =
            derive::derive_execution(&fixture.repo, &registry, key, &declarations).unwrap();
        assert_eq!(
            execution["expected_report"]["diagnostics"][0]["code"],
            "pdf.raster-fallback"
        );
        let bytes = report_bytes(&[fallback]).unwrap();
        assert!(String::from_utf8(bytes).unwrap().contains(
            r#""details":{"kind":"pdf_raster_fallback","recovery":"rasterized","logical_bounds":{"x":1,"y":2.5,"width":3,"height":4},"pixel_width":3,"pixel_height":4,"causes":[{"segments":[{"kind":"node","ordinal":1,"name":"filter"}]}]}"#
        ));
    }

    #[test]
    fn actual_reports_must_equal_expected_reports() {
        let fixture = Fixture::new();
        let find = |case: &str, format: Format| {
            fixture
                .inventory
                .executions
                .iter()
                .find(|e| e.key.case == case && e.key.format == format)
                .unwrap()
                .key
                .clone()
        };
        let pdf = find("focused.embeddable-links.v1", Format::Pdf);
        let png = find("focused.embeddable-links.v1", Format::Png);
        let expected = fixture.record["executions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == key_value(&pdf))
            .unwrap()["expected_report"]["diagnostics"]
            .as_array()
            .unwrap()
            .clone();
        let bytes = report_bytes(&expected).unwrap();
        let empty = report_bytes(&[]).unwrap();
        assert_eq!(
            check_actual_reports(&[(pdf.clone(), bytes.clone()), (png.clone(), empty.clone())])
                .unwrap(),
            (2, 1)
        );
        // PDF compares exact bytes: the same value in another member order fails.
        let reordered =
            canonical_bytes(&json!({"schema_version":1,"diagnostics":expected})).unwrap();
        assert!(check_actual_reports(&[(pdf.clone(), reordered)])
            .unwrap_err()
            .contains("differs from pdf-judgments.json"));
        // PNG is silent for unsafe links, so the PDF diagnostics are wrong there.
        let mut wrong = bytes;
        wrong = String::from_utf8(wrong)
            .unwrap()
            .replace("\"pdf\"", "\"png\"")
            .into_bytes();
        assert!(check_actual_reports(&[(png, wrong)])
            .unwrap_err()
            .contains("manifest-owned expected report"));
        assert!(check_actual_reports(&[(pdf, empty)]).is_err());
    }

    #[test]
    fn glob_matches_segments() {
        assert!(derive::glob("a/*/b-*.json", "a/x/b-1.json"));
        assert!(!derive::glob("a/*/b-*.json", "a/x/y/b-1.json"));
        assert!(derive::glob("**/*.pdf", "a/b/c.pdf"));
        assert!(derive::glob("**/README.md", "README.md"));
    }
}
