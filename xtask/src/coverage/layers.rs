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
use derive::{build_record, completeness};

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
fn current_record(repo: &Repo) -> Result<Value, String> {
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
    let mut rebuilt = build_record(repo, &record["declarations"])?;
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
    let record = current_record(repo)?;
    let problems = completeness(&record, repo, view);
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

fn generate_in(repo: &Repo) -> Result<Value, String> {
    build_record(repo, &existing_declarations(repo)?)
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
        "{} vector families, {} vector domains, {} vector corpora, {} oracle directories, {} registered cases, {} #84 cases, {} adversarial targets, {} regression corpora",
        count(record, "/declarations/vector_families"),
        count(record, "/declarations/vector_domains"),
        count(record, "/vector/corpora"),
        count(record, "/declarations/oracles"),
        count(record, "/focused/cases"),
        count(record, "/representative/cases"),
        count(record, "/adversarial/targets"),
        count(record, "/regression/corpora"),
    )
}

pub fn generate_layers() -> Result<(), String> {
    let repo = Repo::real();
    let view = LedgerView::load()?;
    let record = generate_in(&repo)?;
    write_record(&record)?;
    let problems = completeness(&record, &repo, &view);
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
    const CANDIDATES: &str = "tests/conformance/candidates/registry.json";
    const REPRESENTATIVE_ROW: &str = "interaction:frames-text-assets";

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
            view.clear(REPRESENTATIVE_ROW);
            let record = generate_in(&repo).unwrap();
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
            current_record(&self.repo)
        }
        fn regenerate(&mut self) -> Value {
            let record = generate_in(&self.repo).unwrap();
            self.set(RECORD, record_bytes(&record).unwrap());
            self.record = record.clone();
            record
        }
        fn problems(&self) -> Vec<String> {
            completeness(&self.record, &self.repo, &self.view)
        }
        fn has_problem(&self, expected: &str) {
            let problems = self.problems();
            assert!(
                problems.iter().any(|p| p.contains(expected)),
                "{expected}: {problems:?}"
            );
        }
    }

    fn fails(result: Result<Value, String>, expected: &str) {
        let error = result.unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }

    #[test]
    fn generation_is_deterministic_canonical_and_discovers_every_layer() {
        let fixture = Fixture::new();
        assert_eq!(generate_in(&fixture.repo).unwrap(), fixture.record);
        assert_eq!(fixture.structure().unwrap(), fixture.record);
        let record = &fixture.record;
        assert_eq!(
            record["spec"]["family_sentence"],
            "Exact vectors cover Rough.js, the qualified Perfect Freehand path, PRNG consumption, normalization, geometry, color, shaping/fallback, serialization, diagnostic order, and format planning."
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
        assert_eq!(count(record, "/focused/cases"), 37);
    }

    #[test]
    fn successors_spec_edits_and_catalog_growth_do_not_stale_the_record() {
        // A representative successor, as the V25 apply moves the registry,
        // plus its collection directory and a new intake bundle and public
        // scene, all excluded or checked by rule.
        let mut fixture = Fixture::new();
        let mut registry = fixture.json(CANDIDATES);
        let entry = &mut registry["cases"][0];
        entry["id"] = json!("representative.stage4-frames-text-assets.v26");
        entry["directory"] = json!("tests/conformance/candidates/stage4-frames-text-assets-v26");
        fixture.set_json(CANDIDATES, &registry);
        for path in [
            "tests/conformance/candidates/stage4-frames-text-assets-v26/manifest.json",
            "tests/conformance/intake/new-bundle-v1/manifest.json",
            "tests/public-scenes/new-scene/scene.excalidraw",
        ] {
            fixture.set(path, b"{}".to_vec());
        }
        // A spec edit outside the family sentence.
        let spec = String::from_utf8(fixture.repo.read(derive::SPEC).unwrap()).unwrap();
        fixture.set(
            derive::SPEC,
            format!("{spec}\nA later amendment.\n").into_bytes(),
        );
        assert_eq!(fixture.structure().unwrap(), fixture.record);
        // The successor is judged against the ledger's representative binding.
        fixture.has_problem(
            "representative candidate representative.stage4-frames-text-assets.v26: interaction:frames-text-assets is not owned by the representative binding of this entry",
        );
        // A new catalog row under a declared selector joins its item live.
        let mut fixture = Fixture::new();
        let before = fixture.problems();
        fixture
            .view
            .add_row("codec:png:icc:new-profile", "codec-profile", &[]);
        assert_eq!(fixture.structure().unwrap(), fixture.record);
        let after = fixture.problems();
        assert!(
            after.iter().all(|p| !p.contains("png-color")) && after.len() == before.len(),
            "{after:?}"
        );
    }

    #[test]
    fn the_candidate_registry_holds_exactly_the_bound_representative() {
        let mut fixture = Fixture::new();
        let mut registry = fixture.json(CANDIDATES);
        let second = registry["cases"][0].clone();
        registry["cases"].as_array_mut().unwrap().push(second);
        fixture.set_json(CANDIDATES, &registry);
        fixture.has_problem("exactly one representative entry");
        let mut fixture = Fixture::new();
        let mut registry = fixture.json(CANDIDATES);
        registry["cases"][0]["coverage"] = json!(["interaction:diagnostic-order"]);
        fixture.set_json(CANDIDATES, &registry);
        fixture.has_problem("does not cover interaction:frames-text-assets");
        fixture
            .has_problem("interaction:diagnostic-order is not owned by the representative binding");
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
    fn a_new_case_changes_the_record_and_needs_an_owner() {
        let mut fixture = Fixture::new();
        let mut registry = fixture.json(REGISTRY);
        let mut case = registry["cases"][0].clone();
        case["id"] = json!("focused.unowned.v1");
        registry["cases"].as_array_mut().unwrap().push(case);
        fixture.set_json(REGISTRY, &registry);
        fails(fixture.structure(), "does not reproduce");
        fixture.regenerate();
        fixture.structure().unwrap();
        fixture.has_problem("registered case focused.unowned.v1 is the case owner of none");
    }

    #[test]
    fn new_fuzz_targets_risks_and_regression_ids_fail_until_regenerated() {
        for (path, row) in [
            (
                "crates/excalidraw-render/src/render/pdf_fuzz.rs",
                "layer:adversarial:pdf-fuzz",
            ),
            (
                "crates/excalidraw-render/tests/limits_output.rs",
                "layer:adversarial:limits-output",
            ),
            (
                "crates/excalidraw-render/tests/qualification/asset-fuzz-regressions.json",
                "layer:regression:asset-fuzz",
            ),
        ] {
            let mut fixture = Fixture::new();
            let bytes = if path.ends_with(".json") {
                b"{\"cases\":[]}".to_vec()
            } else {
                b"#[test] fn smoke() {}".to_vec()
            };
            fixture.set(path, bytes);
            fails(fixture.structure(), "does not reproduce");
            fixture.regenerate();
            fixture.has_problem(&format!("has no catalog row {row}"));
        }

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
        // An edit that keeps the target and its risks is not a new record.
        let mut fixture = Fixture::new();
        fixture.set(FONT_FUZZ, format!("{source}\n// comment\n").into_bytes());
        fixture.structure().unwrap();

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
    }

    #[test]
    fn every_layer_row_is_referenced_existing_and_owned() {
        let mut fixture = Fixture::new();
        fixture.view.add_row("layer:vector:orphan", "layer", &[]);
        fixture.structure().unwrap();
        fixture.has_problem("orphan layer row(s) no item references: [\"layer:vector:orphan\"]");

        let mut fixture = Fixture::new();
        fixture.view.clear("layer:vector:rough-js");
        fixture.view.clear("interaction:diagnostic-order");
        for expected in [
            "vector Rough.js: layer row layer:vector:rough-js has no primary owner",
            "vector diagnostic order: no mapped row is owned",
            "adversarial crates/excalidraw-render/src/scene_fuzz: layer:adversarial:scene-fuzz has no primary owner",
            "regression crates/excalidraw-render/tests/qualification/render-fuzz-regressions.json: layer:regression:render-fuzz has no primary owner",
        ] {
            fixture.has_problem(expected);
        }
        // Declared selectors must match current rows, never closure rows.
        for (rows, expected) in [
            (
                json!(["closure:all-five-layers"]),
                "cannot answer a vector item",
            ),
            (json!(["codec:nope:*"]), "matches no catalog row"),
            (
                json!(["layer:focused:frame-export"]),
                "cannot answer a vector item",
            ),
        ] {
            let mut fixture = Fixture::new();
            let mut declarations = fixture.record["declarations"].clone();
            declarations["vector_families"][0]["rows"] = rows;
            fixture.record = build_record(&fixture.repo, &declarations).unwrap();
            fixture.has_problem(expected);
        }
    }

    #[test]
    fn recording_a_reviewed_owner_clears_exactly_its_items() {
        let mut fixture = Fixture::new();
        for row in [
            "layer:vector:shaping-fallback",
            "layer:vector:format-planning",
            "layer:focused:frame-export",
            "layer:vector:color",
        ] {
            fixture.view.clear(row);
        }
        for expected in [
            "vector shaping/fallback: no mapped row is owned",
            "vector format planning: no mapped row is owned",
            "registered case focused.frame-export.v1 is the case owner of none of its rows [\"layer:focused:frame-export\"]",
        ] {
            fixture.has_problem(expected);
        }
        let view = &mut fixture.view;
        view.assign_test(
            "excalidraw-render",
            "lib",
            "render::tests",
            "a_font_fallback_probe_is_one_consulted_face_per_shaped_scalar_occurrence",
            "layer:vector:shaping-fallback",
        )
        .unwrap();
        view.assign_test(
            "excalidraw-render",
            "lib",
            "render::plan_tests",
            "png_surface_follows_the_sizing_rule",
            "layer:vector:format-planning",
        )
        .unwrap();
        view.assign_case("focused.frame-export.v1", "layer:focused:frame-export")
            .unwrap();
        // A case may own only a layer row that names it as its one candidate.
        view.assign_case("focused.frame-export.v1", "layer:vector:color")
            .unwrap();
        assert!(view.owner("layer:vector:color").is_err());
        let after = fixture.problems();
        assert!(
            after.iter().all(|p| !p.contains("shaping/fallback")
                && !p.contains("format planning")
                && !p.contains("focused.frame-export.v1")),
            "{after:?}"
        );
        fixture.has_problem("vector color: layer:vector:color: case owner does not establish");
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

        let bundle = "tests/conformance/intake/varmq-queue-diagram-v1";
        let manifest_path = format!("{bundle}/manifest.json");
        let rebind = |fixture: &mut Fixture, manifest: &Value| {
            let bytes = serde_json::to_vec_pretty(manifest).unwrap();
            let mut case = fixture.json(VARMQ);
            case["intake"]["bytes"] = json!(bytes.len());
            case["intake"]["sha256"] = json!(sha256(&bytes));
            fixture.set(&manifest_path, bytes);
            fixture.set_json(VARMQ, &case);
        };
        // Sanitization: a retained digest that is not the retained scene's.
        let mut fixture = Fixture::new();
        let mut manifest = fixture.json(&manifest_path);
        manifest["sanitization"]["retained_sha256"] = json!("0".repeat(64));
        rebind(&mut fixture, &manifest);
        fails(fixture.structure(), "sanitization retained_sha256");
        // License: an empty SPDX value.
        let mut fixture = Fixture::new();
        let mut manifest = fixture.json(&manifest_path);
        manifest["license"] = json!(" ");
        rebind(&mut fixture, &manifest);
        fails(fixture.structure(), "no license");
        // Crops: only the whole-scene judgment left.
        let mut fixture = Fixture::new();
        let plan_path = format!("{bundle}/plan.json");
        let mut plan = fixture.json(&plan_path);
        plan["regions"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["id"] == "whole");
        fixture.set_json(&plan_path, &plan);
        let plan_bytes = fixture.repo.read(&plan_path).unwrap();
        let mut manifest = fixture.json(&manifest_path);
        manifest["files"]["plan.json"] =
            json!({"bytes":plan_bytes.len(),"sha256":sha256(&plan_bytes)});
        rebind(&mut fixture, &manifest);
        fails(fixture.structure(), "at least one anchored crop");
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
    fn the_family_sentence_is_bound_and_parsed() {
        let spec = "x\nExact vectors cover Rough.js, the qualified\nPerfect Freehand path, and format planning. Next.";
        let (sentence, families) = derive::spec_families(spec).unwrap();
        assert_eq!(
            sentence,
            "Exact vectors cover Rough.js, the qualified Perfect Freehand path, and format planning."
        );
        assert_eq!(
            families,
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
        // A reflowed but identical sentence is the same binding.
        let mut fixture = Fixture::new();
        fixture.set(
            derive::SPEC,
            spec.replacen(
                "the qualified Perfect Freehand path, PRNG",
                "the qualified Perfect\nFreehand path, PRNG",
                1,
            )
            .into_bytes(),
        );
        assert_eq!(fixture.structure().unwrap(), fixture.record);
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
