//! Plumbing shared across the test binaries of this crate. Every binary that
//! needs it declares this module once (as `mod common;` from a file directly
//! under `tests/`, or by path from a `main.rs` in a subdirectory) and uses the
//! part it needs; what it does not use is dead code there, hence the allowance.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::Value;

pub mod approved_scenes;
pub mod clean_baselines;
pub mod codec_fixtures;
pub mod image_scene;
pub mod parse_limits;
pub mod pdf_harness;
pub mod pdf_inspect;
pub mod png_color_reference;
pub mod png_icc_safety;
pub mod png_stage4;
pub mod png_stage4_composition;
pub mod png_stage4_reference;
pub mod png_stage4_review;
pub mod raster;
pub mod registered_cases;
pub mod render;
pub mod reports;
pub mod scratch;
pub mod sfnt;
pub mod svg;
pub mod svg_baselines;

/// The workspace root, two levels above this crate.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}

/// Whether `path` is reference-oracle code, less the fixtures it reads. Codec
/// evidence records that code as historical producer provenance (ADR-0019): the
/// tests never run the oracle, so editing it or updating its dependencies must
/// not stale the frozen reference.
pub fn is_oracle_producer(path: &str) -> bool {
    path.starts_with("tools/reference-oracle/")
        && !path.starts_with("tools/reference-oracle/fixtures/")
}

/// The JSON document at `path`, named in the failure when it is absent or
/// malformed.
pub fn read_json(path: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(path)
            .unwrap_or_else(|error| panic!("{} is present: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("{} is valid JSON: {error}", path.display()))
}

/// Lowercase hex of a digest, as evidence records spell it.
pub fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
