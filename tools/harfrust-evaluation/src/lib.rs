//! Differential shaping probe for #202: rustybuzz 0.20.1 against HarfRust
//! 0.13.3 over every bundled face, using the renderer's shaping settings.
//!
//! The renderer shapes in two configurations (`render/text.rs::shape_text`
//! and `render/font_subset.rs`): with `liga` and `clig` disabled, and with no
//! features, each either guessing segment properties or with an explicit
//! direction. Both shapers see the exact reviewed SFNT bytes the renderer
//! loads, and positions are compared in font units (no scale).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const RUSTYBUZZ: &str = "0.20.1";
pub const HARFRUST: &str = "0.13.3";

pub struct Font {
    pub name: String,
    pub family: &'static str,
    pub sfnt: Vec<u8>,
}

/// Every face the renderer can shape with: the catalog's WOFF2 assets,
/// decoded as the renderer decodes them and checked against the reviewed
/// decode digest, plus the pinned Noto Emoji SFNT.
pub fn fonts() -> Vec<Font> {
    let mut fonts = Vec::new();
    for asset in excalidraw_fonts::revision_1().assets {
        if !asset.upstream_path.ends_with(".woff2") {
            continue;
        }
        let bytes = asset.bytes.expect("the probe builds with every font bundled");
        let decoded = wuff::decompress_woff2(bytes).expect("bundled WOFF2 decodes");
        let sfnt = excalidraw_fonts::verify_reviewed_decode(asset, decoded)
            .expect("decode matches the reviewed digest")
            .into_bytes();
        fonts.push(Font {
            name: asset.upstream_path.to_owned(),
            family: asset.family,
            sfnt,
        });
    }
    fonts.push(Font {
        name: "NotoEmoji".to_owned(),
        family: excalidraw_fonts::NOTO_EMOJI_FAMILY,
        sfnt: excalidraw_fonts::load_noto_emoji_sfnt()
            .expect("Noto Emoji loads")
            .into_bytes(),
    });
    fonts
}

/// Fixed texts every face shapes, by class. Unmapped scalars are
/// deliberate: they exercise `.notdef` handling in every face.
pub fn fixed_corpus() -> Vec<(&'static str, String)> {
    let mut texts: Vec<(&'static str, String)> = vec![
        ("latin", "The quick brown fox jumps over the lazy dog 0123456789".into()),
        ("latin", "Hello, World! It's 3:45 — “quotes” ‘and’ …ellipsis".into()),
        ("latin-precomposed", "àéîõü ÅÇÑØß ĀĒĪŌŪ ŞŢĂ ǅǈ".into()),
        ("combining", "e\u{301} a\u{308}\u{304} o\u{323}\u{302} n\u{303}\u{331} i\u{307}\u{301}".into()),
        ("combining", "x\u{300}\u{301}\u{302}\u{303}\u{304}\u{305}\u{306}\u{307}\u{308}".into()),
        ("combining", "\u{301}leading mark".into()),
        ("vietnamese", "Tiếng Việt có dấu: Chữ Quốc Ngữ".into()),
        ("greek", "Καλημέρα κόσμε ΑΒΓΔ άέήίόύώ".into()),
        ("cyrillic", "Съешь же ещё этих мягких французских булок".into()),
        ("arabic", "مرحبا بالعالم ١٢٣ لا الله".into()),
        ("hebrew", "שלום עולם בְּרֵאשִׁית".into()),
        ("bidi", "abc (שלום) [مرحبا] 123 def".into()),
        ("bidi", "(<[{«‹»›}]>)".into()),
        ("ligature", "ffi ffl fi fl office affluent Th st".into()),
        ("ignorable", "a\u{200d}b\u{200c}c\u{ad}d\u{2060}e\u{fe0f}f\u{200e}g\u{200f}h\u{feff}i".into()),
        ("ignorable", "\u{34f}x\u{180e}y\u{e0001}z".into()),
        ("emoji", "👩‍💻 👍🏽 🇭🇺 1️⃣ ❤️ ✈︎ 👨‍👩‍👧‍👦 🏳️‍🌈".into()),
        ("cjk", "漢字かなカナ한글 ＡＢＣ０１２ 「引用」、。".into()),
        ("cjk", "中文字体测试：这是一段较长的中文文本，用于检验字形与字距。".into()),
        ("terminators", "a\r\nb\nc\rd\u{2028}e\u{2029}f\u{85}g\u{b}h\u{c}i\tj".into()),
        ("controls", "\u{0}\u{1}\u{7f}\u{9f}".into()),
        ("whitespace", "a\u{a0}b\u{2002}c\u{2003}d\u{2009}e\u{200a}f\u{3000}g\u{202f}h".into()),
        ("replacement", "Frame ? name ?? \u{fffd}".into()),
        ("digits", "1,234.56 −7 ½ ¾ ² ³ № ℃ ™ © ®".into()),
        ("symbols", "→ ← ↑ ↓ ⇒ ∀ ∃ ∈ ≠ ≤ ≥ ∞ ✓ ✗ ★ ☆ ♠ ♥".into()),
        ("private-use", "\u{e000}\u{f8ff}\u{f0000}".into()),
        ("supplementary", "𝐀𝐁𝐂 𝔄𝔅 𝟘𝟙 𠀀𠀁 😀".into()),
    ];
    let pangram = "The quick brown fox jumps over the lazy dog. ";
    texts.push(("long", pangram.repeat(10_000 / pangram.len() + 1)));
    let mixed = "abc שלום مرحبا 漢字 e\u{301} 👍🏽 ";
    texts.push(("long", mixed.repeat(200)));
    texts
}

#[derive(Clone, Copy)]
enum Features {
    /// `render/text.rs::shape_text`: `liga` and `clig` off.
    Render,
    /// `render/font_subset.rs`: no features.
    None,
}

#[derive(Clone, Copy)]
enum Direction {
    Guess,
    Ltr,
    Rtl,
}

const CONFIGS: [(Features, Direction); 6] = [
    (Features::Render, Direction::Guess),
    (Features::Render, Direction::Ltr),
    (Features::Render, Direction::Rtl),
    (Features::None, Direction::Guess),
    (Features::None, Direction::Ltr),
    (Features::None, Direction::Rtl),
];

fn config_name(config: (Features, Direction)) -> &'static str {
    match config {
        (Features::Render, Direction::Guess) => "render/guess",
        (Features::Render, Direction::Ltr) => "render/ltr",
        (Features::Render, Direction::Rtl) => "render/rtl",
        (Features::None, Direction::Guess) => "none/guess",
        (Features::None, Direction::Ltr) => "none/ltr",
        (Features::None, Direction::Rtl) => "none/rtl",
    }
}

/// One shaped glyph: id, cluster, x/y advance, x/y offset.
type Glyph = [i64; 6];

fn shape_rustybuzz(face: &rustybuzz::Face<'_>, text: &str, config: (Features, Direction)) -> Vec<Glyph> {
    use rustybuzz::ttf_parser::Tag;
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    match config.1 {
        Direction::Guess => {}
        Direction::Ltr => buffer.set_direction(rustybuzz::Direction::LeftToRight),
        Direction::Rtl => buffer.set_direction(rustybuzz::Direction::RightToLeft),
    }
    buffer.guess_segment_properties();
    let render = [
        rustybuzz::Feature::new(Tag::from_bytes(b"liga"), 0, ..),
        rustybuzz::Feature::new(Tag::from_bytes(b"clig"), 0, ..),
    ];
    let features: &[rustybuzz::Feature] = match config.0 {
        Features::Render => &render,
        Features::None => &[],
    };
    let shaped = rustybuzz::shape(face, features, buffer);
    shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions())
        .map(|(info, pos)| {
            [
                i64::from(info.glyph_id),
                i64::from(info.cluster),
                i64::from(pos.x_advance),
                i64::from(pos.y_advance),
                i64::from(pos.x_offset),
                i64::from(pos.y_offset),
            ]
        })
        .collect()
}

fn shape_harfrust(shaper: &harfrust::Shaper<'_>, text: &str, config: (Features, Direction)) -> Vec<Glyph> {
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(text);
    match config.1 {
        Direction::Guess => {}
        Direction::Ltr => buffer.set_direction(harfrust::Direction::LeftToRight),
        Direction::Rtl => buffer.set_direction(harfrust::Direction::RightToLeft),
    }
    buffer.guess_segment_properties();
    let render = [
        harfrust::Feature::new(harfrust::Tag::new(b"liga"), 0, ..),
        harfrust::Feature::new(harfrust::Tag::new(b"clig"), 0, ..),
    ];
    let features: &[harfrust::Feature] = match config.0 {
        Features::Render => &render,
        Features::None => &[],
    };
    let shaped = shaper.shape(buffer, harfrust::ShapeOptions::new().features(features));
    shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions())
        .map(|(info, pos)| {
            [
                i64::from(info.glyph_id),
                i64::from(info.cluster),
                i64::from(pos.x_advance),
                i64::from(pos.y_advance),
                i64::from(pos.x_offset),
                i64::from(pos.y_offset),
            ]
        })
        .collect()
}

/// Mapped scalars of a face, ascending, from its cmap subtables.
fn mapped_scalars(face: &rustybuzz::Face<'_>) -> Vec<char> {
    let mut scalars = Vec::new();
    if let Some(cmap) = face.tables().cmap {
        for subtable in cmap.subtables {
            if subtable.is_unicode() {
                subtable.codepoints(|cp| {
                    if let Some(c) = char::from_u32(cp) {
                        if face.glyph_index(c).is_some() {
                            scalars.push(c);
                        }
                    }
                });
            }
        }
    }
    scalars.sort_unstable();
    scalars.dedup();
    scalars
}

fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().take(80) {
        if c.is_ascii_graphic() || c == ' ' {
            out.push(c);
        } else {
            let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
        }
    }
    if text.chars().count() > 80 {
        out.push('…');
    }
    out
}

fn difference_kinds(a: &[Glyph], b: &[Glyph]) -> Vec<&'static str> {
    if a.len() != b.len() {
        return vec!["glyph-count"];
    }
    let mut kinds = Vec::new();
    let field = |i: usize| a.iter().zip(b).any(|(x, y)| x[i] != y[i]);
    if field(0) {
        kinds.push("glyph-id");
    }
    if field(1) {
        kinds.push("cluster");
    }
    if field(2) || field(3) {
        kinds.push("advance");
    }
    if field(4) || field(5) {
        kinds.push("offset");
    }
    kinds
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The full differential report. Deterministic, so native and WASM runs can
/// be compared byte for byte.
pub fn report() -> Value {
    let fixed = fixed_corpus();
    let mut cases = 0u64;
    let mut identical = 0u64;
    let mut by_kind: BTreeMap<&str, u64> = BTreeMap::new();
    let mut by_family: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_class: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_config: BTreeMap<&str, u64> = BTreeMap::new();
    let mut examples = Vec::new();
    let mut rb_digest = Sha256::new();
    let mut hr_digest = Sha256::new();
    let mut parse = Vec::new();
    let mut scalar_total = 0usize;

    for font in fonts() {
        let rb_face = rustybuzz::Face::from_slice(&font.sfnt, 0);
        let hr_font = harfrust::FontRef::new(&font.sfnt);
        parse.push(json!({
            "font": font.name, "rustybuzz": rb_face.is_some(), "harfrust": hr_font.is_ok(),
        }));
        let (Some(rb_face), Ok(hr_font)) = (rb_face, hr_font) else {
            continue;
        };
        let data = harfrust::ShaperData::new(&hr_font);
        let shaper = data.shaper(&hr_font).build();

        let scalars = mapped_scalars(&rb_face);
        scalar_total += scalars.len();
        let mut texts: Vec<(String, String)> = fixed
            .iter()
            .map(|(class, text)| ((*class).to_owned(), text.clone()))
            .collect();
        texts.extend(scalars.iter().map(|c| ("cmap-single".to_owned(), c.to_string())));
        texts.extend(
            scalars
                .chunks(24)
                .map(|window| ("cmap-window".to_owned(), window.iter().collect::<String>())),
        );

        for (class, text) in &texts {
            for config in CONFIGS {
                let rb = shape_rustybuzz(&rb_face, text, config);
                let hr = shape_harfrust(&shaper, text, config);
                for (digest, glyphs) in [(&mut rb_digest, &rb), (&mut hr_digest, &hr)] {
                    digest.update((glyphs.len() as u64).to_le_bytes());
                    for glyph in glyphs.iter() {
                        for value in glyph {
                            digest.update(value.to_le_bytes());
                        }
                    }
                }
                cases += 1;
                if rb == hr {
                    identical += 1;
                    continue;
                }
                let kinds = difference_kinds(&rb, &hr);
                for kind in &kinds {
                    *by_kind.entry(kind).or_default() += 1;
                }
                *by_family.entry(font.family.to_owned()).or_default() += 1;
                *by_class.entry(class.clone()).or_default() += 1;
                *by_config.entry(config_name(config)).or_default() += 1;
                if examples.len() < 400 {
                    examples.push(json!({
                        "font": font.name,
                        "class": class,
                        "config": config_name(config),
                        "text": escape(text),
                        "kinds": kinds,
                        "rustybuzz": rb.iter().take(12).collect::<Vec<_>>(),
                        "harfrust": hr.iter().take(12).collect::<Vec<_>>(),
                    }));
                }
            }
        }
    }

    let sweep = scalar_sweep();

    json!({
        "schema": "excalidraw-render/harfrust-evaluation/v1",
        "scalar_sweep": sweep,
        "rustybuzz": RUSTYBUZZ,
        "harfrust": HARFRUST,
        "harfrust_libm": cfg!(feature = "libm"),
        "fixed_texts": fixed.len(),
        "mapped_scalars": scalar_total,
        "cases": cases,
        "identical": identical,
        "differing": cases - identical,
        "by_kind": by_kind,
        "by_family": by_family,
        "by_class": by_class,
        "by_config": by_config,
        "parse": parse,
        "digests": {
            "rustybuzz": hex(&rb_digest.finalize()),
            "harfrust": hex(&hr_digest.finalize()),
        },
        "examples": examples,
    })
}

/// Every Unicode scalar shaped between two letters (`a<c>b`) in the
/// renderer's configuration through the fallback face and the first
/// Excalifont chunk. This covers the default-ignorable class the spec pins to
/// the shaper (spec 8.3, `render/text.rs::is_default_ignorable`), mirroring
/// and `.notdef` handling beyond the scalars a face maps.
fn scalar_sweep() -> Value {
    let fonts = fonts();
    let maps_a = |font: &&Font| {
        rustybuzz::Face::from_slice(&font.sfnt, 0).is_some_and(|face| face.glyph_index('a').is_some())
    };
    let faces = [
        fonts
            .iter()
            .find(|font| font.name.ends_with("LiberationSans-Regular.woff2"))
            .expect("Liberation Sans is bundled"),
        fonts
            .iter()
            .filter(|font| font.family == "Excalifont")
            .find(maps_a)
            .expect("an Excalifont chunk maps Basic Latin"),
    ];
    let mut out = Vec::new();
    for font in faces {
        let rb_face = rustybuzz::Face::from_slice(&font.sfnt, 0).expect("face parses");
        let hr_font = harfrust::FontRef::new(&font.sfnt).expect("face parses");
        let data = harfrust::ShaperData::new(&hr_font);
        let shaper = data.shaper(&hr_font).build();
        let mut differing = Vec::new();
        let mut count = 0u64;
        let mut hidden = (0u64, 0u64);
        for scalar in 0..=0x10_ffffu32 {
            let Some(c) = char::from_u32(scalar) else { continue };
            let text: String = ['a', c, 'b'].into_iter().collect();
            for config in [(Features::Render, Direction::Guess), (Features::Render, Direction::Rtl)] {
                let rb = shape_rustybuzz(&rb_face, &text, config);
                let hr = shape_harfrust(&shaper, &text, config);
                count += 1;
                // A hidden scalar: three glyphs in, the middle one with zero advance.
                let zero = |g: &[Glyph]| g.len() == 3 && g.iter().any(|x| x[1] == 1 && x[2] == 0);
                hidden.0 += u64::from(zero(&rb));
                hidden.1 += u64::from(zero(&hr));
                if rb != hr {
                    differing.push(json!({
                        "scalar": format!("U+{scalar:04X}"),
                        "config": config_name(config),
                        "kinds": difference_kinds(&rb, &hr),
                        "rustybuzz": rb, "harfrust": hr,
                    }));
                }
            }
        }
        out.push(json!({
            "font": font.name,
            "cases": count,
            "zero_advance_middle": {"rustybuzz": hidden.0, "harfrust": hidden.1},
            "differing": differing.len(),
            "differences": differing,
        }));
    }
    json!(out)
}
