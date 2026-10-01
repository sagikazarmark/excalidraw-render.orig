//! Side-by-side demo of this library against official Excalidraw 0.18.1.
//!
//! `demo build` reads `tools/demo/examples.json`, renders every example with
//! the `--locked` release CLI under its case's export options, copies the
//! committed official SVG and PNG evidence beside the renders, prints each
//! official SVG to PDF with Chromium in the pinned oracle container (Excalidraw
//! has no PDF export), and writes one static `index.html`. Nothing under
//! `tests/` is written; every output is a function of the checkout.
use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::{json, Value};

use crate::cli::DemoCommand;
use crate::conformance::repo_root;
use crate::conformance_candidate_native;

type Result<T> = std::result::Result<T, String>;

const EXAMPLES: &str = "tools/demo/examples.json";
const EXAMPLES_SCHEMA: &str = "excalidraw-render/demo-examples/v1";
const PRINT_SCRIPT: &str = "tools/demo/print-official-pdf.mjs";
const ORACLE_PLAYWRIGHT: &str = "tools/reference-oracle/node_modules/playwright/package.json";
/// The reference oracle's pinned Playwright image (`tools/reference-oracle/run-container.sh`).
const ORACLE_IMAGE: &str = "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9";
/// Marks a directory this command built, so a rebuild may replace it.
const MARKER: &str = ".excalidraw-render-demo";
const FORMATS: [&str; 3] = ["svg", "png", "pdf"];

pub fn run(command: DemoCommand) -> Result<()> {
    match command {
        DemoCommand::Build { out } => {
            let out = out.unwrap_or_else(|| repo_root().join("target/demo"));
            build(&out)?;
            println!("built demo: {}", out.join("index.html").display());
            Ok(())
        }
    }
}

/// One rendered row of an example: a light or dark scene export, or one frame.
struct Row {
    label: String,
    /// Directory relative to the output root.
    dir: String,
    official_svg_size: (String, String),
    official_png_size: (u32, u32),
    library_png_size: (u32, u32),
    /// `(format, diagnostics)` from this library's JSON report.
    diagnostics: Vec<(&'static str, Value)>,
}

struct Example {
    id: String,
    title: String,
    caption: String,
    scene: String,
    rows: Vec<Row>,
}

struct Group {
    title: String,
    examples: Vec<Example>,
}

fn build(out: &Path) -> Result<()> {
    let root = repo_root();
    let examples = read_json(&root.join(EXAMPLES))?;
    if examples["schema"] != EXAMPLES_SCHEMA {
        return Err(format!("{EXAMPLES}: schema must be {EXAMPLES_SCHEMA}"));
    }
    if !root.join(ORACLE_PLAYWRIGHT).is_file() {
        return Err("the reference oracle's Playwright is not installed; run tools/reference-oracle/run-container.sh install".into());
    }
    let (cli, _) = conformance_candidate_native::build()?;
    let version = cli_version(&cli)?;
    let commit = commit()?;

    prepare_output(out)?;
    let out = fs::canonicalize(out).map_err(|e| format!("{}: {e}", out.display()))?;

    let mut groups = Vec::new();
    let mut prints = Vec::new();
    for group in array(&examples["groups"], "groups")? {
        let mut built = Vec::new();
        for example in array(&group["examples"], "examples")? {
            let example = build_example(&cli, &out, example, &mut prints)?;
            eprintln!("rendered {}", example.id);
            built.push(example);
        }
        groups.push(Group {
            title: string(&group["title"], "group title")?.into(),
            examples: built,
        });
    }
    print_official_pdfs(&out, &prints)?;
    fs::copy(
        root.join("docs/known-limitations.md"),
        out.join("known-limitations.md"),
    )
    .map_err(|e| format!("known-limitations.md: {e}"))?;
    let html = page(&groups, &version, &commit);
    fs::write(out.join("index.html"), html).map_err(|e| format!("index.html: {e}"))?;
    Ok(())
}

/// Replaces a previous demo build, refusing to delete anything else.
fn prepare_output(out: &Path) -> Result<()> {
    if out.exists() {
        let empty = fs::read_dir(out)
            .map_err(|e| format!("{}: {e}", out.display()))?
            .next()
            .is_none();
        if !empty && !out.join(MARKER).is_file() {
            return Err(format!(
                "{} exists and is not a demo build; choose an empty or new --out",
                out.display()
            ));
        }
        fs::remove_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    fs::write(out.join(MARKER), "").map_err(|e| e.to_string())
}

fn build_example(
    cli: &Path,
    out: &Path,
    example: &Value,
    prints: &mut Vec<Value>,
) -> Result<Example> {
    let id = string(&example["id"], "id")?;
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(format!("example id {id:?} must be lowercase ASCII words"));
    }
    let source = repo_root().join(string(&example["source"], "source")?);
    let case = Case::load(&source)?;
    let dir = format!("examples/{id}");
    fs::create_dir_all(out.join(&dir)).map_err(|e| e.to_string())?;
    let scene = format!("{dir}/{id}.excalidraw");
    copy(&case.scene, &out.join(&scene))?;

    let mut rows = Vec::new();
    for variant in array(&example["variants"], "variants")? {
        match string(variant, "variant")? {
            "frame" => rows.extend(frame_rows(cli, out, &case, &dir)?),
            mode @ ("light" | "dark") => rows.push(scene_row(cli, out, &case, &dir, mode)?),
            other => return Err(format!("{id}: unknown variant {other:?}")),
        }
    }
    for row in &rows {
        let (width, height) = &row.official_svg_size;
        prints.push(json!({
            "svg": format!("/out/{}/official.svg", row.dir),
            "pdf": format!("/out/{}/official.pdf", row.dir),
            "width": width,
            "height": height,
        }));
    }
    Ok(Example {
        id: id.into(),
        title: string(&example["title"], "title")?.into(),
        caption: string(&example["caption"], "caption")?.into(),
        scene,
        rows,
    })
}

/// A conformance case's scene and its official evidence, from either a focused
/// manifest or a representative case record.
struct Case {
    dir: PathBuf,
    scene: PathBuf,
    manifest: Value,
    /// Representative reference rows; `None` for a focused case.
    reference: Option<Value>,
}

impl Case {
    fn load(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join("manifest.json");
        if manifest_path.is_file() {
            let manifest = read_json(&manifest_path)?;
            let scene = dir.join(string(&manifest["scene"]["path"], "scene path")?);
            return Ok(Self {
                dir: dir.into(),
                scene,
                manifest,
                reference: None,
            });
        }
        let manifest = read_json(&dir.join("case.json"))?;
        let intake = repo_root().join(string(&manifest["intake"]["path"], "intake path")?);
        Ok(Self {
            dir: dir.into(),
            scene: intake.join("scene.excalidraw"),
            reference: Some(read_json(&dir.join("reference/reference.json"))?),
            manifest,
        })
    }

    /// The official SVG and PNG and the export options of a light or dark
    /// scene export.
    fn variant(&self, mode: &str) -> Result<(PathBuf, PathBuf, Value)> {
        let dark = mode == "dark";
        let missing = || format!("{}: no {mode} variant", self.dir.display());
        let (evidence, options) = match &self.reference {
            Some(reference) => {
                let row = array(&reference["rows"], "reference rows")?
                    .iter()
                    .find(|row| row["variant"] == mode)
                    .ok_or_else(missing)?;
                (
                    self.dir.join("reference").join(mode),
                    row["options"].clone(),
                )
            }
            None => {
                let variant = array(&self.manifest["variants"], "variants")?
                    .iter()
                    .find(|v| {
                        v["options"]["exportWithDarkMode"] == dark
                            && v["id"] == if dark { "dark-mode" } else { "default" }
                    })
                    .ok_or_else(missing)?;
                let svg = self.dir.join(string(
                    &variant["formats"]["svg"]["artifact"]["path"],
                    "svg path",
                )?);
                let png = self.dir.join(string(
                    &variant["formats"]["png"]["artifact"]["path"],
                    "png path",
                )?);
                return Ok((svg, png, variant["options"].clone()));
            }
        };
        Ok((
            evidence.join("official.svg"),
            evidence.join("official.png"),
            options,
        ))
    }
}

fn scene_row(cli: &Path, out: &Path, case: &Case, dir: &str, mode: &str) -> Result<Row> {
    let (svg, png, options) = case.variant(mode)?;
    check_background(&case.scene, &options)?;
    let args = export_args(&options)?;
    let row_dir = format!("{dir}/{mode}");
    let at = out.join(&row_dir);
    fs::create_dir_all(&at).map_err(|e| e.to_string())?;
    copy(&svg, &at.join("official.svg"))?;
    copy(&png, &at.join("official.png"))?;
    copy(&case.scene, &at.join("scene.excalidraw"))?;
    let mut diagnostics = Vec::new();
    for format in FORMATS {
        // Relative paths from inside the row directory keep the reports free
        // of the output location.
        let status = Command::new(cli)
            .current_dir(&at)
            .args(["scene.excalidraw", "--output"])
            .arg(format!("library.{format}"))
            .args([
                "--format",
                format,
                "--report-format",
                "json",
                "--report-output",
            ])
            .arg(format!("report-{format}.json"))
            .args(&args)
            .status()
            .map_err(|e| e.to_string())?;
        let report = read_json(&at.join(format!("report-{format}.json")))?;
        if !status.success() || report["status"] != "success" {
            return Err(format!(
                "{row_dir}: library {format} render failed: {report}"
            ));
        }
        diagnostics.push((format, report["diagnostics"].clone()));
    }
    fs::remove_file(at.join("scene.excalidraw")).map_err(|e| e.to_string())?;
    Ok(Row {
        label: if mode == "dark" { "Dark mode" } else { "Light" }.into(),
        official_svg_size: svg_size(&at.join("official.svg"))?,
        official_png_size: png_size(&at.join("official.png"))?,
        library_png_size: png_size(&at.join("library.png"))?,
        dir: row_dir,
        diagnostics,
    })
}

/// Every frame of a frame-export case, through the `frames` subcommand.
fn frame_rows(cli: &Path, out: &Path, case: &Case, dir: &str) -> Result<Vec<Row>> {
    let options = &case.manifest["options"];
    let targets = array(&case.manifest["targets"], "frame targets")?;
    // The frames command exports at zero padding with frame rendering on; the
    // remaining options must be ones it can reproduce.
    let mut args = Vec::new();
    for (key, value) in options.as_object().ok_or("frame options")? {
        match (key.as_str(), value) {
            ("exportBackground", Value::Bool(true))
            | ("exportEmbedScene", Value::Bool(false))
            | ("exportWithDarkMode", Value::Bool(false))
            | ("maxWidthOrHeight", Value::Null)
            | ("viewBackgroundColor", _) => {}
            ("exportScale", Value::Number(scale)) => {
                args.extend(["--scale".to_owned(), scale.to_string()])
            }
            _ => {
                return Err(format!(
                    "frame export option {key}={value} is not reproduced"
                ))
            }
        }
    }
    check_background(&case.scene, options)?;
    let staging = out.join(dir).join("frames");
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    copy(&case.scene, &staging.join("scene.excalidraw"))?;
    let mut reports = Vec::new();
    for format in FORMATS {
        let status = Command::new(cli)
            .current_dir(&staging)
            .args([
                "frames",
                "scene.excalidraw",
                "--output",
                format,
                "--format",
                format,
            ])
            .args([
                "--compatibility-revision",
                "1",
                "--report-format",
                "json",
                "--report-output",
            ])
            .arg(format!("report-{format}.json"))
            .args(&args)
            .status()
            .map_err(|e| e.to_string())?;
        let report = read_json(&staging.join(format!("report-{format}.json")))?;
        if !status.success() || report["status"] != "success" {
            return Err(format!("{dir}: library {format} frames failed: {report}"));
        }
        reports.push((format, report));
    }

    let mut rows = Vec::new();
    for target in targets {
        let ordinal = target["frame_ordinal"].as_u64().ok_or("frame ordinal")?;
        let row_dir = format!("{dir}/frame-{ordinal}");
        let at = out.join(&row_dir);
        fs::create_dir_all(&at).map_err(|e| e.to_string())?;
        for format in ["svg", "png"] {
            let artifact = &target["outputs"][format]["artifact"]["path"];
            copy(
                &case.dir.join(string(artifact, "frame artifact")?),
                &at.join(format!("official.{format}")),
            )?;
        }
        let mut diagnostics = Vec::new();
        for (format, report) in &reports {
            let frame = array(&report["frames"], "report frames")?
                .iter()
                .find(|f| f["ordinal"] == ordinal)
                .ok_or_else(|| format!("{dir}: frame {ordinal} missing from the {format} batch"))?;
            if frame["kind"] != target["frame_kind"] || frame["name"] != target["frame_name"] {
                return Err(format!(
                    "{dir}: frame {ordinal} does not match its official target"
                ));
            }
            let produced = string(&frame["output_path"], "frame output path")?;
            fs::rename(staging.join(produced), at.join(format!("library.{format}")))
                .map_err(|e| format!("{produced}: {e}"))?;
            let report = json!({
                "batch_diagnostics": report["diagnostics"],
                "frame": frame,
            });
            fs::write(
                at.join(format!("report-{format}.json")),
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let mut all = report["batch_diagnostics"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            all.extend(frame["diagnostics"].as_array().cloned().unwrap_or_default());
            diagnostics.push((*format, Value::Array(all)));
        }
        let name = match &target["frame_name"] {
            Value::String(name) if !name.is_empty() => format!("“{name}”"),
            Value::String(_) => "empty name".into(),
            _ => "no name".into(),
        };
        rows.push(Row {
            label: format!(
                "Frame {ordinal} ({}, {name})",
                string(&target["frame_kind"], "frame kind")?
            ),
            official_svg_size: svg_size(&at.join("official.svg"))?,
            official_png_size: png_size(&at.join("official.png"))?,
            library_png_size: png_size(&at.join("library.png"))?,
            dir: row_dir,
            diagnostics,
        });
    }
    fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    Ok(rows)
}

/// The CLI flags reproducing an official export's options. An option the CLI
/// cannot express is an error, never silently dropped.
fn export_args(options: &Value) -> Result<Vec<String>> {
    let mut args = vec!["--compatibility-revision".to_owned(), "1".to_owned()];
    let options = options
        .as_object()
        .ok_or("export options must be an object")?;
    for (key, value) in options {
        match (key.as_str(), value) {
            ("exportPadding", Value::Number(n)) => args.extend(["--padding".into(), n.to_string()]),
            ("exportScale", Value::Number(n)) => args.extend(["--scale".into(), n.to_string()]),
            ("exportScale", Value::Null) => {}
            ("maxWidthOrHeight", Value::Number(n)) => {
                args.extend(["--max-dimension".into(), n.to_string()])
            }
            ("maxWidthOrHeight", Value::Null) => {}
            ("exportWithDarkMode", Value::Bool(dark)) => {
                if *dark {
                    args.push("--dark-mode".into());
                }
            }
            ("exportBackground", Value::Bool(background)) => {
                if !background {
                    args.push("--no-background".into());
                }
            }
            ("exportEmbedScene", Value::Bool(embed)) => {
                if *embed {
                    args.push("--embed-scene".into());
                }
            }
            ("frameRendering", Value::Object(frame)) => {
                for (field, flag) in [
                    ("enabled", "--no-frame-rendering"),
                    ("clip", "--no-frame-clipping"),
                    ("name", "--no-frame-names"),
                    ("outline", "--no-frame-outlines"),
                ] {
                    match frame.get(field) {
                        Some(Value::Bool(true)) => {}
                        Some(Value::Bool(false)) => args.push(flag.into()),
                        _ => return Err(format!("frameRendering.{field} must be a boolean")),
                    }
                }
                if frame.len() != 4 {
                    return Err("frameRendering carries an unknown field".into());
                }
            }
            // The CLI reads the background from the scene; check_background
            // proves the two agree.
            ("viewBackgroundColor", Value::String(_)) => {}
            _ => return Err(format!("export option {key}={value} has no CLI equivalent")),
        }
    }
    if options.get("exportScale").is_some_and(Value::is_null)
        == options.get("maxWidthOrHeight").is_some_and(Value::is_null)
    {
        return Err("exactly one of exportScale and maxWidthOrHeight must be set".into());
    }
    Ok(args)
}

/// The CLI takes the view background from the scene's appState, so the
/// official export's `viewBackgroundColor` must be the scene's own.
fn check_background(scene: &Path, options: &Value) -> Result<()> {
    let scene = read_json(scene)?;
    let ours = scene["appState"]["viewBackgroundColor"]
        .as_str()
        .unwrap_or("#ffffff");
    let theirs = string(&options["viewBackgroundColor"], "viewBackgroundColor")?;
    if !ours.eq_ignore_ascii_case(theirs) {
        return Err(format!(
            "official export used viewBackgroundColor {theirs}, the scene carries {ours}"
        ));
    }
    Ok(())
}

/// Prints every official SVG to PDF in one run of the pinned oracle container,
/// without network, with the checkout mounted read-only.
fn print_official_pdfs(out: &Path, jobs: &[Value]) -> Result<()> {
    let jobs_file = out.join("print-jobs.json");
    fs::write(
        &jobs_file,
        serde_json::to_vec(jobs).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let user = format!("{}:{}", id("-u")?, id("-g")?);
    let status = Command::new("docker")
        .args(["run", "--rm", "--init", "--ipc=host", "--network", "none"])
        .args(["--platform", "linux/amd64", "--user", &user])
        .args([
            "--env",
            "HOME=/tmp",
            "--env",
            "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1",
        ])
        .args(["--env", "REFERENCE_ORACLE_CANONICAL=1", "--mount"])
        .arg(format!(
            "type=bind,src={},dst=/workspace,readonly",
            repo_root().display()
        ))
        .arg("--mount")
        .arg(format!("type=bind,src={},dst=/out", out.display()))
        .args([
            "--workdir",
            "/workspace/tools/reference-oracle",
            ORACLE_IMAGE,
            "node",
        ])
        .arg(format!("/workspace/{PRINT_SCRIPT}"))
        .arg("/out/print-jobs.json")
        .status()
        .map_err(|e| format!("docker: {e}"))?;
    if !status.success() {
        return Err("printing the official SVGs to PDF failed".into());
    }
    fs::remove_file(jobs_file).map_err(|e| e.to_string())
}

fn id(flag: &str) -> Result<String> {
    let output = Command::new("id")
        .arg(flag)
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn cli_version(cli: &Path) -> Result<String> {
    let output = Command::new(cli)
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .next()
        .and_then(|line| line.strip_prefix("excalidraw-render "))
        .map(str::to_owned)
        .ok_or_else(|| format!("unexpected --version output: {text}"))
}

/// The checked-out commit, flagged when tracked files differ from it.
fn commit() -> Result<String> {
    let git = |args: &[&str]| -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_root())
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let sha = git(&["rev-parse", "HEAD"])?;
    let dirty = !git(&["status", "--porcelain", "--untracked-files=no"])?.is_empty();
    Ok(if dirty {
        format!("{sha} (with uncommitted changes)")
    } else {
        sha
    })
}

fn svg_size(path: &Path) -> Result<(String, String)> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let document =
        roxmltree::Document::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let svg = document.root_element();
    let attribute = |name| {
        svg.attribute(name)
            .map(str::to_owned)
            .ok_or_else(|| format!("{}: root has no {name}", path.display()))
    };
    Ok((attribute("width")?, attribute("height")?))
}

fn png_size(path: &Path) -> Result<(u32, u32)> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return Err(format!("{}: not a PNG", path.display()));
    }
    let be =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Ok((be(16), be(20)))
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    fs::copy(from, to)
        .map(drop)
        .map_err(|e| format!("{}: {e}", from.display()))
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| format!("{label} must be an array"))
}

fn string<'a>(value: &'a Value, label: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| format!("{label} must be a string"))
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            c => escaped.push(c),
        }
    }
    escaped
}

/// A CSS pixel length for an `<img>` attribute: SVG lengths may be fractional
/// or carry a `px` unit, and the attribute wants a whole number.
fn css_pixels(length: &str) -> String {
    let number = length.trim().trim_end_matches("px");
    number
        .parse::<f64>()
        .map(|n| format!("{}", n.ceil() as u64))
        .unwrap_or_default()
}

fn page(groups: &[Group], version: &str, commit: &str) -> String {
    let mut html = String::new();
    let _ = write!(
        html,
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>excalidraw-render demo</title>
<style>{STYLE}</style>
</head>
<body>
<header class="masthead">
<h1>excalidraw-render <span class="vs">vs</span> Excalidraw 0.18.1</h1>
<p>Each example is rendered by this library (<code>excalidraw-render {version}</code>) and shown beside the official Excalidraw 0.18.1 export of the same scene with the same export options. The renders are not claimed to be pixel-identical; see <a href="known-limitations.md">known limitations</a>.</p>
<dl class="facts">
<div><dt>Library version</dt><dd>{version}</dd></div>
<div><dt>Compatibility target</dt><dd>Excalidraw 0.18.1</dd></div>
<div><dt>Commit</dt><dd><code>{commit}</code></dd></div>
</dl>
<p class="note">Excalidraw has no PDF export. Each “official PDF” below is the official SVG printed by Chromium, not an Excalidraw export.</p>
</header>
<div class="layout">
<nav class="toc" aria-label="Examples">
<h2>Examples</h2>
"#,
        version = escape(version),
        commit = escape(commit),
    );
    for group in groups {
        let _ = write!(html, "<h3>{}</h3>\n<ul>\n", escape(&group.title));
        for example in &group.examples {
            let _ = writeln!(
                html,
                r##"<li><a href="#{}">{}</a></li>"##,
                example.id,
                escape(&example.title)
            );
        }
        html.push_str("</ul>\n");
    }
    html.push_str("</nav>\n<main>\n");
    for group in groups {
        let _ = writeln!(html, "<h2 class=\"group\">{}</h2>", escape(&group.title));
        for example in &group.examples {
            example_html(&mut html, example);
        }
    }
    html.push_str("</main>\n</div>\n</body>\n</html>\n");
    html
}

fn example_html(html: &mut String, example: &Example) {
    let _ = write!(
        html,
        r#"<section class="example" id="{id}">
<h3>{title}</h3>
<p class="caption">{caption}</p>
<p class="downloads"><a href="{scene}" download>Source scene (.excalidraw)</a></p>
"#,
        id = example.id,
        title = escape(&example.title),
        caption = escape(&example.caption),
        scene = example.scene,
    );
    for row in &example.rows {
        let dir = &row.dir;
        let (svg_w, svg_h) = (
            css_pixels(&row.official_svg_size.0),
            css_pixels(&row.official_svg_size.1),
        );
        let (official_w, official_h) = row.official_png_size;
        let (library_w, library_h) = row.library_png_size;
        let count: usize = row
            .diagnostics
            .iter()
            .map(|(_, d)| d.as_array().map_or(0, Vec::len))
            .sum();
        let _ = write!(
            html,
            r#"<div class="variant">
<h4>{label}</h4>
<div class="pair">
<figure><figcaption>Official SVG</figcaption><div class="render"><img src="{dir}/official.svg" alt="Official SVG, {label}" width="{svg_w}" height="{svg_h}" loading="lazy"></div></figure>
<figure><figcaption>excalidraw-render SVG</figcaption><div class="render"><img src="{dir}/library.svg" alt="excalidraw-render SVG, {label}" loading="lazy"></div></figure>
</div>
<div class="pair">
<figure><figcaption>Official PNG</figcaption><div class="render"><img src="{dir}/official.png" alt="Official PNG, {label}" width="{official_w}" height="{official_h}" loading="lazy"></div></figure>
<figure><figcaption>excalidraw-render PNG</figcaption><div class="render"><img src="{dir}/library.png" alt="excalidraw-render PNG, {label}" width="{library_w}" height="{library_h}" loading="lazy"></div></figure>
</div>
<p class="downloads"><a href="{dir}/official.pdf" download>Official SVG printed by Chromium (PDF)</a> <a href="{dir}/library.pdf" download>excalidraw-render PDF</a></p>
<details><summary>Compatibility diagnostics ({count})</summary>
"#,
            label = escape(&row.label),
        );
        for (format, diagnostics) in &row.diagnostics {
            let pretty = serde_json::to_string_pretty(diagnostics).unwrap_or_default();
            let _ = writeln!(
                html,
                r#"<p><a href="{dir}/report-{format}.json">{upper} report</a></p><pre>{}</pre>"#,
                escape(&pretty),
                upper = format.to_ascii_uppercase(),
            );
        }
        html.push_str("</details>\n</div>\n");
    }
    html.push_str("</section>\n");
}

const STYLE: &str = r#"
:root { --ink: #1d1d1f; --muted: #5f6368; --line: #dadce0; --paper: #fafafa; --accent: #6741d9; }
* { box-sizing: border-box; }
body { margin: 0; font: 15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; color: var(--ink); background: var(--paper); }
a { color: var(--accent); }
code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.9em; overflow-wrap: anywhere; }
.masthead { padding: 24px 16px 8px; max-width: 1400px; margin: 0 auto; }
.masthead h1 { margin: 0 0 8px; font-size: 1.6rem; }
.vs { color: var(--muted); font-weight: 400; }
.facts { display: flex; flex-wrap: wrap; gap: 8px 24px; margin: 12px 0; }
.facts div { min-width: 0; }
.facts dt { font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); }
.facts dd { margin: 0; overflow-wrap: anywhere; }
.note { color: var(--muted); }
.layout { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 24px; max-width: 1400px; margin: 0 auto; padding: 0 16px 48px; }
.toc { position: sticky; top: 0; align-self: start; max-height: 100vh; overflow-y: auto; padding: 16px 0; font-size: 0.9rem; }
.toc h2 { font-size: 1rem; margin: 0 0 4px; }
.toc h3 { font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); margin: 12px 0 2px; }
.toc ul { list-style: none; margin: 0; padding: 0; }
.toc a { display: block; padding: 2px 0; text-decoration: none; }
main { min-width: 0; }
.group { margin: 32px 0 8px; padding-bottom: 4px; border-bottom: 2px solid var(--ink); font-size: 1.3rem; }
.example { scroll-margin-top: 56px; padding: 16px 0 24px; border-bottom: 1px solid var(--line); }
.example h3 { margin: 0; font-size: 1.15rem; }
.caption { margin: 4px 0 8px; color: var(--muted); }
.variant h4 { margin: 16px 0 8px; font-size: 0.95rem; }
.pair { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; margin-bottom: 12px; }
figure { margin: 0; min-width: 0; }
figcaption { font-size: 0.8rem; font-weight: 600; color: var(--muted); margin-bottom: 4px; }
.render { border: 1px solid var(--line); padding: 8px; overflow: hidden;
  background: repeating-conic-gradient(#e8e8e8 0 25%, #ffffff 0 50%) 0 0 / 16px 16px; }
.render img { display: block; max-width: 100%; height: auto; }
.downloads { display: flex; flex-wrap: wrap; gap: 4px 16px; margin: 4px 0 8px; font-size: 0.9rem; }
details { margin: 4px 0 0; font-size: 0.9rem; }
summary { cursor: pointer; color: var(--muted); }
pre { background: #f1f3f4; padding: 8px; overflow-x: auto; font-size: 0.8rem; margin: 4px 0 8px; }
@media (max-width: 760px) {
  .layout { display: block; }
  .toc { position: sticky; top: 0; z-index: 1; max-height: none; background: var(--paper); border-bottom: 1px solid var(--line);
    display: flex; gap: 12px; overflow-x: auto; white-space: nowrap; padding: 8px 0; }
  .toc h2, .toc h3 { display: none; }
  .toc ul { display: flex; gap: 12px; }
  .pair { grid-template-columns: minmax(0, 1fr); }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_options_map_to_cli_flags() {
        let options = json!({
            "exportBackground": false, "exportEmbedScene": true, "exportPadding": 0,
            "exportScale": null, "exportWithDarkMode": true,
            "frameRendering": {"enabled": true, "clip": false, "name": true, "outline": false},
            "maxWidthOrHeight": 160, "viewBackgroundColor": "#fffce8"
        });
        assert_eq!(
            export_args(&options).unwrap(),
            [
                "--compatibility-revision",
                "1",
                "--no-background",
                "--embed-scene",
                "--padding",
                "0",
                "--dark-mode",
                "--no-frame-clipping",
                "--no-frame-outlines",
                "--max-dimension",
                "160",
            ]
        );
    }

    #[test]
    fn export_options_without_a_cli_flag_are_rejected() {
        let options = json!({"exportScale": 1, "maxWidthOrHeight": null, "exportingFrame": "f"});
        assert!(export_args(&options)
            .unwrap_err()
            .contains("exportingFrame"));
        let both = json!({"exportScale": 1, "maxWidthOrHeight": 160});
        assert!(export_args(&both).is_err());
    }

    #[test]
    fn html_text_is_escaped_and_svg_lengths_round_up() {
        assert_eq!(
            escape(r#"<a href="x">&'"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        assert_eq!(css_pixels("173.8"), "174");
        assert_eq!(css_pixels("110px"), "110");
    }
}
