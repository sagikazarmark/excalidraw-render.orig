//! Public-API characterization of the unresolved strict-entropy defect (#193).
//! This pins a known limitation, not correct decoding of a malformed image.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use jpeg_encoder::{ColorType, Encoder, SamplingFactor};

fn scene(jpeg: &[u8]) -> SceneDocument {
    let payload = base64::engine::general_purpose::STANDARD.encode(jpeg);
    let source = serde_json::json!({
        "type":"excalidraw", "version":2,
        "elements":[{"id":"image", "type":"image", "x":0, "y":0,
            "width":9, "height":9, "fileId":"asset", "status":"saved",
            "scale":[1,1], "seed":1, "updated":1}],
        "appState":{}, "files":{"asset":{"id":"asset", "mimeType":"image/jpeg",
            "dataURL":format!("data:image/jpeg;base64,{payload}"), "created":1}}
    });
    SceneDocument::parse(&serde_json::to_vec(&source).unwrap()).unwrap()
}

#[test]
fn issue_193_truncated_scan_is_still_accepted_without_decode_failure() {
    let mut jpeg = Vec::new();
    let mut encoder = Encoder::new(&mut jpeg, 100);
    encoder.set_sampling_factor(SamplingFactor::F_4_2);
    encoder
        .encode(&[96, 128, 128].repeat(81), 9, 9, ColorType::Ycbcr)
        .unwrap();
    let scans: Vec<_> = jpeg
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| *pair == [0xff, 0xda])
        .map(|(offset, _)| offset)
        .collect();
    assert_eq!(scans.len(), 3);
    let first = scans[0];
    let length = u16::from_be_bytes([jpeg[first + 2], jpeg[first + 3]]) as usize;
    let entropy = first + 2 + length;
    assert_eq!(scans[1] - entropy, 6);
    let mut truncated = jpeg[..entropy + 1].to_vec();
    truncated.extend_from_slice(&jpeg[scans[1]..]);
    let valid = scene(&jpeg);
    let malformed = scene(&truncated);
    let renderer = Renderer::new();
    for format in [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf] {
        let request = RenderRequest::builder(format).build().unwrap();
        let expected = renderer.render(&valid, &request).unwrap();
        let actual = renderer.render(&malformed, &request).unwrap();
        assert!(
            actual.report().diagnostics().is_empty(),
            "#193 behavior changed; review the known limitation: {:?}",
            actual.report()
        );
        assert_ne!(actual.artifact().bytes(), expected.artifact().bytes());
        assert_eq!(
            actual.artifact().bytes(),
            renderer
                .render(&malformed, &request)
                .unwrap()
                .artifact()
                .bytes()
        );
    }
}
