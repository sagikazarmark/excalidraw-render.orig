use std::{env, fs, path::PathBuf};

fn main() {
    // The runner preserves the original next to the patched staged source.
    let geometry = "../../crates/excalidraw-render/src/render/geometry.rs";
    let original = format!("{geometry}.original");
    let source_path = if std::path::Path::new(&original).exists() {
        original.as_str()
    } else {
        geometry
    };
    println!("cargo:rerun-if-changed={source_path}");
    println!("cargo:rerun-if-changed=adapter.rs");
    println!("cargo:rerun-if-env-changed=KURBO_EXPECTED");
    println!("cargo:rerun-if-env-changed=KURBO_RECORD");
    let source = fs::read_to_string(source_path).unwrap();
    let start = source
        .find("#[derive(Clone, Copy, Debug, PartialEq)]")
        .unwrap();
    let end = source.find("pub(super) enum GeneratedGeometry").unwrap();
    let original = &source[start..end];
    let seam = original.find("fn include_cubic_bounds(").unwrap();
    let candidate = format!(
        "{}{}",
        &original[..seam],
        fs::read_to_string("adapter.rs").unwrap()
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("current.rs"), original).unwrap();
    fs::write(out.join("candidate.rs"), candidate).unwrap();
    let expected = env::var("KURBO_EXPECTED").expect("use run.py to select the observation");
    println!("cargo:rerun-if-changed={expected}");
    fs::write(out.join("expected.json"), fs::read(expected).unwrap()).unwrap();
}
