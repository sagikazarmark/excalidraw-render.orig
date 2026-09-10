//! The scene extent of rotated ellipses, diamonds and linear elements and of
//! curves, held to the reference's `getElementBounds` digit for digit (#172,
//! #173). Every expected `viewBox` here is the official export's: the first
//! is the approved evidence of `focused.rotated-shape-bounds.v1`, the others
//! were measured on the pinned reference profile over scratch scenes that are
//! not evidence (two fresh Chromium processes each, through the reference
//! oracle's browser bundle) while the fix was made.

use excalidraw_render::{OutputFormat, SceneDocument};

use crate::common::render::render_artifact;

/// The root `viewBox` of the scene's SVG.
fn view_box(scene: &[u8]) -> String {
    let scene = SceneDocument::parse(scene).expect("scene is admitted");
    let svg = String::from_utf8(render_artifact(&scene, OutputFormat::Svg)).expect("SVG is UTF-8");
    svg.split("viewBox=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .expect("SVG has a viewBox")
        .to_owned()
}

#[test]
fn rotated_ellipse_diamond_and_line_take_the_reference_shape_extents() {
    // The three elements of `focused.rotated-shape-bounds.v1`, each alone on
    // an edge of the scene at roughness 0: the ellipse's exact extent (not its
    // rotated box, 27 px narrower here), the diamond's four rotated vertices
    // and the line's rotated points set the official
    // `viewBox="0 0 409.20587533404796 146.3390004951341"`.
    assert_eq!(
        view_box(br##"{
          "type":"excalidraw","version":2,
          "elements":[
            {"id":"ellipse-rotated","type":"ellipse","x":30,"y":30,"width":100,"height":60,"angle":1.2,"roughness":0,"seed":11,"updated":1},
            {"id":"diamond-rotated","type":"diamond","x":190,"y":30,"width":100,"height":60,"angle":0.7853981633974483,"roughness":0,"seed":12,"updated":1},
            {"id":"line-rotated","type":"line","x":350,"y":30,"width":120,"height":40,"angle":1.2,"roughness":0,"points":[[0,0],[40,40],[80,0],[120,40]],"seed":13,"updated":1}
          ],
          "appState":{},"files":{}
        }"##),
        "0 0 409.20587533404796 146.3390004951341"
    );
}

#[test]
fn a_curve_extent_is_the_reference_analytic_extremum() {
    // #173's example: a rounded line whose extreme x lies inside its middle
    // segment. The pinned reference's `getMinMaxXYFromCurvePathOps` solves
    // each cubic's derivative (`getCubicBezierCurveBound`), and its
    // `Math.pow(x, 3)` is the platform's, rounded once; the renderer's own
    // solver and twice-rounded cube landed one ulp away
    // (`121.5571655001675`). Official `viewBox="0 0 121.55716550016749
    // 141.7167146921158"`.
    assert_eq!(
        view_box(br##"{
          "type":"excalidraw","version":2,
          "elements":[
            {"id":"bulge","type":"line","x":50,"y":50,"width":100,"height":120,"points":[[0,0],[100,60],[0,120]],"roundness":{"type":2},"seed":77,"updated":1}
          ],
          "appState":{},"files":{}
        }"##),
        "0 0 121.55716550016749 141.7167146921158"
    );
}

#[test]
fn rotated_rough_linear_elements_rotate_their_curve_about_the_points_centre() {
    // A rotated linear element's control points are placed on the page and
    // rotated about the centre of its points' box — the centre
    // `getElementAbsoluteCoords` reads while the shape cache is cold, as it is
    // when `getCanvasSize` measures a freshly restored scene — before their
    // analytic extrema are taken. A four-point line at roughness 1 and 1.2
    // rad, a cartoonist arrow with a `dot` and an `arrow` head at 0.7 rad
    // (whose heads take no part in the extent), a two-point arrow at 0.9 rad,
    // and a filled closed line at 0.5 rad; the curve-box centre lands the
    // second and fourth in another last digit (`125.35508667300522`,
    // `114.17788188143996`).
    for (scene, expected) in [
        (
            br##"{"type":"excalidraw","version":2,"elements":[
              {"id":"rot-line","type":"line","x":100,"y":100,"width":120,"height":40,"angle":1.2,"points":[[0,0],[40,40],[80,0],[120,40]],"seed":101,"updated":1}
            ],"appState":{},"files":{}}"## as &[u8],
            "0 0 71.77587361775602 146.3390004951341",
        ),
        (
            br##"{"type":"excalidraw","version":2,"elements":[
              {"id":"rot-arrow","type":"arrow","x":100,"y":100,"width":140,"height":30,"angle":0.7,"roughness":2,"points":[[0,0],[70,-30],[140,0]],"startArrowhead":"dot","endArrowhead":"arrow","seed":102,"updated":1}
            ],"appState":{},"files":{}}"##,
            "0 0 125.35508667300519 111.19686986718497",
        ),
        (
            br##"{"type":"excalidraw","version":2,"elements":[
              {"id":"two","type":"arrow","x":50,"y":50,"width":140,"height":0,"angle":0.9,"points":[[0,0],[140,0]],"seed":109,"updated":1}
            ],"appState":{},"files":{}}"##,
            "0 0 107.02539555789303 129.6657673478477",
        ),
        (
            br##"{"type":"excalidraw","version":2,"elements":[
              {"id":"loop","type":"line","x":50,"y":50,"width":80,"height":50,"angle":0.5,"backgroundColor":"#e03131","points":[[0,0],[80,0],[80,50],[0,50],[0,0]],"seed":107,"updated":1}
            ],"appState":{},"files":{}}"##,
            "0 0 114.17788188143999 102.23317118285487",
        ),
    ] {
        assert_eq!(view_box(scene), expected);
    }
}

#[test]
fn rotated_rough_ellipse_diamond_and_curve_share_one_scene() {
    // The rotated-shape-bounds layout at roughness 1 with the line rounded:
    // the ellipse's extent ignores its jitter, the diamond's vertices are the
    // exact midpoints (not the drawn `floor(extent / 2) + 1`), and the curve's
    // rotated cubics set the right edge.
    assert_eq!(
        view_box(br##"{
          "type":"excalidraw","version":2,
          "elements":[
            {"id":"ell","type":"ellipse","x":30,"y":30,"width":100,"height":60,"angle":1.2,"seed":104,"updated":1},
            {"id":"dia","type":"diamond","x":190,"y":30,"width":100,"height":60,"angle":0.7853981633974483,"seed":105,"updated":1},
            {"id":"curve","type":"line","x":350,"y":30,"width":120,"height":40,"angle":1.2,"roundness":{"type":2},"points":[[0,0],[40,40],[80,0],[120,40]],"seed":106,"updated":1}
          ],
          "appState":{},"files":{}
        }"##),
        "0 0 410.803351769544 146.59503327141297"
    );
}

#[test]
fn a_rotated_elbow_arrow_at_roughness_zero_rotates_its_route_points() {
    // An elbow arrow keeps the renderer's rule, its declared route's points
    // rotated about their box's centre, which is the reference's extent for a
    // roughness-0 elbow arrow (its Rough.js `linearPath` over the points has
    // no jitter to add).
    assert_eq!(
        view_box(br##"{
          "type":"excalidraw","version":2,
          "elements":[
            {"id":"elbow0","type":"arrow","x":50,"y":50,"width":300,"height":90,"angle":0.4,"roughness":0,"points":[[0,0],[150,0],[150,90],[300,90]],"elbowed":true,"seed":108,"updated":1}
          ],
          "appState":{},"files":{}
        }"##),
        "0 0 261.27064739308696 219.72099215285482"
    );
}
