fn main() {
    for font in harfrust_evaluation::fonts() {
        if !["Cascadia", "ComicShanns", "Liberation"].contains(&font.family) { continue; }
        let face = rustybuzz::Face::from_slice(&font.sfnt, 0).unwrap();
        let (le, ge) = (face.glyph_index('≤'), face.glyph_index('≥'));
        println!("  U+2215={:?} U+29F5={:?} U+221F={:?} U+2BFE={:?}", face.glyph_index('\u{2215}').map(|g| g.0), face.glyph_index('\u{29F5}').map(|g| g.0), face.glyph_index('\u{221F}').map(|g| g.0), face.glyph_index('\u{2BFE}').map(|g| g.0));
        if ge.is_none() { continue; }
        println!("{} ≤={:?} ≥={:?}", font.name.rsplit('/').next().unwrap(), le.map(|g| g.0), ge.map(|g| g.0));
    }
    // Mirroring tables each shaper relies on.
    for c in ['≤', '≥', '(', '<', '«', '∈', '≠', '\u{29F5}', '\u{2215}', '\u{2BFE}', '\u{221F}'] {
        println!("{c} U+{:04X} unicode-bidi-mirroring -> {:?}", c as u32, unicode_bidi_mirroring::get_mirrored(c));
    }
}
