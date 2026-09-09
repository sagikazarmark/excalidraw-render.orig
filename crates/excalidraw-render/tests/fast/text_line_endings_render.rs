//! A text element's `text` breaks lines on `\r\n` and `\r` as well as `\n`,
//! as the reference's `text.replace(/\r\n?/g, "\n").split("\n")` does, while
//! the stored bytes stay as they are. Decided on #110. The line splitting and
//! layout are tested at `text::OfficialFont::layout` (`render/tests.rs`); this
//! render proves the split reaches the raster and raises nothing.

use excalidraw_render::{OutputFormat, SceneDocument};

use crate::common::render::render;

/// One Excalifont text element at 20 px and line height 1.25.
fn text_scene(text: &str) -> SceneDocument {
    let scene = format!(
        r##"{{"type":"excalidraw","version":2,"elements":[{{"id":"text","type":"text","x":0,"y":0,"width":100,"height":50,"fontSize":20,"fontFamily":5,"lineHeight":1.25,"text":{text},"originalText":{text},"seed":1,"updated":1}}],"appState":{{}},"files":{{}}}}"##,
        text = serde_json::to_string(text).expect("text serializes"),
    );
    SceneDocument::parse(scene.as_bytes()).expect("text scene is admitted")
}

#[test]
fn line_endings_do_not_change_the_raster_or_the_diagnostics() {
    let png = render(&text_scene("a\nb"), OutputFormat::Png);
    assert!(png.artifact().bytes().starts_with(b"\x89PNG"));
    assert!(png.report().is_empty(), "{:?}", png.report());
    for text in ["a\r\nb", "a\rb"] {
        let other = render(&text_scene(text), OutputFormat::Png);
        assert_eq!(
            other.artifact().bytes(),
            png.artifact().bytes(),
            "{text:?} rasterizes as LF text does"
        );
        assert!(other.report().is_empty(), "{text:?}: {:?}", other.report());
    }
}
