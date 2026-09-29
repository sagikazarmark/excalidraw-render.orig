//! Shaping throughput for the renderer's configuration: every fixed text
//! through the Excalifont, Liberation and first Xiaolai faces, shaped with a
//! fresh face per call (as `render/text.rs` does) and with a reused one.
use std::time::Instant;

fn main() {
    let fonts: Vec<_> = harfrust_evaluation::fonts()
        .into_iter()
        .filter(|f| f.name.contains("Excalifont") || f.name.contains("Liberation") || f.name.contains("Xiaolai-Regular-0"))
        .collect();
    let texts = harfrust_evaluation::fixed_corpus();
    let feats_rb = [
        rustybuzz::Feature::new(rustybuzz::ttf_parser::Tag::from_bytes(b"liga"), 0, ..),
        rustybuzz::Feature::new(rustybuzz::ttf_parser::Tag::from_bytes(b"clig"), 0, ..),
    ];
    let feats_hr = [
        harfrust::Feature::new(harfrust::Tag::new(b"liga"), 0, ..),
        harfrust::Feature::new(harfrust::Tag::new(b"clig"), 0, ..),
    ];
    const ROUNDS: usize = 30;
    for fresh in [true, false] {
        let mut glyphs = (0usize, 0usize);
        let t = Instant::now();
        for _ in 0..ROUNDS {
            for font in &fonts {
                let reused = rustybuzz::Face::from_slice(&font.sfnt, 0).unwrap();
                for (_, text) in &texts {
                    let face = if fresh { rustybuzz::Face::from_slice(&font.sfnt, 0).unwrap() } else { reused.clone() };
                    let mut b = rustybuzz::UnicodeBuffer::new();
                    b.push_str(text);
                    b.guess_segment_properties();
                    glyphs.0 += rustybuzz::shape(&face, &feats_rb, b).len();
                }
            }
        }
        let rb = t.elapsed();
        let t = Instant::now();
        for _ in 0..ROUNDS {
            for font in &fonts {
                let f = harfrust::FontRef::new(&font.sfnt).unwrap();
                let reused = harfrust::ShaperData::new(&f);
                for (_, text) in &texts {
                    let fresh_data;
                    let data = if fresh { fresh_data = harfrust::ShaperData::new(&f); &fresh_data } else { &reused };
                    let shaper = data.shaper(&f).build();
                    let mut b = harfrust::UnicodeBuffer::new();
                    b.push_str(text);
                    b.guess_segment_properties();
                    glyphs.1 += shaper.shape(b, harfrust::ShapeOptions::new().features(&feats_hr)).len();
                }
            }
        }
        let hr = t.elapsed();
        assert_eq!(glyphs.0, glyphs.1);
        println!("{} face: rustybuzz {:.0?}  harfrust {:.0?}  ratio {:.2} ({} glyphs)",
            if fresh { "fresh" } else { "reused" }, rb, hr, hr.as_secs_f64() / rb.as_secs_f64(), glyphs.0);
    }
}
