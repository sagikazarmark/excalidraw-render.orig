//! Representation normalization for the isolated PDFium worker.

/// Clear hidden color without requantizing any nonzero-alpha sample.
///
/// The pinned AGG backend already produces straight channels. Unpremultiplying
/// them again would corrupt translucent colors, especially at alpha 1.
pub fn canonicalize_rgba(pixels: &mut [[u8; 4]]) {
    for pixel in pixels {
        if pixel[3] == 0 {
            pixel[..3].fill(0);
        }
    }
}
