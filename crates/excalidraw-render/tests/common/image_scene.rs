//! The one-image scene the codec tests share: a saved image element of the
//! given size at the origin over one embedded file. Every codec test used to
//! carry its own copy of this JSON, differing only in the media type.

use base64::Engine as _;
use excalidraw_render::SceneDocument;

/// A base64 data URL declaring `media_type` over `bytes`.
pub fn data_url(media_type: &str, bytes: &[u8]) -> String {
    format!(
        "data:{media_type};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// A scene with one `width`x`height` image element whose file carries `bytes`
/// as `media_type`.
pub fn image_scene(bytes: &[u8], media_type: &str, width: u32, height: u32) -> SceneDocument {
    image_scene_from_data_url(&data_url(media_type, bytes), media_type, width, height)
}

/// [`image_scene`] over a data URL the test built itself, so the declared
/// `media_type` and the URL's own media type may disagree.
pub fn image_scene_from_data_url(
    data_url: &str,
    media_type: &str,
    width: u32,
    height: u32,
) -> SceneDocument {
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[{{
                "id":"image","type":"image","x":0,"y":0,
                "width":{width},"height":{height},"angle":0,"opacity":100,
                "fileId":"asset","status":"saved","scale":[1,1],"seed":1,"updated":1
            }}],
            "appState":{{}},
            "files":{{"asset":{{
                "id":"asset","mimeType":"{media_type}",
                "dataURL":"{data_url}","created":1
            }}}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("image scene is admitted")
}

/// A scene with one image element completed by `element_fields` (its `x`,
/// `y`, `width`, `height` and whatever else the test varies) over the file
/// `file` names by media type and payload, or over no file at all when `file`
/// is `None`, for a scene whose image is missing.
pub fn image_scene_with_element_fields(
    file: Option<(&str, &[u8])>,
    element_fields: &str,
) -> SceneDocument {
    let files = file.map_or_else(String::new, |(media_type, payload)| {
        format!(
            r#""asset":{{"id":"asset","mimeType":"{media_type}","dataURL":"{}","created":1}}"#,
            data_url(media_type, payload)
        )
    });
    let json = format!(
        r#"{{
            "type":"excalidraw","version":2,
            "elements":[{{
                "id":"image","type":"image","fileId":"asset","status":"saved",
                "seed":1,"updated":1,{element_fields}
            }}],
            "appState":{{}},"files":{{{files}}}
        }}"#
    );
    SceneDocument::parse(json.as_bytes()).expect("image scene is admitted")
}
