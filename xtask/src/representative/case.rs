//! Read-only registered case verification and judgment of retained rasters.
use super::*;
use std::path::{Component, PathBuf};
use std::time::Duration;

mod judge;
mod materialize;
mod semantics;

const REGISTRY: &str = "tests/conformance/cases/representative/registry.json";
use super::comparison::COMPARISONS;
const JUDGMENT_RULE: &str = "canonical-budget-or-identical-input-controls-or-exception/v1";

pub(super) fn run(args: &[String]) -> Result<()> {
    if args.first().is_some_and(|a| a == "materialize") {
        return materialize::run(&args[1..]);
    }
    if let [mode, directory] = args {
        if mode == "subject" {
            let case = decode(&read(Path::new(directory), "case.json")?)?;
            validate(
                &crate::conformance::repo_root(),
                Path::new(directory),
                &case,
            )?;
            println!("sha256:{}", subject(&case)?);
            return Ok(());
        }
    }
    if let [mode, directory, collection] = args {
        if mode == "judge" {
            let case = decode(&read(Path::new(directory), "case.json")?)?;
            validate(
                &crate::conformance::repo_root(),
                Path::new(directory),
                &case,
            )?;
            let summary = judge::judge(&case, Path::new(collection))?;
            println!(
                "{}",
                String::from_utf8(json::compact(&summary)?).map_err(|e| e.to_string())?
            );
            return Ok(());
        }
    }
    let (mode, rest) = args.split_first().ok_or("missing case command")?;
    let (directory, rest) = match mode.as_str() {
        "verify" => {
            let (path, rest) = rest.split_first().ok_or("missing case directory")?;
            (Some(Path::new(path)), rest)
        }
        "verify-registry" => (None, rest),
        _ => return Err("usage: representative case verify <directory> [--renderer <cli>] | verify-registry [--renderer <cli>] | judge <case-directory> <viewer-collection>".into()),
    };
    let (renderer, viewers) = match rest
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => (None, false),
        ["--renderer", path] => (Some(PathBuf::from(path)), false),
        ["--renderer", path, "--viewers"] | ["--viewers", "--renderer", path] => {
            (Some(PathBuf::from(path)), true)
        }
        ["--viewers"] => return Err("--viewers requires --renderer".into()),
        _ => return Err("unexpected case option; expected --renderer <cli>".into()),
    };
    let root = crate::conformance::repo_root();
    let summaries = if let Some(directory) = directory {
        vec![verify_case(
            &root,
            directory,
            renderer.as_deref(),
            None,
            viewers,
        )?]
    } else {
        verify_registry(&root, renderer.as_deref(), viewers)?
    };
    for summary in summaries {
        println!(
            "{}",
            String::from_utf8(json::compact(&summary)?).map_err(|e| e.to_string())?
        );
    }
    Ok(())
}

fn repository_path(root: &Path, relative: &str) -> Result<PathBuf> {
    require(
        !relative.is_empty()
            && Path::new(relative)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "case path must be repository-relative",
    )?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    require(path.starts_with(&root), "case path escapes repository")?;
    Ok(path)
}

fn inventory(directory: &Path) -> Result<Value> {
    fn walk(root: &Path, relative: &Path, entries: &mut BTreeMap<String, Value>) -> Result<()> {
        for entry in fs::read_dir(root.join(relative)).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            require(!kind.is_symlink(), "symlink in retained case files")?;
            let path = relative.join(entry.file_name());
            if kind.is_dir() {
                walk(root, &path, entries)?;
            } else {
                require(kind.is_file(), "non-file in retained case")?;
                let name = path.to_str().ok_or("non-UTF8 case filename")?;
                if name != "case.json" {
                    #[cfg(unix)]
                    require(!name.contains('\\'), "backslash in retained case filename")?;
                    require(
                        entries
                            .insert(name.replace('\\', "/"), descriptor(&read(root, name)?))
                            .is_none(),
                        "duplicate retained case filename",
                    )?;
                }
            }
        }
        Ok(())
    }
    let mut entries = BTreeMap::new();
    walk(directory, Path::new(""), &mut entries)?;
    Ok(json!(entries))
}

fn subject(case: &Value) -> Result<String> {
    let mut body = case.clone();
    body.as_object_mut()
        .ok_or("invalid case object")?
        .remove("approval");
    Ok(sha256(&json::compact(&body)?))
}

fn strings<'a>(value: &'a Value, field: &str) -> Result<Vec<&'a str>> {
    array(value, field)?
        .iter()
        .map(|v| v.as_str().ok_or_else(|| format!("invalid {field} entry")))
        .collect()
}

fn validate(root: &Path, directory: &Path, case: &Value) -> Result<PathBuf> {
    require(
        case["schema"] == "excalidraw-render/representative-case/v1"
            && case["layer"] == "representative"
            && case["compatibility_revision"] == 1,
        "invalid case schema",
    )?;
    require(valid_case(text(case, "case_id")?), "invalid case identity")?;
    require(
        case["judgment_rule"] == JUDGMENT_RULE,
        "unsupported judgment rule",
    )?;
    require(
        case["files"] == inventory(directory)?,
        "retained case files changed",
    )?;
    let bundle = repository_path(root, text(&case["intake"], "path")?)?;
    require(
        descriptor(&read(&bundle, "manifest.json")?)
            == json!({"bytes": case["intake"]["bytes"], "sha256": case["intake"]["sha256"]}),
        "intake bundle changed",
    )?;
    let manifest = super::verify(&bundle)?;
    require(
        case["case_id"] == manifest["case_id"],
        "case id differs from intake bundle",
    )?;
    let plan = decode(&read(&bundle, "plan.json")?)?;
    let variants = strings(case, "variants")?;
    require(
        case["variants"]
            == json!(array(&plan, "variants")?
                .iter()
                .map(|v| &v["id"])
                .collect::<Vec<_>>()),
        "variant inventory differs from the plan",
    )?;
    let formats = strings(case, "formats")?;
    require(
        !formats.is_empty()
            && formats.iter().all(|f| FORMATS.contains(f))
            && formats.iter().collect::<BTreeSet<_>>().len() == formats.len(),
        "invalid registered formats",
    )?;
    let comparisons: Vec<_> = COMPARISONS
        .iter()
        .filter(|c| formats.contains(&c.format))
        .map(|c| c.id)
        .collect();
    require(
        case["comparisons"] == json!(comparisons),
        "comparison inventory",
    )?;
    let mut keys = BTreeSet::new();
    for exception in array(case, "exceptions")? {
        fields(
            exception,
            &[
                "variant",
                "comparison_id",
                "region",
                "max_changed_pixels",
                "minimum_ssim_bits",
                "reason",
                "approving_issue",
                "viewer_profile",
            ],
            "invalid exception fields",
        )?;
        let variant = text(exception, "variant")?;
        let comparison = text(exception, "comparison_id")?;
        require(
            variants.contains(&variant) && comparisons.contains(&comparison),
            "exception outside the case",
        )?;
        let profile = COMPARISONS
            .iter()
            .find(|c| c.id == comparison)
            .ok_or("unknown comparison")?
            .profile;
        require(
            exception["viewer_profile"] == json!(profile),
            "exception viewer profile",
        )?;
        let bits = text(exception, "minimum_ssim_bits")?;
        require(
            bits.len() == 16
                && bits
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                && !text(exception, "reason")?.trim().is_empty(),
            "invalid exception ceiling",
        )?;
        super::comparison::ssim(bits)?;
        require(
            exception["max_changed_pixels"].as_u64().is_some(),
            "invalid exception pixel ceiling",
        )?;
        require(
            keys.insert((variant, comparison, text(exception, "region")?)),
            "duplicate exception",
        )?;
    }
    let mut outcomes = BTreeSet::new();
    for outcome in resource_rows(case)? {
        fields(
            outcome,
            &["id", "variant", "format", "overrides", "expected_report"],
            "invalid resource outcome fields",
        )?;
        let id = text(outcome, "id")?;
        require(
            valid_id(id) && outcomes.insert(id),
            "invalid or duplicate resource outcome identity",
        )?;
        require(
            variants.contains(&text(outcome, "variant")?)
                && FORMATS.contains(&text(outcome, "format")?),
            "resource outcome outside the case",
        )?;
        let overrides = outcome["overrides"]
            .as_object()
            .ok_or("invalid resource outcome overrides")?;
        require(
            overrides
                .keys()
                .all(|k| matches!(k.as_str(), "scale" | "padding")),
            "unsupported resource outcome override",
        )?;
        for (key, value) in overrides {
            let n = value
                .as_f64()
                .filter(|n| n.is_finite())
                .ok_or("invalid resource outcome override")?;
            require(
                if key == "scale" { n > 0.0 } else { n >= 0.0 },
                "invalid resource outcome override",
            )?;
        }
        require(
            outcome["expected_report"]["status"] == "failure"
                && outcome["expected_report"].get("artifact") == Some(&Value::Null),
            "a resource outcome expects a failed render",
        )?;
        require(
            outcome["expected_report"]
                .get("error")
                .is_none_or(Value::is_object),
            "invalid expected resource error",
        )?;
    }
    Ok(bundle)
}

fn resource_rows(case: &Value) -> Result<&[Value]> {
    match case.get("resource_outcomes") {
        None => Ok(&[]),
        Some(v) => v
            .as_array()
            .map(Vec::as_slice)
            .ok_or("invalid resource outcomes".into()),
    }
}

fn resource_outcomes(
    case: &Value,
    bundle: &Path,
    renderer: &Path,
    scratch: &Path,
) -> Result<usize> {
    let plan = decode(&read(bundle, "plan.json")?)?;
    let scene = scratch.join("outcome-scene.excalidraw");
    fs::write(&scene, read(bundle, "scene.excalidraw")?).map_err(|e| e.to_string())?;
    for (index, outcome) in resource_rows(case)?.iter().enumerate() {
        let id = text(outcome, "id")?;
        let mut variant = array(&plan, "variants")?
            .iter()
            .find(|v| v["id"] == outcome["variant"])
            .ok_or("resource variant missing")?
            .clone();
        for (key, value) in outcome["overrides"]
            .as_object()
            .ok_or("invalid resource overrides")?
        {
            variant[key] = value.clone();
        }
        let mut reports = Vec::new();
        for run in 0..2 {
            let artifact = scratch.join(format!(
                "outcome-{index}-{run}.{}",
                text(outcome, "format")?
            ));
            let report = scratch.join(format!("outcome-{index}-{run}.json"));
            let args = capture::render_args(
                &scene,
                &artifact,
                &report,
                &variant,
                text(outcome, "format")?,
            )?;
            let status =
                capture::render_process(renderer, &args, scratch, Duration::from_secs(900))?;
            require(!status.success(), &format!("{id}: render succeeded"))?;
            require(
                fs::symlink_metadata(&artifact)
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
                &format!("{id}: a failed render wrote an artifact"),
            )?;
            reports.push(read(
                scratch,
                report
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or("report path")?,
            )?);
        }
        require(
            reports[0] == reports[1],
            &format!("{id}: report differs between fresh processes"),
        )?;
        let observed = decode(&reports[0])?;
        for (key, value) in outcome["expected_report"]
            .as_object()
            .ok_or("invalid expected resource report")?
        {
            if key == "error" {
                for (key, value) in value.as_object().ok_or("invalid expected resource error")? {
                    require(
                        observed["error"].get(key) == Some(value),
                        &format!("{id}: error.{key} differs from expectation"),
                    )?;
                }
            } else {
                require(
                    observed.get(key) == Some(value),
                    &format!("{id}: {key} differs from expectation"),
                )?;
            }
        }
    }
    Ok(resource_rows(case)?.len())
}

fn verify_case(
    root: &Path,
    directory: &Path,
    renderer: Option<&Path>,
    registered_id: Option<&str>,
    viewers: bool,
) -> Result<Value> {
    let raw = read(directory, "case.json")?;
    let case = decode(&raw)?;
    let bundle = validate(root, directory, &case)?;
    if let Some(id) = registered_id {
        require(case["case_id"] == id, "registered case identity differs")?;
    }
    let mut summary = json!({"case_id": case["case_id"], "subject": format!("sha256:{}", subject(&case)?), "approved": !case["approval"].is_null()});
    if let Some(renderer) = renderer {
        let renderer = renderer.canonicalize().map_err(|e| e.to_string())?;
        let binary = descriptor(&fs::read(&renderer).map_err(|e| e.to_string())?);
        let scratch = if viewers {
            capture::Scratch::in_repository()?
        } else {
            capture::Scratch::new()?
        };
        let native = scratch.0.join("native");
        capture::capture(&bundle, &renderer, &native, 900.0)?;
        capture::verify_capture(&bundle, &native)?;
        for variant in strings(&case, "variants")? {
            for format in strings(&case, "formats")? {
                let artifact = read(&native, &format!("{variant}-{format}.{format}"))?;
                let report = decode(&read(&native, &format!("{variant}-{format}-report.json"))?)?;
                let expected =
                    decode(&read(directory, &format!("expected-report-{format}.json"))?)?;
                require(
                    json!({"schema_version": report["schema_version"], "diagnostics": report["diagnostics"]})
                        == expected,
                    &format!("{variant}-{format}: report differs from its expectation"),
                )?;
                semantics::verify(&artifact, format, &case["semantic_expectations"][variant])
                    .map_err(|e| format!("{variant}-{format}: {e}"))?;
            }
        }
        let outcomes = resource_outcomes(&case, &bundle, &renderer, &scratch.0)?;
        if outcomes > 0 {
            summary["resource_outcomes"] = json!(outcomes);
        }
        if viewers {
            let collection = scratch.0.join("viewers");
            evidence::collect(&bundle, &directory.join("reference"), &native, &collection)?;
            let judgment = judge::judge(&case, &collection)?;
            summary["within_controls"] = judgment["within_controls"].clone();
            summary["exceptions_applied"] = judgment["exceptions_applied"].clone();
            summary["viewers"] =
                json!("every registered comparison passes its budget, controls or exception");
        }
        require(
            descriptor(&fs::read(&renderer).map_err(|e| e.to_string())?) == binary,
            "renderer changed during case verification",
        )?;
        require(
            read(directory, "case.json")? == raw,
            "case changed during verification",
        )?;
        validate(root, directory, &case)?;
        summary["native"] = json!("repeatable; reports, dimensions and semantics match");
    }
    Ok(summary)
}

fn verify_registry(root: &Path, renderer: Option<&Path>, viewers: bool) -> Result<Vec<Value>> {
    let raw = read(root, REGISTRY)?;
    let registry = decode(&raw)?;
    require(
        registry["schema"] == "excalidraw-render/representative-case-registry/v1",
        "invalid registry",
    )?;
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut summaries = Vec::new();
    for entry in array(&registry, "cases")? {
        let id = text(entry, "case_id")?;
        let path = repository_path(root, text(entry, "path")?)?;
        require(
            ids.insert(id) && paths.insert(path.clone()),
            "duplicate registry case",
        )?;
        summaries.push(verify_case(root, &path, renderer, Some(id), viewers)?);
    }
    require(
        read(root, REGISTRY)? == raw,
        "registry changed during verification",
    )?;
    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_cases_use_code_review_without_requiring_approval_records() {
        fn copy(source: &Path, destination: &Path) {
            fs::create_dir_all(destination).unwrap();
            for entry in fs::read_dir(source).unwrap() {
                let e = entry.unwrap();
                if e.file_type().unwrap().is_dir() {
                    copy(&e.path(), &destination.join(e.file_name()));
                } else {
                    fs::copy(e.path(), destination.join(e.file_name())).unwrap();
                }
            }
        }
        let scratch = capture::Scratch::new().unwrap();
        let actual = crate::conformance::repo_root();
        let case_path = "tests/conformance/cases/representative/excali-local-slides";
        let bundle_path = "tests/conformance/intake/excali-local-slides-v1";
        copy(&actual.join(case_path), &scratch.0.join(case_path));
        copy(&actual.join(bundle_path), &scratch.0.join(bundle_path));
        let path = scratch.0.join(case_path).join("case.json");
        let mut case = decode(&fs::read(&path).unwrap()).unwrap();
        case["approval"] = Value::Null;
        fs::write(&path, encode(&case).unwrap()).unwrap();
        let registry = json!({"schema":"excalidraw-render/representative-case-registry/v1","cases":[{"case_id":case["case_id"],"path":case_path}]});
        fs::write(scratch.0.join(REGISTRY), encode(&registry).unwrap()).unwrap();
        let summaries = verify_registry(&scratch.0, None, false).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0]["approved"], false);
        fs::write(
            scratch.0.join(case_path).join("expected-report-svg.json"),
            b"{}",
        )
        .unwrap();
        assert!(verify_registry(&scratch.0, None, false)
            .unwrap_err()
            .contains("retained case files changed"));
    }
}
