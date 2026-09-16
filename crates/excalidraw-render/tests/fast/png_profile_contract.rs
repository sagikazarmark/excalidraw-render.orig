//! Authored PNG/APNG sample vectors. Chunk framing, scanline filters and Adam7
//! packing are independent of the production decoder; expected pixels are not
//! obtained from a renderer, decoder, inventory policy or snapshot refresh.
use base64::Engine as _;
use excalidraw_render::{OutputFormat, RenderRequest, Renderer, SceneDocument};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Container {
    Static,
    IncludedDefault,
    SeparateDefault,
}

const FORMATS: [OutputFormat; 3] = [OutputFormat::Svg, OutputFormat::Png, OutputFormat::Pdf];
const LAYOUTS: &[(&str, u8, u8)] = &[
    ("gray1", 0, 1),
    ("gray2", 0, 2),
    ("gray4", 0, 4),
    ("gray8", 0, 8),
    ("gray16", 0, 16),
    ("rgb8", 2, 8),
    ("rgb16", 2, 16),
    ("indexed1", 3, 1),
    ("indexed2", 3, 2),
    ("indexed4", 3, 4),
    ("indexed8", 3, 8),
    ("gray-alpha8", 4, 8),
    ("gray-alpha16", 4, 16),
    ("rgba8", 6, 8),
    ("rgba16", 6, 16),
];
fn transparency(color: u8) -> &'static [&'static str] {
    match color {
        0 | 2 => &["none", "key"],
        3 => &["none", "palette-prefix", "palette-full"],
        4 | 6 => &["intrinsic"],
        _ => unreachable!(),
    }
}
const PALETTE: [[u8; 3]; 4] = [[170, 85, 255], [255, 0, 0], [0, 255, 0], [0, 0, 255]];
fn sample(
    color: u8,
    depth: u8,
    alpha: &str,
    x: usize,
    y: usize,
    phase: usize,
) -> (Vec<u16>, [u8; 4]) {
    // Break four/eight-pixel periodicity so early Adam7 passes do not all
    // contain the same sample, including in the one-bit layouts.
    let n = (1 + x + 3 * y + x / 4 + y / 2 + x / 8 + y / 8 + phase) % 4;
    let full = if depth == 16 {
        65535
    } else {
        (1_u32 << depth) - 1
    };
    let expand = |v: u8| {
        if depth == 16 {
            u16::from(v) * 257
        } else {
            u16::from(v)
        }
    };
    let (samples, mut expected) = match color {
        0 => {
            let value = if depth == 1 {
                (n % 2) as u32
            } else {
                n as u32 * full / 3
            };
            let gray = (value * 255 / full) as u8;
            (
                vec![value as u16],
                [
                    gray,
                    gray,
                    gray,
                    if alpha == "key" && value == 0 { 0 } else { 255 },
                ],
            )
        }
        2 => {
            let rgb = [[0, 85, 170], [255, 170, 85], [85, 255, 0], [170, 0, 255]][n];
            (
                rgb.map(expand).to_vec(),
                [
                    rgb[0],
                    rgb[1],
                    rgb[2],
                    if alpha == "key" && n == 0 { 0 } else { 255 },
                ],
            )
        }
        3 => {
            let index = if depth == 1 { n % 2 } else { n };
            let rgb = PALETTE[index];
            let a = match alpha {
                "none" => 255,
                "palette-prefix" => {
                    if index == 0 {
                        0
                    } else {
                        255
                    }
                }
                "palette-full" => [0, 85, 170, 255][index],
                _ => unreachable!(),
            };
            (vec![index as u16], [rgb[0], rgb[1], rgb[2], a])
        }
        4 => {
            let (g, a) = [(85, 255), (255, 85), (170, 255), (255, 0)][n];
            (vec![expand(g), expand(a)], [g, g, g, a])
        }
        6 => {
            let rgba = [
                [0, 85, 170, 255],
                [255, 0, 0, 85],
                [0, 255, 255, 170],
                [17, 85, 255, 0],
            ][n];
            (rgba.map(expand).to_vec(), rgba)
        }
        _ => unreachable!(),
    };
    if expected[3] == 0 {
        expected[..3].fill(0);
    }
    (samples, expected)
}
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let (da, db, dc) = (
        (p - i16::from(a)).abs(),
        (p - i16::from(b)).abs(),
        (p - i16::from(c)).abs(),
    );
    if da <= db && da <= dc {
        a
    } else if db <= dc {
        b
    } else {
        c
    }
}
#[derive(Clone, Copy)]
struct Vector<'a> {
    color: u8,
    depth: u8,
    alpha: &'a str,
    interlaced: bool,
    width: usize,
    height: usize,
    filter: u8,
}
impl Vector<'_> {
    fn scanlines(self, phase: usize) -> Vec<u8> {
        const ADAM7: [(usize, usize, usize, usize); 7] = [
            (0, 0, 8, 8),
            (4, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        ];
        let channels = match self.color {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => unreachable!(),
        };
        let bpp = (channels * self.depth as usize).div_ceil(8).max(1);
        let passes: &[(usize, usize, usize, usize)] = if self.interlaced {
            &ADAM7
        } else {
            &[(0, 0, 1, 1)]
        };
        let mut result = Vec::new();
        for &(sx, sy, dx, dy) in passes {
            if sx >= self.width || sy >= self.height {
                continue;
            }
            let mut previous = Vec::new();
            for y in (sy..self.height).step_by(dy) {
                let values: Vec<_> = (sx..self.width)
                    .step_by(dx)
                    .flat_map(|x| sample(self.color, self.depth, self.alpha, x, y, phase).0)
                    .collect();
                let mut row = Vec::new();
                if self.depth < 8 {
                    row.resize((values.len() * self.depth as usize).div_ceil(8), 0xff);
                    for (i, &value) in values.iter().enumerate() {
                        let bit = i * self.depth as usize;
                        let shift = 8 - self.depth as usize - bit % 8;
                        let mask = ((1_u16 << self.depth) - 1) as u8;
                        row[bit / 8] = (row[bit / 8] & !(mask << shift)) | ((value as u8) << shift);
                    }
                } else {
                    for value in values {
                        if self.depth == 16 {
                            row.extend_from_slice(&value.to_be_bytes());
                        } else {
                            row.push(value as u8);
                        }
                    }
                }
                result.push(self.filter);
                for (i, &value) in row.iter().enumerate() {
                    let left = if i >= bpp { row[i - bpp] } else { 0 };
                    let up = previous.get(i).copied().unwrap_or(0);
                    let corner = if i >= bpp {
                        previous.get(i - bpp).copied().unwrap_or(0)
                    } else {
                        0
                    };
                    let predictor = match self.filter {
                        0 => 0,
                        1 => left,
                        2 => up,
                        3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                        4 => paeth(left, up, corner),
                        _ => unreachable!(),
                    };
                    result.push(value.wrapping_sub(predictor));
                }
                previous = row;
            }
        }
        result
    }
    fn bytes(self, container: Container) -> Vec<u8> {
        use crate::common::codec_fixtures::append_png_chunk as chunk;
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut header = Vec::new();
        header.extend_from_slice(&(self.width as u32).to_be_bytes());
        header.extend_from_slice(&(self.height as u32).to_be_bytes());
        header.extend([self.depth, self.color, 0, 0, u8::from(self.interlaced)]);
        chunk(&mut bytes, *b"IHDR", &header);
        if self.color == 3 {
            let count = if self.depth == 1 { 2 } else { 4 };
            chunk(
                &mut bytes,
                *b"PLTE",
                &PALETTE[..count]
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<_>>(),
            );
        }
        match self.alpha {
            "key" => {
                let key = if self.color == 0 {
                    vec![0, 0]
                } else {
                    [
                        0_u16,
                        if self.depth == 16 { 0x5555 } else { 85 },
                        if self.depth == 16 { 0xaaaa } else { 170 },
                    ]
                    .into_iter()
                    .flat_map(u16::to_be_bytes)
                    .collect()
                };
                chunk(&mut bytes, *b"tRNS", &key);
            }
            "palette-prefix" => chunk(&mut bytes, *b"tRNS", &[0]),
            "palette-full" => chunk(
                &mut bytes,
                *b"tRNS",
                &[0, 85, 170, 255][..if self.depth == 1 { 2 } else { 4 }],
            ),
            _ => {}
        }
        let compressed =
            |phase| miniz_oxide::deflate::compress_to_vec_zlib(&self.scanlines(phase), 6);
        if container == Container::Static {
            chunk(&mut bytes, *b"IDAT", &compressed(0));
        } else {
            chunk(
                &mut bytes,
                *b"acTL",
                &[2_u32.to_be_bytes(), 0_u32.to_be_bytes()].concat(),
            );
            let mut sequence = 0_u32;
            if container == Container::SeparateDefault {
                chunk(&mut bytes, *b"IDAT", &compressed(2));
            }
            for frame in 0..2 {
                let control = crate::common::codec_fixtures::apng_frame_control(
                    sequence,
                    self.width as u32,
                    self.height as u32,
                );
                sequence += 1;
                chunk(&mut bytes, *b"fcTL", &control);
                if frame == 0 && container == Container::IncludedDefault {
                    chunk(&mut bytes, *b"IDAT", &compressed(frame));
                } else {
                    let mut data = sequence.to_be_bytes().to_vec();
                    sequence += 1;
                    data.extend(compressed(frame));
                    chunk(&mut bytes, *b"fdAT", &data);
                }
            }
        }
        chunk(&mut bytes, *b"IEND", &[]);
        bytes
    }
    fn expected(self) -> Vec<u8> {
        (0..self.height)
            .flat_map(|y| {
                (0..self.width)
                    .flat_map(move |x| sample(self.color, self.depth, self.alpha, x, y, 0).1)
            })
            .collect()
    }
}
fn document(bytes: &[u8], width: usize, height: usize) -> SceneDocument {
    let data = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    );
    SceneDocument::parse(&serde_json::to_vec(&json!({"type":"excalidraw","version":2,"elements":[{"id":"image","type":"image","width":width,"height":height,"fileId":"asset/~","status":"saved","scale":[1,1],"updated":1}],"appState":{},"files":{"asset/~":{"id":"asset/~","mimeType":"image/png","dataURL":data,"created":1}}})).unwrap()).unwrap()
}
#[derive(Clone)]
struct PdfObject {
    dictionary: String,
    stream: Option<Vec<u8>>,
}
fn pdf_objects(bytes: &[u8]) -> BTreeMap<usize, PdfObject> {
    let pdf = crate::common::pdf_inspect::inflate_streams(bytes);
    let mut cursor = 0;
    let mut objects = BTreeMap::new();
    while let Some(offset) = pdf[cursor..].windows(7).position(|w| w == b" 0 obj\n") {
        let marker = cursor + offset;
        let id = pdf[cursor..marker]
            .rsplit(|b| b.is_ascii_whitespace())
            .find(|s| !s.is_empty())
            .unwrap();
        let id = std::str::from_utf8(id).unwrap().parse::<usize>().unwrap();
        let body = marker + 7;
        let end = body
            + pdf[body..]
                .windows(7)
                .position(|w| w == b"endobj\n")
                .unwrap();
        let (dictionary, stream) =
            if let Some(offset) = pdf[body..end].windows(7).position(|w| w == b"stream\n") {
                let start = body + offset + 7;
                let dictionary = std::str::from_utf8(&pdf[body..body + offset])
                    .unwrap()
                    .to_owned();
                let length = pdf_number(&dictionary, "/Length");
                let end = start + length;
                assert_eq!(&pdf[end..end + 18], b"\nendstream\nendobj\n");
                cursor = end + 18;
                (dictionary, Some(pdf[start..end].to_vec()))
            } else {
                cursor = end + 7;
                (
                    std::str::from_utf8(&pdf[body..end]).unwrap().to_owned(),
                    None,
                )
            };
        assert!(objects
            .insert(id, PdfObject { dictionary, stream })
            .is_none());
    }
    objects
}
fn pdf_tokens(dictionary: &str) -> Vec<&str> {
    dictionary
        .split(|c: char| c.is_ascii_whitespace() || matches!(c, '[' | ']'))
        .filter(|s| !s.is_empty())
        .collect()
}
fn pdf_value<'a>(dictionary: &'a str, key: &str) -> &'a str {
    let tokens = pdf_tokens(dictionary);
    let i = tokens.iter().position(|&s| s == key).unwrap();
    tokens[i + 1]
}
fn pdf_number(dictionary: &str, key: &str) -> usize {
    pdf_value(dictionary, key).parse().unwrap()
}
fn pdf_reference(dictionary: &str, key: &str) -> usize {
    let tokens = pdf_tokens(dictionary);
    let i = tokens.iter().position(|&s| s == key).unwrap();
    assert_eq!(&tokens[i + 2..i + 4], ["0", "R"]);
    tokens[i + 1].parse().unwrap()
}
fn assert_pdf_objects(
    objects: &BTreeMap<usize, PdfObject>,
    width: usize,
    height: usize,
    expected: &[u8],
) {
    let pages: Vec<_> = objects
        .values()
        .filter(|o| o.dictionary.contains("/Type /Page\n"))
        .collect();
    assert_eq!(pages.len(), 1);
    let page = &pages[0].dictionary;
    let content = &objects[&pdf_reference(page, "/Contents")];
    assert!(std::str::from_utf8(content.stream.as_ref().unwrap())
        .unwrap()
        .contains("/Im0 Do\n"));
    let xobjects = page
        .split_once("/XObject <<")
        .unwrap()
        .1
        .split_once(">>")
        .unwrap()
        .0;
    let rgb = &objects[&pdf_reference(xobjects, "/Im0")];
    let plane = |object: &PdfObject| {
        assert_eq!(pdf_value(&object.dictionary, "/Subtype"), "/Image");
        assert_eq!(
            (
                pdf_number(&object.dictionary, "/Width"),
                pdf_number(&object.dictionary, "/Height")
            ),
            (width, height)
        );
        assert_eq!(pdf_number(&object.dictionary, "/BitsPerComponent"), 8);
        for unsupported in ["/Decode", "/ImageMask", "/Mask"] {
            assert!(!pdf_tokens(&object.dictionary).contains(&unsupported));
        }
    };
    plane(rgb);
    // The direct writer binds canonical RGB through its sRGB ICC space.
    assert_eq!(pdf_value(&rgb.dictionary, "/ColorSpace"), "/CS1");
    let spaces = page
        .split_once("/ColorSpace <<")
        .unwrap()
        .1
        .split_once(">>")
        .unwrap()
        .0;
    assert_eq!(pdf_value(spaces, "/CS1"), "/ICCBased");
    let profile_id = pdf_reference(spaces, "/ICCBased");
    let profile = &objects[&profile_id];
    assert_eq!(pdf_number(&profile.dictionary, "/N"), 3);
    assert_eq!(pdf_value(&profile.dictionary, "/Alternate"), "/DeviceRGB");
    assert_eq!(&profile.stream.as_ref().unwrap()[16..20], b"RGB ");
    let intent = objects
        .values()
        .find(|o| o.dictionary.contains("/Type /OutputIntent"))
        .unwrap();
    assert_eq!(
        pdf_reference(&intent.dictionary, "/DestOutputProfile"),
        profile_id
    );
    let colors: Vec<_> = expected
        .chunks_exact(4)
        .flat_map(|p| p[..3].iter().copied())
        .collect();
    let alphas: Vec<_> = expected.chunks_exact(4).map(|p| p[3]).collect();
    assert_eq!(rgb.stream.as_ref().unwrap(), &colors);
    if pdf_tokens(&rgb.dictionary).contains(&"/SMask") {
        let alpha = &objects[&pdf_reference(&rgb.dictionary, "/SMask")];
        plane(alpha);
        assert_eq!(pdf_value(&alpha.dictionary, "/ColorSpace"), "/DeviceGray");
        assert_eq!(alpha.stream.as_ref().unwrap(), &alphas);
    } else {
        assert!(
            alphas.iter().all(|&a| a == 255),
            "nonopaque image lost its attached soft mask"
        );
    }
}
fn assert_pdf_pixels(bytes: &[u8], width: usize, height: usize, expected: &[u8]) {
    assert_pdf_objects(&pdf_objects(bytes), width, height, expected);
}
#[test]
fn pdf_sample_oracle_rejects_detached_or_wrongly_bound_planes() {
    let vector = Vector {
        color: 6,
        depth: 8,
        alpha: "intrinsic",
        interlaced: false,
        width: 9,
        height: 9,
        filter: 0,
    };
    let scene = document(&vector.bytes(Container::Static), 9, 9);
    let outcome = Renderer::new()
        .render(
            &scene,
            &RenderRequest::builder(OutputFormat::Pdf)
                .padding(0.0)
                .include_background(false)
                .build()
                .unwrap(),
        )
        .unwrap();
    let original = pdf_objects(outcome.artifact().bytes());
    let expected = vector.expected();
    assert_pdf_objects(&original, 9, 9, &expected);
    let page_id = *original
        .iter()
        .find(|(_, o)| o.dictionary.contains("/Type /Page\n"))
        .unwrap()
        .0;
    let rgb_id = pdf_reference(&original[&page_id].dictionary, "/Im0");
    let mask_id = pdf_reference(&original[&rgb_id].dictionary, "/SMask");
    for mutation in 0..3 {
        let mut objects = original.clone();
        match mutation {
            0 => {
                let rgb = objects.get_mut(&rgb_id).unwrap();
                rgb.dictionary = rgb.dictionary.replace("/SMask ", "/Unused ");
            }
            1 => {
                let page = objects.get_mut(&page_id).unwrap();
                page.dictionary = page.dictionary.replace(
                    &format!("/Im0 {rgb_id} 0 R"),
                    &format!("/Im0 {mask_id} 0 R"),
                );
            }
            2 => {
                let rgb = objects.get_mut(&rgb_id).unwrap();
                rgb.dictionary = rgb.dictionary.replace(
                    &format!("/SMask {mask_id} 0 R"),
                    &format!("/SMask {rgb_id} 0 R"),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            std::panic::catch_unwind(|| assert_pdf_objects(&objects, 9, 9, &expected)).is_err()
        );
    }
}
fn check(vector: Vector<'_>, container: Container) {
    let bytes = vector.bytes(container);
    let expected = vector.expected();
    let scene = document(&bytes, vector.width, vector.height);
    for format in FORMATS {
        let request = RenderRequest::builder(format)
            .padding(0.0)
            .include_background(false)
            .build()
            .unwrap();
        let first = Renderer::new().render(&scene, &request).unwrap();
        let second = Renderer::new().render(&scene, &request).unwrap();
        assert_eq!(first.artifact().bytes(), second.artifact().bytes());
        assert_eq!(first.report(), second.report());
        assert_eq!(
            serde_json::to_vec(first.report()).unwrap(),
            serde_json::to_vec(second.report()).unwrap()
        );
        let mut actual = serde_json::to_value(first.report()).unwrap();
        for row in actual["diagnostics"].as_array_mut().unwrap() {
            assert!(!row["message"].as_str().unwrap().is_empty());
            row.as_object_mut().unwrap().remove("message");
        }
        let rows = if container != Container::Static && format != OutputFormat::Svg {
            json!([{"code":"image.animation-flattened","severity":"info","format":format.as_str(),"effects":["visual"],"element_id":"image","json_pointer":"/elements/0/fileId","details":{"kind":"asset","asset_id":"asset/~","claimed_media_type":null,"validated_media_type":"image/png","recovery":"flattened","recipe_id":null}}])
        } else {
            json!([])
        };
        assert_eq!(actual, json!({"schema_version":1,"diagnostics":rows}));
        match format {
            OutputFormat::Svg => {
                let xml = roxmltree::Document::parse(
                    std::str::from_utf8(first.artifact().bytes()).unwrap(),
                )
                .unwrap();
                let image = xml.descendants().find(|n| n.has_tag_name("image")).unwrap();
                let encoded = image
                    .attribute("href")
                    .unwrap()
                    .strip_prefix("data:image/png;base64,")
                    .unwrap();
                assert_eq!(
                    base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .unwrap(),
                    bytes
                );
            }
            OutputFormat::Png => {
                let decoded = crate::common::raster::decode_png(first.artifact().bytes());
                assert_eq!(
                    (decoded.width as usize, decoded.height as usize),
                    (vector.width, vector.height)
                );
                assert_eq!(decoded.rgba, expected);
                let output = first.artifact().bytes();
                let mut cursor = 8;
                while cursor < output.len() {
                    let length =
                        u32::from_be_bytes(output[cursor..cursor + 4].try_into().unwrap()) as usize;
                    assert!(matches!(
                        &output[cursor + 4..cursor + 8],
                        b"IHDR" | b"IDAT" | b"IEND"
                    ));
                    cursor += length + 12;
                }
            }
            OutputFormat::Pdf => assert_pdf_pixels(
                first.artifact().bytes(),
                vector.width,
                vector.height,
                &expected,
            ),
            _ => unreachable!(),
        }
    }
}
fn inventory_guard() {
    let inventory: Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/conformance/coverage/png-profiles.json"
    )))
    .unwrap();
    let expected:Vec<_>=LAYOUTS.iter().map(|&(name,color,depth)|json!({"name":name,"color_type":color,"depth":depth,"transparency":transparency(color)})).collect();
    assert_eq!(inventory["sample_layouts"], json!(expected));
    assert_eq!(inventory["interlace"], json!(["none", "adam7"]));
    assert_eq!(inventory["row_filters"], json!([0, 1, 2, 3, 4]));
    assert_eq!(inventory["sample_scope"], "untagged-8-bit-lattice");
    assert_eq!(
        inventory["containers"],
        json!(["static", "apng-included-default", "apng-separate-default"])
    );
}
#[test]
fn static_sample_matrix_covers_all_declared_layouts() {
    inventory_guard();
    for &(_, color, depth) in LAYOUTS {
        for &alpha in transparency(color) {
            for interlaced in [false, true] {
                for filter in 0..=4 {
                    for (width, height) in [(1, 1), (9, 9)] {
                        check(
                            Vector {
                                color,
                                depth,
                                alpha,
                                interlaced,
                                width,
                                height,
                                filter,
                            },
                            Container::Static,
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn apng_rgba_matrix_covers_both_default_image_layouts() {
    inventory_guard();
    for container in [Container::IncludedDefault, Container::SeparateDefault] {
        for interlaced in [false, true] {
            for filter in 0..=4 {
                for (width, height) in [(1, 1), (9, 9)] {
                    check(
                        Vector {
                            color: 6,
                            depth: 8,
                            alpha: "intrinsic",
                            interlaced,
                            width,
                            height,
                            filter,
                        },
                        container,
                    );
                }
            }
        }
    }
}
