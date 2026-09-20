//! Image-only facade regression: no earlier colour operator may prime a viewer
//! cache for an illegal image-XObject resource alias (PDF 32000-1 §7.8.3).
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::json;
#[allow(dead_code)]
#[path = "common/pdf_inspect.rs"]
mod pdf_inspect;

fn reference(dictionary: &str, key: &str) -> usize {
    let tokens: Vec<_> = dictionary
        .split(|c: char| c.is_ascii_whitespace() || matches!(c, '[' | ']'))
        .filter(|s| !s.is_empty())
        .collect();
    let at = tokens.iter().position(|t| *t == key).unwrap();
    assert_eq!(&tokens[at + 2..at + 4], ["0", "R"]);
    tokens[at + 1].parse().unwrap()
}
fn object(pdf: &str, id: usize) -> &str {
    pdf.split_once(&format!("\n{id} 0 obj\n"))
        .unwrap()
        .1
        .split_once("endobj\n")
        .unwrap()
        .0
}
fn image_pdf(size: u32, partial_alpha: bool) -> Vec<u8> {
    let mut rgba = Vec::new();
    for y in 0..size {
        for x in 0..size {
            let rgb = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]]
                [usize::from(y >= size / 2) * 2 + usize::from(x >= size / 2)];
            rgba.extend(rgb);
            rgba.push(if partial_alpha && x >= size / 2 {
                128
            } else {
                255
            });
        }
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
    }
    let source = json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":40,"height":40,"fileId":"source","status":"saved","scale":[1,1],"updated":1}],"appState":{},"files":{"source":{"id":"source","mimeType":"image/png","dataURL":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(png)),"created":1}}});
    let scene = SceneDocument::parse(&serde_json::to_vec(&source).unwrap()).unwrap();
    let result = Renderer::new()
        .render(
            &scene,
            &RenderRequest::builder(OutputFormat::Pdf)
                .padding(0.)
                .include_background(false)
                .build()
                .unwrap(),
        )
        .unwrap();
    assert!(result.report().is_empty());
    result.artifact().bytes().to_vec()
}
fn assert_binding(bytes: &[u8]) {
    let pdf = pdf_inspect::pdf_text(bytes);
    let page = object(&pdf, 3);
    let content = object(&pdf, reference(page, "/Contents"));
    assert!(!content.contains("/CS1 cs") && !content.contains("/CS1 CS"));
    let image = object(&pdf, reference(page, "/Im0"));
    assert!(
        image.contains("/ColorSpace [/ICCBased "),
        "image must carry a self-contained colour-space value, not /CS1"
    );
    let icc = reference(image, "/ICCBased");
    let profile = object(&pdf, icc);
    assert!(profile.contains("/N 3\n/Alternate /DeviceRGB"));
    assert_eq!(
        reference(page, "/ICCBased"),
        icc,
        "legal content resource uses the same profile"
    );
    let intent = object(&pdf, reference(object(&pdf, 1), "/OutputIntents"));
    assert_eq!(reference(intent, "/DestOutputProfile"), icc);
    let mask = object(&pdf, reference(image, "/SMask"));
    assert!(mask.contains("/ColorSpace /DeviceGray"));
}
#[test]
fn unprimed_source_images_bind_their_icc_profile_directly_to_the_output_intent() {
    for size in [2, 40] {
        for partial_alpha in [false, true] {
            assert_binding(&image_pdf(size, partial_alpha));
        }
    }
}
#[test]
#[ignore = "requires unchanged pinned PDFium and PDF.js harnesses"]
fn unprimed_source_colours_pass_both_pinned_viewers() {
    use std::{fs, path::Path, process::Command};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let out = root.join(format!(
        "target/pdf-image-icc-controls-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&out).unwrap();
    for size in [2, 40] {
        for partial in [false, true] {
            let bytes = image_pdf(size, partial);
            assert_binding(&bytes);
            assert_eq!(bytes, image_pdf(size, partial));
            let pdf = out.join(format!("source-{size}-{partial}.pdf"));
            fs::write(&pdf, bytes).unwrap();
            for viewer in ["pdfium", "pdfjs"] {
                let mut prior = None;
                for run in 0..2 {
                    let output = out.join(format!("source-{size}-{partial}-{viewer}-{run}.rgba"));
                    let result = Command::new("bash")
                        .current_dir(root)
                        .args([
                            format!("tools/pdf-harness/run-{viewer}.sh"),
                            "render".into(),
                            pdf.to_str().unwrap().into(),
                            output.to_str().unwrap().into(),
                            "40".into(),
                            "40".into(),
                        ])
                        .output()
                        .unwrap();
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    fs::write(output.with_extension("runtime.json"), result.stdout).unwrap();
                    let rgba = fs::read(output).unwrap();
                    assert_eq!(rgba.len(), 6400);
                    let right = if partial { 128 } else { 255 };
                    for ((x, y), expected) in
                        [(5, 5), (34, 5), (5, 34), (34, 34)].into_iter().zip([
                            [255, 0, 0, 255],
                            [0, 255, 0, right],
                            [0, 0, 255, 255],
                            [255, 255, 0, right],
                        ])
                    {
                        assert_eq!(
                            &rgba[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4],
                            expected,
                            "{viewer} size {size} partial {partial} at {x},{y}"
                        );
                    }
                    if let Some(first) = &prior {
                        assert_eq!(first, &rgba);
                    }
                    prior = Some(rgba);
                }
            }
        }
    }
    println!("retained unprimed ICC controls: {}", out.display());
}
