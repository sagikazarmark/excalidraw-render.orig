//! The dots filler's evidence (#137, spec 8.1). The reference's `DotFiller`
//! scatters each dot's centre with the unseeded `Math.random()`, so the pinned
//! reference profile cannot produce official evidence for it: the generator's
//! own repeatability gate rejects the export because two fresh browser
//! processes never agree on the bytes, and the focused case drafted for it
//! (`focused.rough-fillers-dots.v1`) was withdrawn on that ground.
//! Revision 1 draws the scatter from a second Rough.js generator seeded with
//! the element seed, so its output is one deterministic sample of the
//! reference's jitter, and the JS-derived vectors are its evidence: this test
//! renders the withdrawn case's scene document and pins every path against
//! the pinned npm Rough.js 4.6.4 run of `tools/rough-filler-vectors/derive.mjs`
//! over the same scene, which stubs `Math.random()` with the same seeded
//! sample.

use excalidraw_render::{OutputFormat, SceneDocument};
use sha2::{Digest, Sha256};

use crate::common::lower_hex;
use crate::common::render::render_artifact;
use crate::common::svg::path_data;

/// The withdrawn case's scene: dots fills
/// on a rectangle, an ellipse and a diamond at strokeWidth 2 / roughness 1,
/// strokeWidth 1 / roughness 0 and strokeWidth 4 / roughness 1.
const SCENE: &[u8] = include_bytes!("../fixtures/rough-fillers-dots.excalidraw");

#[test]
fn the_dots_scene_matches_the_pinned_roughjs_vectors_in_every_format() {
    let scene = SceneDocument::parse(SCENE).expect("the dots scene is admitted");

    // Every path, the fill first and then the outline of each element in
    // document order, against
    //
    //   node tools/rough-filler-vectors/derive.mjs \
    //     tools/reference-oracle/node_modules/roughjs/bundled/rough.esm.js \
    //     < crates/excalidraw-render/tests/fixtures/rough-fillers-dots.excalidraw
    //
    // under Node 24.19.0 after `tools/reference-oracle/run-container.sh
    // install`, whose `joined_sha256` this is.
    let svg = render_artifact(&scene, OutputFormat::Svg);
    let svg = std::str::from_utf8(&svg).expect("SVG is UTF-8");
    let paths = path_data(svg);
    assert_eq!(
        paths.len(),
        18,
        "nine elements, a dots fill and an outline each"
    );
    assert_eq!(
        lower_hex(&Sha256::digest(paths.join("\n"))),
        "a93d84031a9f37d62c70b6282776ec66905e95590bbfd400953a4fed8387c5c8"
    );

    // The same drawing feeds PNG and PDF without a diagnostic: the dots fill is
    // an admitted `fillStyle`, not a substituted one.
    for format in [OutputFormat::Png, OutputFormat::Pdf] {
        let artifact = render_artifact(&scene, format);
        assert!(!artifact.is_empty());
    }
}
