//! Independent candidate semantics reached through actual SVG references and
//! PDF page/resource/content/annotation references, not marker presence or pins.
use base64::Engine as _;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
type Result<T> = std::result::Result<T, String>;
fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut r = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut out = vec![0; r.output_buffer_size()];
    let i = r.next_frame(&mut out).map_err(|e| e.to_string())?;
    require(
        i.color_type == png::ColorType::Rgba && i.bit_depth == png::BitDepth::Eight,
        "source must be RGBA8",
    )?;
    out.truncate(i.buffer_size());
    Ok((i.width, i.height, out))
}
#[derive(Clone)]
struct Object {
    dictionary: String,
    stream: Option<Vec<u8>>,
}
fn objects(bytes: &[u8]) -> Result<BTreeMap<usize, Object>> {
    let mut at = 0;
    let mut result = BTreeMap::new();
    while let Some(offset) = bytes[at..].windows(7).position(|w| w == b" 0 obj\n") {
        let marker = at + offset;
        let id = std::str::from_utf8(
            bytes[at..marker]
                .rsplit(|b| b.is_ascii_whitespace())
                .find(|s| !s.is_empty())
                .ok_or("missing object ID")?,
        )
        .map_err(|e| e.to_string())?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
        let start = marker + 7;
        let end = start
            + bytes[start..]
                .windows(7)
                .position(|w| w == b"endobj\n")
                .ok_or("missing object end")?;
        let (dictionary, stream) =
            if let Some(off) = bytes[start..end].windows(7).position(|w| w == b"stream\n") {
                let dictionary = std::str::from_utf8(&bytes[start..start + off])
                    .map_err(|e| e.to_string())?
                    .to_owned();
                let body = start + off + 7;
                let n = number(&dictionary, "/Length")?;
                let end = body.checked_add(n).ok_or("stream length overflow")?;
                let data = bytes.get(body..end).ok_or("truncated PDF stream")?;
                require(
                    bytes.get(end..end + 18) == Some(b"\nendstream\nendobj\n"),
                    "PDF stream terminator",
                )?;
                at = end + 18;
                let out = if dictionary.contains("/FlateDecode") {
                    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(data, 64 * 1024 * 1024)
                        .map_err(|e| format!("PDF inflate: {e:?}"))?
                } else {
                    data.to_vec()
                };
                (dictionary, Some(out))
            } else {
                at = end + 7;
                (
                    std::str::from_utf8(&bytes[start..end])
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                    None,
                )
            };
        require(
            result.insert(id, Object { dictionary, stream }).is_none(),
            "duplicate PDF object",
        )?;
    }
    Ok(result)
}
fn tokens(d: &str) -> Vec<&str> {
    d.split(|c: char| c.is_ascii_whitespace() || matches!(c, '[' | ']'))
        .filter(|s| !s.is_empty())
        .collect()
}
fn number(d: &str, key: &str) -> Result<usize> {
    let t = tokens(d);
    let i = t
        .iter()
        .position(|v| *v == key)
        .ok_or(format!("missing {key}"))?;
    t.get(i + 1)
        .ok_or("missing number")?
        .parse()
        .map_err(|e| format!("invalid number: {e}"))
}
fn reference(d: &str, key: &str) -> Result<usize> {
    let t = tokens(d);
    let i = t
        .iter()
        .position(|v| *v == key)
        .ok_or(format!("missing reference {key}"))?;
    require(
        t.get(i + 2..i + 4) == Some(&["0", "R"]),
        "invalid PDF reference",
    )?;
    t[i + 1].parse().map_err(|e| format!("reference: {e}"))
}
fn array<'a>(d: &'a str, key: &str) -> Result<&'a str> {
    let after = d
        .split_once(key)
        .ok_or(format!("missing {key}"))?
        .1
        .trim_start();
    require(after.starts_with('['), "expected array")?;
    Ok(after[1..].split_once(']').ok_or("unterminated array")?.0)
}
fn floats(d: &str, key: &str) -> Result<Vec<f64>> {
    array(d, key)?
        .split_whitespace()
        .map(|v| v.parse().map_err(|e| format!("invalid real: {e}")))
        .collect()
}
fn object(map: &BTreeMap<usize, Object>, id: usize) -> Result<&Object> {
    map.get(&id).ok_or(format!("dangling object {id}"))
}
fn source_pixels(expected: &Value) -> Result<(usize, usize, Vec<u8>)> {
    let w = expected["raster"]["width"]
        .as_u64()
        .ok_or("missing source width")? as usize;
    let h = expected["raster"]["height"]
        .as_u64()
        .ok_or("missing source height")? as usize;
    let mut pixels = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let a = match expected["raster"]["rule"].as_str() {
                Some("red-alpha-checker") => {
                    if (x + 2 * y) % 3 == 0 {
                        0
                    } else if y < 20 {
                        255
                    } else {
                        128
                    }
                }
                Some("red-two-by-two") => {
                    if x == y {
                        255
                    } else {
                        128
                    }
                }
                _ => return Err("unknown authored raster oracle".into()),
            };
            pixels.extend([255, 0, 0, a]);
        }
    }
    Ok((w, h, pixels))
}
fn pdf_contract(bytes: &[u8], expected: &Value, source: &Value) -> Result<Value> {
    pdf_graph(&objects(bytes)?, expected, source)
}
fn pdf_graph(map: &BTreeMap<usize, Object>, expected: &Value, source: &Value) -> Result<Value> {
    let pages: Vec<_> = map
        .values()
        .filter(|o| o.dictionary.contains("/Type /Page\n"))
        .collect();
    require(pages.len() == 1, "exactly one PDF page")?;
    let page = &pages[0].dictionary;
    require(
        floats(page, "/MediaBox")? == [0., 0., 150., 90.]
            && floats(page, "/CropBox")? == [0., 0., 150., 90.],
        "page boxes",
    )?;
    let content = object(map, reference(page, "/Contents")?)?
        .stream
        .as_ref()
        .ok_or("missing page content")?;
    let content = std::str::from_utf8(content).map_err(|e| e.to_string())?;
    require(
        content.contains("/Im0 Do") && content.contains("/Svg0 Do") && content.contains("W\nn\n"),
        "page must invoke raster/nested vectors under clipping",
    )?;
    let rgb = object(map, reference(page, "/Im0")?)?;
    let mask = object(map, reference(&rgb.dictionary, "/SMask")?)?;
    let (w, h, pixels) = source_pixels(expected)?;
    let sample_count = w * h;
    for plane in [rgb, mask] {
        require(
            plane.dictionary.contains("/Subtype /Image")
                && number(&plane.dictionary, "/Width")? == w
                && number(&plane.dictionary, "/Height")? == h
                && number(&plane.dictionary, "/BitsPerComponent")? == 8,
            "image/mask plane shape",
        )?;
    }
    require(
        mask.dictionary.contains("/ColorSpace /DeviceGray"),
        "RGB/mask color spaces",
    )?;
    let space = array(&rgb.dictionary, "/ColorSpace")?;
    let icc = reference(space, "/ICCBased")?;
    require(
        tokens(space) == ["/ICCBased", &icc.to_string(), "0", "R"],
        "image requires a direct ICCBased array",
    )?;
    require(
        reference(page, "/ICCBased")? == icc,
        "image and content colour spaces must share the profile",
    )?;
    let icc_object = object(map, icc)?;
    require(
        number(&icc_object.dictionary, "/N")? == 3
            && icc_object.dictionary.contains("/Alternate /DeviceRGB"),
        "RGB ICC profile dictionary",
    )?;
    let profile = icc_object.stream.as_ref().ok_or("missing ICC stream")?;
    require(
        profile.get(16..20) == Some(b"RGB ") && profile.get(36..40) == Some(b"acsp"),
        "sRGB profile",
    )?;
    let catalog = map
        .values()
        .find(|o| o.dictionary.contains("/Type /Catalog"))
        .ok_or("missing catalog")?;
    let intents = array(&catalog.dictionary, "/OutputIntents")?;
    require(
        tokens(intents).len() == 3,
        "exactly one attached output intent",
    )?;
    let intent = object(map, reference(&catalog.dictionary, "/OutputIntents")?)?;
    require(
        intent.dictionary.contains("/Type /OutputIntent"),
        "attached output intent type",
    )?;
    require(
        reference(&intent.dictionary, "/DestOutputProfile")? == icc,
        "output intent must bind the used RGB space",
    )?;
    let colors = rgb.stream.as_ref().ok_or("missing RGB plane")?;
    let alphas = mask.stream.as_ref().ok_or("missing alpha plane")?;
    require(
        colors.len() == w * h * 3 && alphas.len() == w * h,
        "plane lengths",
    )?;
    for (i, p) in pixels.chunks_exact(4).enumerate() {
        require(
            alphas[i] == p[3],
            "attached PDF alpha sample differs from authored oracle",
        )?;
        let wanted = if p[3] == 0 {
            [0; 3]
        } else {
            [p[0], p[1], p[2]]
        };
        require(
            colors[i * 3..i * 3 + 3] == wanted,
            "PDF RGB sample differs from authored oracle",
        )?;
    }
    require(
        map.values()
            .filter(|o| o.dictionary.contains("/Subtype /Image"))
            .count()
            == 2,
        "no undeclared/full-page raster image",
    )?;
    let form = object(map, reference(page, "/Svg0")?)?;
    require(
        form.dictionary.contains("/Subtype /Form")
            && form.stream.as_ref().is_some_and(|s| !s.is_empty()),
        "nested SVG must be a used vector form",
    )?;
    let font = object(map, reference(page, "/F2")?)?;
    let cmap = object(map, reference(&font.dictionary, "/ToUnicode")?)?
        .stream
        .as_ref()
        .ok_or("missing ToUnicode")?;
    require(
        font.dictionary.contains("/Subtype /Type0")
            && cmap.windows(12).any(|w| w == b"beginbfchar\n"),
        "selectable encoded text font",
    )?;
    let actual_text = "/ActualText <FEFF0053007400610067006500200066006F00750072>";
    require(
        content.contains(actual_text) && content.contains("BT\n/F2 ") && content.contains(" Tj"),
        "actual page text must carry Stage four",
    )?;
    let refs = array(page, "/Annots")?
        .split_whitespace()
        .collect::<Vec<_>>();
    require(refs.len() == 6, "exactly two page annotations")?;
    let mut links = BTreeMap::new();
    for triple in refs.chunks_exact(3) {
        require(triple[1..] == ["0", "R"], "annotation reference")?;
        let a = object(
            map,
            triple[0]
                .parse()
                .map_err(|e| format!("annotation id: {e}"))?,
        )?;
        require(
            a.dictionary.contains("/Subtype /Link") && a.dictionary.contains("/S /URI"),
            "annotation must be a URI link",
        )?;
        let uri = a
            .dictionary
            .split_once("/URI (")
            .ok_or("missing URI")?
            .1
            .split_once(')')
            .ok_or("unterminated URI")?
            .0;
        require(
            links
                .insert(uri.to_owned(), floats(&a.dictionary, "/Rect")?)
                .is_none(),
            "duplicate URI",
        )?;
    }
    require(
        links.len() == 2
            && links.contains_key("https://example.com/text")
            && links.contains_key("https://example.com/raster"),
        "both independent link actions",
    )?;
    require(
        links["https://example.com/raster"] == [135., 30., 150., 60.],
        "raster link must be clipped to its source image's visible frame region",
    )?;
    let text = &source["elements"][1];
    let x = text["x"].as_f64().ok_or("text x")?;
    let y = text["y"].as_f64().ok_or("text y")?;
    let w = text["width"].as_f64().ok_or("text width")?;
    let h = text["height"].as_f64().ok_or("text height")?;
    let angle = text["angle"].as_f64().ok_or("text angle")?;
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    let dx = (w * c.abs() + h * s.abs()) / 2.;
    let dy = (w * s.abs() + h * c.abs()) / 2.;
    let wanted = [
        (x + w / 2. - dx) * 0.75,
        90. - (y + h / 2. + dy) * 0.75,
        (x + w / 2. + dx) * 0.75,
        90. - (y + h / 2. - dy) * 0.75,
    ];
    require(
        links["https://example.com/text"]
            .iter()
            .zip(wanted)
            .all(|(a, b)| (a - b).abs() < 1e-5),
        "text URI must bind its own rotated rectangle",
    )?;
    require(
        map.values().any(|o| {
            o.dictionary.contains("/Type /Metadata")
                && o.stream
                    .as_ref()
                    .is_some_and(|s| s.windows(14).any(|w| w == b"embedded_scene"))
        }),
        "XMP/embedded-scene manifest",
    )?;
    let files: Vec<_> = map
        .values()
        .filter(|o| o.dictionary.contains("/Type /Filespec"))
        .collect();
    require(files.len() == 1, "one embedded scene filespec")?;
    let ef = files[0]
        .dictionary
        .split_once("/EF <<")
        .ok_or("missing attached file reference")?
        .1;
    let attached = object(map, reference(ef, "/F")?)?;
    require(
        attached.dictionary.contains("/Type /EmbeddedFile"),
        "attachment type",
    )?;
    let original: Value =
        serde_json::from_slice(attached.stream.as_ref().ok_or("attachment stream")?)
            .map_err(|e| e.to_string())?;
    require(
        &original == source,
        "embedded scene preserves source values",
    )?;
    Ok(
        json!({"page_count":1,"image_count":2,"attached_alpha_samples":sample_count,"links":links,"text":"Stage four","nested_vector_form":true,"output_intent_bound":true,"embedded_scene_preserved":true}),
    )
}
pub fn verify(root: &Path) -> Result<Value> {
    let read = |name: &str| fs::read(root.join(name)).map_err(|e| e.to_string());
    let source: Value = serde_json::from_slice(&read("scene.json")?).map_err(|e| e.to_string())?;
    let expected: Value =
        serde_json::from_slice(&read("semantic-expectations.json")?).map_err(|e| e.to_string())?;
    let (w, h, pixels) = source_pixels(&expected)?;
    let (sw, sh, actual) = png(&read("raster.png")?)?;
    require(
        (sw as usize, sh as usize) == (w, h) && actual == pixels,
        "source raster differs from independently authored samples",
    )?;
    let svg = read("native.svg")?;
    let text = std::str::from_utf8(&svg).map_err(|e| e.to_string())?;
    let xml = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    require(
        xml.descendants().filter(|n| n.has_tag_name("a")).count() == 2,
        "exactly two SVG hyperlinks",
    )?;
    let a = |uri| {
        xml.descendants()
            .find(|n| n.has_tag_name("a") && n.attribute("href") == Some(uri))
            .ok_or("missing active SVG link")
    };
    let caption = a("https://example.com/text")?;
    require(
        caption.descendants().any(|n| {
            n.has_tag_name("text")
                && n.descendants()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .collect::<String>()
                    == "Stage four"
        }),
        "text link must wrap actual text",
    )?;
    let image_link = a("https://example.com/raster")?;
    let use_node = image_link
        .descendants()
        .find(|n| n.has_tag_name("use"))
        .ok_or("raster link does not invoke image")?;
    let id = use_node
        .attribute("href")
        .and_then(|s| s.strip_prefix('#'))
        .ok_or("missing image symbol ref")?;
    let symbol = xml
        .descendants()
        .find(|n| n.attribute("id") == Some(id))
        .ok_or("dangling image symbol")?;
    let image = symbol
        .descendants()
        .find(|n| n.has_tag_name("image"))
        .ok_or("symbol has no raster")?;
    let url = image.attribute("href").ok_or("missing raster href")?;
    let b = base64::engine::general_purpose::STANDARD
        .decode(
            url.strip_prefix("data:image/png;base64,")
                .ok_or("noncanonical raster URL")?,
        )
        .map_err(|e| e.to_string())?;
    require(
        b == read("raster.png")?,
        "SVG must preserve every raster octet",
    )?;
    require(
        use_node
            .ancestors()
            .any(|n| n.attribute("clip-path").is_some()),
        "used SVG raster must actually be clipped",
    )?;
    require(
        xml.descendants()
            .filter(|n| n.has_tag_name("image"))
            .count()
            == 2,
        "exact raster/nested SVG source inventory",
    )?;
    let pdf = pdf_contract(&read("native.pdf")?, &expected, &source)?;
    Ok(
        json!({"schema":"excalidraw-render/representative-semantics/v1","svg":{"both_links_bound":true,"source_raster_octets_preserved":true,"frame_clip_used":true},"pdf":pdf}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detached_or_corrupted_semantic_objects_never_pass() {
        let root = crate::conformance::repo_root()
            .join("tests/conformance/candidates/stage4-frames-text-assets-v4");
        let source: Value =
            serde_json::from_slice(&fs::read(root.join("scene.json")).unwrap()).unwrap();
        let expected: Value =
            serde_json::from_slice(&fs::read(root.join("semantic-expectations.json")).unwrap())
                .unwrap();
        let mut base = objects(&fs::read(root.join("native.pdf")).unwrap()).unwrap();
        assert!(
            pdf_graph(&base, &expected, &source).is_err(),
            "historical image resource alias must not qualify"
        );
        let page = *base
            .iter()
            .find(|(_, o)| o.dictionary.contains("/Type /Page\n"))
            .unwrap()
            .0;
        let rgb = reference(&base[&page].dictionary, "/Im0").unwrap();
        let icc = reference(&base[&page].dictionary, "/ICCBased").unwrap();
        base.get_mut(&rgb).unwrap().dictionary = base[&rgb].dictionary.replace(
            "/ColorSpace /CS1",
            &format!("/ColorSpace [/ICCBased {icc} 0 R]"),
        );
        pdf_graph(&base, &expected, &source).unwrap();
        let mask = reference(&base[&rgb].dictionary, "/SMask").unwrap();
        for change in 0..9 {
            let mut map = base.clone();
            match change {
                0 => {
                    let image = map.get_mut(&rgb).unwrap();
                    image.dictionary = image.dictionary.replace("/SMask ", "/Detached ");
                }
                1 => map.get_mut(&rgb).unwrap().stream.as_mut().unwrap()[0] ^= 1,
                2 => map.get_mut(&mask).unwrap().stream.as_mut().unwrap()[0] ^= 1,
                3 => {
                    for o in map.values_mut() {
                        o.dictionary = o
                            .dictionary
                            .replace("https://example.com/text", "https://example.com/swapped")
                            .replace("https://example.com/raster", "https://example.com/text")
                            .replace("https://example.com/swapped", "https://example.com/raster");
                    }
                }
                4 => {
                    let p = map.get_mut(&page).unwrap();
                    p.dictionary = p.dictionary.replace("/Im0 ", "/Unused ");
                }
                5 => {
                    let extra = map[&rgb].clone();
                    map.insert(99999, extra);
                }
                6 => {
                    let image = map.get_mut(&rgb).unwrap();
                    image.dictionary = image.dictionary.replace(
                        &format!("/ColorSpace [/ICCBased {icc} 0 R]"),
                        "/ColorSpace /CS1",
                    );
                }
                7 => {
                    let image = map.get_mut(&rgb).unwrap();
                    image.dictionary = image.dictionary.replace(
                        &format!("/ICCBased {icc} 0 R"),
                        &format!("/ICCBased {mask} 0 R"),
                    );
                }
                8 => {
                    let catalog = map
                        .values_mut()
                        .find(|o| o.dictionary.contains("/Type /Catalog"))
                        .unwrap();
                    let intent = reference(&catalog.dictionary, "/OutputIntents").unwrap();
                    catalog.dictionary = catalog.dictionary.replace(
                        &format!("/OutputIntents [{intent} 0 R]"),
                        &format!("/OutputIntents [{mask} 0 R]"),
                    );
                }
                _ => unreachable!(),
            }
            assert!(
                pdf_graph(&map, &expected, &source).is_err(),
                "mutation {change}"
            );
        }
    }
}
