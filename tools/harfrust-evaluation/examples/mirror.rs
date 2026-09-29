fn main() {
    for font in harfrust_evaluation::fonts() {
        if !["Cascadia", "ComicShanns", "Liberation"].contains(&font.family) { continue; }
        let face = rustybuzz::Face::from_slice(&font.sfnt, 0).unwrap();
        let (le, ge) = (face.glyph_index('≤'), face.glyph_index('≥'));
        if ge.is_none() { continue; }
        println!("{} ≤={:?} ≥={:?}", font.name.rsplit('/').next().unwrap(), le.map(|g| g.0), ge.map(|g| g.0));
    }
    // Mirroring tables each shaper relies on.
    for c in ['≤', '≥', '(', '<', '«', '∈', '≠'] {
        println!("{c} U+{:04X} unicode-bidi-mirroring -> {:?}", c as u32, unicode_bidi_mirroring::get_mirrored(c));
    }
}
