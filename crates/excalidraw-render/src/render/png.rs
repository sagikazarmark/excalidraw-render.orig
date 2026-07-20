use crate::RenderRequest;

use super::ResolvedScene;

pub(super) struct PngPlan {
    width: u32,
    height: u32,
    pixel: [u8; 4],
}

impl PngPlan {
    pub(super) fn new(scene: &ResolvedScene, request: &RenderRequest) -> Result<Self, String> {
        let scale = scene.raster_or_pdf_scale(request.sizing());
        let width = pixel_dimension(scene.logical_width * scale)?;
        let height = pixel_dimension(scene.logical_height * scale)?;
        let pixel = if scene.include_background {
            [255, 255, 255, 255]
        } else {
            [0, 0, 0, 0]
        };
        Ok(Self {
            width,
            height,
            pixel,
        })
    }

    pub(super) fn encode(self) -> Result<Vec<u8>, String> {
        let mut pixmap = tiny_skia::Pixmap::new(self.width, self.height)
            .ok_or_else(|| "failed to allocate PNG pixel buffer".to_owned())?;
        for pixel in pixmap.data_mut().chunks_exact_mut(4) {
            pixel.copy_from_slice(&self.pixel);
        }
        pixmap
            .encode_png()
            .map_err(|_| "failed to encode PNG pixels".to_owned())
    }
}

fn pixel_dimension(value: f64) -> Result<u32, String> {
    if !value.is_finite() || value < 1.0 || value >= (u32::MAX as f64) + 1.0 {
        return Err("PNG dimensions must be finite positive 32-bit values".to_owned());
    }
    Ok(value.trunc() as u32)
}
