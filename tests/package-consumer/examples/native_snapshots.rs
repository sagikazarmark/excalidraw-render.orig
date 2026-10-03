//! Native-only development collector. Depends exclusively on the packaged public facade.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use excalidraw_render::{
    ColorMode, FrameOptions, FrameRenderRequest, MetadataOptions, OutputFormat, RenderRequest,
    Renderer, SceneDocument, Sizing,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const INVENTORY: &str = "tests/conformance/coverage/inventory.json";
/// The packaged sources the consumer resolves. Font package versions carry the
/// Excalidraw release as build metadata.
const PACKAGES: [(&str, &str); 4] = [
    ("excalidraw-render", "0.1.0"),
    ("excalidraw-fonts", "0.1.0+excalidraw.0.18.1"),
    ("excalidraw-fonts-xiaolai", "0.1.0+excalidraw.0.18.1"),
    (
        "excalidraw-fonts-xiaolai-extended",
        "0.1.0+excalidraw.0.18.1",
    ),
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("snapshot collection failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [mode, root, output] if mode == "collect" => collect(Path::new(root), Path::new(output)),
        [mode, root, output, order] if mode == "worker" => {
            worker(Path::new(root), Path::new(output), order)
        }
        _ => Err("usage: native_snapshots collect <repository> <new-output-directory>".into()),
    }
}

fn io<T>(result: std::io::Result<T>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

fn hash(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&io(fs::read(path))?).map_err(|e| e.to_string())
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("missing string {key}"))
}

fn safe_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("unsafe repository path {relative}"));
    }
    let path = io(root.join(path).canonicalize())?;
    if !path.starts_with(root) {
        return Err("repository path escapes root".into());
    }
    Ok(path)
}

fn command(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = io(Command::new(args[0])
        .args(&args[1..])
        .current_dir(root)
        .output())?;
    if !output.status.success() {
        return Err(format!(
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().into())
        .map_err(|e| e.to_string())
}

fn describe_file(root: &Path, path: &str) -> Result<Value, String> {
    let bytes = io(fs::read(safe_path(root, path)?))?;
    Ok(json!({"path":path,"sha256":hash(&bytes),"byte_length":bytes.len()}))
}

fn collect(root: &Path, output: &Path) -> Result<(), String> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("canonical native snapshots require Linux x86-64".into());
    }
    let root = io(root.canonicalize())?;
    let version = command(&root, &["rustc", "--version"])?;
    let tools = read_json(&root.join("tests/package-consumer/tool-manifest.json"))?;
    let pinned = text(&tools["rust_toolchain"], "version")?;
    if !version.starts_with(&format!("rustc {pinned} ")) {
        return Err(format!("expected rustc {pinned}, found {version}"));
    }
    let commit = command(&root, &["git", "rev-parse", "HEAD"])?;
    let metadata: Value = serde_json::from_str(&command(
        &root.join("tests/package-consumer"),
        &["cargo", "metadata", "--locked", "--format-version", "1"],
    )?)
    .map_err(|e| e.to_string())?;
    for (package, version) in PACKAGES {
        let matches: Vec<_> = metadata["packages"]
            .as_array()
            .ok_or("missing Cargo packages")?
            .iter()
            .filter(|p| p["name"] == package)
            .collect();
        let [resolved] = matches.as_slice() else {
            return Err(format!("ambiguous package resolution: {package}"));
        };
        let expected = safe_path(
            &root,
            &format!("target/package/{package}-{version}/Cargo.toml"),
        )?;
        if io(Path::new(text(resolved, "manifest_path")?).canonicalize())? != expected {
            return Err(format!(
                "consumer did not resolve packaged source for {package}"
            ));
        }
    }
    // The collector is useful on WIP, but never labels WIP as clean commit evidence.
    let status = command(
        &root,
        &["git", "status", "--porcelain", "--untracked-files=all"],
    )?;
    let mut packages = Vec::new();
    for (package, version) in PACKAGES {
        packages.push(describe_file(
            &root,
            &format!("target/package/{package}-{version}.crate"),
        )?);
    }
    // Refuse overwrite: a failed collection stays inspectable, never a partial success.
    io(fs::create_dir(output))?;
    let output = io(output.canonicalize())?;
    io(fs::create_dir(output.join("blobs")))?;
    io(fs::create_dir(output.join("packages")))?;
    for package in &packages {
        let path = text(package, "path")?;
        let name = Path::new(path).file_name().ok_or("invalid package name")?;
        io(fs::copy(
            safe_path(&root, path)?,
            output.join("packages").join(name),
        ))?;
    }
    let executable = io(std::env::current_exe())?;
    let mut runs = Vec::new();
    for order in ["forward", "reverse"] {
        let child = io(Command::new(&executable)
            .arg("worker")
            .arg(&root)
            .arg(&output)
            .arg(order)
            .spawn())?;
        let pid = child.id();
        let result = io(child.wait_with_output())?;
        if !result.status.success() {
            return Err(format!("{order} worker exited {}", result.status));
        }
        let run = read_json(&output.join(format!("{order}.json")))?;
        if run["process_id"] != json!(format!("native-{order}-{pid}")) {
            return Err("worker process identity does not match spawned child".into());
        }
        runs.push(run);
    }
    let forward = runs[0]["results"]
        .as_array()
        .ok_or("missing forward results")?;
    let reverse = runs[1]["results"]
        .as_array()
        .ok_or("missing reverse results")?;
    if forward.len() != reverse.len() || !forward.iter().eq(reverse.iter().rev()) {
        return Err("forward/reverse process output mismatch".into());
    }
    let count = forward.len();
    let manifest = json!({
        "schema":"excalidraw-render/native-snapshots/v1", "compatibility_revision":1,
        "target":"x86_64-unknown-linux-gnu", "approval":null,
        "commit":commit,"source_status":status,"rustc":version,
        "inventory":describe_file(&root,INVENTORY)?,
        "tool_manifest":describe_file(&root,"tests/package-consumer/tool-manifest.json")?,
        "dagger_lock":describe_file(&root,"dagger.lock")?,
        "consumer_lock":describe_file(&root,"tests/package-consumer/Cargo.lock")?,
        "consumer_manifest":describe_file(&root,"tests/package-consumer/Cargo.toml")?,
        "consumer_config":describe_file(&root,"tests/package-consumer/.cargo/config.toml")?,
        "collector":describe_file(&root,"tests/package-consumer/examples/native_snapshots.rs")?,
        "packages":packages,"runs":runs,
    });
    write_json(&output.join("snapshots.json"), &manifest)?;
    eprintln!("collected {count} keys, two sessions in each of two processes");
    Ok(())
}

fn worker(root: &Path, output: &Path, order: &str) -> Result<(), String> {
    if !matches!(order, "forward" | "reverse") {
        return Err("invalid worker order".into());
    }
    let root = io(root.canonicalize())?;
    let inventory = read_json(&root.join(INVENTORY))?;
    if inventory["schema"] != "excalidraw-render/coverage-inventory/v1"
        || inventory["compatibility_revision"] != 1
    {
        return Err("unsupported inventory".into());
    }
    let registry = read_json(&root.join("tests/conformance/cases/registry.json"))?;
    let mut scenes = BTreeMap::new();
    for case in registry["cases"].as_array().ok_or("missing cases")? {
        let id = text(case, "id")?;
        let dir = text(case, "path")?;
        let manifest = read_json(&safe_path(&root, &format!("{dir}/manifest.json"))?)?;
        let bytes = io(fs::read(safe_path(
            &root,
            &format!("{dir}/{}", text(&manifest["scene"], "path")?),
        )?))?;
        if hash(&bytes) != text(&manifest["scene"], "sha256")? {
            return Err(format!("stale scene {id}"));
        }
        let scene = SceneDocument::parse(&bytes).map_err(|e| format!("{id}: {e}"))?;
        if scenes.insert(id.to_owned(), scene).is_some() {
            return Err("duplicate case".into());
        }
    }
    let mut executions = inventory["executions"]
        .as_array()
        .ok_or("missing executions")?
        .clone();
    if executions.is_empty() {
        return Err("empty execution set".into());
    }
    if order == "reverse" {
        executions.reverse();
    }
    let mut results = Vec::new();
    for execution in executions {
        let key = &execution["key"];
        let scene = scenes
            .get(text(key, "case")?)
            .ok_or("unknown execution case")?;
        let request = map_request(key, &execution["options"]).map_err(|e| format!("{key}: {e}"))?;
        let mut samples = Vec::new();
        for _ in 0..2 {
            // Every public render call creates a fresh render session.
            let renderer = Renderer::new();
            let rendered = match &request {
                Request::Scene(request) => renderer.render(scene, request),
                Request::Frame(ordinal, request) => {
                    let frame = scene
                        .frames()
                        .find(|f| u64::from(f.ordinal()) == *ordinal)
                        .ok_or("frame ordinal absent from scene")?;
                    renderer.render_frame(frame, request)
                }
            };
            let sample = match rendered {
                Ok(outcome) => {
                    json!({"outcome":{"kind":"artifact","bytes":store(output,outcome.artifact().bytes())?},
                    "report":store(output,&serde_json::to_vec(outcome.report()).map_err(|e|e.to_string())?)?})
                }
                Err(error) => json!({"outcome":{"kind":"failure","code":error.code().as_str(),
                    "descriptor":store(output,&serde_json::to_vec(&error.descriptor()).map_err(|e|e.to_string())?)?},
                    "report":store(output,&serde_json::to_vec(error.report()).map_err(|e|e.to_string())?)?}),
            };
            samples.push(sample);
        }
        if samples[0] != samples[1] {
            return Err(format!("fresh-session mismatch: {key}"));
        }
        results.push(json!({"key":key,"samples":samples}));
    }
    write_json(
        &output.join(format!("{order}.json")),
        &json!({"order":order,
        "process_id":format!("native-{order}-{}",std::process::id()),"results":results}),
    )
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    io(fs::write(path, bytes))
}

fn store(output: &Path, bytes: &[u8]) -> Result<Value, String> {
    let sha = hash(bytes);
    let path = output.join("blobs").join(&sha);
    if path.exists() {
        if io(fs::read(&path))? != bytes {
            return Err("blob digest collision".into());
        }
    } else {
        io(fs::write(&path, bytes))?;
    }
    Ok(json!({"sha256":sha,"byte_length":bytes.len()}))
}

enum Request {
    Scene(RenderRequest),
    Frame(u64, FrameRenderRequest),
}

fn map_request(key: &Value, options: &Value) -> Result<Request, String> {
    let format = match text(key, "format")? {
        "svg" => OutputFormat::Svg,
        "png" => OutputFormat::Png,
        "pdf" => OutputFormat::Pdf,
        _ => return Err("unknown format".into()),
    };
    let frame = match text(&key["export"], "kind")? {
        "scene" => None,
        "frame" => Some(
            key["export"]["ordinal"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or("invalid frame ordinal")?,
        ),
        _ => return Err("unknown export kind".into()),
    };
    let mut allowed = BTreeSet::from([
        "exportBackground",
        "exportEmbedScene",
        "exportScale",
        "exportWithDarkMode",
        "maxWidthOrHeight",
        "viewBackgroundColor",
    ]);
    if frame.is_none() {
        allowed.extend(["exportPadding", "frameRendering"]);
    }
    let object = options.as_object().ok_or("options must be an object")?;
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != allowed {
        return Err("unknown or missing request options".into());
    }
    if options["viewBackgroundColor"] != "#ffffff" {
        return Err("background override is not supported by the collector".into());
    }
    let boolean = |name| {
        options[name]
            .as_bool()
            .ok_or_else(|| format!("invalid boolean {name}"))
    };
    let sizing = match (
        options["exportScale"].as_f64(),
        options["maxWidthOrHeight"].as_f64(),
    ) {
        (Some(scale), None) if options["maxWidthOrHeight"].is_null() => Sizing::Scale(scale),
        (None, Some(max)) if options["exportScale"].is_null() => Sizing::MaxDimension(max),
        _ => return Err("exactly one numeric sizing mode is required".into()),
    };
    let color = if boolean("exportWithDarkMode")? {
        ColorMode::Dark
    } else {
        ColorMode::Light
    };
    let metadata = MetadataOptions::new(boolean("exportEmbedScene")?, false, false);
    if let Some(ordinal) = frame {
        // Frame API fixes padding at zero and ignores scene frame chrome controls.
        return FrameRenderRequest::builder(format)
            .include_background(boolean("exportBackground")?)
            .sizing(sizing)
            .color_mode(color)
            .metadata_options(metadata)
            .build()
            .map(|r| Request::Frame(ordinal, r))
            .map_err(|e| e.to_string());
    }
    let controls = options["frameRendering"]
        .as_object()
        .ok_or("missing frame controls")?;
    if controls.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != BTreeSet::from(["enabled", "clip", "name", "outline"])
    {
        return Err("unknown or missing frame controls".into());
    }
    let control = |name| {
        controls[name]
            .as_bool()
            .ok_or_else(|| format!("invalid frame control {name}"))
    };
    RenderRequest::builder(format)
        .include_background(boolean("exportBackground")?)
        .padding(options["exportPadding"].as_f64().ok_or("invalid padding")?)
        .sizing(sizing)
        .color_mode(color)
        .metadata_options(metadata)
        .frame_options(FrameOptions::new(
            control("enabled")?,
            control("clip")?,
            control("name")?,
            control("outline")?,
        ))
        .build()
        .map(Request::Scene)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Value {
        json!({"exportBackground":true,"exportEmbedScene":true,"exportScale":2,
        "exportWithDarkMode":true,"maxWidthOrHeight":null,"viewBackgroundColor":"#ffffff",
        "exportPadding":7,"frameRendering":{"enabled":true,"clip":false,"name":false,"outline":true}})
    }

    #[test]
    fn maps_scene_options_and_frame_zero_padding() {
        let key = json!({"format":"svg","export":{"kind":"scene"}});
        let Request::Scene(r) = map_request(&key, &options()).unwrap() else {
            panic!()
        };
        assert_eq!(r.padding(), 7.0);
        assert_eq!(r.sizing(), Sizing::Scale(2.0));
        assert_eq!(r.color_mode(), ColorMode::Dark);
        assert!(r.metadata_options().embedded_scene());
        assert!(!r.frame_options().clip());
        let mut options = options();
        options.as_object_mut().unwrap().remove("frameRendering");
        options.as_object_mut().unwrap().remove("exportPadding");
        assert!(matches!(
            map_request(
                &json!({"format":"pdf","export":{"kind":"frame","ordinal":2}}),
                &options
            )
            .unwrap(),
            Request::Frame(2, _)
        ));
    }

    #[test]
    fn rejects_ambiguous_or_silently_ignored_options() {
        let key = json!({"format":"png","export":{"kind":"scene"}});
        for (name, value) in [
            ("unknown", json!(true)),
            ("maxWidthOrHeight", json!(100)),
            ("exportScale", json!("2")),
            ("viewBackgroundColor", json!("red")),
            ("exportPadding", json!(-1)),
        ] {
            let mut options = options();
            options[name] = value;
            assert!(map_request(&key, &options).is_err(), "accepted {name}");
        }
    }
}
