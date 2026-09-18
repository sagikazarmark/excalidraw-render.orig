use std::{env, fs, path::PathBuf};

fn main() {
    // Compile the real private reader, not a manually maintained copy. Fail if
    // the seam moves. BMP is included directly with #[path] in lib.rs.
    let source = "../../crates/excalidraw-render/src/render/asset/orientation.rs";
    println!("cargo:rerun-if-changed={source}");
    let source = fs::read_to_string(source).unwrap();
    let start = source.find("pub(super) fn exif_orientation(").unwrap();
    let end = source[start..]
        .find("pub(super) fn oriented_dimensions(")
        .unwrap()
        + start;
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("current_exif.rs"), &source[start..end]).unwrap();
}
