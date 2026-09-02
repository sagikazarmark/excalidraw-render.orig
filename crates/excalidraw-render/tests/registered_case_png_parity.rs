//! Direct-PNG parity for every registered scene case.
//!
//! The corpus stores official browser evidence for each case, but only the three
//! pre-existing cases had a renderer comparison against it. This drives the
//! comparison from `tests/conformance/cases/registry.json`, so a case is covered
//! the moment it is registered rather than when someone remembers to hand-write a
//! test for it.
//!
//! Direct PNG needs no canonical viewer: the renderer's own raster is compared
//! against the browser's `official-png.rgba` under the canonical Pixelmatch and
//! SSIM budgets. SVG raster parity needs a viewer and is not covered here.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use excalidraw_render_compare::{compare, CanonicalRgba8, ComparisonMode, VisualClass};
use png::{BitDepth, ColorType, Decoder, Transformations};
use serde_json::Value;

/// Cases whose direct PNG does not yet match the official evidence, tracked by
/// issue #104. This is a suppression, not an approved exception: these are
/// undiagnosed, so recording them through the manifest `exceptions` mechanism
/// would assert they are accepted compatibility differences, which is not
/// established. Entries leave this list by being diagnosed — fixed if they are
/// defects, or given a recorded visual exception if they are specified
/// differences. A case that starts passing must be removed from the list, which
/// the test enforces so the suppression cannot outlive the divergence.
const KNOWN_DIVERGENT: &[&str] = &[
    "focused.arrow-suite.v1",
    "focused.bound-text-containers.v1",
    "focused.cjk-propagation.v1",
    "focused.ellipse-diamond-fills.v1",
    "focused.embeddable-links.v1",
];

#[test]
fn every_registered_scene_case_matches_its_official_direct_png() {
    let root = repository_root();
    let registry: Value = serde_json::from_slice(
        &fs::read(root.join("tests/conformance/cases/registry.json"))
            .expect("case registry is present"),
    )
    .expect("case registry is valid JSON");

    let mut total = 0usize;
    let mut failures = Vec::new();
    let mut unexpected_passes = Vec::new();

    for case in registry["cases"]
        .as_array()
        .expect("registry cases are an array")
        .iter()
        .filter(|case| case["kind"] == "scene")
    {
        let id = case["id"].as_str().expect("case id is a string");
        total += 1;
        let dir = root.join(case["path"].as_str().expect("case path is a string"));
        let manifest: Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).expect("manifest present"))
                .expect("manifest is valid JSON");

        let variants = manifest["variants"]
            .as_array()
            .expect("manifest variants are an array");
        // Every case declares only the default export today. When a case adds a
        // dark-mode or scale row this must learn to build the matching request
        // rather than quietly skip it, so it fails loudly instead.
        assert_eq!(
            variants.len(),
            1,
            "{id} declares {} variants; this harness only builds the default request",
            variants.len()
        );
        let variant = &variants[0];
        assert_eq!(variant["id"], "default", "{id} has a non-default variant");

        let scene = SceneDocument::parse(
            &fs::read(dir.join("scene.excalidraw")).expect("scene document present"),
        )
        .unwrap_or_else(|error| panic!("{id} scene is admitted: {error:?}"));
        let request = RenderRequest::builder(OutputFormat::Png)
            .build()
            .expect("default PNG request is valid");
        let outcome = Renderer::new()
            .render(&scene, &request)
            .unwrap_or_else(|error| panic!("{id} renders as PNG: {error:?}"));

        let png = &variant["formats"]["png"];
        let expected_report = dir.join(
            png["expected_report"]["path"]
                .as_str()
                .expect("expected report path"),
        );
        // A report mismatch is never suppressed: it means the renderer's
        // diagnostics disagree with what the case declares, which is a contract
        // break rather than a pixel difference.
        assert_eq!(
            serde_json::to_vec(outcome.report()).expect("report serializes"),
            fs::read(&expected_report).expect("expected report present"),
            "{id}: PNG report differs from its declared expectation"
        );

        let raw = &png["raw_raster"];
        let width = raw["width"].as_u64().expect("raster width") as u32;
        let height = raw["height"].as_u64().expect("raster height") as u32;
        let reference = CanonicalRgba8::new(
            width,
            height,
            fs::read(dir.join(raw["path"].as_str().expect("raster path")))
                .expect("official raster present"),
        )
        .expect("official evidence is canonical RGBA8");
        let candidate = decode_png(outcome.artifact().bytes());

        let divergence = if (candidate.width(), candidate.height()) != (width, height) {
            Some(format!(
                "rendered {}x{}, official evidence is {width}x{height}",
                candidate.width(),
                candidate.height()
            ))
        } else {
            let class = visual_class(png["visual_class"].as_str().expect("visual class"));
            let report = compare(&reference, &candidate, class, &[], ComparisonMode::Canonical)
                .expect("dimensions agree");
            (!report.passed()).then(|| {
                format!(
                    "changed_pixels={} ssim={:?}",
                    report.whole.changed_pixels, report.whole.ssim
                )
            })
        };

        match (divergence, KNOWN_DIVERGENT.contains(&id)) {
            (Some(detail), false) => failures.push(format!("{id}: {detail}")),
            (Some(_), true) => {}
            (None, true) => unexpected_passes.push(id.to_owned()),
            (None, false) => {}
        }
    }



    assert!(total > 0, "the registry declared no scene cases");
    assert!(
        unexpected_passes.is_empty(),
        "these cases now match their official direct PNG and must be removed from \
         KNOWN_DIVERGENT (see issue #104): {unexpected_passes:?}"
    );
    assert!(
        failures.is_empty(),
        "{} of {} registered scene cases diverge from their official direct PNG:\n  {}",
        failures.len(),
        total,
        failures.join("\n  ")
    );
}

fn visual_class(name: &str) -> VisualClass {
    match name {
        "exact" => VisualClass::Exact,
        "geometry/image" => VisualClass::GeometryImage,
        "typography" => VisualClass::Typography,
        "mixed/representative" => VisualClass::MixedRepresentative,
        other => panic!("unknown visual class {other:?}"),
    }
}

fn decode_png(bytes: &[u8]) -> CanonicalRgba8 {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("PNG structure is valid");
    let mut rgba = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut rgba).expect("PNG pixels decode");
    assert_eq!(frame.color_type, ColorType::Rgba);
    assert_eq!(frame.bit_depth, BitDepth::Eight);
    rgba.truncate(frame.buffer_size());
    CanonicalRgba8::new(frame.width, frame.height, rgba).expect("decoded PNG is canonical RGBA8")
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is inside the workspace")
        .to_owned()
}
