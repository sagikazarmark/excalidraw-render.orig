//! The PNG codec: chunk-level validation of a static or animated PNG with
//! CRC, ordering and IHDR checks, the animation work estimate, the default
//! frame decode through the `png` crate onto the declared canvas, and the
//! re-encoding a nested raster receives before it is embedded in sanitized
//! SVG.

use std::io::Cursor;

use super::color::{finish_rgba, normalize_rgba, ColorNormalization};
use super::orientation::{exif_orientation, orient_rgba, oriented_dimensions};
use super::{DecodedRaster, ResolveError};

pub(super) const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Clone, Copy, Default)]
pub(super) struct PngSemantics {
    pub(super) cicp: Option<[u8; 4]>,
    pub(super) animation_frames: u32,
    pub(super) orientation: Option<u8>,
}

pub(super) fn read_png_ihdr(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let chunk = bytes
        .get(PNG_SIGNATURE.len()..PNG_SIGNATURE.len() + 25)
        .ok_or(ResolveError::DecodeFailed)?;
    if chunk[..4] != 13_u32.to_be_bytes() || &chunk[4..8] != b"IHDR" {
        return Err(ResolveError::DecodeFailed);
    }
    let width = u32::from_be_bytes(
        chunk[8..12]
            .try_into()
            .map_err(|_| ResolveError::DecodeFailed)?,
    );
    let height = u32::from_be_bytes(
        chunk[12..16]
            .try_into()
            .map_err(|_| ResolveError::DecodeFailed)?,
    );
    if width == 0 || height == 0 {
        return Err(ResolveError::DecodeFailed);
    }
    Ok((width, height))
}

pub(super) fn png_animation_validation_work(
    bytes: &[u8],
    pixels: u64,
) -> Result<u64, ResolveError> {
    let mut offset = PNG_SIGNATURE.len();
    let mut animation_frames = 0_u32;
    let mut first_frame_control_precedes_idat = false;
    while offset < bytes.len() {
        let header_end = offset.checked_add(8).ok_or(ResolveError::DecodeFailed)?;
        let header = bytes
            .get(offset..header_end)
            .ok_or(ResolveError::DecodeFailed)?;
        let length = usize::try_from(u32::from_be_bytes(
            header[..4]
                .try_into()
                .map_err(|_| ResolveError::DecodeFailed)?,
        ))
        .map_err(|_| ResolveError::DecodeFailed)?;
        let chunk_type = &header[4..8];
        let data_end = header_end
            .checked_add(length)
            .ok_or(ResolveError::DecodeFailed)?;
        let crc_end = data_end.checked_add(4).ok_or(ResolveError::DecodeFailed)?;
        let data = bytes
            .get(header_end..data_end)
            .ok_or(ResolveError::DecodeFailed)?;
        match chunk_type {
            b"acTL" if length == 8 => {
                animation_frames = u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| ResolveError::DecodeFailed)?,
                );
            }
            b"fcTL" if animation_frames != 0 => first_frame_control_precedes_idat = true,
            b"IDAT" => break,
            _ => {}
        }
        offset = crc_end;
    }
    if animation_frames == 0 {
        return Ok(0);
    }
    let decoded_frames =
        animation_frames.saturating_add(u32::from(!first_frame_control_precedes_idat));
    u64::from(decoded_frames.saturating_sub(1))
        .checked_mul(pixels)
        .and_then(|work| work.checked_mul(4))
        .ok_or(ResolveError::AssetWork {
            attempted: u64::MAX,
        })
}

pub(super) fn validate_static_png(bytes: &[u8]) -> Result<(u32, u32, PngSemantics), ResolveError> {
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let mut offset = PNG_SIGNATURE.len();
    let mut dimensions = None;
    let mut color_type = None;
    let mut chunk_index = 0_u64;
    let mut saw_plte = false;
    let mut saw_idat = false;
    let mut idat_ended = false;
    let mut saw_iend = false;
    let mut saw_exif = false;
    let mut semantics = PngSemantics::default();
    while offset < bytes.len() {
        let header_end = offset.checked_add(8).ok_or(ResolveError::DecodeFailed)?;
        let header = bytes
            .get(offset..header_end)
            .ok_or(ResolveError::DecodeFailed)?;
        let length = usize::try_from(u32::from_be_bytes(
            header[..4]
                .try_into()
                .map_err(|_| ResolveError::DecodeFailed)?,
        ))
        .map_err(|_| ResolveError::DecodeFailed)?;
        let chunk_type: [u8; 4] = header[4..8]
            .try_into()
            .map_err(|_| ResolveError::DecodeFailed)?;
        if !chunk_type.iter().all(u8::is_ascii_alphabetic) || chunk_type[2].is_ascii_lowercase() {
            return Err(ResolveError::DecodeFailed);
        }
        let data_end = header_end
            .checked_add(length)
            .ok_or(ResolveError::DecodeFailed)?;
        let crc_end = data_end.checked_add(4).ok_or(ResolveError::DecodeFailed)?;
        let data = bytes
            .get(header_end..data_end)
            .ok_or(ResolveError::DecodeFailed)?;
        let expected_crc = u32::from_be_bytes(
            bytes
                .get(data_end..crc_end)
                .ok_or(ResolveError::DecodeFailed)?
                .try_into()
                .map_err(|_| ResolveError::DecodeFailed)?,
        );
        let mut crc = crc32fast::Hasher::new();
        crc.update(&chunk_type);
        crc.update(data);
        if crc.finalize() != expected_crc {
            return Err(ResolveError::DecodeFailed);
        }
        idat_ended |= saw_idat && &chunk_type != b"IDAT";
        match &chunk_type {
            b"IHDR" if chunk_index == 0 && length == 13 => {
                let width = u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| ResolveError::DecodeFailed)?,
                );
                let height = u32::from_be_bytes(
                    data[4..8]
                        .try_into()
                        .map_err(|_| ResolveError::DecodeFailed)?,
                );
                let bit_depth = data[8];
                let png_color_type = data[9];
                let valid_depth = matches!(
                    (png_color_type, bit_depth),
                    (0, 1 | 2 | 4 | 8 | 16) | (2, 8 | 16) | (3, 1 | 2 | 4 | 8) | (4 | 6, 8 | 16)
                );
                if width == 0
                    || height == 0
                    || !valid_depth
                    || data[10] != 0
                    || data[11] != 0
                    || data[12] > 1
                {
                    return Err(ResolveError::DecodeFailed);
                }
                dimensions = Some((width, height));
                color_type = Some(png_color_type);
            }
            b"acTL" if length == 8 && !saw_idat && semantics.animation_frames == 0 => {
                semantics.animation_frames = u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| ResolveError::DecodeFailed)?,
                );
                if semantics.animation_frames == 0 {
                    return Err(ResolveError::DecodeFailed);
                }
            }
            b"acTL" => return Err(ResolveError::DecodeFailed),
            b"cICP" if length == 4 && !saw_plte && !saw_idat && semantics.cicp.is_none() => {
                semantics.cicp = Some(data.try_into().map_err(|_| ResolveError::DecodeFailed)?);
            }
            b"cICP" => return Err(ResolveError::DecodeFailed),
            b"eXIf" if !saw_exif => {
                saw_exif = true;
                semantics.orientation = exif_orientation(data);
            }
            b"eXIf" => return Err(ResolveError::DecodeFailed),
            b"IHDR" => return Err(ResolveError::DecodeFailed),
            b"PLTE"
                if !saw_plte
                    && !saw_idat
                    && matches!(color_type, Some(2 | 3 | 6))
                    && length > 0
                    && length <= 768
                    && length % 3 == 0 =>
            {
                saw_plte = true;
            }
            b"PLTE" => return Err(ResolveError::DecodeFailed),
            b"IDAT" if dimensions.is_some() && !idat_ended => saw_idat = true,
            b"IDAT" => return Err(ResolveError::DecodeFailed),
            b"IEND" if length == 0 && saw_idat && (!matches!(color_type, Some(3)) || saw_plte) => {
                saw_iend = true;
                offset = crc_end;
                break;
            }
            b"IEND" => return Err(ResolveError::DecodeFailed),
            _ if chunk_type[0].is_ascii_uppercase() => {
                return Err(ResolveError::DecodeFailed);
            }
            _ => {}
        }
        chunk_index = chunk_index
            .checked_add(1)
            .ok_or(ResolveError::DecodeFailed)?;
        offset = crc_end;
    }
    if !saw_iend || offset != bytes.len() {
        return Err(ResolveError::DecodeFailed);
    }
    let (source_width, source_height) = dimensions.ok_or(ResolveError::DecodeFailed)?;
    let (width, height) = oriented_dimensions(
        source_width,
        source_height,
        semantics.orientation.unwrap_or(1),
    );
    validate_png_stream(bytes, source_width, source_height)?;
    Ok((width, height, semantics))
}

fn validate_png_stream(bytes: &[u8], width: u32, height: u32) -> Result<(), ResolveError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let logical_bytes = 1_048_576_u64
        .checked_add(32_u64.saturating_mul(u64::from(width.max(height))))
        .ok_or(ResolveError::Memory)?;
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(logical_bytes).map_err(|_| ResolveError::Memory)?,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|_| ResolveError::DecodeFailed)?;
    let default_image_is_separate =
        reader.info().frame_control.is_none() && reader.info().animation_control.is_some();
    let frame_count = reader.info().animation_control.map_or(1, |animation| {
        animation
            .num_frames
            .saturating_add(u32::from(default_image_is_separate))
    });
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(reader.output_buffer_size())
        .map_err(|_| ResolveError::Memory)?;
    frame.resize(reader.output_buffer_size(), 0);
    for _ in 0..frame_count {
        reader
            .next_frame(&mut frame)
            .map_err(|_| ResolveError::DecodeFailed)?;
    }
    reader.finish().map_err(|_| ResolveError::DecodeFailed)
}

pub(super) fn decode_static_png(
    bytes: &[u8],
    pixels: u64,
    color_normalization: ColorNormalization,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let logical_bytes = pixels
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(1_048_576))
        .ok_or(ResolveError::Memory)?;
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(logical_bytes).map_err(|_| ResolveError::Memory)?,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|_| ResolveError::DecodeFailed)?;
    if reader.info().animation_control.is_some() && reader.info().frame_control.is_none() {
        reader
            .next_frame_info()
            .map_err(|_| ResolveError::DecodeFailed)?;
    }
    let output_size = reader.output_buffer_size();
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(output_size)
        .map_err(|_| ResolveError::Memory)?;
    decoded.resize(output_size, 0);
    let output = reader
        .next_frame(&mut decoded)
        .map_err(|_| ResolveError::DecodeFailed)?;
    decoded.truncate(output.buffer_size());
    let rgba_size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let frame_pixels = u64::from(output.width)
        .checked_mul(u64::from(output.height))
        .ok_or(ResolveError::Memory)?;
    let frame_rgba_size = usize::try_from(frame_pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut frame_rgba = Vec::new();
    frame_rgba
        .try_reserve_exact(frame_rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    match output.color_type {
        png::ColorType::Rgba => frame_rgba.extend_from_slice(&decoded),
        png::ColorType::Rgb => {
            for pixel in decoded.as_chunks::<3>().0 {
                frame_rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &value in &decoded {
                frame_rgba.extend_from_slice(&[value, value, value, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in decoded.as_chunks::<2>().0 {
                frame_rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
        }
        png::ColorType::Indexed => return Err(ResolveError::DecodeFailed),
    }
    if frame_rgba.len() != frame_rgba_size {
        return Err(ResolveError::DecodeFailed);
    }
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    rgba.resize(rgba_size, 0);
    let frame_control = reader.info().frame_control;
    let (x_offset, y_offset) =
        frame_control.map_or((0, 0), |frame| (frame.x_offset, frame.y_offset));
    let canvas_width = usize::try_from(reader.info().width).map_err(|_| ResolveError::Memory)?;
    let frame_width = usize::try_from(output.width).map_err(|_| ResolveError::Memory)?;
    let x_offset = usize::try_from(x_offset).map_err(|_| ResolveError::Memory)?;
    let y_offset = usize::try_from(y_offset).map_err(|_| ResolveError::Memory)?;
    for y in 0..usize::try_from(output.height).map_err(|_| ResolveError::Memory)? {
        let source_start = y
            .checked_mul(frame_width)
            .and_then(|value| value.checked_mul(4))
            .ok_or(ResolveError::Memory)?;
        let source_end = source_start
            .checked_add(frame_width.saturating_mul(4))
            .ok_or(ResolveError::Memory)?;
        let destination_start = y
            .checked_add(y_offset)
            .and_then(|value| value.checked_mul(canvas_width))
            .and_then(|value| value.checked_add(x_offset))
            .and_then(|value| value.checked_mul(4))
            .ok_or(ResolveError::Memory)?;
        let destination_end = destination_start
            .checked_add(frame_width.saturating_mul(4))
            .ok_or(ResolveError::Memory)?;
        let source = frame_rgba
            .get(source_start..source_end)
            .ok_or(ResolveError::DecodeFailed)?;
        let destination = rgba
            .get_mut(destination_start..destination_end)
            .ok_or(ResolveError::DecodeFailed)?;
        destination.copy_from_slice(source);
    }
    normalize_rgba(&mut rgba, color_normalization)?;
    if let Some(orientation) = orientation {
        rgba = orient_rgba(
            &rgba,
            reader.info().width,
            reader.info().height,
            orientation,
        )?;
    }
    finish_rgba(rgba)
}

pub(super) fn encode_normalized_png(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Vec<u8>, ResolveError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Best);
        encoder.set_filter(png::FilterType::NoFilter);
        let mut writer = encoder
            .write_header()
            .map_err(|_| ResolveError::DecodeFailed)?;
        writer
            .write_image_data(rgba)
            .map_err(|_| ResolveError::DecodeFailed)?;
    }
    Ok(bytes)
}
