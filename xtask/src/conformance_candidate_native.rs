//! Fresh-process native evidence and replay. This is also the executable side
//! of the supplemental representative corpus; static registry rows are not runs.
use crate::conformance::{canonical_bytes, repo_root, sha256};
use crate::conformance_candidate_gate::{descriptor, descriptor_set, read, value};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
type Result<T> = std::result::Result<T, String>;
pub const FORMATS: [&str; 3] = ["svg", "png", "pdf"];
const BUILD_ARGS: [&str; 6] = [
    "build",
    "--release",
    "--locked",
    "-p",
    "excalidraw-render-cli",
    "--message-format=json-render-diagnostics",
];
pub const OPTIONS: [&str; 10] = [
    "--compatibility-revision",
    "1",
    "--padding",
    "0",
    "--no-background",
    "--no-frame-names",
    "--no-frame-outlines",
    "--embed-scene",
    "--embed-provenance",
    "--embed-diagnostics",
];

fn tree(at: &Path, paths: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(at).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err(format!("producer symlink {}", entry.path().display()));
        }
        if kind.is_dir() {
            if !matches!(name.to_str(), Some("target" | "tests" | "benches")) {
                tree(&entry.path(), paths)?;
            }
        } else {
            paths.push(
                entry
                    .path()
                    .strip_prefix(repo_root())
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("producer path encoding")?
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}
pub fn producers() -> Result<Value> {
    let mut paths = vec![
        "Cargo.toml".into(),
        "Cargo.lock".into(),
        "docs/renderer-spec-v1.md".into(),
    ];
    for name in [
        "excalidraw-render",
        "excalidraw-render-cli",
        "excalidraw-render-assets-base",
        "excalidraw-render-assets-data-000",
        "excalidraw-render-assets-data-001",
    ] {
        tree(&repo_root().join("crates").join(name), &mut paths)?;
    }
    for name in [".cargo", "rust-toolchain", "rust-toolchain.toml"] {
        let p = repo_root().join(name);
        if p.is_dir() {
            tree(&p, &mut paths)?;
        } else if p.is_file() {
            paths.push(name.into());
        }
    }
    paths.sort();
    paths
        .iter()
        .map(|p| descriptor(&repo_root(), p))
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}
pub fn check_producers(record: &Value) -> Result<()> {
    if record != &producers()? {
        return Err("stale native production/source bindings".into());
    }
    Ok(())
}
fn tool(program: &str, args: &[&str]) -> Result<String> {
    let o = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(format!("{program}: {}", String::from_utf8_lossy(&o.stderr)));
    }
    String::from_utf8(o.stdout).map_err(|e| e.to_string())
}
fn release_command() -> Command {
    let mut command = Command::new("cargo");
    command
        .current_dir(repo_root())
        .args(BUILD_ARGS)
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_RELEASE_DEBUG", "0")
        .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
        .env("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS", "false")
        .env("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS", "false")
        .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1")
        .env("CARGO_PROFILE_RELEASE_LTO", "true")
        .env("CARGO_PROFILE_RELEASE_PANIC", "abort")
        .env("RUSTFLAGS", "")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTC")
        .env_remove("CARGO_BUILD_RUSTC");
    command
}
fn build_command_identity() -> Value {
    json!(std::iter::once("cargo")
        .chain(BUILD_ARGS)
        .collect::<Vec<_>>())
}
pub(crate) fn validate_build_identity(identity: &Value) -> Result<()> {
    if identity["profile"] != "release"
        || identity["build_command"] != build_command_identity()
        || identity["incremental"] != false
        || identity["debug"] != 0
        || identity["rustflags"] != json!([])
        || identity["codegen_units"] != 1
        || identity["lto"] != true
        || identity["panic"] != "abort"
        || identity["cargo_profile"]
            != json!({"opt_level":"3","debuginfo":0,"debug_assertions":false,"overflow_checks":false,"test":false})
    {
        return Err("native capture requires release --locked qualification and its exact release profile identity".into());
    }
    Ok(())
}
pub fn build() -> Result<(PathBuf, Value)> {
    let rustc = tool("rustc", &["-vV"])?;
    if !rustc.starts_with("rustc 1.97.1 ")
        || !rustc.lines().any(|s| s == "host: x86_64-unknown-linux-gnu")
    {
        return Err("native candidate replay requires pinned Rust 1.97.1 Linux x86_64".into());
    }
    let cargo = tool("cargo", &["--version"])?;
    let result = release_command().output().map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "native producer build failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    let (executable, cargo_profile) = String::from_utf8_lossy(&result.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find_map(|v| {
            if v["reason"] == "compiler-artifact" && v["target"]["name"] == "excalidraw-render" {
                v["executable"]
                    .as_str()
                    .map(|p| (PathBuf::from(p), v["profile"].clone()))
            } else {
                None
            }
        })
        .ok_or("Cargo did not return the native CLI executable")?;
    let identity = json!({"rustc":rustc.trim(),"cargo":cargo.trim(),"executable_sha256":sha256(&fs::read(&executable).map_err(|e|e.to_string())?),"profile":"release","build_command":build_command_identity(),"cargo_profile":cargo_profile,"incremental":false,"debug":0,"rustflags":[],"codegen_units":1,"lto":true,"panic":"abort"});
    validate_build_identity(&identity)?;
    Ok((executable, identity))
}
pub fn render(
    executable: &Path,
    source: &[u8],
    format: &str,
    options: &[&str],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut child = Command::new(executable)
        .args([
            "-",
            "--output",
            "-",
            "--format",
            format,
            "--report-format",
            "json",
        ])
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("native stdin")?
        .write_all(source)
        .map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "native {format} replay failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok((output.stdout, output.stderr))
}
fn report(bytes: &[u8], artifact: &[u8], format: &str, expected: &Value) -> Result<()> {
    let actual: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("native {format} report: {e}"))?;
    let media = match format {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "pdf" => "application/pdf",
        _ => return Err("unsupported native format".into()),
    };
    let wanted = json!({"schema_version":1,"status":"success","renderer_version":env!("CARGO_PKG_VERSION"),"compatibility_revision":1,"excalidraw_release":"v0.18.1","reference_profile_digest":"sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71","format":format,"artifact":{"media_type":media,"byte_length":artifact.len(),"sha256":format!("sha256:{}",sha256(artifact))},"error":null,"diagnostics":expected["diagnostics"]});
    if expected != &json!({"schema_version":1,"diagnostics":[]}) || actual != wanted {
        return Err(format!(
            "native {format} report differs from the independently authored contract"
        ));
    }
    Ok(())
}
pub fn capture(directory: &str) -> Result<Value> {
    let root = crate::conformance_candidate_gate::candidate_root(directory)?;
    for name in [
        "native-capture.json",
        "native.svg",
        "native.png",
        "native.pdf",
    ] {
        if root.join(name).exists() {
            return Err(format!("refusing to overwrite native evidence {name}"));
        }
    }
    let before = producers()?;
    let (executable, build) = build()?;
    let source = read(&root, "scene.json")?;
    let mut rows = Vec::new();
    let mut outputs = Vec::new();
    for format in FORMATS {
        let expected = value(&root, &format!("expected-report-{format}.json"))?;
        let first = render(&executable, &source, format, &OPTIONS)?;
        report(&first.1, &first.0, format, &expected)?;
        let second = render(&executable, &source, format, &OPTIONS)?;
        if first != second {
            return Err(format!(
                "native {format} is not fresh-process byte/report stable"
            ));
        }
        rows.push(json!({"format":format,"fresh_process_runs":2,"artifact":{"path":format!("native.{format}"),"byte_length":first.0.len(),"sha256":sha256(&first.0)},"report":{"path":format!("native-{format}-report.json"),"byte_length":first.1.len(),"sha256":sha256(&first.1)}}));
        outputs.push((format, first));
    }
    check_producers(&before)?;
    if source != read(&root, "scene.json")? {
        return Err("scene changed during capture".into());
    }
    for (format, (artifact, report)) in outputs {
        write_new(&root.join(format!("native.{format}")), &artifact)?;
        write_new(&root.join(format!("native-{format}-report.json")), &report)?;
    }
    let record = json!({"schema":"excalidraw-render/candidate-native-capture/v1","source":descriptor(&root,"scene.json")?,"request_args":OPTIONS,"producer_sources":before,"build":build,"rows":rows});
    write_new(
        &root.join("native-capture.json"),
        &[canonical_bytes(&record)?, b"\n".to_vec()].concat(),
    )?;
    Ok(record)
}
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(bytes).map_err(|e| e.to_string())
}

pub fn replay(directory: &str) -> Result<Value> {
    let root = crate::conformance_candidate_gate::candidate_root(directory)?;
    let record = value(&root, "native-capture.json")?;
    if record["schema"] != "excalidraw-render/candidate-native-capture/v1"
        || record["request_args"] != json!(OPTIONS)
        || record["source"] != descriptor(&root, "scene.json")?
    {
        return Err("native capture source/request/schema mismatch".into());
    }
    validate_build_identity(&record["build"])?;
    check_producers(&record["producer_sources"])?;
    let (executable, build) = build()?;
    if record["build"] != build {
        return Err("stale native compiler/executable identity".into());
    }
    let source = read(&root, "scene.json")?;
    let rows = record["rows"].as_array().ok_or("native capture rows")?;
    if rows.len() != 3 {
        return Err("native replay requires all three formats".into());
    }
    let mut results = Vec::new();
    for (format, row) in FORMATS.into_iter().zip(rows) {
        if row["format"] != format || row["fresh_process_runs"] != 2 {
            return Err("missing/duplicate native execution format".into());
        }
        descriptor_set(
            &root,
            &json!([row["artifact"], row["report"]]),
            &[
                format!("native.{format}"),
                format!("native-{format}-report.json"),
            ],
        )?;
        let wanted = (
            read(&root, &format!("native.{format}"))?,
            read(&root, &format!("native-{format}-report.json"))?,
        );
        let expected = value(&root, &format!("expected-report-{format}.json"))?;
        for _ in 0..2 {
            let actual = render(&executable, &source, format, &OPTIONS)?;
            report(&actual.1, &actual.0, format, &expected)?;
            check_output(&actual, &wanted, format)?;
        }
        results.push(json!({"format":format,"fresh_process_runs":2,"artifact_sha256":sha256(&wanted.0),"report_sha256":sha256(&wanted.1)}));
    }
    check_producers(&record["producer_sources"])?;
    if source != read(&root, "scene.json")? {
        return Err("scene changed during replay".into());
    }
    Ok(
        json!({"schema":"excalidraw-render/candidate-native-replay/v1","build":build,"rows":results}),
    )
}
fn check_output(
    actual: &(Vec<u8>, Vec<u8>),
    wanted: &(Vec<u8>, Vec<u8>),
    format: &str,
) -> Result<()> {
    if actual.0 != wanted.0 {
        return Err(format!("stale native {format} artifact"));
    }
    if actual.1 != wanted.1 {
        return Err(format!("stale native {format} report"));
    }
    Ok(())
}

/// Qualify the one user-authorized native Excalifont byte-canary change against
/// a fresh, independently rendered official canvas. No approved evidence moves.
pub fn qualify_text_canary(directory: &str) -> Result<Value> {
    let root = crate::conformance_candidate_gate::candidate_root(directory)?;
    let provenance = value(&root, "provenance.json")?;
    if provenance["fresh_process_runs"] != 2
        || provenance["browser"] != "149.0.7827.55"
        || provenance["width"] != 118
        || provenance["height"] != 102
    {
        return Err("unqualified canary reference".into());
    }
    for source in provenance["sources"]
        .as_array()
        .ok_or("canary reference producer chain")?
    {
        let path = source["path"].as_str().ok_or("source path")?;
        if source["sha256"] != sha256(&read(&repo_root(), path)?) {
            return Err(format!("stale canary reference producer {path}"));
        }
    }
    let before = producers()?;
    let (executable, build) = build()?;
    let source = read(&root, "scene.json")?;
    let mut png_bytes = Vec::new();
    let mut rows = Vec::new();
    for (format, expected) in [
        (
            "svg",
            "25ce18f298281f6a899de079bd4e065d3f0e62d0e863fb431f8e55cc3350a90c",
        ),
        (
            "png",
            "9b2da5d13b6a2c672ab80be51edbeeb9491195b57939dea9f48e80e3887f3b95",
        ),
        (
            "pdf",
            "ed5dd7b2faf6fcd7c56c6afd5f8adbfb01d4326079414de7b16ef9d53cc2c6bb",
        ),
    ] {
        let a = render(&executable, &source, format, &[])?;
        let b = render(&executable, &source, format, &[])?;
        if a != b || sha256(&a.0) != expected {
            return Err(format!("canary {format} differs from the independently asserted native sparse-scene canary"));
        }
        report(
            &a.1,
            &a.0,
            format,
            &json!({"schema_version":1,"diagnostics":[]}),
        )?;
        write_new(&root.join(format!("native.{format}")), &a.0)?;
        if format == "png" {
            png_bytes = a.0;
        }
        rows.push(json!({"format":format,"sha256":expected,"fresh_process_runs":2}));
    }
    let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).map_err(|e| e.to_string())?;
    if (info.width, info.height) != (118, 102)
        || info.color_type != png::ColorType::Rgba
        || info.bit_depth != png::BitDepth::Eight
    {
        return Err("canary pixel format/extent".into());
    }
    use excalidraw_render_compare::{CanonicalRgba8, ComparisonMode, SsimOutcome, VisualClass};
    let reference = CanonicalRgba8::new(118, 102, read(&root, "official-png.rgba")?)
        .map_err(|e| e.to_string())?;
    let actual = CanonicalRgba8::new(118, 102, rgba).map_err(|e| e.to_string())?;
    let comparison = excalidraw_render_compare::compare(
        &reference,
        &actual,
        VisualClass::Typography,
        &[],
        ComparisonMode::Canonical,
    )
    .map_err(|e| e.to_string())?;
    if !comparison.passed() {
        return Err(format!(
            "independent rotated canary typography budget failed: {:?}",
            comparison.whole
        ));
    }
    check_producers(&before)?;
    let result = json!({"schema":"excalidraw-render/excalifont-canary-qualification/v1","build":build,"native_producers":before,"reference_provenance":descriptor(&root,"provenance.json")?,"rows":rows,"changed_pixels":comparison.whole.changed_pixels,"ssim_bits":match comparison.whole.ssim{SsimOutcome::Score(s)=>format!("{:016x}",s.to_bits()),_=>return Err("canary SSIM".into())},"canonical_typography_passed":true,"old_native_png_sha256":"cb90c943bc71d5eec983bf2c933e1c3c16dd4ca7b5fc14ba79f0230404209534","new_native_png_sha256":"9b2da5d13b6a2c672ab80be51edbeeb9491195b57939dea9f48e80e3887f3b95"});
    write_new(&root.join("qualification.json"), &canonical_bytes(&result)?)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_qualification_binds_actual_command_and_profile() {
        let command = release_command();
        assert_eq!(
            command
                .get_args()
                .map(|a| a.to_str().unwrap())
                .collect::<Vec<_>>(),
            BUILD_ARGS
        );
        let identity = json!({"profile":"release","build_command":build_command_identity(),"cargo_profile":{"opt_level":"3","debuginfo":0,"debug_assertions":false,"overflow_checks":false,"test":false},"incremental":false,"debug":0,"rustflags":[],"codegen_units":1,"lto":true,"panic":"abort"});
        validate_build_identity(&identity).unwrap();
        for mutation in 0..10 {
            let mut bad = identity.clone();
            match mutation {
                0 => bad["profile"] = json!("dev"),
                1 => {
                    bad["build_command"].as_array_mut().unwrap().remove(2);
                }
                2 => {
                    bad["build_command"].as_array_mut().unwrap().remove(3);
                }
                3 => bad["cargo_profile"]["opt_level"] = json!("0"),
                4 => bad["cargo_profile"]["debug_assertions"] = json!(true),
                5 => bad["cargo_profile"]["test"] = json!(true),
                6 => bad["rustflags"] = json!(["-Copt-level=0"]),
                7 => bad["lto"] = json!(false),
                8 => bad["panic"] = json!("unwind"),
                _ => bad["incremental"] = json!(true),
            }
            assert!(
                validate_build_identity(&bad).is_err(),
                "mutation {mutation}"
            );
        }
        let old = value(
            &repo_root(),
            "tests/conformance/candidates/stage4-frames-text-assets-v6/native-capture.json",
        )
        .unwrap();
        assert!(validate_build_identity(&old["build"])
            .unwrap_err()
            .contains("release --locked"));
    }
    #[test]
    fn fresh_native_replay_rejects_rehashed_artifacts_reports_sources_and_builds() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = format!(
            "target/stage4-native-replay-test-{}-{nonce}",
            std::process::id()
        );
        let root = repo_root().join(&directory);
        fs::create_dir(&root).unwrap();
        let original =
            repo_root().join("tests/conformance/candidates/stage4-frames-text-assets-v4");
        for name in [
            "scene.json",
            "expected-report-svg.json",
            "expected-report-png.json",
            "expected-report-pdf.json",
        ] {
            fs::write(root.join(name), read(&original, name).unwrap()).unwrap();
        }
        let base = capture(&directory).unwrap();
        let result = replay(&directory).unwrap();
        assert_eq!(result["rows"].as_array().unwrap().len(), 3);
        // Rehash the corrupted file in the stored capture, so these controls
        // must reach and fail the actual fresh producer, not just a hash check.
        for (name, key) in [
            ("native.svg", "artifact"),
            ("native-svg-report.json", "report"),
        ] {
            let before = read(&root, name).unwrap();
            let mut changed = before.clone();
            changed.push(b' ');
            fs::write(root.join(name), changed).unwrap();
            let mut record = base.clone();
            record["rows"][0][key] = descriptor(&root, name).unwrap();
            fs::write(
                root.join("native-capture.json"),
                canonical_bytes(&record).unwrap(),
            )
            .unwrap();
            assert!(replay(&directory)
                .unwrap_err()
                .contains(&format!("stale native svg {key}")));
            fs::write(root.join(name), before).unwrap();
        }
        let mut record = base.clone();
        record["producer_sources"][0]["sha256"] = json!("stale");
        fs::write(
            root.join("native-capture.json"),
            canonical_bytes(&record).unwrap(),
        )
        .unwrap();
        assert!(replay(&directory)
            .unwrap_err()
            .contains("production/source"));
        let mut record = base.clone();
        record["build"]["executable_sha256"] = json!("stale");
        fs::write(
            root.join("native-capture.json"),
            canonical_bytes(&record).unwrap(),
        )
        .unwrap();
        assert!(replay(&directory)
            .unwrap_err()
            .contains("compiler/executable"));
        fs::write(
            root.join("native-capture.json"),
            canonical_bytes(&base).unwrap(),
        )
        .unwrap();
        let mut scene = read(&root, "scene.json").unwrap();
        scene.push(b' ');
        fs::write(root.join("scene.json"), scene).unwrap();
        assert!(replay(&directory)
            .unwrap_err()
            .contains("source/request/schema"));
        fs::write(
            root.join("scene.json"),
            read(&original, "scene.json").unwrap(),
        )
        .unwrap();
        let mut expected = value(&root, "expected-report-svg.json").unwrap();
        expected["diagnostics"] = json!([{"unexpected":true}]);
        fs::write(
            root.join("expected-report-svg.json"),
            canonical_bytes(&expected).unwrap(),
        )
        .unwrap();
        assert!(replay(&directory)
            .unwrap_err()
            .contains("independently authored contract"));
    }
    #[test]
    fn report_and_artifact_are_independent_replay_gates() {
        let valid = (b"artifact".to_vec(), b"report".to_vec());
        check_output(&valid, &valid, "svg").unwrap();
        for which in 0..2 {
            let mut changed = valid.clone();
            if which == 0 {
                changed.0.push(0);
            } else {
                changed.1.push(0);
            }
            assert!(check_output(&changed, &valid, "svg").is_err());
        }
    }
    #[test]
    fn cli_success_alone_does_not_satisfy_the_report_contract() {
        let artifact = b"test";
        let expected = json!({"schema_version":1,"diagnostics":[]});
        let base = json!({"schema_version":1,"status":"success","renderer_version":env!("CARGO_PKG_VERSION"),"compatibility_revision":1,"excalidraw_release":"v0.18.1","reference_profile_digest":"sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71","format":"svg","artifact":{"media_type":"image/svg+xml","byte_length":4,"sha256":format!("sha256:{}",sha256(artifact))},"error":null,"diagnostics":[]});
        report(&canonical_bytes(&base).unwrap(), artifact, "svg", &expected).unwrap();
        for key in [
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
        ] {
            let mut bad = base.clone();
            bad[key] = json!("mutated");
            assert!(
                report(&canonical_bytes(&bad).unwrap(), artifact, "svg", &expected).is_err(),
                "{key}"
            );
        }
    }
}
