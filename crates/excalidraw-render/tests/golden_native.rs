//! The golden bytes of every registered export target and facade scene.
//!
//! Every registered case variant and frame target, and every project-authored
//! facade scene in `tests/conformance/golden/facade-scenes.json` under the
//! default request, in each of the three formats, renders through the public
//! facade twice (forward then reverse order, a fresh `Renderer` each time) and
//! must produce byte for byte the artifact and report recorded in
//! `tests/conformance/golden/native.json`, whose blobs hold the bytes
//! themselves. This is the artifact byte stability promise of ADR-0002 as a
//! per-change gate: an intentional output change is adopted with
//!
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p excalidraw-render --test golden_native
//! ```
//!
//! on any host (ADR-0024: the bytes do not depend on the host), and reviewed as
//! the diff of the record and its blobs.
//!
//! In browser WASM (`just golden-wasm`) the same keys must match the same
//! record, except the PNG differences `tests/conformance/golden/experimental-wasm.json`
//! records for #178, which must appear exactly as recorded and all appear, so
//! the difference can neither grow nor go stale.
//!
//! The scene documents and the execution list are embedded through the
//! generated `golden/inputs.rs`, so the browser build needs no file system; the
//! native test derives both from the registry and the facade scene list and
//! fails until the generated module is current.

#![allow(clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};

use excalidraw_render::{
    ColorMode, FrameOptions, FrameRenderRequest, MetadataOptions, OutputFormat, RenderRequest,
    Renderer, SceneDocument, Sizing,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[path = "golden/inputs.rs"]
mod inputs;

const SCHEMA: &str = "excalidraw-render/golden-native/v1";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Format {
    Svg,
    Png,
    Pdf,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Export {
    Scene {},
    Frame { ordinal: u64 },
}

/// One export target: a case variant or frame target in one format.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct Key {
    case: String,
    variant: String,
    export: Export,
    format: Format,
}

impl Key {
    /// Whether this is the default scene PNG of `case`, the one target a #178
    /// record may name.
    fn is_default_scene_png_of(&self, case: &str) -> bool {
        self.case == case
            && self.variant == "default"
            && self.format == Format::Png
            && self.export == Export::Scene {}
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Execution {
    key: Key,
    options: Value,
}

/// The digest and length that stand for a byte sequence in the record.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ByteDigest {
    sha256: String,
    byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Outcome {
    Artifact {
        bytes: ByteDigest,
    },
    Failure {
        code: String,
        descriptor: ByteDigest,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Sample {
    outcome: Outcome,
    report: ByteDigest,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Expectation {
    key: Key,
    sample: Sample,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    compatibility_revision: u64,
    expectations: Vec<Expectation>,
}

/// A rendered target: its sample and the bytes behind each digest.
struct Measured {
    sample: Sample,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    blobs: Vec<(String, Vec<u8>)>,
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn describe(bytes: &[u8]) -> ByteDigest {
    ByteDigest {
        sha256: sha256(bytes),
        byte_length: bytes.len() as u64,
    }
}

fn executions() -> Vec<Execution> {
    serde_json::from_str(inputs::EXECUTIONS).expect("generated executions parse")
}

enum Request {
    Scene(RenderRequest),
    Frame(u64, FrameRenderRequest),
}

/// The request a registered variant's options describe. Every option the
/// manifests use is mapped; an unknown or missing one fails, so an option the
/// renderer would ignore cannot pass silently.
fn map_request(key: &Key, options: &Value) -> Result<Request, String> {
    let format = match key.format {
        Format::Svg => OutputFormat::Svg,
        Format::Png => OutputFormat::Png,
        Format::Pdf => OutputFormat::Pdf,
    };
    let frame = match key.export {
        Export::Scene {} => None,
        Export::Frame { ordinal } if ordinal > 0 => Some(ordinal),
        Export::Frame { .. } => return Err("invalid frame ordinal".into()),
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
        return Err("background override is not mapped".into());
    }
    let boolean = |name: &str| {
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
    let metadata = MetadataOptions::default().with_embedded_scene(boolean("exportEmbedScene")?);
    if let Some(ordinal) = frame {
        // The frame API fixes padding at zero and has no scene frame chrome.
        return FrameRenderRequest::builder(format)
            .include_background(boolean("exportBackground")?)
            .sizing(sizing)
            .color_mode(color)
            .metadata_options(metadata)
            .build()
            .map(|request| Request::Frame(ordinal, request))
            .map_err(|error| error.to_string());
    }
    let controls = options["frameRendering"]
        .as_object()
        .ok_or("missing frame controls")?;
    if controls.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != BTreeSet::from(["enabled", "clip", "name", "outline"])
    {
        return Err("unknown or missing frame controls".into());
    }
    let control = |name: &str| {
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
        .frame_options(
            FrameOptions::default()
                .with_enabled(control("enabled")?)
                .with_clip(control("clip")?)
                .with_show_name(control("name")?)
                .with_outline(control("outline")?),
        )
        .build()
        .map(Request::Scene)
        .map_err(|error| error.to_string())
}

/// Renders one target through a fresh renderer.
fn render(execution: &Execution, scenes: &BTreeMap<String, SceneDocument>) -> Measured {
    let key = &execution.key;
    let scene = scenes
        .get(&key.case)
        .unwrap_or_else(|| panic!("{} has an embedded scene", key.case));
    let request = map_request(key, &execution.options)
        .unwrap_or_else(|error| panic!("{key:?} options map to a request: {error}"));
    let renderer = Renderer::new();
    let rendered = match &request {
        Request::Scene(request) => renderer.render(scene, request),
        Request::Frame(ordinal, request) => {
            let frame = scene
                .frames()
                .find(|frame| u64::from(frame.ordinal()) == *ordinal)
                .unwrap_or_else(|| panic!("{key:?}: frame ordinal {ordinal} is in the scene"));
            renderer.render_frame(frame, request)
        }
    };
    let mut blobs = Vec::new();
    let mut store = |bytes: Vec<u8>| {
        let described = describe(&bytes);
        blobs.push((described.sha256.clone(), bytes));
        described
    };
    let sample = match rendered {
        Ok(outcome) => {
            let artifact = store(outcome.artifact().bytes().to_vec());
            let report = store(serde_json::to_vec(outcome.report()).expect("report serializes"));
            Sample {
                outcome: Outcome::Artifact { bytes: artifact },
                report,
            }
        }
        Err(error) => {
            let descriptor =
                store(serde_json::to_vec(&error.descriptor()).expect("descriptor serializes"));
            let report = store(serde_json::to_vec(error.report()).expect("report serializes"));
            Sample {
                outcome: Outcome::Failure {
                    code: error.code().as_str().to_owned(),
                    descriptor,
                },
                report,
            }
        }
    };
    Measured { sample, blobs }
}

fn parse_scenes(executions: &[Execution]) -> BTreeMap<String, SceneDocument> {
    let mut scenes = BTreeMap::new();
    for execution in executions {
        let case = &execution.key.case;
        if scenes.contains_key(case) {
            continue;
        }
        let source = inputs::scene(case)
            .unwrap_or_else(|| panic!("{case} is embedded in golden/inputs.rs; regenerate it"));
        let scene = SceneDocument::parse(source.as_bytes())
            .unwrap_or_else(|error| panic!("{case} parses: {error}"));
        scenes.insert(case.clone(), scene);
    }
    scenes
}

/// Every target rendered once in the given order.
fn measure(reverse: bool) -> Vec<(Key, Measured)> {
    let mut executions = executions();
    assert!(
        !executions.is_empty(),
        "the generated execution list is empty"
    );
    let scenes = parse_scenes(&executions);
    if reverse {
        executions.reverse();
    }
    let mut measured: Vec<_> = executions
        .iter()
        .map(|execution| (execution.key.clone(), render(execution, &scenes)))
        .collect();
    if reverse {
        measured.reverse();
    }
    measured
}

/// Both orders, which must agree on every target.
#[cfg(not(target_arch = "wasm32"))]
fn measure_both_orders() -> Vec<(Key, Measured)> {
    let forward = measure(false);
    let reverse = measure(true);
    assert_eq!(forward.len(), reverse.len());
    for ((key, first), (other, second)) in forward.iter().zip(&reverse) {
        assert_eq!(key, other);
        assert_eq!(
            first.sample, second.sample,
            "{key:?} renders the same bytes in both orders"
        );
    }
    forward
}

/// The differences between a measurement and the record: `(key, expected,
/// actual)`, with `None` for a side that has no entry.
fn differences<'a>(
    record: &'a Record,
    measured: &'a [(Key, Measured)],
) -> Vec<(&'a Key, Option<&'a Sample>, Option<&'a Sample>)> {
    let expected: BTreeMap<_, _> = record
        .expectations
        .iter()
        .map(|expectation| (&expectation.key, &expectation.sample))
        .collect();
    let actual: BTreeMap<_, _> = measured
        .iter()
        .map(|(key, measured)| (key, &measured.sample))
        .collect();
    let keys: BTreeSet<_> = expected.keys().chain(actual.keys()).copied().collect();
    keys.into_iter()
        .filter_map(|key| {
            let (expected, actual) = (expected.get(key).copied(), actual.get(key).copied());
            (expected != actual).then_some((key, expected, actual))
        })
        .collect()
}

fn describe_differences(differences: &[(&Key, Option<&Sample>, Option<&Sample>)]) -> String {
    differences
        .iter()
        .map(|(key, expected, actual)| {
            format!("  {key:?}\n    recorded: {expected:?}\n    actual:   {actual:?}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    const RECORD: &str = "tests/conformance/golden/native.json";
    const BLOBS: &str = "tests/conformance/golden/blobs";
    const EXPERIMENTAL_WASM: &str = "tests/conformance/golden/experimental-wasm.json";
    const INPUTS: &str = "crates/excalidraw-render/tests/golden/inputs.rs";
    const REGISTRY: &str = "tests/conformance/cases/registry.json";
    const FACADE_SCENES: &str = "tests/conformance/golden/facade-scenes.json";
    const FACADE_SCHEMA: &str = "excalidraw-render/golden-facade-scenes/v1";

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate is inside the workspace")
            .to_owned()
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(
            &fs::read(path)
                .unwrap_or_else(|error| panic!("{} is present: {error}", path.display())),
        )
        .unwrap_or_else(|error| panic!("{} is valid JSON: {error}", path.display()))
    }

    fn updating() -> bool {
        matches!(
            std::env::var("UPDATE_GOLDEN").as_deref(),
            Ok("1") | Ok("true")
        )
    }

    /// The execution list and the scene paths the registry and the facade scene
    /// list describe today.
    fn derive_inputs() -> (Vec<Execution>, BTreeMap<String, String>) {
        let root = root();
        let registry = read_json(&root.join(REGISTRY));
        let mut executions = Vec::new();
        let mut scenes = BTreeMap::new();
        for case in registry["cases"].as_array().expect("registry cases") {
            let id = case["id"].as_str().expect("case id").to_owned();
            let dir = case["path"].as_str().expect("case path");
            let manifest = read_json(&root.join(dir).join("manifest.json"));
            let scene = format!(
                "{dir}/{}",
                manifest["scene"]["path"].as_str().expect("scene")
            );
            let bytes = fs::read(root.join(&scene)).expect("scene is present");
            assert_eq!(
                sha256(&bytes),
                manifest["scene"]["sha256"],
                "{id}: the manifest describes its scene"
            );
            assert!(
                scenes.insert(id.clone(), scene).is_none(),
                "duplicate case {id}"
            );
            let mut append = |variant: &str, export: Export, options: &Value| {
                for format in [Format::Svg, Format::Png, Format::Pdf] {
                    executions.push(Execution {
                        key: Key {
                            case: id.clone(),
                            variant: variant.to_owned(),
                            export: export.clone(),
                            format,
                        },
                        options: options.clone(),
                    });
                }
            };
            if case["kind"] == "frame" {
                for target in manifest["targets"].as_array().expect("frame targets") {
                    let ordinal = target["frame_ordinal"]
                        .as_u64()
                        .filter(|ordinal| *ordinal > 0)
                        .expect("frame ordinal");
                    append("default", Export::Frame { ordinal }, &manifest["options"]);
                }
            } else {
                for variant in manifest["variants"].as_array().expect("variants") {
                    append(
                        variant["id"].as_str().expect("variant id"),
                        Export::Scene {},
                        &variant["options"],
                    );
                }
            }
        }
        // The project-authored facade scenes: no official evidence, one
        // default request each, every format.
        let facade = read_json(&root.join(FACADE_SCENES));
        assert_eq!(facade["schema"], FACADE_SCHEMA);
        for scene in facade["scenes"].as_array().expect("facade scenes") {
            let id = scene["id"].as_str().expect("facade scene id");
            assert!(id.starts_with("facade."), "{id} is a facade id");
            let path = scene["path"].as_str().expect("facade scene path");
            assert!(root.join(path).is_file(), "{path} is present");
            assert!(
                scenes.insert(id.to_owned(), path.to_owned()).is_none(),
                "duplicate case {id}"
            );
            for format in [Format::Svg, Format::Png, Format::Pdf] {
                executions.push(Execution {
                    key: Key {
                        case: id.to_owned(),
                        variant: "default".to_owned(),
                        export: Export::Scene {},
                        format,
                    },
                    options: facade["options"].clone(),
                });
            }
        }
        executions.sort_by(|a, b| a.key.cmp(&b.key));
        (executions, scenes)
    }

    /// The generated inputs module for today's registry.
    fn generate_inputs(executions: &[Execution], scenes: &BTreeMap<String, String>) -> String {
        let executions = serde_json::to_string(executions).expect("executions serialize");
        assert!(
            !executions.contains("\"##"),
            "the execution JSON fits a r##\"…\"## literal"
        );
        let mut arms = String::new();
        for (case, path) in scenes {
            // Relative to crates/excalidraw-render/tests/golden/.
            arms.push_str(&format!(
                "        {case:?} => include_str!(\"../../../../{path}\"),\n"
            ));
        }
        format!(
            "//! Generated by `UPDATE_GOLDEN=1 cargo test -p excalidraw-render --test golden_native`\n\
             //! from the case registry and the facade scene list; do not edit. The golden test\n\
             //! regenerates it when either, a manifest or a scene moves, and fails until it is\n\
             //! current.\n\
             \n\
             /// The export targets of every registered case and facade scene: `{{key, options}}`\n\
             /// rows in key order, as JSON.\n\
             pub const EXECUTIONS: &str = r##\"{executions}\"##;\n\
             \n\
             /// The scene document of a registered case or facade scene, embedded so the\n\
             /// browser build can read it.\n\
             pub fn scene(case: &str) -> Option<&'static str> {{\n\
             \x20   Some(match case {{\n\
             {arms}\
             \x20       _ => return None,\n\
             \x20   }})\n\
             }}\n"
        )
    }

    /// The generated module must describe today's registry. Updating rewrites
    /// it, and asks for a rebuild, since the embedded scenes are compiled in.
    fn require_current_inputs() {
        let (executions, scenes) = derive_inputs();
        let generated = generate_inputs(&executions, &scenes);
        let path = root().join(INPUTS);
        let current = fs::read_to_string(&path).expect("generated inputs module is present");
        if current == generated && super::executions() == executions {
            return;
        }
        if updating() {
            fs::write(&path, generated).expect("generated inputs module is writable");
            panic!(
                "{INPUTS} regenerated for the current registry; rebuild and run the golden test again"
            );
        }
        panic!(
            "{INPUTS} is stale for the current registry; run UPDATE_GOLDEN=1 cargo test -p excalidraw-render --test golden_native"
        );
    }

    fn load_record() -> Record {
        let path = root().join(RECORD);
        let record: Record = serde_json::from_slice(
            &fs::read(&path).unwrap_or_else(|error| panic!("{RECORD} is present: {error}")),
        )
        .unwrap_or_else(|error| panic!("{RECORD} parses: {error}"));
        assert_eq!(record.schema, SCHEMA);
        assert_eq!(record.compatibility_revision, 1);
        record
    }

    fn write_record(measured: &[(Key, Measured)]) {
        let root = root();
        let blobs = root.join(BLOBS);
        fs::create_dir_all(&blobs).expect("blob directory");
        let mut referenced = BTreeSet::new();
        for (_, measured) in measured {
            for (sha, bytes) in &measured.blobs {
                referenced.insert(sha.clone());
                let path = blobs.join(sha);
                if path.exists() {
                    assert_eq!(
                        fs::read(&path).expect("blob"),
                        *bytes,
                        "blob {sha} collides"
                    );
                } else {
                    fs::write(&path, bytes).expect("blob is writable");
                }
            }
        }
        for entry in fs::read_dir(&blobs).expect("blob directory lists") {
            let entry = entry.expect("blob entry");
            let name = entry.file_name().to_string_lossy().into_owned();
            if !referenced.contains(&name) {
                fs::remove_file(entry.path()).expect("stale blob is removable");
            }
        }
        let record = Record {
            schema: SCHEMA.into(),
            compatibility_revision: 1,
            expectations: measured
                .iter()
                .map(|(key, measured)| Expectation {
                    key: key.clone(),
                    sample: measured.sample.clone(),
                })
                .collect(),
        };
        let mut bytes = serde_json::to_vec_pretty(&record).expect("record serializes");
        bytes.push(b'\n');
        fs::write(root.join(RECORD), bytes).expect("record is writable");
    }

    #[test]
    fn every_registered_export_target_renders_its_golden_bytes() {
        require_current_inputs();
        let measured = measure_both_orders();
        if updating() {
            write_record(&measured);
            println!(
                "recorded {} golden targets into {RECORD}; review the diff of the record and {BLOBS}",
                measured.len()
            );
            return;
        }
        let record = load_record();
        let differences = differences(&record, &measured);
        assert!(
            differences.is_empty(),
            "{} of {} golden targets differ from {RECORD}:\n{}\n\nIf the change is intended, adopt it with UPDATE_GOLDEN=1 cargo test -p excalidraw-render --test golden_native and review the diff.",
            differences.len(),
            record.expectations.len(),
            describe_differences(&differences)
        );
    }

    /// The record's blobs hold exactly the bytes it names, and nothing else.
    /// While the record is being rewritten this stands aside; the next run
    /// judges what was written.
    #[test]
    fn the_golden_record_names_its_blobs_exactly() {
        if updating() {
            return;
        }
        let record = load_record();
        let root = root();
        let mut referenced = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for expectation in &record.expectations {
            assert!(
                keys.insert(&expectation.key),
                "duplicate key {:?}",
                expectation.key
            );
            let mut described = vec![&expectation.sample.report];
            match &expectation.sample.outcome {
                Outcome::Artifact { bytes } => described.push(bytes),
                Outcome::Failure { descriptor, .. } => described.push(descriptor),
            }
            for bytes in described {
                let blob = fs::read(root.join(BLOBS).join(&bytes.sha256))
                    .unwrap_or_else(|error| panic!("blob {} is present: {error}", bytes.sha256));
                assert_eq!(sha256(&blob), bytes.sha256, "blob content matches its name");
                assert_eq!(blob.len() as u64, bytes.byte_length);
                referenced.insert(bytes.sha256.clone());
            }
        }
        let present: BTreeSet<_> = fs::read_dir(root.join(BLOBS))
            .expect("blob directory")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            present, referenced,
            "every blob is referenced and every reference present"
        );
    }

    /// The #178 record names registered cases, each once, with distinct bytes.
    #[test]
    fn the_experimental_wasm_record_is_well_formed() {
        if updating() {
            return;
        }
        let record = super::wasm_differences(
            &fs::read_to_string(root().join(EXPERIMENTAL_WASM)).expect("record is present"),
        );
        let golden = load_record();
        let cases: BTreeSet<_> = golden
            .expectations
            .iter()
            .map(|expectation| expectation.key.case.as_str())
            .collect();
        for difference in &record {
            assert!(
                cases.contains(difference.case.as_str()),
                "{} is registered",
                difference.case
            );
            assert_ne!(
                difference.native, difference.wasm,
                "{} records a difference",
                difference.case
            );
            let expectation = golden
                .expectations
                .iter()
                .find(|expectation| expectation.key.is_default_scene_png_of(&difference.case))
                .unwrap_or_else(|| panic!("{} has a default scene PNG target", difference.case));
            assert_eq!(
                expectation.sample.outcome,
                Outcome::Artifact {
                    bytes: difference.native.clone()
                },
                "{}: the recorded native bytes are the golden bytes",
                difference.case
            );
        }
    }
}

/// One recorded browser-WASM difference (#178): the default scene PNG of a case
/// whose native and WASM bytes differ by tiny-skia's cubic-root rounding.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WasmDifference {
    case: String,
    native: ByteDigest,
    wasm: ByteDigest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WasmDifferences {
    schema: String,
    issue: u64,
    measurement: Value,
    cases: Vec<WasmDifference>,
}

fn wasm_differences(source: &str) -> Vec<WasmDifference> {
    let record: WasmDifferences = serde_json::from_str(source).expect("#178 record parses");
    assert_eq!(
        record.schema,
        "excalidraw-render/experimental-wasm-differences/v1"
    );
    assert_eq!(record.issue, 178);
    assert!(record.measurement.is_object());
    let cases: BTreeSet<_> = record.cases.iter().map(|case| &case.case).collect();
    assert_eq!(
        cases.len(),
        record.cases.len(),
        "each case is recorded once"
    );
    record.cases
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    wasm_bindgen_test_configure!(run_in_dedicated_worker);

    const RECORD_SOURCE: &str = include_str!("../../../tests/conformance/golden/native.json");
    const EXPERIMENTAL_SOURCE: &str =
        include_str!("../../../tests/conformance/golden/experimental-wasm.json");

    /// Whether a difference is exactly a recorded #178 pair.
    fn is_recorded(key: &Key, expected: &Sample, actual: &Sample, record: &WasmDifference) -> bool {
        key.is_default_scene_png_of(&record.case)
            && expected.outcome
                == Outcome::Artifact {
                    bytes: record.native.clone(),
                }
            && actual.outcome
                == Outcome::Artifact {
                    bytes: record.wasm.clone(),
                }
            && actual.report == expected.report
    }

    fn judge(reverse: bool) {
        let record: Record = serde_json::from_str(RECORD_SOURCE).expect("record parses");
        assert_eq!(record.schema, SCHEMA);
        let allowed = wasm_differences(EXPERIMENTAL_SOURCE);
        let measured = measure(reverse);
        let differences = differences(&record, &measured);
        let mut used = BTreeSet::new();
        let mut unrecorded = Vec::new();
        for (key, expected, actual) in &differences {
            let recorded = match (expected, actual) {
                (Some(expected), Some(actual)) => allowed
                    .iter()
                    .find(|record| is_recorded(key, expected, actual, record)),
                _ => None,
            };
            match recorded {
                Some(record) => {
                    used.insert(record.case.clone());
                }
                None => unrecorded.push((*key, *expected, *actual)),
            }
        }
        assert!(
            unrecorded.is_empty(),
            "{} browser-WASM targets differ from the golden record beyond the recorded #178 pairs:\n{}",
            unrecorded.len(),
            describe_differences(&unrecorded)
        );
        let stale: Vec<_> = allowed
            .iter()
            .filter(|record| !used.contains(&record.case))
            .map(|record| record.case.as_str())
            .collect();
        assert!(
            stale.is_empty(),
            "recorded #178 differences were not observed and are stale: {stale:?}"
        );
    }

    #[wasm_bindgen_test]
    fn every_registered_export_target_renders_its_golden_bytes_forward() {
        judge(false);
    }

    #[wasm_bindgen_test]
    fn every_registered_export_target_renders_its_golden_bytes_reverse() {
        judge(true);
    }
}
