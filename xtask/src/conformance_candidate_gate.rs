//! Corpus adapter. Evidence inspection is distinct from successful execution:
//! neither stored pass flags nor native hash pins can authorize a candidate.
use crate::conformance::{canonical_bytes, repo_root, sha256};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
};
type Result<T> = std::result::Result<T, String>;
pub(crate) fn candidate_root(directory: &str) -> Result<std::path::PathBuf> {
    if Path::new(directory)
        .components()
        .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err("candidate directory must be repository-relative".into());
    }
    let root = repo_root().canonicalize().map_err(|e| e.to_string())?;
    let path = root
        .join(directory)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(&root) {
        return Err("candidate directory escapes repository".into());
    }
    Ok(path)
}
pub(crate) fn descriptor_set(root: &Path, descriptors: &Value, required: &[String]) -> Result<()> {
    let rows = descriptors.as_array().ok_or("missing descriptor set")?;
    let mut names = BTreeSet::new();
    for d in rows {
        let path = d["path"].as_str().ok_or("missing descriptor path")?;
        if !names.insert(path.to_owned()) || !required.iter().any(|r| r == path) {
            return Err("unexpected or duplicate evidence descriptor".into());
        }
        check_descriptor(root, d)?;
    }
    if names != required.iter().cloned().collect() {
        return Err("incomplete evidence descriptor set".into());
    }
    Ok(())
}
pub(crate) fn read(root: &Path, path: &str) -> Result<Vec<u8>> {
    if Path::new(path)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("unsafe candidate path".into());
    }
    let p = root
        .join(path)
        .canonicalize()
        .map_err(|e| format!("candidate evidence {path}: {e}"))?;
    if !p.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
        return Err("candidate path escapes root".into());
    }
    fs::read(p).map_err(|e| e.to_string())
}
/// Source identities describe the run that produced frozen evidence. Approval
/// binds these records; replay checks today's renderer against the frozen bytes.
/// Do not require today's source files to be byte-identical to historical ones.
pub(crate) fn historical_sources(descriptors: &Value) -> Result<()> {
    let rows = descriptors
        .as_array()
        .ok_or("missing historical production/source bindings")?;
    if rows.is_empty() {
        return Err("empty historical production/source bindings".into());
    }
    let mut names = BTreeSet::new();
    for row in rows {
        let path = row["path"].as_str().ok_or("historical source path")?;
        let hash = row["sha256"].as_str().unwrap_or_default();
        if path.is_empty()
            || Path::new(path)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || !names.insert(path)
            || hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || row.get("byte_length").is_some_and(|n| n.as_u64().is_none())
            || row.as_object().is_none_or(|o| {
                o.keys()
                    .any(|k| !matches!(k.as_str(), "path" | "sha256" | "byte_length"))
            })
        {
            return Err("invalid historical production/source binding".into());
        }
    }
    Ok(())
}

pub(crate) fn require_historical_sources(descriptors: &Value, required: &[&str]) -> Result<()> {
    historical_sources(descriptors)?;
    let rows = descriptors.as_array().ok_or("historical sources")?;
    for path in required {
        if !rows.iter().any(|row| row["path"] == *path) {
            return Err(format!(
                "missing historical production/source binding: {path}"
            ));
        }
    }
    Ok(())
}
pub(crate) fn value(root: &Path, path: &str) -> Result<Value> {
    serde_json::from_slice(&read(root, path)?).map_err(|e| e.to_string())
}
pub(crate) fn descriptor(root: &Path, path: &str) -> Result<Value> {
    let b = read(root, path)?;
    Ok(json!({"path":path,"byte_length":b.len(),"sha256":sha256(&b)}))
}
/// Checks a record against today's file and returns its bytes. A historical
/// record (`historical_record`) is checked for shape only and yields none.
fn check_descriptor(root: &Path, d: &Value) -> Result<Option<Vec<u8>>> {
    let p = d["path"].as_str().ok_or("descriptor lacks path")?;
    if historical_record(p) {
        historical_binding(d)?;
        return Ok(None);
    }
    let b = read(root, p)?;
    if d["sha256"] != json!(sha256(&b))
        || d.get("byte_length").is_some_and(|n| n != &json!(b.len()))
    {
        return Err(format!("stale candidate evidence {p}"));
    }
    Ok(Some(b))
}
/// Whether `path` is producer code whose digest describes only the run that
/// captured evidence (ADR-0019): the reference oracle, less the fixtures it
/// reads, and the PDF.js harness. The gates never run these producers, so
/// editing one or updating its dependencies does not stale evidence.
/// `catalog verify` still reads the Excalidraw release from the oracle
/// lockfile, and the PDF.js adapter records the release it ran.
pub(crate) fn historical_producer(path: &str) -> bool {
    (path.starts_with("tools/reference-oracle/")
        && !path.starts_with("tools/reference-oracle/fixtures/"))
        || path.starts_with("tools/pdf-harness/pdfjs/")
        || path == "tools/pdf-harness/run-pdfjs.sh"
}
pub(crate) const COMPARATOR_PROFILE: &str =
    "tests/conformance/comparator-profiles/canonical-visual-v1.json";
const COMPARATOR_CLOSURE: &str =
    "tests/conformance/comparator-profiles/canonical-visual-v1.closure.json";
/// Whether a record in kept evidence names something that describes only how
/// the evidence was produced or measured (ADR-0019): producer code, a viewer
/// profile or adapter, or the comparator profile. The gate recomputes every
/// judgment from the stored rasters with the live comparator, and the live
/// viewers and comparator are qualified on their own, so these records are
/// checked for shape, not against today's files, which may have moved on or
/// gone.
pub(crate) fn historical_record(path: &str) -> bool {
    historical_producer(path)
        || path == COMPARATOR_PROFILE
        || path == COMPARATOR_CLOSURE
        || path.starts_with("tests/conformance/viewer-profiles/")
        || path.starts_with("tools/pdf-harness/")
        || path.starts_with("tools/svg-viewer-rust/")
}
/// Checks a historical record's shape: a safe path and a SHA-256 digest.
pub(crate) fn historical_binding(d: &Value) -> Result<()> {
    let mut binding = json!({"path": d["path"], "sha256": d["sha256"]});
    if let Some(n) = d.get("byte_length") {
        binding["byte_length"] = n.clone();
    }
    historical_sources(&json!([binding]))
}
fn repo_chain(v: &Value, visited: &mut BTreeSet<String>) -> Result<()> {
    repo_chain_at(v, visited, &repo_root())
}
fn repo_chain_at(v: &Value, visited: &mut BTreeSet<String>, context: &Path) -> Result<()> {
    if let Some(o) = v.as_object() {
        if let (Some(path), Some(_)) = (o.get("path").and_then(Value::as_str), o.get("sha256")) {
            if historical_record(path) {
                historical_binding(v)?;
                return Ok(());
            }
            // Producer records use repository paths; vendored vector manifests
            // use paths relative to their own directory. Resolve both within
            // the repository, and reject ambiguous references.
            let root = repo_root();
            let global = root.join(path);
            let local = context.join(path);
            if global.exists()
                && local.exists()
                && global.canonicalize().map_err(|e| e.to_string())?
                    != local.canonicalize().map_err(|e| e.to_string())?
            {
                return Err(format!("ambiguous producer descriptor {path}"));
            }
            let resolved = if global.exists() { global } else { local };
            let relative = resolved
                .strip_prefix(&root)
                .map_err(|_| "producer descriptor escapes repository")?
                .to_str()
                .ok_or("non-UTF8 producer path")?;
            let mut resolved_descriptor = v.clone();
            resolved_descriptor["path"] = json!(relative);
            let bytes = check_descriptor(&root, &resolved_descriptor)?
                .ok_or("descriptor resolves to a historical record")?;
            // Reference profiles also inventory paths *inside the canonical
            // OCI image* (including its system fonts). Their immutable bytes
            // are bound here; the collector's profile:verify validates those
            // container paths. Never reinterpret them as host/repository files.
            if path.ends_with(".json")
                && !relative.starts_with("tests/oracle/reference-profiles/")
                && visited.insert(relative.into())
            {
                let nested: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                repo_chain_at(
                    &nested,
                    visited,
                    resolved.parent().ok_or("descriptor parent")?,
                )?;
            }
        }
        for (name, child) in o {
            if name == "sources"
                && o.get("schema").and_then(Value::as_str)
                    == Some("excalidraw-render/candidate-svg-text-audit-profile/v1")
            {
                historical_sources(child)?;
                continue;
            }
            repo_chain_at(child, visited, context)?;
        }
    } else if let Some(a) = v.as_array() {
        for child in a {
            repo_chain_at(child, visited, context)?;
        }
    }
    Ok(())
}
fn files_under(root: &Path, at: &Path, files: &mut Vec<String>) -> Result<()> {
    for e in fs::read_dir(at).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            files_under(root, &e.path(), files)?;
        } else {
            let path = e
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("non-UTF8 evidence path")?
                .replace('\\', "/");
            if !matches!(
                path.as_str(),
                "manifest.json" | "approval-subject.json" | "decision-subject.json" | "README.md"
            ) && !path.ends_with("/README.md")
            {
                files.push(path);
            }
        }
    }
    Ok(())
}
fn source_files(at: &Path, files: &mut Vec<String>) -> Result<()> {
    for e in fs::read_dir(at).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            source_files(&e.path(), files)?;
        } else if e.path().extension().is_some_and(|x| x == "rs") {
            files.push(
                e.path()
                    .strip_prefix(repo_root())
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("source path")?
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}
fn viewer_sources() -> Vec<String> {
    [
        "tools/reference-oracle/scripts/stage4-candidate-views.mjs",
        "tools/reference-oracle/scripts/stage4-viewer-assets.mjs",
        "xtask/src/conformance_candidate.rs",
        "Cargo.toml",
        "Cargo.lock",
        "xtask/Cargo.toml",
        "tools/reference-oracle/package-lock.json",
        "tools/reference-oracle/scripts/svg-viewer.mjs",
        "tools/reference-oracle/scripts/svg-viewer-text-audit.mjs",
        "tools/reference-oracle/run-svg-text-audit.sh",
        "tools/svg-viewer-rust/src/main.rs",
        "tools/svg-viewer-rust/Cargo.toml",
        "tools/svg-viewer-rust/Cargo.lock",
        "tools/pdf-harness/run-pdfium.sh",
        "tools/pdf-harness/pdfium/render.py",
        "tools/pdf-harness/pdfium.lock.json",
        "tools/pdf-harness/run-pdfjs.sh",
        "tools/pdf-harness/pdfjs/render.mjs",
        "tools/pdf-harness/pdfjs/package-lock.json",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
fn font_audit_sources(directory: &str) -> Vec<String> {
    let mut paths = [
        "tools/reference-oracle/scripts/svg-viewer.mjs",
        "tools/reference-oracle/scripts/svg-viewer-text-audit.mjs",
        "tools/reference-oracle/run-svg-text-audit.sh",
        "tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs",
        "tests/conformance/viewer-profiles/canonical-svg-chromium-v1.json",
        "tools/reference-oracle/package-lock.json",
    ]
    .map(str::to_owned)
    .to_vec();
    paths.extend([
        format!("{directory}/native.svg"),
        format!("{directory}/native-assets.json"),
    ]);
    paths
}
fn producer_sets(directory: &str, collection: &Value, profile: &Value) -> Result<()> {
    let mut viewer_required = viewer_sources();
    let rust_pdfium = collection["captures"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["id"] == "native-pdfium")
        .flat_map(|c| c["runs"].as_array().into_iter().flatten())
        .any(|r| r.get("adapter").is_some());
    if rust_pdfium {
        viewer_required.retain(|p| p != "tools/pdf-harness/pdfium/render.py");
        viewer_required.retain(|p| p != "tools/pdf-harness/pdfium.lock.json");
        viewer_required.extend(crate::pdf_viewers::rust_pdfium_sources()?);
        // Earlier Rust captures also bound the retired native lock, and the
        // earliest the historical wheel lock beside it; retain each complete
        // inventory, so a wheel lock alone stays incomplete.
        let recorded = |path: &str| {
            collection["sources"]
                .as_array()
                .is_some_and(|sources| sources.iter().any(|s| s["path"] == path))
        };
        let native_lock = crate::pdf_viewers::RETIRED_NATIVE_LOCK;
        if recorded(native_lock) {
            viewer_required.push(native_lock.to_owned());
            if recorded("tools/pdf-harness/pdfium.lock.json") {
                viewer_required.push("tools/pdf-harness/pdfium.lock.json".to_owned());
            }
        }
        for capture in collection["captures"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["id"] == "native-pdfium")
        {
            for run in capture["runs"].as_array().into_iter().flatten() {
                if run["adapter"] != "pdfium-render/0.9.4"
                    || run["pdfium"] != "152.0.7947.0"
                    || !crate::pdf_viewers::is_library_sha256(&run["library_sha256"])
                    || run.get("pypdfium2").is_some()
                {
                    return Err("invalid or mixed PDFium producer identity".into());
                }
            }
        }
    }
    for (sources, required) in [
        (&collection["sources"], viewer_required),
        (&profile["sources"], font_audit_sources(directory)),
    ] {
        historical_sources(sources)?;
        let paths: BTreeSet<_> = sources
            .as_array()
            .ok_or("historical sources")?
            .iter()
            .filter_map(|s| s["path"].as_str())
            .collect();
        if paths != required.iter().map(String::as_str).collect() {
            return Err("incomplete historical producer set".into());
        }
        // Captured inputs remain live byte checks, unlike producer code.
        for source in sources.as_array().ok_or("historical sources")? {
            if source["path"]
                .as_str()
                .is_some_and(|p| p.starts_with(&format!("{directory}/")))
            {
                check_descriptor(&repo_root(), source)?;
            }
        }
    }
    Ok(())
}
fn collection(directory: &str) -> Result<Value> {
    crate::svg_viewers::verify().map_err(|e| format!("SVG viewer qualification: {e}"))?;
    crate::pdf_viewers::verify().map_err(|e| format!("PDF viewer qualification: {e}"))?;
    let root = repo_root();
    let data = value(
        &root,
        &format!("{directory}/viewer-evidence/collection.json"),
    )?;
    if data["schema"] != "excalidraw-render/candidate-viewer-collection/v1"
        || data["candidate_directory"] != directory
    {
        return Err("collection belongs to a different candidate or schema".into());
    }
    descriptor_set(
        &root,
        &data["profiles"],
        &[
            format!("{directory}/chromium-text-profile.json"),
            "tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json".into(),
            "tests/conformance/viewer-profiles/canonical-pdf-pdfjs-v1.json".into(),
            "tests/conformance/viewer-profiles/canonical-svg-pure-rust-v1.json".into(),
            "tests/conformance/comparator-profiles/canonical-visual-v1.json".into(),
        ],
    )?;
    descriptor_set(
        &root,
        &data["assets"],
        &[
            format!("{directory}/official-assets.json"),
            format!("{directory}/native-assets.json"),
        ],
    )?;
    let profile = value(&root, &format!("{directory}/chromium-text-profile.json"))?;
    producer_sets(directory, &data, &profile)?;
    crate::conformance_candidate::verify_assets(directory)?;
    repo_chain(&data["profiles"], &mut BTreeSet::new())?;
    repo_chain(&data["assets"], &mut BTreeSet::new())?;
    let expected = [
        "official-chromium",
        "native-chromium",
        "native-pdfium",
        "native-pdfjs",
        "official-rust",
        "native-rust",
    ];
    let captures = data["captures"]
        .as_array()
        .ok_or("missing viewer captures")?;
    let mut seen = BTreeSet::new();
    for c in captures {
        let id = c["id"].as_str().ok_or("missing capture ID")?;
        if !expected.contains(&id) || !seen.insert(id) {
            return Err("unknown or duplicate capture".into());
        }
        let (input, profile) = match id {
            "official-chromium" => (
                "official.svg",
                format!("{directory}/chromium-text-profile.json"),
            ),
            "native-chromium" => (
                "native.svg",
                format!("{directory}/chromium-text-profile.json"),
            ),
            "native-pdfium" => (
                "native.pdf",
                "tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json".into(),
            ),
            "native-pdfjs" => (
                "native.pdf",
                "tests/conformance/viewer-profiles/canonical-pdf-pdfjs-v1.json".into(),
            ),
            "official-rust" => (
                "official-rust.svg",
                "tests/conformance/viewer-profiles/canonical-svg-pure-rust-v1.json".into(),
            ),
            "native-rust" => (
                "native-rust.svg",
                "tests/conformance/viewer-profiles/canonical-svg-pure-rust-v1.json".into(),
            ),
            _ => return Err("unknown capture".into()),
        };
        if c["input"]["path"] != format!("{directory}/{input}")
            || c["viewer_profile"] != profile
            || c["raster"]["path"] != format!("{directory}/viewer-evidence/{id}.rgba")
        {
            return Err("capture input/profile/output binding mismatch".into());
        }
        check_descriptor(&root, &c["input"])?;
        let bytes =
            check_descriptor(&root, &c["raster"])?.ok_or("raster is a historical record")?;
        if bytes.len() != 200 * 120 * 4 {
            return Err("wrong viewer dimensions".into());
        }
        let runs = c["runs"].as_array().ok_or("missing repeat runs")?;
        if runs.len() != 2 {
            return Err("two fresh runs required".into());
        }
        for r in runs {
            if r["width"] != 200 || r["height"] != 120 {
                return Err("viewer dimensions drifted".into());
            }
            if id.ends_with("chromium") {
                if r["browser_version"] != "149.0.7827.55"
                    || r["lifecycle_updates"] != 2
                    || r["animation_offset_microseconds"] != 0
                    || r["rgba_sha256"] != c["raster"]["sha256"]
                {
                    return Err("invalid canonical SVG capture".into());
                }
                let fonts = r["platform_fonts"]
                    .as_array()
                    .ok_or("missing CDP font audit")?;
                if fonts.is_empty() || fonts.iter().any(|f| f["isCustomFont"] != true) {
                    return Err("font audit is empty or contains ambient fonts".into());
                }
            } else if !id.ends_with("rust") && r["sha256"] != c["raster"]["sha256"] {
                return Err("PDF viewer output mismatch".into());
            }
        }
        if id.ends_with("chromium")
            && runs[0]["container_namespace"] == runs[1]["container_namespace"]
        {
            return Err("SVG repeat reused a process namespace".into());
        }
        let original = c["raster"]["path"].as_str().ok_or("raster path")?;
        let repeat = original
            .strip_suffix(".rgba")
            .ok_or("raster extension")?
            .to_owned()
            + ".repeat.rgba";
        if read(&root, &repeat)? != bytes {
            return Err("viewer repeat changed raster bytes".into());
        }
    }
    if seen.len() != expected.len() {
        return Err("incomplete SVG/PDF viewer set".into());
    }
    let native = captures
        .iter()
        .find(|c| c["id"] == "native-chromium")
        .ok_or("native Chromium capture")?;
    if profile["reference_profile_sha256"]
        != sha256(&read(
            &root,
            "tests/oracle/reference-profiles/revision-1.json",
        )?)
        || profile["qualification"]["positive_raster_sha256"] != native["raster"]["sha256"]
    {
        return Err("text audit is detached from the reference/candidate".into());
    }
    if profile["schema"] != "excalidraw-render/candidate-svg-text-audit-profile/v1"
        || profile["id"] != "canonical-svg.chromium.text-audit.v1"
        || profile["base_profile"] != "canonical-svg.chromium.v1"
        || profile["qualification"]["text_present"] != true
        || profile["qualification"]["all_used_fonts_custom"] != true
        || profile["qualification"]["negative_controls"]
            != json!([
            {"id":"ambient-font","rejected":true,"reason":"ambient platform font used"},
            {"id":"invalid-format","rejected":true,"reason":"@font-face src must be one WOFF2"}])
    {
        return Err("unqualified text-audit adapter".into());
    }
    Ok(data)
}
fn actual(directory: &str) -> Result<(Value, Value)> {
    collection(directory)?;
    let root = repo_root().join(directory);
    authoring(&root)?;
    let p = value(&root, "provenance.json")?;
    if p["sources"].as_array().is_none_or(Vec::is_empty) {
        return Err("missing reference producer chain".into());
    }
    // This adapter qualifies one authored revision. A crop change is a new
    // qualification, never a way to dilute or omit a failing region.
    if value(&root, "judgment-plan.json")?
        != json!({"whole_class":"geometry-image","crops":[{"id":"typography","x":10,"y":12,"width":124,"height":41,"class":"typography"},{"id":"raster","x":178,"y":38,"width":22,"height":44,"class":"geometry-image"},{"id":"nested-svg","x":38,"y":53,"width":36,"height":28,"class":"geometry-image"}],"exceptions":[]})
    {
        return Err("candidate crop qualification changed".into());
    }
    historical_sources(&p["sources"])?;
    if p["fresh_process_runs"] != 2 || p["browser"] != "149.0.7827.55" || p["network"] != "none" {
        return Err("invalid reference producer protocol".into());
    }
    for f in ["svg", "png", "pdf"] {
        if read(&root, &format!("expected-report-{f}.json"))?
            != br#"{"schema_version":1,"diagnostics":[]}"#
        {
            return Err("report expectations must remain independently authored".into());
        }
    }
    let judgments = crate::conformance_candidate_exceptions::evaluate(directory)?;
    let semantics = crate::conformance_candidate_semantics::verify(&root)?;
    Ok((judgments, semantics))
}
pub fn materialize(directory: &str) -> Result<Value> {
    let root = candidate_root(directory)?;
    for name in [
        "manifest.json",
        "judgments.json",
        "semantics.json",
        "decision-subject.json",
        "approval-subject.json",
    ] {
        if root.join(name).exists() {
            return Err(format!(
                "refusing to overwrite candidate evidence {name}; author a new revision"
            ));
        }
    }
    let (judgments, semantics) = actual(directory)?;
    crate::conformance_candidate_native::replay(directory)?;
    fs::write(
        root.join("judgments.json"),
        [canonical_bytes(&judgments)?, b"\n".to_vec()].concat(),
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        root.join("semantics.json"),
        [canonical_bytes(&semantics)?, b"\n".to_vec()].concat(),
    )
    .map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    files_under(&root, &root, &mut files)?;
    files.sort();
    let files = files
        .iter()
        .map(|p| descriptor(&root, p))
        .collect::<Result<Vec<_>>>()?;
    let sources = adapter_sources()?;
    let collection = value(&root, "viewer-evidence/collection.json")?;
    let manifest = json!({"schema":"excalidraw-render/representative-candidate/v3","case_id":case_id(directory)?,"layer":"representative","compatibility_revision":1,"approval":null,"originating_issue":"https://github.com/sagikazarmark/excalidraw-render.orig/issues/85","coverage":["interaction:frames-text-assets"],"files":files,"producer_sources":sources,"profiles":collection["profiles"],"judgment_plan":value(&root,"judgment-plan.json")?,"semantic_expectations":value(&root,"semantic-expectations.json")?,"exception_proposal":judgments["exception_proposal"],"request_args":crate::conformance_candidate_native::OPTIONS,"native_capture":descriptor(&root,"native-capture.json")?,"license":{"expression":"MIT OR Apache-2.0","copyright":"Copyright 2026 excalidraw-render contributors"}});
    fs::write(
        root.join("manifest.json"),
        [canonical_bytes(&manifest)?, b"\n".to_vec()].concat(),
    )
    .map_err(|e| e.to_string())?;
    let passed = judgments["rows"]
        .as_array()
        .ok_or("judgment rows")?
        .iter()
        .all(|r| r["passed"] == true);
    Ok(
        json!({"status":if passed{"verified"}else{"pending-judgment"},"gates_passed":passed,"judgments":judgments}),
    )
}
fn bound_manifest(directory: &str) -> Result<Value> {
    let root = candidate_root(directory)?;
    let manifest = value(&root, "manifest.json")?;
    if manifest["schema"] != "excalidraw-render/representative-candidate/v3"
        || manifest["coverage"] != json!(["interaction:frames-text-assets"])
        || manifest["case_id"] != case_id(directory)?
        || manifest["layer"] != "representative"
        || manifest["compatibility_revision"] != 1
    {
        return Err(
            "retired diagnostic candidate requires the complete v3 native replay adapter".into(),
        );
    }
    let mut names = Vec::new();
    crate::conformance_candidate_native::validate_build_identity(
        &value(&root, "native-capture.json")?["build"],
    )?;
    files_under(&root, &root, &mut names)?;
    descriptor_set(&root, &manifest["files"], &names)?;
    require_historical_sources(
        &manifest["producer_sources"],
        &[
            "Cargo.toml",
            "Cargo.lock",
            "xtask/Cargo.toml",
            "docs/renderer-spec-v1.md",
            "xtask/src/main.rs",
            "xtask/src/conformance_candidate_gate.rs",
            "xtask/src/conformance_candidate_native.rs",
        ],
    )?;
    if manifest["request_args"] != json!(crate::conformance_candidate_native::OPTIONS)
        || manifest["native_capture"] != descriptor(&root, "native-capture.json")?
    {
        return Err("native request/capture detached from manifest".into());
    }
    crate::conformance_candidate_native::historical_producers(
        &value(&root, "native-capture.json")?["producer_sources"],
    )?;
    if manifest["profiles"] != value(&root, "viewer-evidence/collection.json")?["profiles"] {
        return Err("manifest profiles are detached from captures".into());
    }
    if manifest["exception_proposal"] != crate::conformance_candidate_exceptions::proposed(&root)? {
        return Err("exception proposal is detached from the case manifest".into());
    }
    repo_chain(&manifest["profiles"], &mut BTreeSet::new())?;
    Ok(manifest)
}
fn verify_and_replay(directory: &str) -> Result<Value> {
    let manifest = bound_manifest(directory)?;
    let root = candidate_root(directory)?;
    let (judgments, semantics) = actual(directory)?;
    if value(&root, "judgments.json")? != judgments || value(&root, "semantics.json")? != semantics
    {
        return Err("stored candidate judgments/semantics do not match checked results".into());
    }
    if manifest["judgment_plan"] != value(&root, "judgment-plan.json")?
        || manifest["semantic_expectations"] != value(&root, "semantic-expectations.json")?
    {
        return Err("unbound candidate expectations".into());
    }
    let failed: Vec<_> = judgments["rows"]
        .as_array()
        .ok_or("rows")?
        .iter()
        .filter(|r| r["passed"] != true)
        .map(|r| r["id"].as_str().unwrap_or("unknown"))
        .collect();
    if !failed.is_empty() {
        return Err(format!(
            "candidate effective gates failed: {}",
            failed.join(", ")
        ));
    }
    let native = crate::conformance_candidate_native::replay(directory)?;
    let judgments_hash = sha256(&canonical_bytes(&judgments)?);
    let semantics_hash = sha256(&canonical_bytes(&semantics)?);
    let rows=native["rows"].as_array().ok_or("native replay rows")?.iter().map(|row|json!({"execution_key":format!("{}:default:{}",manifest["case_id"].as_str().unwrap_or_default(),row["format"].as_str().unwrap_or_default()),"native":row,"judgments_sha256":judgments_hash,"semantics_sha256":semantics_hash})).collect::<Vec<_>>();
    Ok(
        json!({"schema":"excalidraw-render/representative-executions/v1","case_id":manifest["case_id"],"executions":rows}),
    )
}
/// Diagnostic inspection does not register successful corpus executions.
/// Invalid or stale evidence is an error; valid but out-of-budget observations
/// are returned as pending judgments, with no eligible execution keys.
pub fn inspect(directory: &str) -> Result<Value> {
    let root = candidate_root(directory)?;
    if root.join("manifest.json").exists() {
        bound_manifest(directory)?;
    }
    let (judgments, semantics) = actual(directory)?;
    let native = crate::conformance_candidate_native::replay(directory)?;
    inspection(judgments, semantics, native)
}
fn inspection(judgments: Value, semantics: Value, native: Value) -> Result<Value> {
    let rows = judgments["rows"]
        .as_array()
        .ok_or("inspection requires comparison rows")?;
    let expected = [
        "svg-chromium",
        "svg-rust",
        "pdf-pdfium",
        "pdf-pdfjs",
        "png-canvas",
    ];
    if rows.len() != expected.len()
        || rows
            .iter()
            .zip(expected)
            .any(|(r, id)| r["id"] != id || r["passed"].as_bool().is_none())
    {
        return Err("inspection requires the complete comparison set".into());
    }
    let failed: Vec<_> = rows
        .iter()
        .filter(|r| r["passed"] != true)
        .map(|r| r["id"].clone())
        .collect();
    Ok(
        json!({"schema":"excalidraw-render/representative-inspection/v1","status":if failed.is_empty(){"checks_passed"}else{"pending_judgment"},"inspection_only":true,"eligible_executions":[],"failed_comparisons":failed,"native_replay":native,"judgments":judgments,"semantics":semantics}),
    )
}
pub fn verify_registry() -> Result<()> {
    execute_registry().map(|_| ())
}
pub fn execute(directory: &str) -> Result<Value> {
    verify_and_replay(directory)
}
pub fn execute_registry() -> Result<Value> {
    let registry = value(&repo_root(), "tests/conformance/candidates/registry.json")?;
    if registry["schema"] != "excalidraw-render/representative-corpus-adapter/v1" {
        return Err("invalid candidate corpus registry".into());
    }
    let rows = registry["cases"]
        .as_array()
        .ok_or("missing candidate cases")?;
    if rows.len() != 1 {
        return Err("candidate corpus requires its one authored revision".into());
    }
    let mut results = Vec::new();
    for row in rows {
        let directory = row["directory"].as_str().ok_or("missing directory")?;
        let id = case_id(directory)?;
        if row["id"] != id
            || row["coverage"] != json!(["interaction:frames-text-assets"])
            || row["execution_keys"]
                != json!(crate::conformance_candidate_native::FORMATS
                    .map(|format| format!("{id}:default:{format}")))
        {
            return Err("candidate registry/execution identities changed".into());
        }
        if row["formats"] != json!(["svg", "png", "pdf"]) {
            return Err("candidate executions may not omit formats".into());
        }
        let manifest = value(&candidate_root(directory)?, "manifest.json")?;
        let registered = row.get("exceptions").cloned().unwrap_or(json!([]));
        let declared = manifest["exception_proposal"]
            .get("exceptions")
            .cloned()
            .unwrap_or(json!([]));
        if registered != declared {
            return Err("exception scopes do not match the registered candidate".into());
        }
        results.push(execute(directory)?);
    }
    Ok(json!({"schema":"excalidraw-render/representative-corpus-results/v1","cases":results}))
}

#[cfg(test)]
pub(crate) fn approval_bundle(manifest: &Value) -> Result<Value> {
    let mut bare = manifest.clone();
    bare.as_object_mut()
        .ok_or("manifest object")?
        .remove("approval");
    Ok(
        json!({"schema":"excalidraw-render/conformance-approval-subject/v1","manifest":bare,"files":manifest["files"]}),
    )
}
pub(crate) fn case_id(directory: &str) -> Result<String> {
    let name = Path::new(directory)
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or("candidate name")?;
    let revision = name
        .strip_prefix("stage4-frames-text-assets-v")
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or("retired diagnostic candidate requires the complete v3 native replay adapter")?;
    if revision < 4 {
        return Err("retired candidate revision".into());
    }
    Ok(format!(
        "representative.stage4-frames-text-assets.v{revision}"
    ))
}
const AUTHORING: [&str; 10] = [
    "scene.json",
    "raster.png",
    "nested.svg",
    "official.svg",
    "official.png",
    "official-png.rgba",
    "judgment-plan.json",
    "semantic-expectations.json",
    "expected-report-svg.json",
    "expected-report-png.json",
];
fn authoring(root: &Path) -> Result<()> {
    let original = repo_root().join("tests/conformance/candidates/stage4-frames-text-assets-v4");
    for (path, hash) in [
        (
            "scene.json",
            "f9b7d01d634a222effe360a172ceec9183de72390a43d5fd73d9a39872094dc4",
        ),
        (
            "judgment-plan.json",
            "73b8e2333b62ea403b05542b4ab8d363638cc6665928b6814e2fc0457d82b97f",
        ),
        (
            "semantic-expectations.json",
            "1ad1f8c9298457bf1a80e99effd57d30ad79d4c5ca05a9929ff1542d0b894a64",
        ),
    ] {
        if sha256(&read(&original, path)?) != hash {
            return Err(format!("frozen v4 authoring changed: {path}"));
        }
    }
    for file in AUTHORING.into_iter().chain(["expected-report-pdf.json"]) {
        if read(root, file)? != read(&original, file)? {
            return Err(format!(
                "candidate changed frozen v4 authoring/reference {file}"
            ));
        }
    }
    Ok(())
}
pub(crate) fn adapter_sources() -> Result<Value> {
    let mut paths = vec![
        "xtask/Cargo.toml".into(),
        "Cargo.lock".into(),
        "Cargo.toml".into(),
        "docs/renderer-spec-v1.md".into(),
    ];
    source_files(&repo_root().join("xtask/src"), &mut paths)?;
    paths.sort();
    paths
        .iter()
        .map(|p| descriptor(&repo_root(), p))
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}
pub fn revise(directory: &str) -> Result<()> {
    case_id(directory)?;
    let path = Path::new(directory);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("unsafe candidate revision directory".into());
    }
    let parent = path.parent().ok_or("candidate parent")?;
    candidate_root(parent.to_str().ok_or("parent encoding")?)?;
    let root = repo_root().join(path);
    fs::create_dir(&root).map_err(|e| e.to_string())?;
    let original = repo_root().join("tests/conformance/candidates/stage4-frames-text-assets-v4");
    for file in AUTHORING
        .into_iter()
        .chain(["expected-report-pdf.json", "provenance.json"])
    {
        crate::conformance_candidate_native::write_new(&root.join(file), &read(&original, file)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provenance_requires_every_viewer_and_font_audit_producer() {
        let dir = "tests/conformance/candidates/stage4-frames-text-assets-v4";
        let captured = value(
            &repo_root(),
            &format!("{dir}/viewer-evidence/collection.json"),
        )
        .unwrap();
        let descriptors = |paths: Vec<String>| {
            Value::Array(
                paths
                    .iter()
                    .map(|p| {
                        // Retired producers keep their captured record.
                        if !repo_root().join(p).exists() {
                            captured["sources"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .find(|s| s["path"] == *p)
                                .unwrap()
                                .clone()
                        } else {
                            descriptor(&repo_root(), p).unwrap()
                        }
                    })
                    .collect(),
            )
        };
        let collection = json!({"sources":descriptors(viewer_sources())});
        let profile = json!({"sources":descriptors(font_audit_sources(dir))});
        producer_sets(dir, &collection, &profile).unwrap();
        let mut historical = collection.clone();
        historical["sources"][0]["sha256"] = json!("a".repeat(64));
        producer_sets(dir, &historical, &profile).unwrap();
        // A captured input is never treated as historical producer code.
        let mut detached = profile.clone();
        for source in detached["sources"].as_array_mut().unwrap() {
            if source["path"] == format!("{dir}/native.svg") {
                source["sha256"] = json!("a".repeat(64));
            }
        }
        assert!(producer_sets(dir, &collection, &detached).is_err());
        for font in [false, true] {
            let original = if font { &profile } else { &collection };
            for omitted in 0..original["sources"].as_array().unwrap().len() {
                let mut bad = original.clone();
                bad["sources"].as_array_mut().unwrap().remove(omitted);
                let result = if font {
                    producer_sets(dir, &collection, &bad)
                } else {
                    producer_sets(dir, &bad, &profile)
                };
                assert!(result.is_err(), "omitted producer {omitted}, font={font}");
            }
            for mutation in 0..3 {
                let mut bad = original.clone();
                match mutation {
                    0 => {
                        let duplicate = bad["sources"][0].clone();
                        bad["sources"].as_array_mut().unwrap().push(duplicate);
                    }
                    1 => bad["sources"][0]["sha256"] = json!("stale"),
                    _ => bad["sources"][0] = descriptor(&repo_root(), "AGENTS.md").unwrap(),
                }
                let result = if font {
                    producer_sets(dir, &collection, &bad)
                } else {
                    producer_sets(dir, &bad, &profile)
                };
                assert!(result.is_err(), "producer mutation {mutation}, font={font}");
            }
        }
    }
    #[test]
    fn inspection_reports_measurements_without_registering_executions() {
        let judgments = value(
            &repo_root(),
            "tests/conformance/candidates/stage4-frames-text-assets-v4/judgments.json",
        )
        .unwrap();
        let result = inspection(
            judgments.clone(),
            json!({"checked":true}),
            json!({"rows":[]}),
        )
        .unwrap();
        assert_eq!(result["status"], "pending_judgment");
        assert_eq!(
            result["failed_comparisons"],
            json!(["pdf-pdfium", "pdf-pdfjs", "png-canvas"])
        );
        assert_eq!(result["eligible_executions"], json!([]));
        assert!(result.get("approval_subject_sha256").is_none());
        let mut passing = judgments.clone();
        for row in passing["rows"].as_array_mut().unwrap() {
            row["passed"] = json!(true);
        }
        let ready = inspection(passing, json!({}), json!({})).unwrap();
        assert_eq!(ready["status"], "checks_passed");
        assert!(ready.get("approval_status").is_none());
        assert_eq!(ready["eligible_executions"], json!([]));
        assert!(ready.get("approval_subject_sha256").is_none());
        let mut incomplete = judgments;
        incomplete["rows"].as_array_mut().unwrap().pop();
        assert!(inspection(incomplete, json!({}), json!({})).is_err());
    }
    #[test]
    fn withdrawn_v1_candidate_has_no_execution_adapter() {
        {
            assert!(
                execute("tests/conformance/candidates/stage4-frames-text-assets")
                    .unwrap_err()
                    .contains("complete v3 native replay adapter")
            );
        }
    }
    #[test]
    fn existing_candidate_materialization_is_immutable() {
        let dir = "tests/conformance/candidates/stage4-frames-text-assets";
        let root = candidate_root(dir).unwrap();
        let before = read(&root, "manifest.json").unwrap();
        assert!(materialize(dir)
            .unwrap_err()
            .contains("refusing to overwrite"));
        assert_eq!(read(&root, "manifest.json").unwrap(), before);
    }
    #[test]
    fn historical_records_are_checked_for_shape_not_against_live_files() {
        let root = repo_root();
        let profile = value(
            &root,
            "tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json",
        )
        .unwrap();
        repo_chain(&profile, &mut BTreeSet::new()).unwrap();
        let mut malformed = profile;
        let source = malformed["implementation"]["sources"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|s| s["path"] == "tools/pdf-harness/run-pdfium.sh")
            .unwrap();
        source["sha256"] = json!("invalid");
        assert!(repo_chain(&malformed, &mut BTreeSet::new()).is_err());
        // A measurement record need not match, or name, a live file.
        for path in [
            COMPARATOR_PROFILE,
            "tests/conformance/viewer-profiles/canonical-svg-pure-rust-v1.json",
            "tools/pdf-harness/pdfium.lock.json",
        ] {
            let record = json!({"path": path, "byte_length": 1, "sha256": "0".repeat(64)});
            assert_eq!(check_descriptor(&root, &record).unwrap(), None);
            for (key, bad) in [("sha256", json!("00")), ("byte_length", json!("1"))] {
                let mut malformed = record.clone();
                malformed[key] = bad;
                assert!(check_descriptor(&root, &malformed).is_err());
            }
        }
        // Evidence itself stays live.
        let stale = json!({"path": "Cargo.toml", "sha256": "0".repeat(64)});
        assert!(check_descriptor(&root, &stale).is_err());
    }

    #[test]
    fn producer_sets_reject_missing_duplicate_stale_and_detached_entries() {
        let root = repo_root();
        let path = "tests/oracle/reference-profiles/revision-1.json";
        let d = descriptor(&root, path).unwrap();
        let required = vec![path.into()];
        descriptor_set(&root, &json!([d]), &required).unwrap();
        for bad in [
            json!([]),
            json!([d, d]),
            json!([{"path":path,"sha256":"00"}]),
            json!([descriptor(&root, "Cargo.toml").unwrap()]),
        ] {
            assert!(descriptor_set(&root, &bad, &required).is_err());
        }
    }
    #[test]
    fn candidate_paths_cannot_escape_the_repository() {
        for path in ["/tmp", "../excalidraw-render.orig", "."] {
            assert!(candidate_root(path).is_err());
        }
    }
}
