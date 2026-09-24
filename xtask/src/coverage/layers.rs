//! `closure:all-five-layers`: one #85-approved record that maps every item of
//! the five conformance layers (spec 14.2) to the ownership ledger. Items are
//! discovered by the rules in `derive.rs`, which is bound by digest; the only
//! authored part is `declarations` (vector family, corpus and oracle mappings).
//! Verification rebuilds the record, requires equality, and then requires
//! completeness against the ledger's current owners.
use super::ownership::LedgerView;
use super::pdf_judgments::Repo;
use super::*;
use crate::conformance::verify_approval_for_issue;

mod derive;
use derive::{build_record, completeness, CatalogRows};

pub(super) const RECORD: &str = "tests/conformance/coverage/layers.json";
pub(super) const SCHEMA: &str = "excalidraw-render/five-layers/v1";
const SUBJECT: &str = "excalidraw-render/five-layers-approval-subject/v1";

fn record_bytes(record: &Value) -> Result<Vec<u8>, String> {
    Ok([&canonical_bytes(record)?[..], b"\n"].concat())
}

fn subject_of(record: &Value) -> Result<String, String> {
    let mut record = record.clone();
    record
        .as_object_mut()
        .ok_or("layers record must be an object")?
        .remove("approval");
    Ok(sha256(&canonical_bytes(
        &json!({"schema":SUBJECT,"record":record}),
    )?))
}

/// The structural checks: canonical bytes, identity and exact rebuild.
fn current_record(repo: &Repo, view: &LedgerView) -> Result<Value, String> {
    let bytes = repo.read(RECORD)?;
    let record: Value = parse(&bytes)?;
    if bytes != record_bytes(&record)? {
        return Err("layers.json is not canonical JSON plus LF".into());
    }
    if record["schema"] != SCHEMA
        || record["compatibility_revision"] != 1
        || record["obligation"] != "closure:all-five-layers"
        || record["approving_issue"] != 85
    {
        return Err("invalid layers record identity".into());
    }
    let mut rebuilt = build_record(repo, &CatalogRows::of(view)?, &record["declarations"])?;
    rebuilt["approval"] = record["approval"].clone();
    if rebuilt != record {
        return Err(
            "derivation does not reproduce layers.json; run layers-generate and review the diff"
                .into(),
        );
    }
    Ok(record)
}

/// Every check: structure, completeness against `view`, then the approval.
fn verify_record(repo: &Repo, view: &LedgerView, require_approval: bool) -> Result<Value, String> {
    let record = current_record(repo, view)?;
    let problems = completeness(&record, view);
    if !problems.is_empty() {
        return Err(format!(
            "five-layer completeness fails with {} problem(s):\n  {}",
            problems.len(),
            problems.join("\n  ")
        ));
    }
    if require_approval || !record["approval"].is_null() {
        if record["approval"].is_null() {
            return Err("layers.json awaits explicit #85 approval".into());
        }
        verify_approval_for_issue(&record["approval"], &subject_of(&record)?, "85")?;
    }
    Ok(record)
}

fn existing_declarations(repo: &Repo) -> Result<Value, String> {
    let record: Value = parse(
        &repo
            .read(RECORD)
            .map_err(|e| format!("{e}; author declarations in {RECORD} before generating"))?,
    )?;
    Ok(record["declarations"].clone())
}

fn generate_in(repo: &Repo, view: &LedgerView) -> Result<Value, String> {
    build_record(repo, &CatalogRows::of(view)?, &existing_declarations(repo)?)
}

/// Adds the approval to a record whose subject equals `expected`.
fn apply_approval(
    record: &mut Value,
    login: &str,
    date: &str,
    url: &str,
    expected: &str,
) -> Result<(), String> {
    if !record["approval"].is_null() {
        return Err("layers.json is already approved".into());
    }
    let subject = subject_of(record)?;
    if expected.strip_prefix("sha256:").unwrap_or(expected) != subject {
        return Err(format!(
            "layers approval subject mismatch: computed sha256:{subject}"
        ));
    }
    record["approval"] = json!({"approved_by":login,"approved_at":date,"approval_url":url,
        "approval_subject_sha256":format!("sha256:{subject}")});
    verify_approval_for_issue(&record["approval"], &subject, "85")
}

fn write_record(record: &Value) -> Result<(), String> {
    fs::write(repo_root().join(RECORD), record_bytes(record)?).map_err(|e| e.to_string())
}

fn count(record: &Value, pointer: &str) -> usize {
    record
        .pointer(pointer)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn summary(record: &Value) -> String {
    format!(
        "{} vector families, {} vector domains, {} vector corpora, {} oracle directories, {} registered cases, {} representative candidates, {} #84 cases, {} adversarial targets, {} regression corpora",
        count(record, "/vector/families"),
        count(record, "/vector/domains"),
        count(record, "/vector/corpora"),
        count(record, "/vector/oracles"),
        count(record, "/focused/cases"),
        count(record, "/representative/candidates"),
        count(record, "/representative/cases"),
        count(record, "/adversarial/targets"),
        count(record, "/regression/corpora"),
    )
}

pub fn generate_layers() -> Result<(), String> {
    let repo = Repo::real();
    let view = LedgerView::load()?;
    let record = generate_in(&repo, &view)?;
    write_record(&record)?;
    let problems = completeness(&record, &view);
    println!(
        "wrote {RECORD}: {}; {} completeness problem(s); approval subject sha256:{}",
        summary(&record),
        problems.len(),
        subject_of(&record)?
    );
    for problem in problems {
        println!("  {problem}");
    }
    Ok(())
}

pub fn layers_subject() -> Result<String, String> {
    let record = verify_record(&Repo::real(), &LedgerView::load()?, false)?;
    subject_of(&record)
}

pub fn record_layers_approval(
    login: &str,
    date: &str,
    url: &str,
    expected: &str,
) -> Result<(), String> {
    let repo = Repo::real();
    let view = LedgerView::load()?;
    let mut record = verify_record(&repo, &view, false)?;
    apply_approval(&mut record, login, date, url, expected)?;
    write_record(&record)?;
    verify_record(&repo, &view, true).map(|_| ())
}

/// The compiled-in `layers` record-owner verifier (ADR-0015): exactly
/// `verify-layers --require-approval`, run in-process.
pub(super) fn verify_approved_record() -> Result<(), String> {
    verify_record(&Repo::real(), &LedgerView::load()?, true).map(|_| ())
}

/// The record's own approval subject computation, without the `sha256:` prefix.
pub(super) fn record_subject(record: &Value) -> Result<String, String> {
    subject_of(record)
}

pub fn verify_layers(require_approval: bool) -> Result<(), String> {
    let record = verify_record(&Repo::real(), &LedgerView::load()?, require_approval)?;
    println!(
        "verified layers.json: {}; every item maps to owned ledger rows; approval {}",
        summary(&record),
        if record["approval"].is_null() {
            "pending (#85)"
        } else {
            "verified"
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str =
        "https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1";
    const FONT_FUZZ: &str = "crates/excalidraw-render/src/render/font_subset_fuzz.rs";
    const SCENE_REGRESSIONS: &str =
        "crates/excalidraw-render/tests/qualification/scene-fuzz-regressions.json";
    const VARMQ: &str = "tests/conformance/cases/representative/varmq-queue-diagram/case.json";

    struct Fixture {
        repo: Repo,
        view: LedgerView,
        record: Value,
    }

    impl Fixture {
        /// The repository's declarations, rebuilt in memory and installed as
        /// the record, so the tests never depend on the committed record.
        fn new() -> Self {
            let mut repo = Repo::real();
            let mut view = LedgerView::load().unwrap();
            // Completeness tests never execute the representative's release
            // gate; its binding is the ledger's own concern.
            view.clear(ownership_representative_row());
            let record = generate_in(&repo, &view).unwrap();
            repo.overrides
                .insert(RECORD.into(), Some(record_bytes(&record).unwrap()));
            Self { repo, view, record }
        }
        fn set(&mut self, path: &str, bytes: Vec<u8>) {
            self.repo.overrides.insert(path.into(), Some(bytes));
        }
        fn set_json(&mut self, path: &str, value: &Value) {
            self.set(path, serde_json::to_vec_pretty(value).unwrap());
        }
        fn json(&self, path: &str) -> Value {
            parse(&self.repo.read(path).unwrap()).unwrap()
        }
        fn structure(&self) -> Result<Value, String> {
            current_record(&self.repo, &self.view)
        }
        fn regenerate(&mut self) -> Value {
            let record = generate_in(&self.repo, &self.view).unwrap();
            self.set(RECORD, record_bytes(&record).unwrap());
            record
        }
    }

    fn fails(result: Result<Value, String>, expected: &str) {
        let error = result.unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }

    #[test]
    fn generation_is_deterministic_canonical_and_discovers_every_layer() {
        let fixture = Fixture::new();
        assert_eq!(
            generate_in(&fixture.repo, &fixture.view).unwrap(),
            fixture.record
        );
        assert_eq!(fixture.structure().unwrap(), fixture.record);
        let record = &fixture.record;
        let families: Vec<_> = record["vector"]["families"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["family"].as_str().unwrap())
            .collect();
        assert_eq!(
            families,
            [
                "Rough.js",
                "the qualified Perfect Freehand path",
                "PRNG consumption",
                "normalization",
                "geometry",
                "color",
                "shaping/fallback",
                "serialization",
                "diagnostic order",
                "format planning"
            ]
        );
        let targets: BTreeMap<_, _> = record["adversarial"]["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                (
                    t["row"].as_str().unwrap(),
                    t["required_risks"].as_array().unwrap().len(),
                )
            })
            .collect();
        assert_eq!(targets["layer:adversarial:scene-fuzz"], 10);
        assert_eq!(targets["layer:adversarial:font-subset-fuzz"], 7);
        assert_eq!(targets["layer:adversarial:asset-fuzz"], 0);
        assert_eq!(targets.len(), 6);
        assert_eq!(count(record, "/regression/corpora"), 3);
        assert_eq!(count(record, "/representative/cases"), 3);
        let excluded = &record["representative"]["excluded"];
        assert!(excluded["intake_bundles"]
            .as_array()
            .unwrap()
            .contains(&json!("tests/conformance/intake/velox-design-v1")));
        // Structural checks never read the ledger's dispositions: the subject
        // is independent of which owners are recorded.
        let mut view = LedgerView::load().unwrap();
        view.clear("interaction:diagnostic-order");
        assert_eq!(generate_in(&fixture.repo, &view).unwrap(), fixture.record);
    }

    #[test]
    fn a_new_vector_corpus_or_oracle_directory_must_be_declared() {
        for path in [
            "crates/excalidraw-render/tests/fixtures/color-vectors.json",
            "tools/geometry-vectors/derive.mjs",
            "tests/conformance/comparator-vectors/ssim-v1/manifest.json",
            "tests/oracle/avif-container/reference.json",
        ] {
            let mut fixture = Fixture::new();
            fixture.set(path, b"{}".to_vec());
            fails(fixture.structure(), "undeclared");
        }
        let mut fixture = Fixture::new();
        fixture.set(
            "tests/conformance/comparator-vectors/ssim-v1/vectors.json",
            b"{}".to_vec(),
        );
        fails(fixture.structure(), "has no manifest.json");
        // A changed corpus byte is a changed record.
        let mut fixture = Fixture::new();
        fixture.set("tools/rough-filler-vectors/derive.mjs", b"changed".to_vec());
        fails(fixture.structure(), "does not reproduce");
    }

    #[test]
    fn a_new_case_candidate_or_intake_bundle_changes_the_record_and_needs_an_owner() {
        let mut fixture = Fixture::new();
        let mut registry = fixture.json(REGISTRY);
        let mut case = registry["cases"][0].clone();
        case["id"] = json!("focused.unowned.v1");
        registry["cases"].as_array_mut().unwrap().push(case);
        fixture.set_json(REGISTRY, &registry);
        fails(fixture.structure(), "does not reproduce");
        let record = fixture.regenerate();
        fixture.structure().unwrap();
        let problems = completeness(&record, &fixture.view);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("registered case focused.unowned.v1 is the case owner of none")),
            "{problems:?}"
        );

        let mut fixture = Fixture::new();
        fixture.set(
            "tests/conformance/candidates/stage4-frames-text-assets-v25/manifest.json",
            b"{}".to_vec(),
        );
        fails(fixture.structure(), "does not reproduce");
        let mut fixture = Fixture::new();
        fixture.set(
            "tests/conformance/intake/new-bundle-v1/manifest.json",
            b"{}".to_vec(),
        );
        fails(fixture.structure(), "does not reproduce");
    }

    #[test]
    fn new_fuzz_targets_risks_and_regression_ids_fail_until_regenerated() {
        let mut fixture = Fixture::new();
        fixture.set(
            "crates/excalidraw-render/src/render/pdf_fuzz.rs",
            b"#[test] fn smoke() {}".to_vec(),
        );
        fails(
            fixture.structure(),
            "has no catalog row layer:adversarial:pdf-fuzz",
        );
        let mut fixture = Fixture::new();
        fixture.set(
            "crates/excalidraw-render/tests/limits_output.rs",
            b"#[test] fn smoke() {}".to_vec(),
        );
        fails(fixture.structure(), "layer:adversarial:limits-output");

        let mut fixture = Fixture::new();
        let source = String::from_utf8(fixture.repo.read(FONT_FUZZ).unwrap()).unwrap();
        let added = source
            .replacen("REQUIRED_RISKS: [Risk; 7]", "REQUIRED_RISKS: [Risk; 8]", 1)
            .replacen(
                "    Risk::CacheMissHitReplay,\n];",
                "    Risk::CacheMissHitReplay,\n    Risk::NewRisk,\n];",
                1,
            );
        assert_ne!(added, source);
        fixture.set(FONT_FUZZ, added.clone().into_bytes());
        fails(fixture.structure(), "does not reproduce");
        let record = fixture.regenerate();
        assert!(record["adversarial"]["targets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["required_risks"]
                .as_array()
                .unwrap()
                .contains(&json!("NewRisk"))));
        let inconsistent = added.replacen("[Risk; 8]", "[Risk; 9]", 1);
        fixture.set(FONT_FUZZ, inconsistent.into_bytes());
        fails(fixture.structure(), "length or uniqueness");

        let mut fixture = Fixture::new();
        let mut corpus = fixture.json(SCENE_REGRESSIONS);
        let mut case = corpus["cases"][0].clone();
        case["id"] = json!("new-confirmed-failure");
        corpus["cases"].as_array_mut().unwrap().push(case.clone());
        fixture.set_json(SCENE_REGRESSIONS, &corpus);
        fails(fixture.structure(), "does not reproduce");
        corpus["cases"].as_array_mut().unwrap().push(case);
        fixture.set_json(SCENE_REGRESSIONS, &corpus);
        fails(fixture.structure(), "duplicate regression case id");
        let mut fixture = Fixture::new();
        fixture.set(
            "crates/excalidraw-render/tests/qualification/asset-fuzz-regressions.json",
            b"{\"cases\":[]}".to_vec(),
        );
        fails(
            fixture.structure(),
            "has no catalog row layer:regression:asset-fuzz",
        );
    }

    #[test]
    fn every_layer_row_is_referenced_and_owned() {
        let mut fixture = Fixture::new();
        fixture.view.add_row("layer:vector:orphan", "layer", &[]);
        fails(fixture.structure(), "orphan layer row");

        // A mapped layer row without an owner, and an item with no owned row.
        let fixture = Fixture::new();
        let mut view = LedgerView::load().unwrap();
        view.clear("layer:vector:rough-js");
        view.clear("interaction:diagnostic-order");
        let problems = completeness(&fixture.record, &view);
        for expected in [
            "vector Rough.js: layer row layer:vector:rough-js has no primary owner",
            "vector diagnostic order: no mapped row is owned",
            "adversarial crates/excalidraw-render/src/scene_fuzz: layer:adversarial:scene-fuzz has no primary owner",
            "regression crates/excalidraw-render/tests/qualification/render-fuzz-regressions.json: layer:regression:render-fuzz has no primary owner",
        ] {
            assert!(problems.iter().any(|p| p == expected), "{expected}: {problems:?}");
        }
        view.clear(ownership_representative_row());
        let problems = completeness(&fixture.record, &view);
        assert!(problems.iter().any(|p| p.contains(
            "representative candidate representative.stage4-frames-text-assets.v24: interaction:frames-text-assets is not owned"
        )));
        // A closure row can never answer an item.
        let mut declarations = fixture.record["declarations"].clone();
        declarations["vector_families"][0]["rows"] = json!(["closure:all-five-layers"]);
        let catalog = CatalogRows::of(&fixture.view).unwrap();
        assert!(build_record(&fixture.repo, &catalog, &declarations)
            .unwrap_err()
            .contains("cannot answer a vector item"));
        declarations["vector_families"][0]["rows"] = json!(["codec:nope:*"]);
        assert!(build_record(&fixture.repo, &catalog, &declarations)
            .unwrap_err()
            .contains("matches no catalog row"));
    }

    #[test]
    fn recording_a_reviewed_owner_clears_exactly_its_items() {
        let fixture = Fixture::new();
        let mut view = LedgerView::load().unwrap();
        view.clear(ownership_representative_row());
        for row in [
            "layer:vector:shaping-fallback",
            "layer:vector:format-planning",
            "layer:focused:frame-export",
        ] {
            view.clear(row);
        }
        let before = completeness(&fixture.record, &view);
        for expected in [
            "vector shaping/fallback: no mapped row is owned",
            "vector format planning: no mapped row is owned",
            "registered case focused.frame-export.v1 is the case owner of none of its rows [\"layer:focused:frame-export\"]",
        ] {
            assert!(before.iter().any(|p| p == expected), "{expected}: {before:?}");
        }
        view.assign_test(
            executable_renderer(),
            "lib",
            "render::tests",
            "a_font_fallback_probe_is_one_consulted_face_per_shaped_scalar_occurrence",
            "layer:vector:shaping-fallback",
        )
        .unwrap();
        view.assign_test(
            executable_renderer(),
            "lib",
            "render::plan_tests",
            "png_surface_follows_the_sizing_rule",
            "layer:vector:format-planning",
        )
        .unwrap();
        view.assign_case("focused.frame-export.v1", "layer:focused:frame-export")
            .unwrap();
        // A case may own only a layer row that names it as its one candidate.
        view.clear("layer:vector:color");
        view.assign_case("focused.frame-export.v1", "layer:vector:color")
            .unwrap();
        assert!(view.owner("layer:vector:color").is_err());
        let after = completeness(&fixture.record, &view);
        assert!(
            after.iter().all(|p| !p.contains("shaping/fallback")
                && !p.contains("format planning")
                && !p.contains("focused.frame-export.v1")),
            "{after:?}"
        );
        assert!(
            after
                .iter()
                .any(|p| p
                    .contains("vector color: layer:vector:color: case owner does not establish")),
            "{after:?}"
        );
    }

    fn executable_renderer() -> &'static str {
        "excalidraw-render"
    }

    fn ownership_representative_row() -> &'static str {
        "interaction:frames-text-assets"
    }

    #[test]
    fn representative_84_cases_need_approval_sanitization_license_and_crops() {
        // case.py's subject computation reproduces every approved subject.
        for entry in Fixture::new().record["representative"]["cases"]
            .as_array()
            .unwrap()
        {
            let case: Value =
                parse(&read(&format!("{}/case.json", entry["path"].as_str().unwrap())).unwrap())
                    .unwrap();
            assert_eq!(
                case["approval"]["approval_subject_sha256"],
                entry["subject_sha256"]
            );
        }
        let edit = |edit: &dyn Fn(&mut Value), expected: &str| {
            let mut fixture = Fixture::new();
            let mut case = fixture.json(VARMQ);
            edit(&mut case);
            fixture.set(VARMQ, crate::conformance::canonical_bytes(&case).unwrap());
            fails(fixture.structure(), expected);
        };
        edit(
            &|c| c["approval"]["approval_url"] = json!(URL.replace("/issues/85#", "/issues/84#")),
            "approval identity, date, or URL is invalid",
        );
        edit(&|c| c["approval"] = Value::Null, "is not approved");
        edit(
            &|c| c["semantic_expectations"]["light"]["links"] = json!(1),
            "approval subject does not match",
        );
        edit(&|c| c["comparisons"] = json!([]), "comparisons are empty");
        edit(
            &|c| c["intake"]["sha256"] = json!("0".repeat(64)),
            "intake bundle changed",
        );

        // Sanitization: a retained digest that is not the retained scene's,
        // with the case rebound to the changed manifest.
        let mut fixture = Fixture::new();
        let bundle = "tests/conformance/intake/varmq-queue-diagram-v1";
        let mut manifest = fixture.json(&format!("{bundle}/manifest.json"));
        manifest["sanitization"]["retained_sha256"] = json!("0".repeat(64));
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        let mut case = fixture.json(VARMQ);
        case["intake"]["bytes"] = json!(bytes.len());
        case["intake"]["sha256"] = json!(sha256(&bytes));
        fixture.set(&format!("{bundle}/manifest.json"), bytes);
        fixture.set_json(VARMQ, &case);
        fails(fixture.structure(), "sanitization retained_sha256");

        // Crops: only the whole-scene judgment left.
        let mut fixture = Fixture::new();
        let plan_path = format!("{bundle}/plan.json");
        let mut plan = fixture.json(&plan_path);
        plan["regions"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["id"] == "whole");
        fixture.set_json(&plan_path, &plan);
        let mut manifest = fixture.json(&format!("{bundle}/manifest.json"));
        manifest["files"]["plan.json"] = derive_descriptor(&fixture.repo.read(&plan_path).unwrap());
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        let mut case = fixture.json(VARMQ);
        case["intake"]["bytes"] = json!(bytes.len());
        case["intake"]["sha256"] = json!(sha256(&bytes));
        fixture.set(&format!("{bundle}/manifest.json"), bytes);
        fixture.set_json(VARMQ, &case);
        fails(fixture.structure(), "at least one anchored crop");
    }

    fn derive_descriptor(bytes: &[u8]) -> Value {
        json!({"bytes":bytes.len(),"sha256":sha256(bytes)})
    }

    #[test]
    fn python_subject_serialization_matches_case_py() {
        // Expected strings are `json.dumps(v, sort_keys=True,
        // separators=(",", ":"), ensure_ascii=False)` under CPython.
        let value = json!({"b":[1,-2,0.5,1.0,1e16,1.5e-5,123456.789,-0.0],"a":"é\u{1}\"\\\n/","ä":null,"Z":true});
        let mut out = String::new();
        derive::python_json(&value, &mut out).unwrap();
        assert_eq!(
            out,
            "{\"Z\":true,\"a\":\"é\\u0001\\\"\\\\\\n/\",\"b\":[1,-2,0.5,1.0,1e+16,1.5e-05,123456.789,-0.0],\"ä\":null}"
        );
        assert_eq!(derive::python_float(1e-4).unwrap(), "0.0001");
        assert_eq!(
            derive::python_float(1234567890123456.0).unwrap(),
            "1234567890123456.0"
        );
        assert_eq!(derive::python_float(2.5e-300).unwrap(), "2.5e-300");
    }

    #[test]
    fn required_risks_are_parsed_from_syntax_not_text() {
        let parse = |text: &str| derive::required_risks(&[("t.rs".into(), text.into())]);
        assert_eq!(
            parse("mod corpus { const REQUIRED_RISKS: [Risk; 2] = [Risk::A, Risk::B]; }").unwrap(),
            ["A", "B"]
        );
        // A comment or string mentioning it is not a declaration.
        assert!(parse(
            "// const REQUIRED_RISKS: [Risk; 1] = [Risk::A];\nconst X: &str = \"REQUIRED_RISKS\";"
        )
        .unwrap()
        .is_empty());
        assert!(parse("const REQUIRED_RISKS: [Risk; 1] = [Risk::A, Risk::B];").is_err());
        assert!(parse("const REQUIRED_RISKS: [Risk; 2] = [Risk::A, Risk::A];").is_err());
        assert!(parse("const REQUIRED_RISKS: [Risk; 1] = [Risk::A]; mod m { const REQUIRED_RISKS: [Risk; 1] = [Risk::B]; }").is_err());
    }

    #[test]
    fn spec_families_follow_the_sentence() {
        let spec = "x\nExact vectors cover Rough.js, the qualified\nPerfect Freehand path, and format planning. Next.";
        assert_eq!(
            derive::spec_families(spec).unwrap(),
            [
                "Rough.js",
                "the qualified Perfect Freehand path",
                "format planning"
            ]
        );
        let mut fixture = Fixture::new();
        let spec = String::from_utf8(fixture.repo.read(derive::SPEC).unwrap()).unwrap();
        fixture.set(
            derive::SPEC,
            spec.replacen(
                "diagnostic order, and format planning.",
                "diagnostic order, format planning, and caching.",
                1,
            )
            .into_bytes(),
        );
        fails(fixture.structure(), "declarations.vector_families");
    }

    #[test]
    fn approval_binds_the_exact_subject_on_issue_85() {
        let fixture = Fixture::new();
        let subject = subject_of(&fixture.record).unwrap();
        let mut record = fixture.record.clone();
        assert!(
            apply_approval(&mut record, "sagikazarmark", "2026-09-24", URL, "sha256:00")
                .unwrap_err()
                .contains("subject mismatch")
        );
        let wrong = URL.replace("/issues/85#", "/issues/44#");
        assert!(apply_approval(
            &mut record.clone(),
            "sagikazarmark",
            "2026-09-24",
            &wrong,
            &subject
        )
        .is_err());
        apply_approval(&mut record, "sagikazarmark", "2026-09-24", URL, &subject).unwrap();
        assert_eq!(subject_of(&record).unwrap(), subject);
        assert!(apply_approval(
            &mut record.clone(),
            "sagikazarmark",
            "2026-09-24",
            URL,
            &subject
        )
        .unwrap_err()
        .contains("already approved"));
        // An approved record is structurally current; a changed declaration
        // no longer carries its approval.
        let mut fixture = fixture;
        fixture.set(RECORD, record_bytes(&record).unwrap());
        fixture.structure().unwrap();
        let mut changed = record;
        changed["declarations"]["vector_families"][0]["rationale"] = json!("changed");
        let changed_subject = subject_of(&changed).unwrap();
        assert_ne!(changed_subject, subject);
        assert!(verify_approval_for_issue(&changed["approval"], &changed_subject, "85").is_err());
    }
}
