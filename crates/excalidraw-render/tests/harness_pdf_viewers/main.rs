//! The PDF gates that need the exact-pinned PDFium, PDF.js and qpdf harness
//! (`just pdf-harness`). Every test here is `#[ignore]`d for plain
//! `cargo test`; the `harness` nextest profile selects this binary by name and
//! runs it with `--run-ignored only`, so a gate cannot drift out of the
//! pdf-harness-gates workflow by being renamed.
//!
//! Each module holds the gates of one approved input. Where the fast tier
//! inspects the same render for structure (the rectangle outline and the
//! backend qualification scenes), both take it from `common::approved_scenes`,
//! so the artifact a viewer rasterizes here is the artifact inspected there.

#[path = "../common/mod.rs"]
mod common;

mod adapters;
mod backend_qualification;
mod excalifont_text;
mod frame_export;
mod rectangle_outline;
