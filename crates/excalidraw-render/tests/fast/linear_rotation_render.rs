//! Where a rotated linear element's render transform rotates it, per format
//! (#177): the SVG about the centre of its points' box, as the official SVG
//! does (`renderElementToSvg` reads `getElementAbsoluteCoords` on a cold
//! `ShapeCache`), PNG and PDF about the centre of its drawn curve's box, as the
//! official canvas does (`renderElement` warms the cache first). The scene is
//! `focused.rotated-rough-linear.v1`'s, whose official SVG the transforms here
//! are read from, and the expected values are that official export's.

use std::fs;

use excalidraw_render::{OutputFormat, SceneDocument};

use crate::common::pdf_inspect::pdf_text;
use crate::common::render::render_artifact;
use crate::common::repository_root;

const CASE: &str = "tests/conformance/cases/focused/rotated-rough-linear";

/// The six elements of the case in paint order with the `translate(...)
/// rotate(...)` their groups carry in the official SVG: a roughness-1 elbow
/// arrow at 1.55 rad, a roughness-1 line at 0.4 rad, a roughness-1 two-point
/// arrow at 1.1 rad, a roughness-2 arrow at 0.7 rad, and a roughness-0 line
/// and arrow as controls. Every `rotate` centre is the points' box centre
/// (`150 45` for the 300 by 90 route, `65 20` for the 130 by 40 line, `70 0`
/// for the flat two-point arrow, `70 -15` for the bent one), not the curve's.
const OFFICIAL_TRANSFORMS: [(&str, &str, bool); 6] = [
    (
        "elbow-rough-1",
        "translate(-94.5224254561289 115.90333187954272) rotate(88.8084582452776 150 45)",
        true,
    ),
    (
        "line-rough-1",
        "translate(165.4775745438711 55.90333187954272) rotate(22.91831180523293 65 20)",
        true,
    ),
    (
        "arrow-rough-1",
        "translate(345.4775745438711 95.90333187954272) rotate(63.02535746439057 70 0)",
        true,
    ),
    (
        "arrow-rough-2",
        "translate(165.4775745438711 215.90333187954272) rotate(40.10704565915762 70 -15)",
        true,
    ),
    (
        "line-rough-0",
        "translate(345.4775745438711 215.90333187954272) rotate(22.91831180523293 65 20)",
        false,
    ),
    (
        "arrow-rough-0",
        "translate(525.4775745438711 115.90333187954272) rotate(63.02535746439057 70 0)",
        false,
    ),
];

fn case_scene() -> SceneDocument {
    SceneDocument::parse(
        &fs::read(repository_root().join(CASE).join("scene.excalidraw"))
            .expect("the registered scene is present"),
    )
    .expect("the registered scene is admitted")
}

/// Every distinct `transform="..."` value of the document in first-use order.
fn transforms(svg: &str) -> Vec<&str> {
    let mut seen = Vec::new();
    for value in svg
        .split("transform=\"")
        .skip(1)
        .map(|tail| tail.split_once('"').expect("attribute terminates").0)
    {
        if !seen.contains(&value) {
            seen.push(value);
        }
    }
    seen
}

/// `(tx, ty, degrees, cx, cy)` of one `translate(tx ty) rotate(degrees cx cy)`.
fn parse_transform(value: &str) -> [f64; 5] {
    let numbers: Vec<f64> = value
        .split(['(', ')', ' '])
        .filter_map(|token| token.parse().ok())
        .collect();
    numbers
        .try_into()
        .expect("translate and rotate carry five numbers")
}

/// Where a rotation by the matrix whose `(cos, sin)` is given, about `(cx, cy)`
/// after the translation `(tx, ty)`, lands the element's local origin on the
/// page, `t + c - R c`: the translation a PDF `cm` carries in `e f`. The
/// rotation's sine and cosine are read from the PDF's own matrix rather than
/// recomputed from the SVG's degrees, so the two placements are compared
/// under one rotation (and the test stays off the transcendental functions
/// `portable_math` reserves, ADR-0007).
fn origin_offset([tx, ty, _degrees, cx, cy]: [f64; 5], (cos, sin): (f64, f64)) -> (f64, f64) {
    (tx + cx - cos * cx + sin * cy, ty + cy - sin * cx - cos * cy)
}

#[test]
fn the_svg_rotates_every_linear_element_about_its_points_box_like_the_official_export() {
    let svg =
        String::from_utf8(render_artifact(&case_scene(), OutputFormat::Svg)).expect("SVG is UTF-8");
    let official = fs::read_to_string(repository_root().join(CASE).join("evidence/official.svg"))
        .expect("official evidence is present");
    let expected: Vec<&str> = OFFICIAL_TRANSFORMS
        .iter()
        .map(|(_, transform, _)| *transform)
        .collect();
    assert_eq!(
        transforms(&official),
        expected,
        "the pinned transforms are the official export's, in paint order"
    );
    assert_eq!(
        transforms(&svg),
        expected,
        "every linear element's group rotates about its points' box centre, digit for digit"
    );
}

#[test]
fn the_pdf_rotates_a_rough_linear_element_about_its_curve_box_and_a_smooth_one_about_its_points() {
    // The PDF's `cm` places each element's local origin at `t + c - R c` for
    // the centre it rotates about. With the curve's box centre that is the
    // points-centre placement only where the drawn outline's extent is the
    // points' box — at roughness 0, where the curve has no jitter — and off it
    // by `(R - I)(c_curve - c_points)` where the jitter overshoots the points'
    // box, up to a couple of pixels here: the arrow at roughness 2 sits
    // 1.57 px left of where the SVG puts it, the two-point arrow at
    // roughness 1 0.86 px right and 0.53 px down.
    // Krilla writes each drawing's matrix composed with the page's
    // logical-pixel mapping `x' = 0.75 x, y' = H - 0.75 y` (ADR-0021); undoing
    // it recovers the element matrix, which every drawing of one element (its
    // outline, then its arrowheads) repeats.
    let pdf = pdf_text(&render_artifact(&case_scene(), OutputFormat::Pdf));
    let height: f64 = pdf
        .split("/MediaBox [0 0 ")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .and_then(|rest| rest.split(' ').nth(1))
        .expect("the page has a media box")
        .parse()
        .expect("the page height is a number");
    let mut matrices: Vec<[f64; 6]> = Vec::new();
    for line in pdf.lines().filter(|line| line.ends_with(" cm")) {
        let numbers: Vec<f64> = line
            .split(' ')
            .take(6)
            .map(|token| token.parse().expect("matrix components are numbers"))
            .collect();
        let [a, b, c, d, e, f]: [f64; 6] = numbers.try_into().expect("a cm carries six components");
        let element = [
            a / 0.75,
            -b / 0.75,
            c / 0.75,
            -d / 0.75,
            e / 0.75,
            (height - f) / 0.75,
        ];
        let same = |x: &[f64; 6], y: &[f64; 6]| x.iter().zip(y).all(|(p, q)| (p - q).abs() < 1e-4);
        if same(&element, &[1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
            || matrices.last().is_some_and(|last| same(last, &element))
        {
            continue;
        }
        matrices.push(element);
    }
    assert_eq!(matrices.len(), 6, "one element matrix per linear element");
    for ((id, official, rough), matrix) in OFFICIAL_TRANSFORMS.iter().zip(&matrices) {
        let (expected_x, expected_y) =
            origin_offset(parse_transform(official), (matrix[0], matrix[1]));
        let (actual_x, actual_y) = (matrix[4], matrix[5]);
        let (dx, dy) = (actual_x - expected_x, actual_y - expected_y);
        let distance = (dx * dx + dy * dy).sqrt();
        if *rough {
            assert!(
                distance > 0.1,
                "{id}: the PDF rotates about the curve's box centre, which the roughness \
                 jitter moves off the points' box centre the SVG uses; the placements agree \
                 to {distance} px"
            );
        } else {
            // PDF numbers are single precision (ADR-0021), far below the
            // 0.1 px the rough placements differ by.
            assert!(
                distance < 1e-4,
                "{id}: at roughness 0 the curve's box is the points' box, so the PDF places \
                 the element where the SVG does; they differ by {distance} px"
            );
        }
    }
    // The placements, for the record: the curve-centre `e f` of each rough
    // element's `cm` as the renderer computes it, which the single-precision
    // PDF carries to within 1e-4 px (ADR-0021).
    let recorded = [
        (97.23558281494505, 9.889289697332622),
        (178.18471419585404, 32.12689199348241),
        (384.58717248073543, 34.04690040389817),
        (170.7067370739422, 167.0187242376655),
        (358.39697677985663, 192.16991974942275),
        (563.7258460440808, 53.51881667524224),
    ];
    assert_eq!(matrices.len(), recorded.len());
    for (matrix, (x, y)) in matrices.iter().zip(recorded) {
        assert!(
            (matrix[4] - x).abs() < 1e-4 && (matrix[5] - y).abs() < 1e-4,
            "placement {} {} is not {x} {y}",
            matrix[4],
            matrix[5]
        );
    }
}
