//! Public-API characterization of the known strict-entropy defect (#193): a
//! truncated scan is decoded as if it were intact. This holds the defect, not
//! correct decoding of a malformed image.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use jpeg_encoder::{ColorType, Encoder, SamplingFactor};

use crate::common::characterization::known_defect;

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
        // The truncated scan renders, with no diagnostic, something other than
        // the intact image; a fix that reports or refuses it is a moved pin.
        let observe = || match renderer.render(&malformed, &request) {
            Ok(actual) => Ok((
                actual
                    .report()
                    .diagnostics()
                    .iter()
                    .map(|diagnostic| diagnostic.code().as_str().to_owned())
                    .collect::<Vec<_>>(),
                actual.artifact().bytes() == expected.artifact().bytes(),
                actual.artifact().bytes().to_vec(),
            )),
            Err(error) => Err(error.code().as_str().to_owned()),
        };
        let first = observe();
        let summary = first
            .as_ref()
            .map(|(codes, intact, _)| (codes.clone(), *intact));
        known_defect("#193", &format!("{format:?}"), summary, Ok((vec![], false)));
        assert_eq!(first, observe(), "{format:?} is deterministic");
    }
}
