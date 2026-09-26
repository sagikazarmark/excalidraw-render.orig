//! Create an unapproved case from explicit expectations and retained reference
//! bytes. Materialization never selects native observations as expectations.
use super::*;
use crate::cli::MaterializeArgs;

pub(super) fn run(args: MaterializeArgs) -> Result<()> {
    let spec = decode(&fs::read(&args.spec).map_err(|e| e.to_string())?)?;
    let summary = materialize(
        &args.output,
        &args.intake,
        &args.reference,
        &args.evidence_record,
        &spec,
    )?;
    println!(
        "{}",
        String::from_utf8(json::compact(&summary)?).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn materialize(
    output: &Path,
    bundle: &Path,
    reference: &Path,
    evidence_record: &Path,
    spec: &Value,
) -> Result<Value> {
    let object = spec.as_object().ok_or("invalid case spec")?;
    let required = [
        "case_id",
        "formats",
        "semantic_expectations",
        "expected_reports",
        "exceptions",
    ];
    require(
        required.iter().all(|k| object.contains_key(*k))
            && object
                .keys()
                .all(|k| required.contains(&k.as_str()) || k == "resource_outcomes"),
        "unknown or missing case spec field",
    )?;
    let (reference_record, plan, _) = evidence::validate_reference(bundle, reference)?;
    let manifest = super::super::verify(bundle)?;
    require(
        spec["case_id"] == manifest["case_id"],
        "case id differs from the intake bundle",
    )?;
    let formats = strings(spec, "formats")?;
    require(
        !formats.is_empty()
            && formats.iter().all(|f| FORMATS.contains(f))
            && formats.iter().collect::<BTreeSet<_>>().len() == formats.len(),
        "invalid registered formats",
    )?;
    let variants = array(&plan, "variants")?
        .iter()
        .map(|v| text(v, "id"))
        .collect::<Result<Vec<_>>>()?;
    let semantics = spec["semantic_expectations"]
        .as_object()
        .ok_or("missing semantic expectations")?;
    require(
        semantics
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            == variants.iter().copied().collect(),
        "semantic expectation variants differ",
    )?;
    for variant in &variants {
        validate_semantic_expectations(&semantics[*variant], &formats)
            .map_err(|e| format!("{variant}: {e}"))?;
    }
    let reports = spec["expected_reports"]
        .as_object()
        .ok_or("invalid expected reports")?;
    require(
        reports.keys().map(String::as_str).collect::<BTreeSet<_>>()
            == formats.iter().copied().collect(),
        "expected report formats differ",
    )?;
    let mut files = Files::new();
    files.insert(
        "reference/reference.json".into(),
        read(reference, "reference.json")?,
    );
    for row in array(&reference_record, "rows")? {
        let variant = text(row, "variant")?;
        for name in ["official.svg", "official.png", "official-png.rgba.zlib"] {
            files.insert(
                format!("reference/{variant}/{name}"),
                read(&reference.join(variant), name)?,
            );
        }
    }
    for format in &formats {
        let report = &reports[*format];
        fields(
            report,
            &["schema_version", "diagnostics"],
            "invalid expected report envelope",
        )?;
        require(
            report["schema_version"].as_u64() == Some(1) && report["diagnostics"].is_array(),
            "invalid expected report",
        )?;
        files.insert(
            format!("expected-report-{format}.json"),
            json::compact(report)?,
        );
    }
    // Evidence binding is provenance, not an approval or a claim of correctness.
    let case = json!({"schema":"excalidraw-render/representative-case/v1","case_id":spec["case_id"],"layer":"representative","compatibility_revision":1,
        "originating_issue":"https://github.com/sagikazarmark/excalidraw-render.orig/issues/84",
        "intake":evidence::bind(&bundle.join("manifest.json"))?,"evidence_record":evidence::bind(evidence_record)?,
        "variants":variants,"formats":formats,"comparisons":COMPARISONS.iter().filter(|c|formats.contains(&c.format)).map(|c|c.id).collect::<Vec<_>>(),
        "judgment_rule":JUDGMENT_RULE,"semantic_expectations":spec["semantic_expectations"],"exceptions":spec["exceptions"],
        "resource_outcomes":spec.get("resource_outcomes").cloned().unwrap_or(json!([])),"files":files.iter().map(|(n,b)|(n,descriptor(b))).collect::<BTreeMap<_,_>>(),"approval":null});
    let mut case = case;
    case["intake"]["path"] = json!(evidence::relative(bundle)?);
    // Validate in a private staging tree before publishing the destination.
    let staging = capture::Scratch::new()?;
    write_files(&staging.0, &files)?;
    validate(&crate::conformance::repo_root(), &staging.0, &case)?;
    fs::create_dir(output).map_err(|e| format!("cannot create case output: {e}"))?;
    let result = (|| {
        write_files(output, &files)?;
        evidence::write_new(&output.join("case.json"), &encode(&case)?)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(output);
    }
    result?;
    Ok(
        json!({"case_id":case["case_id"],"subject":format!("sha256:{}",subject(&case)?),"approval":null}),
    )
}

fn validate_semantic_expectations(expected: &Value, formats: &[&str]) -> Result<()> {
    require(expected.is_object(), "invalid semantic expectation")?;
    for (format, field) in [("svg", "svg_size"), ("pdf", "pdf_media_box")] {
        if formats.contains(&format) {
            require(
                expected[field].as_array().is_some_and(|dimensions| {
                    dimensions.len() == 2
                        && dimensions.iter().all(|v| {
                            v.as_str()
                                .and_then(|s| s.parse::<f64>().ok())
                                .is_some_and(|n| n.is_finite() && n > 0.0)
                        })
                }),
                &format!("invalid semantic expectation {field}"),
            )?;
        }
    }
    if formats.contains(&"svg") {
        for field in ["min_text_elements", "links"] {
            require(
                expected[field].as_u64().is_some(),
                &format!("invalid semantic expectation {field}"),
            )?;
        }
        for field in ["embedded_scene", "root_theme_filter"] {
            require(
                expected[field].is_boolean(),
                &format!("invalid semantic expectation {field}"),
            )?;
        }
    }
    if formats.contains(&"png") {
        require(
            expected["png_size"].as_array().is_some_and(|dimensions| {
                dimensions.len() == 2
                    && dimensions.iter().all(|v| {
                        v.as_u64()
                            .is_some_and(|n| n > 0 && u32::try_from(n).is_ok())
                    })
            }),
            "invalid semantic expectation png_size",
        )?;
    }
    if formats.contains(&"svg") || formats.contains(&"pdf") {
        let mut seen = BTreeSet::new();
        require(
            expected["notices"].as_array().is_some_and(|notices| {
                notices.iter().all(|v| {
                    v.as_str()
                        .is_some_and(|s| !s.trim().is_empty() && seen.insert(s))
                })
            }),
            "invalid semantic expectation notices",
        )?;
    }
    Ok(())
}

fn write_files(root: &Path, files: &Files) -> Result<()> {
    for (name, bytes) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().ok_or("file parent")?).map_err(|e| e.to_string())?;
        evidence::write_new(&path, bytes)?;
    }
    Ok(())
}
