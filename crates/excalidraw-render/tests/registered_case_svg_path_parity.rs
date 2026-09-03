//! SVG path-geometry parity for every registered scene case.
//!
//! This compares the `d` attribute of every `<path>` the renderer emits against
//! the official export's, in document order. It is a sharper check than raster
//! comparison for geometry and a far cheaper one: no canonical viewer, no
//! rasterization, and no antialiasing noise to threshold against. A single
//! wrong coordinate shows up as an exact mismatch.
//!
//! It exists because two geometry defects were caught this way and one wrong fix
//! was caught only this way. Zeroing the roughness of elbow arrows made a scene
//! extent match official to fifteen digits while corrupting three paths; path
//! comparison rejects it immediately.
//!
//! It also compares the root `viewBox`, which is where scene-bounds defects
//! live: a bounds error emits every path at identical coordinates inside a
//! differently sized document, so path data alone cannot see it.
//!
//! Path data and extent are the invariants, not SVG structure: the renderer
//! legitimately differs from the official export in how it positions text and
//! groups elements.

use std::fs;
use std::path::{Path, PathBuf};

use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::Value;

#[test]
fn every_registered_scene_case_matches_its_official_svg_path_geometry() {
    let root = repository_root();
    let registry: Value = serde_json::from_slice(
        &fs::read(root.join("tests/conformance/cases/registry.json"))
            .expect("case registry is present"),
    )
    .expect("case registry is valid JSON");

    let mut total = 0usize;
    let mut failures = Vec::new();

    for case in registry["cases"]
        .as_array()
        .expect("registry cases are an array")
        .iter()
        .filter(|case| case["kind"] == "scene")
    {
        let id = case["id"].as_str().expect("case id is a string");
        let dir = root.join(case["path"].as_str().expect("case path is a string"));
        total += 1;

        let scene = SceneDocument::parse(
            &fs::read(dir.join("scene.excalidraw")).expect("scene document present"),
        )
        .unwrap_or_else(|error| panic!("{id} scene is admitted: {error:?}"));
        let request = RenderRequest::builder(OutputFormat::Svg)
            .build()
            .expect("default SVG request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("{id} renders as SVG: {error:?}"));

        let candidate = path_data(
            std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8"),
        );
        let official = path_data(
            &fs::read_to_string(dir.join("evidence/official.svg")).expect("official SVG present"),
        );

        let candidate_box = view_box(
            std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8"),
        );
        let official_box = view_box(
            &fs::read_to_string(dir.join("evidence/official.svg")).expect("official SVG present"),
        );

        let extent = (candidate_box != official_box).then(|| {
            format!("viewBox {candidate_box:?} against official {official_box:?}")
        });
        let paths = if candidate.len() != official.len() {
            Some(format!(
                "emits {} paths, official has {}",
                candidate.len(),
                official.len()
            ))
        } else {
            candidate
                .iter()
                .zip(&official)
                .position(|(a, b)| a != b)
                .map(|index| {
                    format!(
                        "path {index} differs\n      ours     {}\n      official {}",
                        truncate(&candidate[index]),
                        truncate(&official[index])
                    )
                })
        };

        // Both dimensions are reported, not just the first to fail. A change that
        // corrupts path data while making an extent agree produces an entry for
        // each, and reporting only one would hide the more serious half.
        for (dimension, divergence) in [("extent", extent), ("paths", paths)] {
            if let Some(detail) = divergence {
                failures.push(format!("{id} [{dimension}]: {detail}"));
            }
        }
    }

    assert!(total > 0, "the registry declared no scene cases");
    assert!(
        failures.is_empty(),
        "{} divergence(s) from the official export across {total} cases:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// The root `viewBox`, which carries the scene's computed extent.
fn view_box(svg: &str) -> String {
    svg.find("viewBox=\"")
        .map(|start| &svg[start + "viewBox=\"".len()..])
        .and_then(|rest| rest.find('"').map(|end| rest[..end].to_owned()))
        .expect("SVG root declares a viewBox")
}

/// Every `<path>` element's `d` attribute, in document order. The attribute is
/// located by name rather than by position: the official export writes `fill`
/// before `d` on freedraw strokes and `d` first elsewhere.
fn path_data(svg: &str) -> Vec<String> {
    let mut data = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find("<path") {
        rest = &rest[start + "<path".len()..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[..end];
        if let Some(attribute) = tag.find(" d=\"") {
            let value = &tag[attribute + " d=\"".len()..];
            if let Some(close) = value.find('"') {
                data.push(value[..close].to_owned());
            }
        }
        rest = &rest[end..];
    }
    data
}

fn truncate(value: &str) -> String {
    if value.len() <= 90 {
        value.to_owned()
    } else {
        format!("{}...", &value[..90])
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}
