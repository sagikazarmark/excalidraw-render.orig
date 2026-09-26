use super::*;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const SCHEMA: &str = "excalidraw-render/representative-intake-capture/v2";

pub(super) struct Scratch(pub(super) PathBuf);
impl Scratch {
    pub(super) fn new() -> Result<Self> {
        Self::in_directory(&std::env::temp_dir())
    }

    pub(super) fn in_repository() -> Result<Self> {
        let parent = crate::conformance::repo_root().join("target");
        fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        Self::in_directory(&parent)
    }

    fn in_directory(parent: &Path) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let path = parent.join(format!(
                "representative-capture-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("cannot allocate capture scratch directory".into())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn request_args(variant: &Value, format: &str) -> Result<Vec<String>> {
    let mut args = vec![
        "--format".into(),
        format.into(),
        "--compatibility-revision".into(),
        "1".into(),
        "--padding".into(),
        json::number(&variant["padding"])?,
        "--scale".into(),
        json::number(&variant["scale"])?,
        "--report-format".into(),
        "json".into(),
    ];
    if variant["color_mode"] == "dark" {
        args.push("--dark-mode".into());
    }
    if variant["include_background"] == false {
        args.push("--no-background".into());
    }
    if variant["metadata"] == true {
        args.extend(
            ["--embed-scene", "--embed-provenance", "--embed-diagnostics"].map(str::to_owned),
        );
    }
    Ok(args)
}

pub(super) fn render_args(
    scene: &Path,
    artifact: &Path,
    report: &Path,
    variant: &Value,
    format: &str,
) -> Result<Vec<String>> {
    let path_arg = |p: &Path| p.to_str().map(str::to_owned).ok_or("non-UTF8 render path");
    let mut args = vec![
        path_arg(scene)?,
        "--output".into(),
        path_arg(artifact)?,
        "--report-output".into(),
        path_arg(report)?,
    ];
    args.extend(request_args(variant, format)?);
    Ok(args)
}

fn execution_row(variant: &Value, format: &str) -> Result<Value> {
    let key = format!("{}-{format}", text(variant, "id")?);
    Ok(
        json!({"variant": variant["id"], "format": format, "fresh_process_runs": 2,
        "request_args": request_args(variant, format)?, "artifact": format!("{key}.{format}"), "report": format!("{key}-report.json")}),
    )
}

fn validate_descriptor(value: &Value) -> Result<()> {
    fields(value, &["bytes", "sha256"], "invalid file descriptor")?;
    require(
        value["bytes"].as_u64().is_some_and(|n| n > 0)
            && value["sha256"].as_str().is_some_and(digest),
        "invalid descriptor length/digest",
    )
}

fn validate_report(artifact: &[u8], report: &[u8], format: &str) -> Result<()> {
    let parsed = decode(report)?;
    fields(
        &parsed,
        &[
            "schema_version",
            "status",
            "renderer_version",
            "compatibility_revision",
            "excalidraw_release",
            "reference_profile_digest",
            "format",
            "artifact",
            "error",
            "diagnostics",
        ],
        "invalid report envelope",
    )?;
    require(
        parsed["schema_version"].as_u64() == Some(1)
            && parsed["status"] == "success"
            && parsed["error"].is_null(),
        "invalid success report",
    )?;
    require(
        parsed["compatibility_revision"].as_u64() == Some(1)
            && parsed["excalidraw_release"] == "v0.18.1",
        "unexpected compatibility identity",
    )?;
    require(
        !text(&parsed, "renderer_version")?.is_empty(),
        "missing renderer version",
    )?;
    require(
        text(&parsed, "reference_profile_digest")?
            .strip_prefix("sha256:")
            .is_some_and(digest),
        "invalid profile identity",
    )?;
    require(parsed["format"] == format, "invalid report format")?;
    for d in array(&parsed, "diagnostics")? {
        fields(
            d,
            &[
                "code",
                "severity",
                "format",
                "effects",
                "element_id",
                "json_pointer",
                "message",
                "details",
            ],
            "invalid diagnostic envelope",
        )?;
        require(
            d["format"] == format && matches!(text(d, "severity")?, "info" | "warning"),
            "invalid diagnostic format/severity",
        )?;
        require(
            !text(d, "code")?.is_empty() && d["message"].is_string(),
            "invalid diagnostic identity",
        )?;
        require(
            d["element_id"].is_null() || d["element_id"].is_string(),
            "invalid diagnostic element identity",
        )?;
        let pointer = text(d, "json_pointer")?;
        require(
            pointer.is_empty() || pointer.starts_with('/'),
            "invalid diagnostic pointer",
        )?;
        let effects = array(d, "effects")?;
        let ordered = ["input_repair", "visual", "semantic", "metadata"]
            .iter()
            .filter(|e| effects.contains(&json!(e)))
            .map(|e| json!(e))
            .collect::<Vec<_>>();
        require(
            !effects.is_empty() && *effects == ordered,
            "diagnostic effects must be unique and ordered",
        )?;
        require(
            d["details"].is_null() || d["details"].is_object(),
            "invalid diagnostic details container",
        )?;
    }
    let (media, carrier) = match format {
        "svg" => (
            "image/svg+xml",
            artifact[..artifact.len().min(1000)]
                .windows(4)
                .any(|w| w == b"<svg"),
        ),
        "png" => ("image/png", artifact.starts_with(b"\x89PNG\r\n\x1a\n")),
        "pdf" => ("application/pdf", artifact.starts_with(b"%PDF-")),
        _ => return Err("invalid capture format".into()),
    };
    require(
        parsed["artifact"]
            == json!({"media_type": media, "byte_length": artifact.len(), "sha256": format!("sha256:{}", sha256(artifact))}),
        "report detached from requested artifact",
    )?;
    require(carrier, "wrong artifact carrier")
}

pub(super) fn terminate_tree(child: &mut std::process::Child) {
    // Unix groups survive the leader's exit. Windows taskkill is best-effort:
    // a terminated wrapper may no longer expose its descendant tree.
    #[cfg(unix)]
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(windows)]
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &child.id().to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

fn render(renderer: &Path, args: &[String], scratch: &Path, timeout: Duration) -> Result<()> {
    let status = render_process(renderer, args, scratch, timeout)?;
    require(
        status.success(),
        &format!(
            "render failed: {}",
            String::from_utf8_lossy(&fs::read(scratch.join("stderr")).unwrap_or_default())
        ),
    )
}

pub(super) fn render_process(
    renderer: &Path,
    args: &[String],
    scratch: &Path,
    timeout: Duration,
) -> Result<std::process::ExitStatus> {
    // Redirect rather than pipe: a verbose child must not deadlock on a full
    // stdout/stderr pipe while the parent polls its deadline.
    let error_path = scratch.join("stderr");
    let stderr = fs::File::create(&error_path).map_err(|e| e.to_string())?;
    let mut command = Command::new(renderer);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .map_err(|e| format!("cannot start renderer: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Also clear background descendants if a wrapper exits early.
                terminate_tree(&mut child);
                return Ok(status);
            }
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(10))
            }
            result => {
                // Wrappers may have a live renderer child. Terminate the
                // isolated group before removing its scratch directory.
                terminate_tree(&mut child);
                return Err(match result {
                    Err(e) => e.to_string(),
                    _ => "render process timeout".into(),
                });
            }
        }
    }
}

fn tool_record(path: &Path) -> Result<Value> {
    Ok(
        json!({"implementation": "cargo-xtask-representative", "version": env!("CARGO_PKG_VERSION"),
        "executable": descriptor(&fs::read(path).map_err(|e| e.to_string())?)}),
    )
}

pub(super) fn capture(
    bundle: &Path,
    renderer: &Path,
    output: &Path,
    timeout: f64,
) -> Result<Value> {
    require(
        timeout.is_finite() && timeout > 0.0 && timeout <= 1800.0,
        "invalid process timeout",
    )?;
    let manifest = verify(bundle)?;
    let inputs = bundle_files(bundle)?;
    require(
        materialize(&inputs)? == manifest,
        "intake changed before capture",
    )?;
    let intake = descriptor(&encode(&manifest)?);
    let renderer = renderer.canonicalize().map_err(|e| e.to_string())?;
    let binary = descriptor(&fs::read(&renderer).map_err(|e| e.to_string())?);
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let tool = tool_record(&executable)?;
    require(
        fs::symlink_metadata(output).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "capture destination already exists or is inaccessible",
    )?;
    let plan = decode(&inputs["plan.json"])?;
    let mut keys = Vec::new();
    for variant in array(&plan, "variants")? {
        for format in FORMATS {
            keys.push((variant, format));
        }
    }
    let mut first = BTreeMap::new();
    let scratch = Scratch::new()?;
    let scene = scratch.0.join("scene.excalidraw");
    fs::write(&scene, &inputs["scene.excalidraw"]).map_err(|e| e.to_string())?;
    for pass in 0..2 {
        let entries: Vec<_> = if pass == 0 {
            keys.clone()
        } else {
            keys.iter().rev().copied().collect()
        };
        for (variant, format) in entries {
            let key = format!("{}-{format}", text(variant, "id")?);
            let artifact = scratch.0.join(format!("{pass}-{key}.{format}"));
            let report = scratch.0.join(format!("{pass}-{key}-report.json"));
            let args = render_args(&scene, &artifact, &report, variant, format)?;
            render(
                &renderer,
                &args,
                &scratch.0,
                Duration::from_secs_f64(timeout),
            )?;
            let observed = (
                read(
                    &scratch.0,
                    artifact
                        .file_name()
                        .and_then(|s| s.to_str())
                        .ok_or("artifact name")?,
                )?,
                read(
                    &scratch.0,
                    report
                        .file_name()
                        .and_then(|s| s.to_str())
                        .ok_or("report name")?,
                )?,
            );
            validate_report(&observed.0, &observed.1, format)?;
            if pass == 0 {
                first.insert(key, observed);
            } else {
                require(
                    first.get(&key) == Some(&observed),
                    &format!("artifact/report bytes changed between fresh processes: {key}"),
                )?;
            }
        }
    }
    require(
        descriptor(&fs::read(&renderer).map_err(|e| e.to_string())?) == binary,
        "renderer changed during capture",
    )?;
    require(verify(bundle)? == manifest, "intake changed during capture")?;
    require(
        tool_record(&executable)? == tool,
        "tool changed during capture",
    )?;
    let mut rows = Vec::new();
    let mut files = Files::new();
    for (variant, format) in keys {
        let key = format!("{}-{format}", text(variant, "id")?);
        let (artifact, report) = first.remove(&key).ok_or("missing observed render")?;
        files.insert(format!("{key}.{format}"), artifact);
        files.insert(format!("{key}-report.json"), report);
        rows.push(execution_row(variant, format)?);
    }
    let record = json!({"schema": SCHEMA, "case_id": manifest["case_id"], "status": "observed-unapproved", "eligible_executions": [], "approval": null,
        "intake_manifest": intake, "renderer_binary": binary, "tool": tool, "rows": rows,
        "files": files.iter().map(|(name,bytes)| (name, descriptor(bytes))).collect::<BTreeMap<_,_>>()});
    publish(output, &files, "capture.json", &record)?;
    Ok(record)
}

pub(super) fn verify_capture(bundle: &Path, output: &Path) -> Result<Value> {
    let manifest = verify(bundle)?;
    let record = decode(&read(output, "capture.json")?)?;
    fields(
        &record,
        &[
            "schema",
            "case_id",
            "status",
            "eligible_executions",
            "approval",
            "intake_manifest",
            "renderer_binary",
            "tool",
            "rows",
            "files",
        ],
        "invalid capture envelope",
    )?;
    for key in ["intake_manifest", "renderer_binary"] {
        validate_descriptor(&record[key])?;
    }
    if record["schema"] == "excalidraw-render/representative-intake-capture/v1" {
        // Historical observer identity, just like the v2 executable descriptor.
        // Actual files, reports, requests and intake bindings are checked below.
        validate_descriptor(&record["tool"])?;
    } else {
        require(record["schema"] == SCHEMA, "unsupported capture schema")?;
        fields(
            &record["tool"],
            &["implementation", "version", "executable"],
            "invalid capture producer",
        )?;
        require(
            record["tool"]["implementation"] == "cargo-xtask-representative"
                && !text(&record["tool"], "version")?.is_empty(),
            "invalid capture producer identity",
        )?;
        // Historical producer identity is retained, not compared with a later
        // xtask build. Verification checks the actual observations below.
        validate_descriptor(&record["tool"]["executable"])?;
    }
    require(
        record["case_id"] == manifest["case_id"],
        "capture identity changed",
    )?;
    require(
        record["status"] == "observed-unapproved" && record["eligible_executions"] == json!([]),
        "capture cannot grant approval",
    )?;
    require(
        record["intake_manifest"] == descriptor(&read(bundle, "manifest.json")?),
        "capture intake binding changed",
    )?;
    let plan = decode(&read(bundle, "plan.json")?)?;
    let mut rows = Vec::new();
    let mut files = BTreeMap::new();
    for variant in array(&plan, "variants")? {
        for format in FORMATS {
            let row = execution_row(variant, format)?;
            let a = text(&row, "artifact")?;
            let r = text(&row, "report")?;
            let artifact = read(output, a)?;
            let report = read(output, r)?;
            validate_report(&artifact, &report, format)?;
            files.insert(a.to_owned(), descriptor(&artifact));
            files.insert(r.to_owned(), descriptor(&report));
            rows.push(row);
        }
    }
    require(
        record["rows"] == json!(rows) && record["files"] == json!(files),
        "capture execution inventory or file binding changed",
    )?;
    require(
        file_names(output)?
            == files
                .keys()
                .cloned()
                .chain(["capture.json".into()])
                .collect(),
        "capture file inventory changed",
    )?;
    Ok(record)
}
