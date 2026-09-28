//! Decoding the renderer's PNG artifacts for pixel probes. The renderer emits
//! straight-alpha RGBA8, so the decode asserts that and hands back the pixels
//! in that layout.

use std::io::Cursor;

use excalidraw_render_compare::CanonicalRgba8;

/// One decoded PNG: `rgba` holds `width * height` top-left row-major RGBA8
/// pixels.
pub struct DecodedPng {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl DecodedPng {
    /// The four channels of the pixel at (`x`, `y`).
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let width = usize::try_from(self.width).expect("width fits usize");
        assert!(x < width, "x={x} is outside a {width}-pixel-wide PNG");
        let offset = (y * width + x) * 4;
        self.rgba[offset..offset + 4]
            .try_into()
            .expect("pixel has four channels")
    }

    /// The same pixels as the comparator's canonical raster.
    pub fn into_canonical(self) -> CanonicalRgba8 {
        CanonicalRgba8::new(self.width, self.height, self.rgba).expect("PNG is canonical RGBA8")
    }
}

/// Decodes one of the renderer's PNG artifacts without any transformation,
/// asserting it is eight-bit RGBA.
pub fn decode_png(bytes: &[u8]) -> DecodedPng {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("PNG structure is valid");
    let mut rgba = vec![0; reader.output_buffer_size().expect("PNG buffer fits")];
    let frame = reader.next_frame(&mut rgba).expect("PNG pixels decode");
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    assert_eq!(frame.bit_depth, png::BitDepth::Eight);
    rgba.truncate(frame.buffer_size());
    DecodedPng {
        width: frame.width,
        height: frame.height,
        rgba,
    }
}
