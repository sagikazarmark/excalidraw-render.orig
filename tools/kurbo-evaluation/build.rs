use std::{env, fs, path::PathBuf};

fn main() {
    let geometry = "../../crates/excalidraw-render/src/render/geometry.rs";
    let cubic = "../../crates/excalidraw-render/src/render/geometry/cubic.rs";
    // The runner preserves the original module next to the staged candidate.
    let original = format!("{cubic}.original");
    let source_path = if std::path::Path::new(&original).exists() {
        original.as_str()
    } else {
        cubic
    };
    println!("cargo:rerun-if-changed={geometry}");
    println!("cargo:rerun-if-changed={cubic}");
    println!("cargo:rerun-if-changed={source_path}");
    println!("cargo:rerun-if-changed=adapter.rs");
    println!("cargo:rerun-if-env-changed=KURBO_EXPECTED");
    println!("cargo:rerun-if-env-changed=KURBO_RECORD");
    let source = fs::read_to_string(geometry).unwrap();
    let start = source
        .find("#[derive(Clone, Copy, Debug, PartialEq)]")
        .unwrap();
    let end = source.find("pub(super) enum GeneratedGeometry").unwrap();
    let traversal = &source[start..end];
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (mode, module) in [("current", source_path), ("candidate", "adapter.rs")] {
        let module_path = out.join(format!("{mode}-cubic.rs"));
        fs::copy(module, &module_path).unwrap();
        fs::write(
            out.join(format!("{mode}.rs")),
            format!("#[path = {module_path:?}]\nmod cubic;\n{traversal}"),
        )
        .unwrap();
    }
    let expected = env::var("KURBO_EXPECTED").expect("use run.py to select the observation");
    println!("cargo:rerun-if-changed={expected}");
    fs::write(out.join("expected.json"), fs::read(expected).unwrap()).unwrap();
}
