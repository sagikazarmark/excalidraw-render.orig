use crate::RenderRequest;

use super::{FormatEncodeError, ResolvedScene};

pub(super) struct PngPlan {
    width: u32,
    height: u32,
    pixel: [u8; 4],
}

impl PngPlan {
    pub(super) fn new(
        scene: &ResolvedScene,
        request: &RenderRequest,
    ) -> Result<Self, FormatEncodeError> {
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

    pub(super) fn encode(&self) -> Result<Vec<u8>, FormatEncodeError> {
        const MAX_STORED_BLOCK_BYTES: usize = u16::MAX as usize;

        // Stored DEFLATE makes the exact PNG size known before fallible output allocation.
        let row_bytes = usize::try_from(self.width)
            .ok()
            .and_then(|width| width.checked_mul(4))
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or_else(png_size_error)?;
        let raw_bytes = row_bytes
            .checked_mul(usize::try_from(self.height).map_err(|_| png_size_error())?)
            .ok_or_else(png_size_error)?;
        let block_count = raw_bytes.div_ceil(MAX_STORED_BLOCK_BYTES);
        let zlib_bytes = 2_usize
            .checked_add(block_count.checked_mul(5).ok_or_else(png_size_error)?)
            .and_then(|bytes| bytes.checked_add(raw_bytes))
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or_else(png_size_error)?;
        let output_bytes = 8_usize
            .checked_add(12 + 13)
            .and_then(|bytes| bytes.checked_add(12))
            .and_then(|bytes| bytes.checked_add(zlib_bytes))
            .and_then(|bytes| bytes.checked_add(12))
            .ok_or_else(png_size_error)?;
        let idat_bytes = u32::try_from(zlib_bytes).map_err(|_| png_size_error())?;

        let mut png = Vec::new();
        png.try_reserve_exact(output_bytes)
            .map_err(|_| FormatEncodeError::MemoryUnavailable {
                subject_kind: "artifact",
                subject: "png_output",
            })?;
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

        let mut ihdr = [0_u8; 13];
        ihdr[..4].copy_from_slice(&self.width.to_be_bytes());
        ihdr[4..8].copy_from_slice(&self.height.to_be_bytes());
        ihdr[8] = 8;
        ihdr[9] = 6;
        append_chunk(&mut png, *b"IHDR", &ihdr)?;

        png.extend_from_slice(&idat_bytes.to_be_bytes());
        let idat_start = png.len();
        png.extend_from_slice(b"IDAT");
        png.extend_from_slice(&[0x78, 0x01]);
        let mut adler_a = 1_u32;
        let mut adler_b = 0_u32;
        let mut offset = 0_usize;
        while offset < raw_bytes {
            let block_bytes = (raw_bytes - offset).min(MAX_STORED_BLOCK_BYTES);
            png.push(u8::from(offset + block_bytes == raw_bytes));
            let block_bytes = u16::try_from(block_bytes).map_err(|_| png_size_error())?;
            png.extend_from_slice(&block_bytes.to_le_bytes());
            png.extend_from_slice(&(!block_bytes).to_le_bytes());
            for raw_offset in offset..offset + usize::from(block_bytes) {
                let row_offset = raw_offset % row_bytes;
                let byte = if row_offset == 0 {
                    0
                } else {
                    self.pixel[(row_offset - 1) % 4]
                };
                png.push(byte);
                adler_a = (adler_a + u32::from(byte)) % 65_521;
                adler_b = (adler_b + adler_a) % 65_521;
            }
            offset += usize::from(block_bytes);
        }
        png.extend_from_slice(&((adler_b << 16) | adler_a).to_be_bytes());
        let idat_crc = crc32(&png[idat_start..]);
        png.extend_from_slice(&idat_crc.to_be_bytes());
        append_chunk(&mut png, *b"IEND", &[])?;

        if png.len() != output_bytes {
            return Err(FormatEncodeError::Encoding(
                "PNG encoder length invariant failed".to_owned(),
            ));
        }
        Ok(png)
    }

    pub(super) const fn width(&self) -> u32 {
        self.width
    }

    pub(super) const fn height(&self) -> u32 {
        self.height
    }
}

fn append_chunk(output: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) -> Result<(), FormatEncodeError> {
    let length = u32::try_from(data.len()).map_err(|_| png_size_error())?;
    output.extend_from_slice(&length.to_be_bytes());
    let crc_start = output.len();
    output.extend_from_slice(&kind);
    output.extend_from_slice(data);
    output.extend_from_slice(&crc32(&output[crc_start..]).to_be_bytes());
    Ok(())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn png_size_error() -> FormatEncodeError {
    FormatEncodeError::Encoding("PNG dimensions exceed encoder capacity".to_owned())
}

fn pixel_dimension(value: f64) -> Result<u32, FormatEncodeError> {
    if !value.is_finite() || value < 1.0 || value >= (u32::MAX as f64) + 1.0 {
        return Err(FormatEncodeError::Encoding(
            "PNG dimensions must be finite positive 32-bit values".to_owned(),
        ));
    }
    Ok(value.trunc() as u32)
}
