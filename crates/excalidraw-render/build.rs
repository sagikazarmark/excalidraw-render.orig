//! Admits the release-mode qualification tests.
//!
//! `EXCALIDRAW_RENDER_RELEASE_QUALIFICATION=1` sets the `release_qualification`
//! cfg, which lifts the `#[ignore]` from the corpus-driven tests the push gate
//! skips. The library's own code never reads it; it is a build-time switch
//! rather than a Cargo feature so it is not part of the published interface.
//! The package's `include` list leaves this script out, so the published
//! crate has no build script and dependents never run it.

const VARIABLE: &str = "EXCALIDRAW_RENDER_RELEASE_QUALIFICATION";

fn main() {
    println!("cargo::rustc-check-cfg=cfg(release_qualification)");
    println!("cargo::rerun-if-env-changed={VARIABLE}");
    match std::env::var(VARIABLE) {
        Err(std::env::VarError::NotPresent) => {}
        Ok(value) if value.is_empty() || value == "0" => {}
        Ok(value) if value == "1" => println!("cargo::rustc-cfg=release_qualification"),
        other => panic!("{VARIABLE} must be unset, 0 or 1, got {other:?}"),
    }
}
