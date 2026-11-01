//! Viewer observations: collection is create-only; verification reconstructs
//! crops, assets and comparisons rather than trusting stored pass flags.
use super::*;
use crate::cli::EvidenceCommand;
use std::path::{Component, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

mod receipts;
mod regions;

pub(super) const IMAGE: &str = "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9";
const SCHEMA: &str = "excalidraw-render/representative-intake-viewers/v2";
const VIEWPORT: &str = "truncate fractional viewport at origin; never resize or align";
const REFERENCE_SOURCES: [&str; 6] = [
    "tools/representative-intake/reference.mjs",
    "tools/reference-oracle/src/browser-entry.js",
    "tools/reference-oracle/package-lock.json",
    "tools/reference-oracle/vite.config.mjs",
    "tests/oracle/reference-profiles/revision-1.json",
    "tests/oracle/reference-profiles/revision-1-assets.json",
];
const VIEWERS: [&str; 6] = [
    "official-chromium",
    "native-chromium",
    "native-pdfium",
    "native-pdfjs",
    "official-rust",
    "native-rust",
];
const REFERENCE_FILES: [&str; 3] = ["official.svg", "official.png", "official-png.rgba.zlib"];
// Common producers plus the PDFium adapter's versioned inventory below.
const SOURCE_PATHS: &[&str] = &[
    "tools/representative-intake/svg-viewer.mjs",
    "tools/representative-intake/svg-viewer.sh",
    "tools/representative-intake/font-resolution.mjs",
    "tools/representative-intake/viewer-fonts/src/main.rs",
    "tools/representative-intake/viewer-fonts/Cargo.toml",
    "tools/representative-intake/viewer-fonts/Cargo.lock",
    "tools/reference-oracle/scripts/svg-viewer.mjs",
    "tools/reference-oracle/package-lock.json",
    "tools/svg-viewer-rust/src/main.rs",
    "tools/svg-viewer-rust/Cargo.toml",
    "tools/svg-viewer-rust/Cargo.lock",
    "tools/pdf-harness/run-pdfium.sh",
    "tools/pdf-harness/run-pdfjs.sh",
    "tools/pdf-harness/pdfjs/render.mjs",
    "tools/pdf-harness/pdfjs/package-lock.json",
];
const PDFIUM_PYTHON_SOURCE_PATHS: &[&str] = &[
    "tools/pdf-harness/pdfium.lock.json",
    "tools/pdf-harness/pdfium/render.py",
];
const PROFILES: &[&str] = &[
    "tests/oracle/reference-profiles/revision-1.json",
    "tests/oracle/reference-profiles/revision-1-assets.json",
    "tests/conformance/comparator-profiles/canonical-visual-v1.json",
    "tests/conformance/viewer-profiles/canonical-svg-chromium-v1.json",
    "tests/conformance/viewer-profiles/canonical-svg-pure-rust-v1.json",
    "tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json",
    "tests/conformance/viewer-profiles/canonical-pdf-pdfjs-v1.json",
];

pub(super) fn run(command: EvidenceCommand) -> Result<()> {
    match command {
        EvidenceCommand::Collect(args) => {
            collect(&args.bundle, &args.reference, &args.native, &args.output)?;
        }
        EvidenceCommand::Verify(args) => {
            verify_evidence(&args.bundle, &args.reference, &args.native, &args.output)?;
        }
    }
    println!(
        "{}",
        json!({"status":"verified-observations","approval":null,"eligible_executions":[]})
    );
    Ok(())
}

pub(super) fn relative(path: &Path) -> Result<String> {
    let root = crate::conformance::repo_root()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let absolute = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    absolute
        .strip_prefix(root)
        .map_err(|_| "viewer path outside repository")?
        .to_str()
        .map(|s| s.replace('\\', "/"))
        .ok_or("non-UTF8 viewer path".into())
}

pub(super) fn bind(path: &Path) -> Result<Value> {
    let name = relative(path)?;
    let bytes = crate::conformance::read_repository_file(&crate::conformance::repo_root(), &name)?;
    Ok(json!({"path":name,"bytes":bytes.len(),"sha256":sha256(&bytes)}))
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(bytes).map_err(|e| e.to_string())
}

/// Capture subprocess output without pipe deadlocks; the same deadline and
/// Unix process-group cleanup as native capture applies to harness commands.
pub(super) fn command(program: &str, args: &[String], timeout: u64) -> Result<Vec<u8>> {
    let scratch = capture::scratch()?;
    let mut args = args.to_vec();
    let cid = scratch.path().join("container-id");
    if program == "docker" && args.first().is_some_and(|a| a == "run") {
        args.splice(
            1..1,
            ["--cidfile".into(), cid.to_string_lossy().into_owned()],
        );
    }
    let stdout = fs::File::create(scratch.path().join("stdout")).map_err(|e| e.to_string())?;
    let stderr = fs::File::create(scratch.path().join("stderr")).map_err(|e| e.to_string())?;
    let mut command = Command::new(program);
    command
        .args(&args)
        .current_dir(crate::conformance::repo_root())
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                capture::terminate_tree(&mut child);
                remove_container(&cid);
                require(
                    status.success(),
                    &format!(
                        "{program} failed: {}",
                        String::from_utf8_lossy(
                            &fs::read(scratch.path().join("stderr")).unwrap_or_default()
                        )
                    ),
                )?;
                return read(scratch.path(), "stdout");
            }
            Ok(None) if started.elapsed() < Duration::from_secs(timeout) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            other => {
                capture::terminate_tree(&mut child);
                remove_container(&cid);
                return Err(match other {
                    Err(e) => e.to_string(),
                    _ => format!("{program} process timeout"),
                });
            }
        }
    }
}

fn remove_container(cid: &Path) {
    let Ok(id) = fs::read_to_string(cid) else {
        return;
    };
    let id = id.trim();
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return;
    }
    // Killing the Docker client does not stop the daemon-owned container.
    if let Ok(mut child) = Command::new("docker")
        .args(["rm", "-f", id])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        let deadline = Instant::now();
        while matches!(child.try_wait(), Ok(None)) && deadline.elapsed() < Duration::from_secs(15) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub(super) fn docker_args() -> Result<Vec<String>> {
    let uid = String::from_utf8(command("id", &["-u".into()], 10)?).map_err(|e| e.to_string())?;
    let gid = String::from_utf8(command("id", &["-g".into()], 10)?).map_err(|e| e.to_string())?;
    Ok(vec![
        "run".into(),
        "--rm".into(),
        "--init".into(),
        "--ipc=host".into(),
        "--network".into(),
        "none".into(),
        "--platform".into(),
        "linux/amd64".into(),
        "--user".into(),
        format!("{}:{}", uid.trim(), gid.trim()),
        "--env".into(),
        "HOME=/tmp".into(),
        "--env".into(),
        "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1".into(),
        "--env".into(),
        "REFERENCE_ORACLE_CANONICAL=1".into(),
        "--mount".into(),
        format!(
            "type=bind,src={},dst=/workspace",
            crate::conformance::repo_root().display()
        ),
        "--workdir".into(),
        "/workspace/tools/reference-oracle".into(),
        IMAGE.into(),
    ])
}

fn tree(directory: &Path) -> Result<BTreeMap<String, Value>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Value>) -> Result<()> {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            require(!kind.is_symlink(), "symlink in viewer collection")?;
            if kind.is_dir() {
                walk(root, &entry.path(), out)?;
            } else {
                require(kind.is_file(), "non-file in viewer collection")?;
                let path = entry.path();
                let name = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("non-UTF8 collection file")?;
                #[cfg(unix)]
                require(!name.contains('\\'), "backslash in collection filename")?;
                if name != "viewers.json" {
                    out.insert(
                        name.replace('\\', "/"),
                        descriptor(&fs::read(path).map_err(|e| e.to_string())?),
                    );
                }
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    walk(directory, directory, &mut files)?;
    Ok(files)
}

fn inputs(bundle: &Path, reference: &Path, native: &Path) -> Result<Value> {
    [
        bundle.join("manifest.json"),
        bundle.join("plan.json"),
        bundle.join("scene.excalidraw"),
        reference.join("reference.json"),
        native.join("capture.json"),
    ]
    .iter()
    .map(|p| bind(p))
    .collect::<Result<Vec<_>>>()
    .map(|v| json!(v))
}

fn profiles() -> Result<Value> {
    PROFILES
        .iter()
        .map(|p| bind(&crate::conformance::repo_root().join(p)))
        .collect::<Result<Vec<_>>>()
        .map(|v| json!(v))
}

fn source_paths(reference: &Value, adapter: receipts::PdfiumAdapter) -> Result<BTreeSet<String>> {
    let pdfium = match adapter {
        receipts::PdfiumAdapter::Python => PDFIUM_PYTHON_SOURCE_PATHS
            .iter()
            .map(|p| (*p).to_owned())
            .collect(),
        receipts::PdfiumAdapter::Rust => crate::pdf_viewers::rust_pdfium_sources()?,
    };
    let mut paths = SOURCE_PATHS
        .iter()
        .map(|p| (*p).to_owned())
        .chain(pdfium)
        .collect::<BTreeSet<_>>();
    for row in array(reference, "sources")? {
        paths.insert(text(row, "path")?.into());
    }
    Ok(paths)
}

fn sources(reference: &Value) -> Result<Value> {
    source_paths(reference, receipts::PdfiumAdapter::Rust)?
        .iter()
        .map(|p| bind(&crate::conformance::repo_root().join(p)))
        .collect::<Result<Vec<_>>>()
        .map(|v| json!(v))
}

fn verify_sources(reference: &Value, record: &Value) -> Result<()> {
    // A v2 collection can retain either adapter, but every PDFium observation
    // must agree. Do not accept a union inventory or infer identity from paths.
    let mut adapter = None;
    for capture in array(record, "captures")? {
        if capture["viewer"] != "native-pdfium" {
            continue;
        }
        let runs = array(capture, "runs")?;
        require(runs.len() == 2, "invalid raster capture repeats")?;
        for receipt in runs {
            let kind = receipts::PdfiumAdapter::from_receipt(receipt)?;
            require(
                adapter.is_none_or(|previous| previous == kind),
                "mixed PDFium adapter identities",
            )?;
            adapter = Some(kind);
        }
    }
    let adapter = adapter.ok_or("missing PDFium adapter identity")?;
    let required = source_paths(reference, adapter)?;
    // Sources are retained provenance. Validate complete membership and shape,
    // rather than invalidating measured observations on source-only refactors.
    let mut seen = BTreeSet::new();
    for source in array(record, "sources")? {
        fields(
            source,
            &["path", "bytes", "sha256"],
            "invalid source descriptor",
        )?;
        let path = text(source, "path")?;
        require(
            !path.is_empty()
                && Path::new(path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                && source["bytes"].as_u64().is_some_and(|n| n > 0)
                && source["sha256"].as_str().is_some_and(digest)
                && seen.insert(path.to_owned()),
            "invalid or duplicate source descriptor",
        )?;
    }
    // Earlier Rust collections also bound the retired native lock, and the
    // earliest the Python-era wheel lock beside it.
    let retired = |paths: &[&str]| {
        let mut inventory = required.clone();
        inventory.extend(paths.iter().map(|p| (*p).to_owned()));
        inventory
    };
    let native_lock = crate::pdf_viewers::RETIRED_NATIVE_LOCK;
    require(
        seen == required
            || (adapter == receipts::PdfiumAdapter::Rust
                && (seen == retired(&[native_lock])
                    || seen == retired(&[native_lock, "tools/pdf-harness/pdfium.lock.json"]))),
        "incomplete producer inventory",
    )
}

pub(super) fn validate_reference(
    bundle: &Path,
    reference_root: &Path,
) -> Result<(Value, Value, Value)> {
    super::verify(bundle)?;
    let plan = decode(&read(bundle, "plan.json")?)?;
    let scene = decode(&read(bundle, "scene.excalidraw")?)?;
    let reference = decode(&read(reference_root, "reference.json")?)?;
    fields(
        &reference,
        &[
            "schema",
            "status",
            "approval",
            "eligible_executions",
            "case_id",
            "scene_sha256",
            "plan_sha256",
            "browser",
            "network",
            "sources",
            "rows",
        ],
        "invalid reference envelope",
    )?;
    require(
        reference["status"] == "observed-unapproved"
            && reference["eligible_executions"] == json!([]),
        "invalid reference observation state",
    )?;
    require(
        reference["schema"] == "excalidraw-render/representative-intake-reference/v1"
            && reference["case_id"] == plan["case_id"]
            && reference["browser"] == "149.0.7827.55"
            && reference["network"] == "none",
        "invalid reference identity or environment",
    )?;
    let mut reference_sources = BTreeSet::new();
    for source in array(&reference, "sources")? {
        fields(
            source,
            &["path", "sha256"],
            "invalid reference source descriptor",
        )?;
        let path = text(source, "path")?;
        require(
            !path.is_empty()
                && Path::new(path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                && reference_sources.insert(path)
                && source["sha256"].as_str().is_some_and(digest),
            "invalid reference provenance",
        )?;
        if PROFILES.contains(&path) {
            require(
                bind(&crate::conformance::repo_root().join(path))?["sha256"] == source["sha256"],
                "reference profile changed",
            )?;
        }
    }
    require(
        reference_sources == REFERENCE_SOURCES.into_iter().collect(),
        "incomplete reference producer inventory",
    )?;
    require(
        reference["scene_sha256"] == sha256(&read(bundle, "scene.excalidraw")?)
            && reference["plan_sha256"] == sha256(&read(bundle, "plan.json")?),
        "reference detached from intake",
    )?;
    require(
        json!(array(&reference, "rows")?
            .iter()
            .map(|r| &r["variant"])
            .collect::<Vec<_>>())
            == json!(array(&plan, "variants")?
                .iter()
                .map(|v| &v["id"])
                .collect::<Vec<_>>()),
        "reference variant inventory differs",
    )?;
    for row in array(&reference, "rows")? {
        require(
            row["fresh_browser_runs"].as_u64() == Some(2),
            "reference requires two browser runs",
        )?;
        let name = text(row, "variant")?;
        require(valid_id(name), "invalid reference variant")?;
        let files = row["files"].as_object().ok_or("reference files")?;
        require(
            files.keys().map(String::as_str).collect::<BTreeSet<_>>()
                == REFERENCE_FILES.into_iter().collect(),
            "incomplete reference file inventory",
        )?;
        for (file, binding) in files {
            require(
                descriptor(&read(&reference_root.join(name), file)?) == *binding,
                "reference file changed",
            )?;
        }
        regions::derive(&scene, &plan, row)?;
    }
    Ok((reference, plan, scene))
}

struct Publication {
    path: PathBuf,
    complete: bool,
}
impl Drop for Publication {
    fn drop(&mut self) {
        if !self.complete {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(super) fn collect(
    bundle: &Path,
    reference_root: &Path,
    native: &Path,
    output: &Path,
) -> Result<Value> {
    let (reference, plan, scene) = validate_reference(bundle, reference_root)?;
    capture::verify_capture(bundle, native)?;
    let input_bindings = inputs(bundle, reference_root, native)?;
    let source_bindings = sources(&reference)?;
    let profile_bindings = profiles()?;
    // Harness asset declarations name repository paths; publication cannot move
    // after derivation. A guard removes the entire directory on any failure.
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    relative(parent)?;
    fs::create_dir(output).map_err(|e| format!("cannot create viewer collection: {e}"))?;
    let mut publication = Publication {
        path: output.to_owned(),
        complete: false,
    };
    let mut captures = Vec::new();
    let mut judgments = Vec::new();
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let producer = descriptor(&fs::read(&executable).map_err(|e| e.to_string())?);
    // Build the existing standalone Rust viewer with its own lockfile.
    command(
        "cargo",
        &[
            "build",
            "--locked",
            "--release",
            "--manifest-path",
            "tools/svg-viewer-rust/Cargo.toml",
            "--target-dir",
            "target/representative-svg-viewer",
        ]
        .map(str::to_owned),
        600,
    )?;
    for row in array(&reference, "rows")? {
        let variant = text(row, "variant")?;
        let directory = output.join(variant);
        fs::create_dir(&directory).map_err(|e| e.to_string())?;
        for name in REFERENCE_FILES {
            write_new(
                &directory.join(name),
                &read(&reference_root.join(variant), name)?,
            )?;
        }
        for format in FORMATS {
            write_new(
                &directory.join(format!("native.{format}")),
                &read(native, &format!("{variant}-{format}.{format}"))?,
            )?;
            write_new(
                &directory.join(format!("native-{format}-report.json")),
                &read(native, &format!("{variant}-{format}-report.json"))?,
            )?;
        }
        write_new(
            &directory.join("regions.json"),
            &encode(&regions::derive(&scene, &plan, row)?)?,
        )?;
        viewer_assets::collect(&directory)?;
        let (width, height) = receipts::dimensions(row)?;
        for viewer in VIEWERS {
            captures.push(receipts::collect(
                &directory, variant, viewer, width, height,
            )?);
        }
        let mut comparisons = comparison::Variant::load(&directory, None)?;
        for spec in comparison::COMPARISONS {
            let result = comparisons.report(spec)?;
            let report = directory.join(format!("{}-comparison.json", spec.id));
            write_new(&report, &encode(&result)?)?;
            judgments.push(json!({"variant":variant,"comparison":spec.id,"report":bind(&report)?,"result":result}));
        }
    }
    require(
        inputs(bundle, reference_root, native)? == input_bindings
            && sources(&reference)? == source_bindings
            && profiles()? == profile_bindings,
        "viewer inputs or harness sources changed during collection",
    )?;
    require(
        descriptor(&fs::read(executable).map_err(|e| e.to_string())?) == producer,
        "xtask changed during collection",
    )?;
    let record = json!({"schema":SCHEMA,"status":"observed-unapproved","approval":null,"eligible_executions":[],
        "producer":{"implementation":"cargo-xtask-representative","version":env!("CARGO_PKG_VERSION"),"executable":producer},
        "sources":source_bindings,"profiles":profile_bindings,"inputs":input_bindings,"reference":bind(&reference_root.join("reference.json"))?,"native":bind(&native.join("capture.json"))?,
        "files":tree(output)?,"captures":captures,"judgments":judgments,"viewport_adapter":VIEWPORT});
    write_new(&output.join("viewers.json"), &encode(&record)?)?;
    verify_evidence(bundle, reference_root, native, output)?;
    publication.complete = true;
    Ok(record)
}

pub(super) fn verify_evidence(
    bundle: &Path,
    reference_root: &Path,
    native: &Path,
    output: &Path,
) -> Result<Value> {
    let (reference, plan, scene) = validate_reference(bundle, reference_root)?;
    capture::verify_capture(bundle, native)?;
    let record = decode(&read(output, "viewers.json")?)?;
    fields(
        &record,
        &[
            "schema",
            "status",
            "approval",
            "eligible_executions",
            "producer",
            "sources",
            "profiles",
            "inputs",
            "reference",
            "native",
            "files",
            "captures",
            "judgments",
            "viewport_adapter",
        ],
        "invalid viewer collection envelope",
    )?;
    require(
        record["viewport_adapter"] == VIEWPORT,
        "unsupported viewport adapter",
    )?;
    require(
        record["schema"] == SCHEMA,
        "unsupported viewer collection; Rust evidence verification requires v2",
    )?;
    require(
        record["status"] == "observed-unapproved" && record["eligible_executions"] == json!([]),
        "evidence cannot grant approval",
    )?;
    require(
        record["inputs"] == inputs(bundle, reference_root, native)?
            && record["profiles"] == profiles()?
            && record["reference"] == bind(&reference_root.join("reference.json"))?
            && record["native"] == bind(&native.join("capture.json"))?,
        "evidence input/profile bindings changed",
    )?;
    let producer = &record["producer"];
    require(
        producer["implementation"] == "cargo-xtask-representative"
            && !text(producer, "version")?.is_empty(),
        "invalid evidence producer",
    )?;
    let binary = &producer["executable"];
    fields(binary, &["bytes", "sha256"], "invalid producer descriptor")?;
    require(
        binary["bytes"].as_u64().is_some_and(|n| n > 0)
            && binary["sha256"].as_str().is_some_and(digest),
        "invalid producer descriptor",
    )?;
    verify_sources(&reference, &record)?;
    require(
        record["files"] == json!(tree(output)?),
        "viewer file inventory or digest changed",
    )?;
    let mut expected_captures = Vec::new();
    let mut expected_judgments = Vec::new();
    for row in array(&reference, "rows")? {
        let variant = text(row, "variant")?;
        let directory = output.join(variant);
        viewer_assets::verify_assets(&directory)?;
        require(
            decode(&read(&directory, "regions.json")?)? == regions::derive(&scene, &plan, row)?,
            "crop plan detached from source anchors",
        )?;
        for name in REFERENCE_FILES {
            require(
                read(&directory, name)? == read(&reference_root.join(variant), name)?,
                "copied reference changed",
            )?;
        }
        for format in FORMATS {
            require(
                read(&directory, &format!("native.{format}"))?
                    == read(native, &format!("{variant}-{format}.{format}"))?,
                "copied native artifact changed",
            )?;
            require(
                read(&directory, &format!("native-{format}-report.json"))?
                    == read(native, &format!("{variant}-{format}-report.json"))?,
                "copied native report changed",
            )?;
        }
        let (width, height) = receipts::dimensions(row)?;
        for viewer in VIEWERS {
            expected_captures.push((variant, viewer, width, height));
        }
        let mut comparisons = comparison::Variant::load(&directory, None)?;
        for spec in comparison::COMPARISONS {
            let path = directory.join(format!("{}-comparison.json", spec.id));
            let result = comparisons.report(spec)?;
            require(
                decode(&read(&directory, &format!("{}-comparison.json", spec.id))?)? == result,
                "stored judgment differs from canonical comparison",
            )?;
            expected_judgments.push(json!({"variant":variant,"comparison":spec.id,"report":bind(&path)?,"result":result}));
        }
    }
    require(
        record["judgments"] == json!(expected_judgments),
        "comparison report detached or incomplete judgment inventory",
    )?;
    let captures = array(&record, "captures")?;
    require(
        captures.len() == expected_captures.len(),
        "incomplete viewer inventory",
    )?;
    for (capture, (variant, viewer, width, height)) in captures.iter().zip(expected_captures) {
        require(
            capture["variant"] == variant && capture["viewer"] == viewer,
            "invalid viewer order/identity",
        )?;
        receipts::verify(&output.join(variant), capture, viewer, width, height)?;
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdfium_source_inventories_follow_receipts_without_changing_v2() {
        let reference = json!({"sources": REFERENCE_SOURCES.map(|path| json!({"path":path}))});
        let legacy = json!({"pdfium":"152.0.7947.0","pypdfium2":"5.12.1"});
        let current = json!({
            "pdfium":"152.0.7947.0","adapter":"pdfium-render/0.9.4",
            "library_sha256":"61c9f745c6296a1050599a99a1ed985036411b591a11bd2a41bafe530ecb4f33"
        });
        let inventory = |adapter| {
            json!(source_paths(&reference, adapter)
                .unwrap()
                .iter()
                .map(|path| {
                    // Historical source bytes need not exist in the checkout.
                    json!({"path":path,"bytes":1,"sha256":"0".repeat(64)})
                })
                .collect::<Vec<_>>())
        };
        let old_sources = inventory(receipts::PdfiumAdapter::Python);
        let new_sources = inventory(receipts::PdfiumAdapter::Rust);
        let old_paths = source_paths(&reference, receipts::PdfiumAdapter::Python).unwrap();
        let new_paths = source_paths(&reference, receipts::PdfiumAdapter::Rust).unwrap();
        assert_eq!(old_paths.len(), 22);
        assert_eq!(new_paths.len(), 24);
        assert_eq!(
            old_paths
                .difference(&new_paths)
                .map(String::as_str)
                .collect::<Vec<_>>(),
            PDFIUM_PYTHON_SOURCE_PATHS
        );
        assert_eq!(
            new_paths
                .difference(&old_paths)
                .cloned()
                .collect::<BTreeSet<_>>(),
            crate::pdf_viewers::rust_pdfium_sources()
                .unwrap()
                .into_iter()
                .collect()
        );
        for (receipt, sources, wrong_sources, other) in [
            (&legacy, &old_sources, &new_sources, &current),
            (&current, &new_sources, &old_sources, &legacy),
        ] {
            let original = json!({"schema":SCHEMA,"sources":sources,"captures":[
                {"variant":"light","viewer":"native-pdfium","runs":[receipt,receipt]},
                {"variant":"dark","viewer":"native-pdfium","runs":[receipt,receipt]}
            ]});
            verify_sources(&reference, &original).unwrap();
            if receipt == &current {
                let retired = |path: &str| json!({"path":path, "bytes":1, "sha256":"0".repeat(64)});
                let mut native_lock = original.clone();
                native_lock["sources"]
                    .as_array_mut()
                    .unwrap()
                    .push(retired(crate::pdf_viewers::RETIRED_NATIVE_LOCK));
                verify_sources(&reference, &native_lock).unwrap();
                let mut earlier_rust = native_lock.clone();
                earlier_rust["sources"]
                    .as_array_mut()
                    .unwrap()
                    .push(retired("tools/pdf-harness/pdfium.lock.json"));
                verify_sources(&reference, &earlier_rust).unwrap();
                // The wheel lock never appeared without the native lock.
                let mut wheel_only = original.clone();
                wheel_only["sources"]
                    .as_array_mut()
                    .unwrap()
                    .push(retired("tools/pdf-harness/pdfium.lock.json"));
                assert!(verify_sources(&reference, &wheel_only).is_err());
            }
            let mut changed = original.clone();
            changed["sources"] = wrong_sources.clone();
            assert!(verify_sources(&reference, &changed).is_err());
            changed = original.clone();
            changed["sources"]
                .as_array_mut()
                .unwrap()
                .push(sources[0].clone());
            assert!(verify_sources(&reference, &changed).is_err());
            changed = original.clone();
            changed["sources"].as_array_mut().unwrap().remove(0);
            assert!(verify_sources(&reference, &changed).is_err());
            changed = original.clone();
            changed["captures"][0]["runs"][1] = other.clone();
            assert!(verify_sources(&reference, &changed).is_err());
            changed = original.clone();
            changed["captures"][1]["runs"] = json!([other, other]);
            assert!(verify_sources(&reference, &changed).is_err());
        }
    }

    #[test]
    fn reference_metadata_cannot_omit_producers_runs_or_change_options() {
        let root = crate::conformance::repo_root();
        let bundle = root.join("tests/conformance/intake/velox-design-v2");
        let reference = root.join("tests/conformance/cases/representative/velox-design/reference");
        let original = decode(&read(&reference, "reference.json").unwrap()).unwrap();
        validate_reference(&bundle, &reference).unwrap();
        let scratch = capture::scratch().unwrap();
        for name in ["light", "dark"] {
            fs::create_dir(scratch.path().join(name)).unwrap();
            for file in REFERENCE_FILES {
                fs::copy(
                    reference.join(name).join(file),
                    scratch.path().join(name).join(file),
                )
                .unwrap();
            }
        }
        for mutation in 0..5 {
            let mut changed = original.clone();
            match mutation {
                0 => {
                    changed["sources"].as_array_mut().unwrap().remove(0);
                }
                1 => changed["status"] = json!("failed"),
                2 => changed["rows"][0]["fresh_browser_runs"] = json!(0),
                3 => changed["rows"][0]["options"]["exportPadding"] = json!(11),
                _ => changed["eligible_executions"] = json!(["forged"]),
            }
            fs::write(
                scratch.path().join("reference.json"),
                encode(&changed).unwrap(),
            )
            .unwrap();
            assert!(
                validate_reference(&bundle, scratch.path()).is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    #[ignore = "requires Docker and pinned Playwright image"]
    fn timed_out_docker_command_removes_the_container() {
        let scratch = capture::scratch().unwrap();
        let name = format!("representative-timeout-{}", std::process::id());
        let args = ["run", "--rm", "--name", &name, IMAGE, "sleep", "60"].map(str::to_owned);
        assert!(command("docker", &args, 1).unwrap_err().contains("timeout"));
        let result = Command::new("docker")
            .args(["inspect", &name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(!result.success());
        drop(scratch);
    }
}
