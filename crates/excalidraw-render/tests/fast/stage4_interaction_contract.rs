//! Stage-4 interaction contracts. Representative-scene approval is a separate
//! obligation: this authored tracer deliberately makes no approval claim.
use crate::common::lower_hex;
use base64::Engine as _;
use excalidraw_render::{
    ColorMode, ErrorCode, FrameOptions, FrameRenderRequest, MetadataOptions, OutputFormat,
    RenderRequest, Renderer, ResourceLimits, SceneDocument,
};
use serde_json::{json, Value};
const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];

#[test]
fn source_image_projection_keeps_top_rows_and_canvas_filter_weights() {
    let root = crate::common::repository_root()
        .join("tests/conformance/candidates/stage4-frames-text-assets");
    let scene = SceneDocument::parse(&std::fs::read(root.join("scene.json")).unwrap()).unwrap();
    let request = |format| {
        RenderRequest::builder(format)
            .padding(0.)
            .include_background(false)
            .frame_options(FrameOptions::default().with_name(false).with_outline(false))
            .build()
            .unwrap()
    };
    let pdf = Renderer::new()
        .render(&scene, &request(OutputFormat::Pdf))
        .unwrap();
    let text = crate::common::pdf_inspect::pdf_text(pdf.artifact().bytes());
    assert!(
        // The element-space matrix 40 0 0 -40 0 40 for the element at
        // (180, 40), in top-down page points (ADR-0021): the top row lands at
        // the element's top edge, page y 60.
        text.contains("30 0 0 30 135 30 cm\n/x0 Do"),
        "top-down samples must map to the element top"
    );
    assert_eq!(
        text.matches("/Interpolate true").count(),
        2,
        "RGB and its attached alpha plane use the same interpolation"
    );
    let png = Renderer::new()
        .render(&scene, &request(OutputFormat::Png))
        .unwrap();
    let image = crate::common::raster::decode_png(png.artifact().bytes());
    let expected = [
        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 247, 239, 239, 231, 223, 215, 207,
        207, 199, 191, 183, 175, 175, 167, 159, 151, 143, 143, 135, 128, 128, 128, 128, 128, 128,
        128, 128, 128, 128,
    ];
    for (y, alpha) in expected.into_iter().enumerate() {
        assert_eq!(image.pixel(180, y + 40)[3], alpha, "row {y}");
    }
}

// The retired v1 artifact/manifest test was not an approval-aware corpus
// consumer. Its replacement is xtask's candidate-executions command, exercised
// by xtask/tests/representative_candidate.rs: approved cases replay fresh native
// SVG/PNG/PDF and bind the reports, independent semantics and viewer judgments.
fn parse(source: &Value) -> SceneDocument {
    SceneDocument::parse(&serde_json::to_vec(source).unwrap()).unwrap()
}
fn file(bytes: &[u8], mime: &str) -> Value {
    json!({"id":"asset","mimeType":mime,"dataURL":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)),"created":1})
}
fn silent(report: &excalidraw_render::CompatibilityReport) {
    use sha2::{Digest, Sha256};
    for (bytes, expected) in [
        (
            include_bytes!("../common/codec_fixtures.rs").as_slice(),
            "e85ca0eac6e85596a5ece930e2c8a0470dce63761f0a122927a7ce8b4f36c509",
        ),
        (
            include_bytes!("../common/pdf_inspect.rs").as_slice(),
            "28e13e1b91eacaf9743e41a1f43aecc1ecb7cab0020d9ac21c0368377500cda2",
        ),
        (
            include_bytes!("../common/raster.rs").as_slice(),
            "46a6ed5daacc715f1f3c28c2b806b95a04e13206b4bc7f6d13c0ea50c99b8e10",
        ),
    ] {
        assert_eq!(lower_hex(&Sha256::digest(bytes)), expected);
    }
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        json!({"schema_version":1,"diagnostics":[]})
    );
}

#[test]
fn nested_svg_dark_mode_inverts_nested_content_in_every_format() {
    for (index, content) in [
        r##"<rect width="20" height="20" fill="#1e1e1e"/>"##.to_owned(),
        format!(r#"<image width="20" height="20" href="{}"/>"#,file(&crate::common::codec_fixtures::solid_png(2,2,[30,30,30,255]),"image/png")["dataURL"].as_str().unwrap()),
        r##"<defs><linearGradient id="g"><stop stop-color="#1e1e1e"/><stop offset="1" stop-color="#1e1e1e"/></linearGradient></defs><rect width="20" height="20" fill="url(#g)"/>"##.to_owned(),
    ].into_iter().enumerate() {
        let nested=format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">{content}</svg>"#);
        let source=json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":20,"height":20,"fileId":"asset","status":"saved","scale":[1,1],"updated":1}],"appState":{},"files":{"asset":file(nested.as_bytes(),"image/svg+xml")}});
        for format in FORMATS {
            let render=|mode|Renderer::new().render(&parse(&source),&RenderRequest::builder(format).padding(0.0).include_background(false).color_mode(mode).build().unwrap()).unwrap();
            let light=render(ColorMode::Light);let dark=render(ColorMode::Dark);let again=render(ColorMode::Dark);
            silent(light.report());silent(dark.report());assert_eq!(dark.report(),again.report());assert_eq!(dark.artifact().bytes(),again.artifact().bytes());
            match format {
                OutputFormat::Pdf=>{
                    // #154: PDF lowers dark mode into the nested content, vector
                    // paints, gradient stops and nested raster pixels alike.
                    assert_ne!(light.artifact().bytes(),dark.artifact().bytes(),"nested content stays light");
                    if index != 1 {let text=crate::common::pdf_inspect::pdf_text(dark.artifact().bytes());assert!(text.contains("0.827451 0.827451 0.827451"),"lowered #d3d3d3 missing");}
                    let pdf=crate::common::pdf_inspect::pdf_text(dark.artifact().bytes());assert_eq!(pdf.contains("/Subtype /Image"),index==1,"only the nested raster is an image; the rest stays vector");
                }
                OutputFormat::Png=>{
                    let l=crate::common::raster::decode_png(light.artifact().bytes());let d=crate::common::raster::decode_png(dark.artifact().bytes());
                    assert_eq!((d.width,d.height),(20,20));assert_eq!(l.pixel(10,10),[30,30,30,255]);assert_eq!(d.pixel(10,10),[211,211,211,255]);
                }
                OutputFormat::Svg=>{let s=std::str::from_utf8(dark.artifact().bytes()).unwrap();assert!(s.contains(" filter=\"invert(93%) hue-rotate(180deg)\">"));assert!(!s.contains("filter=\"url(#"));assert_ne!(dark.artifact().bytes(),light.artifact().bytes());}
                _=>unreachable!(),
            }
        }
    }
}

#[test]
fn lazy_frames_reset_limits_replay_assets_and_keep_errors_atomic() {
    let png = crate::common::codec_fixtures::solid_png(3, 2, [255, 0, 0, 255]);
    let mut elements = Vec::new();
    for (i, width) in [3, 4, 3].into_iter().enumerate() {
        elements.push(json!({"id":format!("frame-{i}"),"type":"frame","x":i*10,"width":width,"height":2,"updated":1}));
        elements.push(json!({"id":format!("image-{i}"),"type":"image","x":i*10,"width":3,"height":2,"frameId":format!("frame-{i}"),"fileId":"asset","status":"saved","scale":[1,1],"updated":1}));
    }
    let scene = parse(
        &json!({"type":"excalidraw","version":2,"elements":elements,"appState":{},"files":{"asset":file(&png,"image/png")}}),
    );
    let frames: Vec<_> = scene.frames().collect();
    for format in FORMATS {
        let (limits, code) = match format {
            OutputFormat::Svg => (
                ResourceLimits::builder().max_svg_dimension(3.0),
                ErrorCode::RESOURCE_SVG_DIMENSIONS_EXCEEDED,
            ),
            OutputFormat::Png => (
                ResourceLimits::builder().max_raster_surface_pixels(6),
                ErrorCode::RESOURCE_RASTER_SURFACE_EXCEEDED,
            ),
            OutputFormat::Pdf => (
                ResourceLimits::builder().max_pdf_page_points(2.25),
                ErrorCode::RESOURCE_PDF_PAGE_DIMENSIONS_EXCEEDED,
            ),
            _ => unreachable!(),
        };
        // Every target may decode the one 6-pixel identity; the lazy cache must
        // replay its charge, but earlier targets cannot consume the next budget.
        let request = FrameRenderRequest::builder(format)
            .include_background(false)
            .resource_limits(
                limits
                    .max_decoded_pixels_per_asset(6)
                    .max_total_decoded_pixels(6)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let renderer = Renderer::new();
        let mut batch = renderer
            .render_frames(&scene, [frames[2], frames[1], frames[0]], &request)
            .unwrap();
        for (i, frame) in frames.iter().enumerate() {
            let direct = renderer.render_frame(*frame, &request);
            let entry = batch.next().unwrap();
            assert_eq!(entry.frame().ordinal(), i as u32 + 1);
            let replay = entry.into_result();
            if i == 1 {
                let direct = direct.unwrap_err();
                let replay = replay.unwrap_err();
                assert_eq!(direct.descriptor().code(), code);
                assert_eq!(direct.descriptor(), replay.descriptor());
                assert_eq!(direct.report(), replay.report());
            } else {
                let direct = direct.unwrap();
                let replay = replay.unwrap();
                silent(direct.report());
                assert_eq!(direct.report(), replay.report());
                assert_eq!(direct.artifact().bytes(), replay.artifact().bytes());
                if format == OutputFormat::Png {
                    assert_eq!(
                        crate::common::raster::decode_png(direct.artifact().bytes()).rgba,
                        [255, 0, 0, 255].repeat(6)
                    );
                }
            }
        }
        assert!(batch.next().is_none());
        assert!(batch.global_diagnostics().is_empty());
    }
}

#[test]
fn representative_frames_text_assets_links_metadata_tracer() {
    let png = crate::common::codec_fixtures::solid_png(3, 2, [255, 0, 0, 255]);
    let nested=br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#0000ff"/></svg>"##;
    let source = json!({"type":"excalidraw","version":2,"elements":[
        {"id":"frame","type":"frame","width":200,"height":100,"updated":1},
        {"id":"text","type":"text","x":10,"y":10,"width":80,"height":15,"text":"Stage four","fontFamily":3,"fontSize":12,"lineHeight":1.25,"frameId":"frame","link":"https://example.com/text","updated":1},
        {"id":"raster","type":"image","x":180,"y":30,"width":40,"height":20,"fileId":"asset","status":"saved","scale":[1,1],"frameId":"frame","link":"https://example.com/raster","updated":1},
        {"id":"nested","type":"image","x":50,"y":50,"width":20,"height":20,"fileId":"nested","status":"saved","scale":[1,1],"frameId":"frame","updated":1}
    ],"appState":{},"files":{"asset":file(&png,"image/png"),"nested":file(nested,"image/svg+xml")}});
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .include_background(false)
            .frame_options(FrameOptions::default().with_name(false).with_outline(false))
            .metadata_options(
                MetadataOptions::default()
                    .with_embedded_scene(true)
                    .with_provenance(true)
                    .with_diagnostic_summary(true),
            )
            .build()
            .unwrap();
        let first = Renderer::new().render(&parse(&source), &request).unwrap();
        let second = Renderer::new().render(&parse(&source), &request).unwrap();
        silent(first.report());
        assert_eq!(first.report(), second.report());
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        let bytes = if format == OutputFormat::Pdf {
            crate::common::pdf_inspect::inflate_streams(first.artifact().bytes())
        } else {
            first.artifact().bytes().to_vec()
        };
        assert!(bytes
            .windows(b"embedded_scene".len())
            .any(|w| w == b"embedded_scene"));
        match format {
            OutputFormat::Svg => {
                let doc = roxmltree::Document::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
                assert_eq!(
                    doc.descendants()
                        .filter(|n| n.has_tag_name("image"))
                        .count(),
                    2
                );
                assert!(doc.descendants().any(|n| n.has_tag_name("clipPath")));
                assert!(doc.descendants().any(|n| n.has_tag_name("a")
                    && n.attribute("href") == Some("https://example.com/text")));
                assert!(doc.descendants().any(|n| n.has_tag_name("text")
                    && n.descendants()
                        .filter(|n| n.is_text())
                        .filter_map(|n| n.text())
                        .collect::<String>()
                        == "Stage four"));
            }
            OutputFormat::Png => {
                let p = crate::common::raster::decode_png(&bytes);
                assert_eq!(p.pixel(190, 40), [255, 0, 0, 255]);
                assert_eq!(p.pixel(60, 60), [0, 0, 255, 255]);
                if p.width > 201 {
                    assert_eq!(p.pixel(201, 40), [0; 4]);
                }
            }
            OutputFormat::Pdf => {
                let s = crate::common::pdf_inspect::pdf_text(&bytes);
                for marker in [
                    "/Subtype /Image",
                    "/Subtype /Link",
                    "https://example.com/text",
                    "W\nn\n",
                    "/Font",
                ] {
                    assert!(s.contains(marker), "{marker}");
                }
            }
            _ => unreachable!(),
        }
    }
}
