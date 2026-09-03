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
//! differently sized document, so path data alone cannot see it. The root
//! `width` and `height` are compared too: an export scale multiplies only those
//! and leaves the `viewBox` logical, so they are the one place a scaled SVG
//! variant differs from the default and the one place a scaling defect shows.
//!
//! Path data and extent are the invariants, not SVG structure: the renderer
//! legitimately differs from the official export in how it positions text and
//! groups elements. Every registered variant is compared, since dark mode and
//! sizing must leave path geometry untouched.

// Included by path rather than through `common/mod.rs` so this binary does not
// also compile the PDF harness helpers, and the PDF binaries do not compile this.
#[path = "common/registered_cases.rs"]
mod registered_cases;

use std::fs;
use std::path::{Path, PathBuf};

use excalidraw_render::{OutputFormat, Renderer, SceneDocument};
use serde_json::Value;

use registered_cases::variant_request;

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
        let manifest: Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("manifest present"))
                .expect("manifest is valid JSON");
        total += 1;

        let scene = SceneDocument::parse(
            &fs::read(dir.join("scene.excalidraw")).expect("scene document present"),
        )
        .unwrap_or_else(|error| panic!("{id} scene is admitted: {error:?}"));
        for variant in manifest["variants"]
            .as_array()
            .expect("manifest variants are an array")
        {
            let variant_id = variant["id"].as_str().expect("variant id is a string");
            let request = variant_request(
                OutputFormat::Svg,
                &variant["options"],
                &format!("{id} variant {variant_id}"),
            );
            let outcome = Renderer::new()
                .render(&scene, &request)
                .unwrap_or_else(|error| panic!("{id} renders as SVG: {error:?}"));
            let candidate_svg =
                std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
            let official_path = variant["formats"]["svg"]["artifact"]["path"]
                .as_str()
                .expect("official SVG path");
            let official_svg =
                fs::read_to_string(dir.join(official_path)).expect("official SVG present");

            // The viewBox is the logical extent; width and height are that extent
            // times the export scale.
            let extent = ["viewBox", "width", "height"]
                .into_iter()
                .filter_map(|attribute| {
                    let ours = root_attribute(candidate_svg, attribute);
                    let theirs = root_attribute(&official_svg, attribute);
                    (ours != theirs)
                        .then(|| format!("{attribute} {ours:?} against official {theirs:?}"))
                })
                .collect::<Vec<_>>();
            let extent = (!extent.is_empty()).then(|| extent.join("; "));

            let candidate = path_data(candidate_svg);
            let official = path_data(&official_svg);
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

            // Both dimensions are reported, not just the first to fail. A change
            // that corrupts path data while making an extent agree produces an
            // entry for each, and reporting only one would hide the more serious
            // half.
            for (dimension, divergence) in [("extent", extent), ("paths", paths)] {
                if let Some(detail) = divergence {
                    failures.push(format!("{id} [{variant_id}] [{dimension}]: {detail}"));
                }
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

/// An attribute of the `<svg>` root, located by name within the root tag so a
/// same-named attribute on a nested element cannot stand in for it.
fn root_attribute(svg: &str, name: &str) -> String {
    let start = svg.find("<svg").expect("SVG has a root element");
    let tag = &svg[start..];
    let tag = &tag[..tag.find('>').expect("SVG root tag closes")];
    let marker = format!(" {name}=\"");
    let value = &tag[tag
        .find(&marker)
        .unwrap_or_else(|| panic!("SVG root declares {name}"))
        + marker.len()..];
    value[..value.find('"').expect("attribute value closes")].to_owned()
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
