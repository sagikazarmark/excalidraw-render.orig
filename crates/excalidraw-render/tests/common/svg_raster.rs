//! Rasterizing an SVG document with the workspace `resvg`, resolving its text
//! through the faces the document itself embeds, for the parity harnesses that
//! judge an SVG as a raster: the registered-case harness and the frame-export
//! case. The workspace resvg is not the pinned pure-Rust viewer, so its output
//! is a judgment of document parity under one rasterizer, never evidence.

use std::collections::BTreeSet;
use std::sync::Arc;

use base64::Engine;
use excalidraw_render_compare::CanonicalRgba8;
use resvg::tiny_skia::{Pixmap, Transform};
use usvg::fontdb;

/// System families the official export names without embedding, and the
/// renderer's manifested substitute for each. The official raster evidence was
/// produced with whatever the reference container resolved these to; document
/// parity needs one face on both sides, and the substitute is the face the
/// renderer's own SVG embeds for the same family.
pub const SYSTEM_FAMILY_SUBSTITUTES: &[(&str, &str)] =
    &[("Helvetica", "fonts/Liberation/LiberationSans-Regular.woff2")];

/// Rasterizes one SVG document with the workspace `resvg`, resolving text
/// through the faces the document itself embeds.
pub fn rasterize(svg: &str, context: &str) -> CanonicalRgba8 {
    let mut database = fontdb::Database::new();
    let mut registered = BTreeSet::new();
    for face in embedded_faces(svg) {
        let sfnt = match face.media_type.as_str() {
            "font/woff2" => wuff::decompress_woff2(&face.bytes)
                .unwrap_or_else(|error| panic!("{context}: embedded WOFF2 decodes: {error:?}")),
            _ => face.bytes.clone(),
        };
        register_face(&mut database, sfnt, &face.family);
        registered.insert(face.family);
    }
    for family in referenced_families(svg) {
        if registered.contains(&family) {
            continue;
        }
        if let Some((_, asset)) = SYSTEM_FAMILY_SUBSTITUTES
            .iter()
            .find(|(system, _)| *system == family)
        {
            let woff2 = excalidraw_fonts::load_asset(asset)
                .expect("substitute asset verifies")
                .expect("substitute asset is manifested");
            let sfnt = wuff::decompress_woff2(woff2.bytes()).expect("substitute WOFF2 decodes");
            register_face(&mut database, sfnt, &family);
            registered.insert(family);
        }
    }

    let options = usvg::Options {
        fontdb: Arc::new(database),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(svg, &options)
        .unwrap_or_else(|error| panic!("{context}: SVG parses: {error}"));
    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height()).expect("raster size is valid");
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());

    // tiny-skia stores premultiplied pixels; the comparator wants straight
    // alpha, un-premultiplied by the canonical integer rule (spec 14.3,
    // `(c * 255 + alpha / 2) / alpha`) rather than tiny-skia's float division,
    // so that a pinned measurement here is the one the pinned viewer would take.
    let mut rgba = Vec::with_capacity(pixmap.pixels().len() * 4);
    for pixel in pixmap.pixels() {
        let alpha = u32::from(pixel.alpha());
        let straight = |channel: u8| -> u8 {
            (u32::from(channel) * 255 + alpha / 2)
                .checked_div(alpha)
                .map_or(0, |value| value.min(255) as u8)
        };
        rgba.extend_from_slice(&[
            straight(pixel.red()),
            straight(pixel.green()),
            straight(pixel.blue()),
            pixel.alpha(),
        ]);
    }
    CanonicalRgba8::new(size.width(), size.height(), rgba).expect("raster is canonical RGBA8")
}

/// Loads one sfnt and registers it under the CSS family the document uses for
/// it, whatever the font's own name table says: a subset chunk of Xiaolai is
/// `Xiaolai` to the document even when its name table says otherwise, and the
/// renderer's aliases (`ExcalidrawRender-9`) name no real family at all.
fn register_face(database: &mut fontdb::Database, sfnt: Vec<u8>, family: &str) {
    for id in database.load_font_source(fontdb::Source::Binary(Arc::new(sfnt))) {
        let mut info = database.face(id).expect("loaded face is present").clone();
        database.remove_face(id);
        info.families = vec![(family.to_owned(), fontdb::Language::English_UnitedStates)];
        database.push_face_info(info);
    }
}

/// One `@font-face` declaration's family and decoded payload.
#[derive(Debug, PartialEq)]
pub struct EmbeddedFace {
    pub family: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// Every `@font-face` in the document. Accepts both the official export's
/// spacing (`font-family: Excalifont; src: url(...);`) and the renderer's
/// (`font-family:'Excalifont';src:url(...)`).
pub fn embedded_faces(svg: &str) -> Vec<EmbeddedFace> {
    let mut faces = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find("@font-face") {
        rest = &rest[start + "@font-face".len()..];
        let Some(open) = rest.find('{') else { break };
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let block = &rest[open + 1..open + close];
        rest = &rest[open + close..];

        let family = declaration(block, "font-family")
            .expect("@font-face declares font-family")
            .trim_matches(|c| c == '\'' || c == '"')
            .to_owned();
        let src = declaration(block, "src").expect("@font-face declares src");
        let url = src
            .strip_prefix("url(")
            .and_then(|url| url.split_once(')'))
            .map(|(url, _)| url)
            .expect("src is a url()");
        let (media_type, encoded) = url
            .strip_prefix("data:")
            .and_then(|data| data.split_once(";base64,"))
            .expect("src is a base64 data URL");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("font data URL is valid base64");
        faces.push(EmbeddedFace {
            family,
            media_type: media_type.to_owned(),
            bytes,
        });
    }
    faces
}

/// The value of one `name: value;` declaration inside a CSS block. A `;` inside
/// parentheses does not end a declaration, since a data URL carries one.
fn declaration<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut entries = Vec::new();
    for (index, character) in block.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => {
                entries.push(&block[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    entries.push(&block[start..]);
    entries.into_iter().find_map(|entry| {
        let (key, value) = entry.split_once(':')?;
        (key.trim() == name).then(|| value.trim())
    })
}

/// Every family named by a `font-family="..."` attribute, split on commas, in
/// document order and without duplicates.
pub fn referenced_families(svg: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut families = Vec::new();
    let mut rest = svg;
    while let Some(start) = rest.find(" font-family=\"") {
        rest = &rest[start + " font-family=\"".len()..];
        let Some(end) = rest.find('"') else { break };
        for family in rest[..end].split(',') {
            let family = family.trim().trim_matches(|c| c == '\'' || c == '"');
            if !family.is_empty() && seen.insert(family.to_owned()) {
                families.push(family.to_owned());
            }
        }
        rest = &rest[end..];
    }
    families
}
