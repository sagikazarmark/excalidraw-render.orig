//! Pending red regression template. Copy to crates/excalidraw-render/tests/
//! when qualifying a production adapter; the unchanged renderer fails it.
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};

#[test]
fn fractional_rounded_rectangle_matches_reference_horizontal_edge_coverage() {
    let scene = SceneDocument::parse(include_bytes!("../../../tests/conformance/proposals/velox84-png-isolation-v1/controls/minimal-rectangle/scene.excalidraw")).unwrap();
    let outcome = Renderer::new().render(&scene, &RenderRequest::builder(OutputFormat::Png).build().unwrap()).unwrap();
    assert!(outcome.report().is_empty());
    let mut reader = png::Decoder::new(std::io::Cursor::new(outcome.artifact().bytes())).read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (1955, 515));
    // Independent pinned Chromium canvas observations, not native snapshots.
    // Coverage arithmetic can differ by one channel unit across rasterizers.
    for (y, expected) in [(33, [198,198,198,255]), (34,[30,30,30,255]), (35,[79,83,85,255])] {
        let offset = (y * info.width as usize + 1350) * 4;
        for (actual, expected) in pixels[offset..offset+4].iter().zip(expected) {
            assert!(actual.abs_diff(expected) <= 1, "row {y}: {:?}, expected {expected}", &pixels[offset..offset+4]);
        }
    }
}
