//! Plumbing shared across the test binaries of this crate. Every binary that
//! needs it declares this module once (as `mod common;` from a file directly
//! under `tests/`, or by path from a `main.rs` in a subdirectory) and uses the
//! part it needs; what it does not use is dead code there, hence the allowance.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub mod approved_scenes;
pub mod image_scene;
pub mod parse_limits;
pub mod pdf_harness;
pub mod pdf_inspect;
pub mod raster;
pub mod registered_cases;
pub mod render;
pub mod reports;
pub mod scratch;
pub mod svg_baselines;

/// The workspace root, two levels above this crate.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}
