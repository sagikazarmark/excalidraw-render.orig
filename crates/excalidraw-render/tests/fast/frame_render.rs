//! The frame family's end-to-end smokes. Name geometry, colour and truncation,
//! clip resolution and the frame controls are asserted once on the resolved
//! scene in `render::resolve_tests`; the PNG surface and shared clip masks on
//! the PNG plan in `render::plan_tests`. What stays here reaches the bytes:
//! nested clipping in every format, the grey name ink and the rounded clip
//! mask a PNG actually paints, and group membership.

use crate::common::pdf_inspect::pdf_text;
use excalidraw_render::{DiagnosticCode, FrameOptions, OutputFormat, RenderRequest, SceneDocument};

use crate::common::raster::decode_png;
use crate::common::render::render_with_request;

const NESTED_FRAME_SCENE: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"outer","type":"frame","x":0,"y":0,"width":100,"height":60,"name":"Outer","updated":1},
        {"id":"inner","type":"frame","x":30,"y":10,"width":50,"height":40,"frameId":"outer","opacity":50,"updated":1},
        {"id":"child","type":"rectangle","x":60,"y":20,"width":80,"height":20,"frameId":"inner","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"opacity":50,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

/// The #115 scene: one named 580x580 frame clipping three filled rectangles,
/// the last of which straddles the frame's right edge.
const NAMED_FRAME_SCENE: &[u8] = br##"{
    "type":"excalidraw","version":2,
    "elements":[
        {"id":"clip-frame","type":"frame","name":"Clip","x":0,"y":0,"width":580,"height":580,"roughness":0,"updated":1},
        {"id":"cell-0","type":"rectangle","x":35,"y":35,"width":30,"height":30,"frameId":"clip-frame","strokeColor":"#1971c2","backgroundColor":"#a5d8ff","fillStyle":"solid","roughness":0,"updated":1},
        {"id":"cell-1","type":"rectangle","x":75,"y":35,"width":30,"height":30,"frameId":"clip-frame","strokeColor":"#1971c2","backgroundColor":"#a5d8ff","fillStyle":"solid","roughness":0,"updated":1},
        {"id":"cell-2","type":"rectangle","x":555,"y":35,"width":30,"height":30,"frameId":"clip-frame","strokeColor":"#1971c2","backgroundColor":"#a5d8ff","fillStyle":"solid","roughness":0,"updated":1}
    ],
    "appState":{},"files":{}
}"##;

#[test]
pub(super) fn frame_name_is_painted_grey_above_the_frame_in_png() {
    let scene = SceneDocument::parse(NAMED_FRAME_SCENE).expect("named frame is admitted");
    let png = render(&scene, OutputFormat::Png, FrameOptions::default());
    let decoded = decode_png(png.artifact().bytes());

    // The name box spans y 10..27.5 in the canvas; "Clip" at 14 px is under
    // 30 px wide, so every inked pixel is inside x 10..40.
    let mut inked = 0;
    for y in 10..28 {
        for x in 10..40 {
            let [red, green, blue, alpha] = decoded.pixel(x, y);
            assert_eq!(alpha, 255);
            if [red, green, blue] == [255, 255, 255] {
                continue;
            }
            inked += 1;
            // #999999 blended with the white background stays grey and no
            // darker than #999999; #1b1b1f would break both.
            assert_eq!(red, green, "name ink is grey at ({x}, {y})");
            assert_eq!(green, blue, "name ink is grey at ({x}, {y})");
            assert!(red >= 0x99, "name ink is #999999 or lighter at ({x}, {y})");
        }
    }
    assert!(inked > 20, "the name is painted: {inked} inked pixels");
    for x in 10..40 {
        assert_eq!(
            decoded.pixel(x, 5),
            [255, 255, 255, 255],
            "nothing above the name"
        );
    }
}

#[test]
pub(super) fn nested_frames_clip_direct_geometry_in_every_format() {
    let scene = SceneDocument::parse(NESTED_FRAME_SCENE).expect("nested frame scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format, FrameOptions::default());
        assert!(
            outcome.report().diagnostics().iter().all(|diagnostic| {
                diagnostic.code() != DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
            }),
            "framed geometry must remain directly renderable in {format:?}"
        );

        match format {
            OutputFormat::Svg => {
                let svg = std::str::from_utf8(outcome.artifact().bytes()).expect("SVG is UTF-8");
                // One clip per frame, defined once in defs: the outer frame
                // clips the inner frame, the inner frame clips the child.
                assert_eq!(svg.matches("<clipPath").count(), 2);
                assert_eq!(svg.matches("<clipPath id=\"frame-clip-0\">").count(), 1);
                assert_eq!(svg.matches("<clipPath id=\"frame-clip-1\">").count(), 1);
                assert_eq!(
                    svg.matches("<g clip-path=\"url(#frame-clip-0)\">").count(),
                    1
                );
                assert_eq!(
                    svg.matches("<g clip-path=\"url(#frame-clip-1)\">").count(),
                    1
                );
                // The outline is the rounded frame rect, not a path.
                assert!(svg.contains(concat!(
                    "<rect data-frame-outline=\"outer\" width=\"100\" height=\"60\" ",
                    "rx=\"8\" ry=\"8\" fill=\"none\" stroke=\"#bbbbbb\" stroke-width=\"2\"/>"
                )));
                assert!(svg.contains("<text data-frame-name=\"outer\""));
                assert!(!svg.contains("fill=\"#f1f3f5\""));
                assert!(svg.contains("fill-opacity=\"0.25\""));
            }
            OutputFormat::Png => {
                let decoded = decode_png(outcome.artifact().bytes());
                assert_ne!(decoded.pixel(75, 64), [255, 255, 255, 255]);
                assert_eq!(decoded.pixel(100, 64), [255, 255, 255, 255]);
            }
            OutputFormat::Pdf => {
                let pdf = pdf_text(outcome.artifact().bytes());
                assert!(pdf.matches("W\nn\n").count() >= 2);
                assert!(!pdf.contains("/Subtype /Image"));
            }
            _ => unreachable!("the test enumerates every supported output format"),
        }
    }
}

/// Format-specific parity for a frame's own opacity (#137): the reference
/// SVG's frame `<rect>` carries no opacity attribute, while its canvas paints
/// the outline under `globalAlpha` from the element's render opacity. The
/// 50 % inner frame therefore serializes without `opacity` in SVG and paints
/// half-strength in PNG; its opacity still multiplies its child's in both.
#[test]
fn frame_outline_opacity_is_dropped_in_svg_and_kept_in_png() {
    let scene = SceneDocument::parse(NESTED_FRAME_SCENE).expect("nested frame scene is admitted");

    let svg_outcome = render(&scene, OutputFormat::Svg, FrameOptions::default());
    let svg = std::str::from_utf8(svg_outcome.artifact().bytes()).expect("SVG is UTF-8");
    assert!(svg.contains(concat!(
        "<g transform=\"translate(40 40.5) rotate(0 25 20)\">",
        "<rect data-frame-outline=\"inner\" width=\"50\" height=\"40\" rx=\"8\" ry=\"8\" ",
        "fill=\"none\" stroke=\"#bbbbbb\" stroke-width=\"2\"/></g>"
    )));
    assert!(
        !svg.contains(" opacity=\""),
        "no element group carries opacity"
    );
    assert!(svg.contains("fill-opacity=\"0.25\""));

    let png = render(&scene, OutputFormat::Png, FrameOptions::default());
    let decoded = decode_png(png.artifact().bytes());
    // The left edges of both outlines run through y = 60, well inside their
    // straight segments; the 2 px stroke centred on x = 10 (outer) and x = 40
    // (inner) covers those pixel columns completely.
    let outer = decoded.pixel(10, 60);
    let inner = decoded.pixel(40, 60);
    assert_eq!(outer, [0xbb, 0xbb, 0xbb, 255], "outer outline is #bbb");
    assert_eq!(
        inner,
        [0xdd, 0xdd, 0xdd, 255],
        "inner outline is #bbb at 50 % over white"
    );
}

#[test]
pub(super) fn frame_clipping_uses_the_rounded_frame_boundary() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"frame","type":"frame","x":0,"y":0,"width":100,"height":60,"updated":1},
            {"id":"child","type":"rectangle","x":0,"y":0,"width":100,"height":60,"frameId":"frame","strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("rounded frame scene is admitted");
    let outcome = render(
        &scene,
        OutputFormat::Png,
        FrameOptions::default().with_name(false).with_outline(false),
    );
    let decoded = decode_png(outcome.artifact().bytes());
    assert_eq!(decoded.pixel(11, 11), [255, 255, 255, 255]);
    assert_ne!(decoded.pixel(20, 20), [255, 255, 255, 255]);
}

#[test]
pub(super) fn group_membership_preserves_direct_paint_order_in_every_format() {
    let scene = SceneDocument::parse(br##"{
        "type":"excalidraw","version":2,
        "elements":[
            {"id":"back","type":"rectangle","x":0,"y":0,"width":40,"height":40,"groupIds":["group"],"strokeColor":"#1971c2","backgroundColor":"#1971c2","fillStyle":"solid","roughness":0,"updated":1},
            {"id":"front","type":"ellipse","x":20,"y":20,"width":40,"height":40,"groupIds":["group"],"strokeColor":"#e03131","backgroundColor":"#e03131","fillStyle":"solid","roughness":0,"opacity":50,"updated":1}
        ],
        "appState":{},"files":{}
    }"##)
    .expect("grouped scene is admitted");

    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let outcome = render(&scene, format, FrameOptions::default());
        assert!(outcome.report().diagnostics().iter().all(|diagnostic| {
            diagnostic.code() != DiagnosticCode::COMPATIBILITY_KNOWN_ELEMENT_REPLACED
        }));
        if format == OutputFormat::Pdf {
            assert!(!pdf_text(outcome.artifact().bytes()).contains("/Subtype /Image"));
        }
    }
}

fn render(
    scene: &SceneDocument,
    format: OutputFormat,
    frame_options: FrameOptions,
) -> excalidraw_render::RenderOutcome {
    let request = RenderRequest::builder(format)
        .frame_options(frame_options)
        .build()
        .expect("frame render request is valid");
    render_with_request(scene, &request)
}
