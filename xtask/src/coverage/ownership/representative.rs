//! The only supplemental executable owner admitted here is the approved
//! frames/text/assets representative case. Recorded commands are never executed
//! as arbitrary shell input: dispatch is to the checked candidate consumer.
use super::*;

pub(super) const OBLIGATION: &str = "interaction:frames-text-assets";
const REGISTRY_PATH: &str = "tests/conformance/candidates/registry.json";
const ADAPTER_SOURCES: [&str; 7] = [
    "xtask/src/main.rs",
    "xtask/src/coverage.rs",
    "xtask/src/coverage/ownership.rs",
    "xtask/src/coverage/ownership/representative.rs",
    "xtask/src/coverage/ownership/authority_migration.rs",
    "xtask/src/conformance_candidate_gate.rs",
    "xtask/src/conformance_candidate_native.rs",
];
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding {
    case: String,
    registry: Descriptor,
    manifest: Descriptor,
    approval_subject_sha256: String,
    command: Vec<String>,
}
impl Binding {
    /// The bound candidate case.
    pub(super) fn case(&self) -> &str {
        &self.case
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema: String,
    cases: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    directory: String,
    formats: Vec<String>,
    approval_required: bool,
    coverage: Vec<String>,
    execution_keys: Vec<String>,
    consumer: Vec<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    exceptions: Vec<Value>,
}
fn directory(case: &str) -> Result<String, String> {
    let suffix = case
        .strip_prefix("representative.stage4-frames-text-assets.v")
        .ok_or("unsupported representative owner case")?;
    let revision = suffix
        .parse::<u32>()
        .map_err(|_| "unsupported representative owner revision")?;
    if revision < 4 || suffix != revision.to_string() {
        return Err("unsupported representative owner revision".into());
    }
    Ok(format!(
        "tests/conformance/candidates/stage4-frames-text-assets-v{revision}"
    ))
}
fn command(directory: &str) -> Vec<String> {
    [
        "cargo",
        "run",
        "--release",
        "--locked",
        "-p",
        "xtask",
        "--",
        "conformance",
        "candidate-verify",
        directory,
    ]
    .map(str::to_owned)
    .to_vec()
}
fn entry(bytes: &[u8], case: &str) -> Result<Entry, String> {
    let registry: Registry = parse(bytes)?;
    if registry.schema != "excalidraw-render/representative-corpus-adapter/v1"
        || registry.cases.len() != 1
    {
        return Err("representative owner requires the exact single-case registry adapter".into());
    }
    let row = registry
        .cases
        .into_iter()
        .next()
        .ok_or("missing representative registry case")?;
    if row.id != case {
        return Err("unknown/unregistered representative owner case".into());
    }
    let dir = directory(case)?;
    if row.directory != dir
        || row.formats != ["svg", "png", "pdf"]
        || !row.approval_required
        || row.coverage != [OBLIGATION]
        || row.execution_keys != ["svg", "png", "pdf"].map(|f| format!("{case}:default:{f}"))
        || row.consumer != command(&dir)
        || row.status.as_ref().is_some_and(|s| s.trim().is_empty())
    {
        return Err("representative registry must select its exact release approval-aware consumer and three execution keys".into());
    }
    Ok(row)
}
fn check_binding(
    binding: &Binding,
    registry_bytes: &[u8],
    manifest_bytes: &[u8],
) -> Result<(String, Value), String> {
    let dir = directory(&binding.case)?;
    if binding.registry.path != REGISTRY_PATH
        || binding.registry.sha256 != sha256(registry_bytes)
        || binding.manifest.path != format!("{dir}/manifest.json")
        || binding.manifest.sha256 != sha256(manifest_bytes)
    {
        return Err("stale or redirected representative registry/manifest binding".into());
    }
    if binding.command != command(&dir) {
        return Err(
            "representative owner command is not the exact release approval-aware consumer".into(),
        );
    }
    let row = entry(registry_bytes, &binding.case)?;
    let manifest: Value = parse(manifest_bytes)?;
    if manifest["schema"] != "excalidraw-render/representative-candidate/v3"
        || manifest["case_id"] != binding.case
        || manifest["layer"] != "representative"
        || manifest["compatibility_revision"] != 1
        || manifest["coverage"] != json!([OBLIGATION])
        || json!(row.exceptions)
            != manifest["exception_proposal"]
                .get("exceptions")
                .cloned()
                .unwrap_or(json!([]))
    {
        return Err("representative manifest identity/scopes differ from its registry".into());
    }
    let hash = sha256(&canonical_bytes(
        &crate::conformance_candidate_gate::approval_bundle(&manifest)?,
    )?);
    if binding.approval_subject_sha256 != format!("sha256:{hash}") {
        return Err("stale representative owner approval subject".into());
    }
    verify_approval_for_issue(&manifest["approval"], &hash, "85")?;
    Ok((dir, manifest))
}
fn receipts(binding: &Binding, result: &Value) -> Result<(), String> {
    if result["schema"] != "excalidraw-render/representative-executions/v1"
        || result["approval_required"] != true
        || result["case_id"] != binding.case
        || format!(
            "sha256:{}",
            result["approval_subject_sha256"]
                .as_str()
                .unwrap_or_default()
        ) != binding.approval_subject_sha256
    {
        return Err("representative consumer did not return bound approved executions".into());
    }
    let rows = result["executions"]
        .as_array()
        .ok_or("missing representative executions")?;
    if rows.len() != 3 {
        return Err("representative consumer omitted an execution".into());
    }
    for (row, format) in rows.iter().zip(["svg", "png", "pdf"]) {
        if row["execution_key"] != format!("{}:default:{format}", binding.case)
            || row["native"]["format"] != format
            || row["native"]["fresh_process_runs"] != 2
            || row["approval_subject_sha256"] != result["approval_subject_sha256"]
        {
            return Err(
                "representative consumer returned an unknown, duplicate or incomplete execution"
                    .into(),
            );
        }
    }
    Ok(())
}
fn checked_operation(
    binding: &Binding,
    registry: &[u8],
    manifest: &[u8],
    execute: impl FnOnce(&str, &Value) -> Result<Value, String>,
) -> Result<(), String> {
    let (dir, manifest) = check_binding(binding, registry, manifest)?;
    receipts(binding, &execute(&dir, &manifest)?)
}
pub(super) fn verify(obligation: &str, binding: &Binding) -> Result<(), String> {
    if obligation != OBLIGATION {
        return Err("representative adapter cannot own an unrelated obligation".into());
    }
    checked_operation(
        binding,
        &read(REGISTRY_PATH)?,
        &read(&binding.manifest.path)?,
        |dir, manifest| {
            check_adapter_sources(manifest)?;
            // This consumer verifies current sources, approval, semantic and visual
            // judgments, and fresh release native artifacts/reports for all formats.
            crate::conformance_candidate_gate::execute(dir)
        },
    )
}
fn check_adapter_sources(manifest: &Value) -> Result<(), String> {
    let sources = manifest["producer_sources"]
        .as_array()
        .ok_or("missing representative adapter source bindings")?;
    for path in ADAPTER_SOURCES {
        let current = crate::conformance_candidate_gate::descriptor(&repo_root(), path)?;
        if sources
            .iter()
            .filter(|s| s["path"] == path)
            .collect::<Vec<_>>()
            != vec![&current]
        {
            return Err(format!(
                "representative owner adapter source is unbound or stale: {path}"
            ));
        }
    }
    Ok(())
}
pub(super) fn approved_owner(case: &str) -> Result<Owner, String> {
    let registry_bytes = read(REGISTRY_PATH)?;
    let row = entry(&registry_bytes, case)?;
    let path = format!("{}/manifest.json", row.directory);
    let manifest: Value = parse(&read(&path)?)?;
    let hash = sha256(&canonical_bytes(
        &crate::conformance_candidate_gate::approval_bundle(&manifest)?,
    )?);
    let binding = Binding {
        case: case.into(),
        registry: descriptor(REGISTRY_PATH)?,
        manifest: descriptor(&path)?,
        approval_subject_sha256: format!("sha256:{hash}"),
        command: command(&row.directory),
    };
    verify(OBLIGATION, &binding)?;
    Ok(Owner::Representative { binding })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn representative_owner_release_positive_path_uses_real_gate_in_isolated_fixture() {
        // Synthetic authorization exists only below target/. Reuse qualified
        // viewer bytes only after fresh release SVG/PDF are byte-identical to
        // their inputs. This tests the verifier, not a new corpus approval.
        use crate::{
            conformance_candidate as assets, conformance_candidate_gate as gate,
            conformance_candidate_native as native,
        };
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let parent = format!(
            "target/representative-owner-positive-{}-{nonce}",
            std::process::id()
        );
        fs::create_dir(repo_root().join(&parent)).unwrap();
        let dir = format!("{parent}/stage4-frames-text-assets-v7");
        gate::revise(&dir).unwrap();
        let root = repo_root().join(&dir);
        for name in ["assets", "viewer-evidence"] {
            fs::create_dir(root.join(name)).unwrap();
        }
        native::capture(&dir).unwrap();
        let old = "tests/conformance/candidates/stage4-frames-text-assets-v6";
        assert_eq!(
            gate::read(&root, "native.svg").unwrap(),
            read(&format!("{old}/native.svg")).unwrap()
        );
        crate::conformance_pdf_bridge::validate(&gate::read(&root, "native.pdf").unwrap()).unwrap();
        let status = std::process::Command::new("node")
            .current_dir(repo_root())
            .args([
                "tools/reference-oracle/scripts/stage4-viewer-assets.mjs",
                &dir,
            ])
            .status()
            .unwrap();
        assert!(status.success());
        assets::prepare_assets(&dir).unwrap();
        let rebase = |path: &str| path.replace(old, &dir);
        let mut profile: Value =
            parse(&read(&format!("{old}/chromium-text-profile.json")).unwrap()).unwrap();
        for d in profile["sources"].as_array_mut().unwrap() {
            *d = gate::descriptor(&repo_root(), &rebase(d["path"].as_str().unwrap())).unwrap();
        }
        fs::write(
            root.join("chromium-text-profile.json"),
            canonical_bytes(&profile).unwrap(),
        )
        .unwrap();
        let mut collection: Value =
            parse(&read(&format!("{old}/viewer-evidence/collection.json")).unwrap()).unwrap();
        collection["candidate_directory"] = json!(dir);
        for row in collection["captures"].as_array_mut().unwrap() {
            let path = row["raster"]["path"].as_str().unwrap();
            let new = rebase(path);
            for suffix in [".rgba", ".repeat.rgba"] {
                let from = path.strip_suffix(".rgba").unwrap().to_owned() + suffix;
                let to = new.strip_suffix(".rgba").unwrap().to_owned() + suffix;
                fs::write(repo_root().join(to), read(&from).unwrap()).unwrap();
            }
            row["raster"] = gate::descriptor(&repo_root(), &new).unwrap();
            row["input"] = gate::descriptor(
                &repo_root(),
                &rebase(row["input"]["path"].as_str().unwrap()),
            )
            .unwrap();
            row["viewer_profile"] = json!(rebase(row["viewer_profile"].as_str().unwrap()));
        }
        for field in ["sources", "profiles", "assets"] {
            for d in collection[field].as_array_mut().unwrap() {
                *d = gate::descriptor(&repo_root(), &rebase(d["path"].as_str().unwrap())).unwrap();
            }
        }
        fs::write(
            root.join("viewer-evidence/collection.json"),
            canonical_bytes(&collection).unwrap(),
        )
        .unwrap();
        crate::conformance_candidate_exceptions::prepare(&dir).unwrap();
        let prepared = gate::materialize(&dir).unwrap();
        assert_eq!(prepared["gates_passed"], true);
        assert!(gate::execute(&dir).is_err(), "proposal is not approval");
        let mut manifest = gate::value(&root, "manifest.json").unwrap();
        let hash = prepared["approval_subject_sha256"].as_str().unwrap();
        manifest["approval"] = json!({"approved_by":"synthetic-test","approved_at":"2026-09-19","approval_url":"https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1","approval_subject_sha256":format!("sha256:{hash}")});
        let manifest_bytes = canonical_bytes(&manifest).unwrap();
        fs::write(root.join("manifest.json"), &manifest_bytes).unwrap();
        let (mut binding, registry, _, _) = fixture();
        let mut registry: Value = parse(&registry).unwrap();
        registry["cases"][0]["exceptions"] = manifest["exception_proposal"]["exceptions"].clone();
        let registry_bytes = canonical_bytes(&registry).unwrap();
        binding.registry.sha256 = sha256(&registry_bytes);
        binding.manifest.sha256 = sha256(&manifest_bytes);
        binding.approval_subject_sha256 = format!("sha256:{hash}");
        checked_operation(
            &binding,
            &registry_bytes,
            &manifest_bytes,
            |logical, manifest| {
                assert_eq!(logical, directory(&binding.case).unwrap());
                check_adapter_sources(manifest)?;
                gate::execute(&dir)
            },
        )
        .unwrap();
        // The same real gate refuses corrupted artifact evidence even with the
        // fixture's previously matching approval object still present.
        let mut corrupted = gate::read(&root, "native.png").unwrap();
        corrupted.push(0);
        fs::write(root.join("native.png"), corrupted).unwrap();
        assert!(gate::execute(&dir)
            .unwrap_err()
            .contains("stale candidate evidence"));
    }
    #[test]
    fn representative_owner_sources_are_in_the_final_bundle_and_cannot_be_omitted() {
        let sources = crate::conformance_candidate_gate::adapter_sources().unwrap();
        let valid = json!({"producer_sources":sources});
        check_adapter_sources(&valid).unwrap();
        for path in ADAPTER_SOURCES {
            let mut bad = valid.clone();
            bad["producer_sources"]
                .as_array_mut()
                .unwrap()
                .retain(|s| s["path"] != path);
            assert!(check_adapter_sources(&bad).is_err(), "{path}");
        }
        let mut bad = valid;
        let source = bad["producer_sources"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|s| s["path"] == ADAPTER_SOURCES[0])
            .unwrap();
        source["sha256"] = json!("stale");
        assert!(check_adapter_sources(&bad).is_err());
    }
    fn fixture() -> (Binding, Vec<u8>, Vec<u8>, Value) {
        // Isolated in-memory authorization fixture: not corpus evidence.
        let case = "representative.stage4-frames-text-assets.v7";
        let dir = directory(case).unwrap();
        let keys = ["svg", "png", "pdf"].map(|f| format!("{case}:default:{f}"));
        let registry = json!({"schema":"excalidraw-render/representative-corpus-adapter/v1","cases":[{"id":case,"directory":dir,"formats":["svg","png","pdf"],"approval_required":true,"coverage":[OBLIGATION],"execution_keys":keys,"consumer":command(&dir),"exceptions":[]}]});
        let mut manifest = json!({"schema":"excalidraw-render/representative-candidate/v3","case_id":case,"layer":"representative","compatibility_revision":1,"coverage":[OBLIGATION],"files":[],"approval":null,"exception_proposal":null});
        let hash = sha256(
            &canonical_bytes(
                &crate::conformance_candidate_gate::approval_bundle(&manifest).unwrap(),
            )
            .unwrap(),
        );
        manifest["approval"] = json!({"approved_by":"synthetic-test","approved_at":"2026-09-19","approval_url":"https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-1","approval_subject_sha256":format!("sha256:{hash}")});
        let registry = canonical_bytes(&registry).unwrap();
        let manifest = canonical_bytes(&manifest).unwrap();
        let binding = Binding {
            case: case.into(),
            registry: Descriptor {
                path: REGISTRY_PATH.into(),
                sha256: sha256(&registry),
            },
            manifest: Descriptor {
                path: format!("{dir}/manifest.json"),
                sha256: sha256(&manifest),
            },
            approval_subject_sha256: format!("sha256:{hash}"),
            command: command(&dir),
        };
        let executions=["svg","png","pdf"].map(|f|json!({"execution_key":format!("{case}:default:{f}"),"approval_subject_sha256":hash,"native":{"format":f,"fresh_process_runs":2}}));
        let result = json!({"schema":"excalidraw-render/representative-executions/v1","approval_required":true,"case_id":case,"approval_subject_sha256":hash,"executions":executions});
        (binding, registry, manifest, result)
    }
    #[test]
    fn representative_owner_requires_approval_before_invoking_consumer() {
        let (binding, registry, bytes, result) = fixture();
        let calls = std::cell::Cell::new(0);
        checked_operation(&binding, &registry, &bytes, |_, _| {
            calls.set(calls.get() + 1);
            Ok(result.clone())
        })
        .unwrap();
        assert_eq!(calls.get(), 1);
        let mut manifest: Value = parse(&bytes).unwrap();
        manifest["approval"] = Value::Null;
        let unapproved = canonical_bytes(&manifest).unwrap();
        let mut changed = binding.clone();
        changed.manifest.sha256 = sha256(&unapproved);
        assert!(checked_operation(&changed, &registry, &unapproved, |_, _| {
            calls.set(calls.get() + 1);
            Ok(result.clone())
        })
        .is_err());
        assert_eq!(calls.get(), 1);
        assert!(checked_operation(&binding, &registry, &bytes, |_, _| Err(
            "dead consumer".into()
        ))
        .unwrap_err()
        .contains("dead consumer"));
        let mut incomplete = result.clone();
        incomplete["executions"].as_array_mut().unwrap().pop();
        assert!(checked_operation(&binding, &registry, &bytes, |_, _| Ok(incomplete)).is_err());
    }
    #[test]
    fn representative_owner_rejects_unknown_redirected_stale_and_arbitrary_commands() {
        let (binding, registry, manifest, result) = fixture();
        for change in 0..7 {
            let mut bad = binding.clone();
            match change {
                0 => bad.case = "representative.unknown.v7".into(),
                1 => bad.registry.sha256 = "0".repeat(64),
                2 => bad.manifest.sha256 = "0".repeat(64),
                3 => bad.approval_subject_sha256 = format!("sha256:{}", "0".repeat(64)),
                4 => bad.command = vec!["cargo".into(), "test".into(), "-p".into(), "xtask".into()],
                5 => bad.manifest.path = "elsewhere/manifest.json".into(),
                _ => bad.command.retain(|s| s != "--release"),
            }
            assert!(
                checked_operation(&bad, &registry, &manifest, |_, _| Ok(result.clone())).is_err(),
                "{change}"
            );
        }
        assert!(verify("codec:png:limits", &binding)
            .unwrap_err()
            .contains("unrelated"));
        let mut duplicate = result;
        duplicate["executions"][2]["execution_key"] =
            duplicate["executions"][0]["execution_key"].clone();
        assert!(receipts(&binding, &duplicate).is_err());
    }
}
