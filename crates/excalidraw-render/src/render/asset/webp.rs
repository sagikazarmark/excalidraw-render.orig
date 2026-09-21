//! The WebP codec: RIFF chunk walking that validates the container and each
//! animation frame as a standalone image, the animation work estimate,
//! EXIF-orientation and ICC reads through `image-webp`, and the default frame
//! decode with color normalization and orientation applied.

use std::io::Cursor;
use std::sync::Arc;

use image_webp::{DecodingError as WebPDecodingError, WebPDecoder};

use super::color::{finish_rgba, normalize_rgba, ColorNormalization};
use super::orientation::{exif_orientation, orient_rgba, oriented_dimensions};
use super::{decoder_workspace_bytes, try_copy, DecodedRaster, ResolveError};

pub(super) const WEBP_RIFF_SIGNATURE: &[u8; 4] = b"RIFF";
pub(super) const WEBP_SIGNATURE: &[u8; 4] = b"WEBP";

#[derive(Clone, Copy)]
pub(super) struct WebPSemantics {
    pub(super) animation_frames: u32,
    pub(super) orientation: Option<u8>,
}

pub(super) fn webp_decoder(bytes: &[u8]) -> Result<WebPDecoder<Cursor<&[u8]>>, ResolveError> {
    let mut decoder = WebPDecoder::new(Cursor::new(bytes)).map_err(map_webp_error)?;
    let (width, height) = decoder.dimensions();
    decoder.set_memory_limit(
        usize::try_from(decoder_workspace_bytes(width, height)?)
            .map_err(|_| ResolveError::Memory)?,
    );
    Ok(decoder)
}

pub(super) fn read_webp_dimensions(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    if !bytes.starts_with(WEBP_RIFF_SIGNATURE) || bytes.get(8..12) != Some(WEBP_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let decoder = webp_decoder(bytes)?;
    let dimensions = decoder.dimensions();
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return Err(ResolveError::WebPDecodeFailed);
    }
    Ok(dimensions)
}

pub(super) fn webp_animation_validation_work(bytes: &[u8]) -> Result<u64, ResolveError> {
    let decoder = webp_decoder(bytes)?;
    let (frames, additional_frame_pixels, _) = scan_webp_animation(bytes, decoder.dimensions())?;
    if frames != decoder.num_frames() {
        return Err(ResolveError::WebPDecodeFailed);
    }
    additional_frame_pixels
        .checked_mul(4)
        .ok_or(ResolveError::AssetWork {
            attempted: u64::MAX,
        })
}

fn scan_webp_animation(bytes: &[u8], canvas: (u32, u32)) -> Result<(u32, u64, bool), ResolveError> {
    let declared_size = usize::try_from(u32::from_le_bytes(
        bytes
            .get(4..8)
            .ok_or(ResolveError::WebPDecodeFailed)?
            .try_into()
            .map_err(|_| ResolveError::WebPDecodeFailed)?,
    ))
    .map_err(|_| ResolveError::WebPDecodeFailed)?;
    if declared_size.checked_add(8) != Some(bytes.len()) {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let mut offset = 12_usize;
    let mut frames = 0_u32;
    let mut additional_frame_pixels = 0_u64;
    let mut animation_has_alpha = false;
    while offset < bytes.len() {
        let (kind, data, next) = webp_chunk_at(bytes, offset)?;
        if kind == *b"ANMF" {
            let frame_header = data.get(..16).ok_or(ResolveError::WebPDecodeFailed)?;
            if frame_header[15] & !0x03 != 0 {
                return Err(ResolveError::WebPDecodeFailed);
            }
            let x = read_webp_u24(&frame_header[..3])?.saturating_mul(2);
            let y = read_webp_u24(&frame_header[3..6])?.saturating_mul(2);
            let width = u64::from(read_webp_u24(&frame_header[6..9])?).saturating_add(1);
            let height = u64::from(read_webp_u24(&frame_header[9..12])?).saturating_add(1);
            if u64::from(x).saturating_add(width) > u64::from(canvas.0)
                || u64::from(y).saturating_add(height) > u64::from(canvas.1)
            {
                return Err(ResolveError::WebPDecodeFailed);
            }
            animation_has_alpha |=
                validate_webp_frame_chunks(data.get(16..).ok_or(ResolveError::WebPDecodeFailed)?)?;
            frames = frames
                .checked_add(1)
                .ok_or(ResolveError::WebPDecodeFailed)?;
            if frames > 1 {
                additional_frame_pixels = additional_frame_pixels
                    .checked_add(width.checked_mul(height).ok_or(ResolveError::AssetWork {
                        attempted: u64::MAX,
                    })?)
                    .ok_or(ResolveError::AssetWork {
                        attempted: u64::MAX,
                    })?;
            }
        }
        offset = next;
    }
    if offset != bytes.len() {
        return Err(ResolveError::WebPDecodeFailed);
    }
    Ok((frames, additional_frame_pixels, animation_has_alpha))
}

fn webp_chunk_at(bytes: &[u8], offset: usize) -> Result<([u8; 4], &[u8], usize), ResolveError> {
    let header = bytes
        .get(offset..offset.saturating_add(8))
        .ok_or(ResolveError::WebPDecodeFailed)?;
    let kind = header[..4]
        .try_into()
        .map_err(|_| ResolveError::WebPDecodeFailed)?;
    let length = usize::try_from(u32::from_le_bytes(
        header[4..8]
            .try_into()
            .map_err(|_| ResolveError::WebPDecodeFailed)?,
    ))
    .map_err(|_| ResolveError::WebPDecodeFailed)?;
    let data_start = offset
        .checked_add(8)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    let data_end = data_start
        .checked_add(length)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    let data = bytes
        .get(data_start..data_end)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    let next = data_end
        .checked_add(length & 1)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    bytes
        .get(data_end..next)
        .ok_or(ResolveError::WebPDecodeFailed)?;
    Ok((kind, data, next))
}

fn validate_webp_frame_chunks(chunks: &[u8]) -> Result<bool, ResolveError> {
    let (first, first_data, next) = webp_chunk_at(chunks, 0)?;
    match first {
        kind if kind == *b"VP8 " => {
            if next != chunks.len() {
                return Err(ResolveError::WebPDecodeFailed);
            }
            Ok(false)
        }
        kind if kind == *b"VP8L" => {
            if next != chunks.len() {
                return Err(ResolveError::WebPDecodeFailed);
            }
            webp_lossless_has_alpha(first_data)
        }
        kind if kind == *b"ALPH" => {
            let (second, _, end) = webp_chunk_at(chunks, next)?;
            if second != *b"VP8 " || end != chunks.len() {
                return Err(ResolveError::WebPDecodeFailed);
            }
            Ok(true)
        }
        _ => Err(ResolveError::WebPDecodeFailed),
    }
}

fn webp_lossless_has_alpha(data: &[u8]) -> Result<bool, ResolveError> {
    if data.first() != Some(&0x2f) {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let header = u32::from_le_bytes(
        data.get(1..5)
            .ok_or(ResolveError::WebPDecodeFailed)?
            .try_into()
            .map_err(|_| ResolveError::WebPDecodeFailed)?,
    );
    Ok(header & (1 << 28) != 0)
}

fn read_webp_u24(bytes: &[u8]) -> Result<u32, ResolveError> {
    let bytes: [u8; 3] = bytes
        .try_into()
        .map_err(|_| ResolveError::WebPDecodeFailed)?;
    Ok(u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16))
}

pub(super) fn validate_webp(bytes: &[u8]) -> Result<(u32, u32, WebPSemantics), ResolveError> {
    let mut decoder = webp_decoder(bytes)?;
    let (width, height) = decoder.dimensions();
    let animation_frames = decoder.num_frames().max(1);
    let orientation = decoder
        .exif_metadata()
        .map_err(map_webp_error)?
        .as_deref()
        .and_then(webp_exif_orientation);
    if decoder.is_animated() {
        let (_, _, frames_have_alpha) = scan_webp_animation(bytes, (width, height))?;
        if decoder.has_alpha() != frames_have_alpha {
            return Err(ResolveError::WebPDecodeFailed);
        }
        validate_webp_animation_frames(bytes)?;
    } else {
        let output_size = decoder.output_buffer_size().ok_or(ResolveError::Memory)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(output_size)
            .map_err(|_| ResolveError::Memory)?;
        output.resize(output_size, 0);
        decoder.read_image(&mut output).map_err(map_webp_error)?;
    }
    let (width, height) = oriented_dimensions(width, height, orientation.unwrap_or(1));
    Ok((
        width,
        height,
        WebPSemantics {
            animation_frames,
            orientation,
        },
    ))
}

fn validate_webp_animation_frames(bytes: &[u8]) -> Result<(), ResolveError> {
    let mut offset = 12_usize;
    while offset < bytes.len() {
        let (kind, data, next) = webp_chunk_at(bytes, offset)?;
        if kind == *b"ANMF" {
            validate_webp_animation_frame(data)?;
        }
        offset = next;
    }
    Ok(())
}

fn validate_webp_animation_frame(frame: &[u8]) -> Result<(), ResolveError> {
    let (standalone, width, height) = standalone_webp_frame(frame)?;
    let mut decoder = webp_decoder(&standalone)?;
    if decoder.dimensions() != (width, height) || decoder.is_animated() {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let output_size = decoder.output_buffer_size().ok_or(ResolveError::Memory)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_size)
        .map_err(|_| ResolveError::Memory)?;
    output.resize(output_size, 0);
    decoder.read_image(&mut output).map_err(map_webp_error)
}

fn standalone_webp_frame(frame: &[u8]) -> Result<(Vec<u8>, u32, u32), ResolveError> {
    let frame_header = frame.get(..16).ok_or(ResolveError::WebPDecodeFailed)?;
    if frame_header[15] & !0x03 != 0 {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let width = read_webp_u24(&frame_header[6..9])?.saturating_add(1);
    let height = read_webp_u24(&frame_header[9..12])?.saturating_add(1);
    let chunks = frame.get(16..).ok_or(ResolveError::WebPDecodeFailed)?;
    let has_alpha = validate_webp_frame_chunks(chunks)?;
    let needs_vp8x = has_alpha && chunks.starts_with(b"ALPH");
    let body_size = chunks
        .len()
        .checked_add(if needs_vp8x { 18 } else { 0 })
        .ok_or(ResolveError::Memory)?;
    let riff_size = body_size
        .checked_add(4)
        .and_then(|size| u32::try_from(size).ok())
        .ok_or(ResolveError::Memory)?;
    let standalone_size = body_size.checked_add(12).ok_or(ResolveError::Memory)?;
    let mut standalone = Vec::new();
    standalone
        .try_reserve_exact(standalone_size)
        .map_err(|_| ResolveError::Memory)?;
    standalone.extend_from_slice(b"RIFF");
    standalone.extend_from_slice(&riff_size.to_le_bytes());
    standalone.extend_from_slice(b"WEBP");
    if needs_vp8x {
        standalone.extend_from_slice(b"VP8X");
        standalone.extend_from_slice(&10_u32.to_le_bytes());
        standalone.extend_from_slice(&[0x10, 0, 0, 0]);
        // ANMF and VP8X both store width/height minus one as little-endian u24.
        standalone.extend_from_slice(&frame_header[6..12]);
    }
    standalone.extend_from_slice(chunks);
    Ok((standalone, width, height))
}

fn webp_exif_orientation(exif: &[u8]) -> Option<u8> {
    exif_orientation(exif.strip_prefix(b"Exif\0\0").unwrap_or(exif))
}

pub(super) fn decode_webp_default_frame(
    bytes: &[u8],
    source_width: u32,
    source_height: u32,
    pixels: u64,
    color_normalization: ColorNormalization,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    let decoder = webp_decoder(bytes)?;
    // Frame zero is drawn onto transparent black. Do not delegate this to the
    // animation compositor: opaque source channels do not imply an opaque
    // canvas, and blending against an empty destination must preserve samples.
    let mut first_frame = None;
    let mut placement = (0_u32, 0_u32, source_width, source_height);
    if decoder.is_animated() {
        let mut offset = 12;
        while offset < bytes.len() {
            let (kind, data, next) = webp_chunk_at(bytes, offset)?;
            if kind == *b"ANMF" {
                let header = data.get(..16).ok_or(ResolveError::WebPDecodeFailed)?;
                let (standalone, width, height) = standalone_webp_frame(data)?;
                let x = read_webp_u24(&header[..3])?
                    .checked_mul(2)
                    .ok_or(ResolveError::WebPDecodeFailed)?;
                let y = read_webp_u24(&header[3..6])?
                    .checked_mul(2)
                    .ok_or(ResolveError::WebPDecodeFailed)?;
                if x.checked_add(width).is_none_or(|v| v > source_width)
                    || y.checked_add(height).is_none_or(|v| v > source_height)
                {
                    return Err(ResolveError::WebPDecodeFailed);
                }
                placement = (x, y, width, height);
                first_frame = Some(standalone);
                break;
            }
            offset = next;
        }
        if first_frame.is_none() {
            return Err(ResolveError::WebPDecodeFailed);
        }
    }
    let mut decoder = if let Some(frame) = &first_frame {
        drop(decoder);
        let decoder = webp_decoder(frame)?;
        if decoder.dimensions() != (placement.2, placement.3) || decoder.is_animated() {
            return Err(ResolveError::WebPDecodeFailed);
        }
        decoder
    } else {
        decoder
    };
    let output_size = decoder.output_buffer_size().ok_or(ResolveError::Memory)?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(output_size)
        .map_err(|_| ResolveError::Memory)?;
    decoded.resize(output_size, 0);
    decoder.read_image(&mut decoded).map_err(map_webp_error)?;
    let channels = if decoder.has_alpha() { 4_u8 } else { 3_u8 };
    drop(decoder);
    drop(first_frame);

    let (x, y, width, height) = placement;
    let frame_pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(ResolveError::Memory)?;
    let expected = usize::try_from(
        frame_pixels
            .checked_mul(u64::from(channels))
            .ok_or(ResolveError::Memory)?,
    )
    .map_err(|_| ResolveError::Memory)?;
    if decoded.len() != expected {
        return Err(ResolveError::WebPDecodeFailed);
    }
    let x = usize::try_from(x).map_err(|_| ResolveError::Memory)?;
    let y = usize::try_from(y).map_err(|_| ResolveError::Memory)?;
    let width = usize::try_from(width).map_err(|_| ResolveError::Memory)?;
    let canvas_width = usize::try_from(source_width).map_err(|_| ResolveError::Memory)?;

    let rgba_size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    rgba.resize(rgba_size, 0);
    for (i, pixel) in decoded.chunks_exact(usize::from(channels)).enumerate() {
        let row = y.checked_add(i / width).ok_or(ResolveError::Memory)?;
        let col = x.checked_add(i % width).ok_or(ResolveError::Memory)?;
        let start = row
            .checked_mul(canvas_width)
            .and_then(|n| n.checked_add(col))
            .and_then(|n| n.checked_mul(4))
            .ok_or(ResolveError::Memory)?;
        let end = start.checked_add(4).ok_or(ResolveError::Memory)?;
        let target = rgba
            .get_mut(start..end)
            .ok_or(ResolveError::WebPDecodeFailed)?;
        target[..3].copy_from_slice(&pixel[..3]);
        target[3] = if channels == 4 { pixel[3] } else { 255 };
        if target[3] == 0 {
            target.fill(0);
        }
    }
    drop(decoded);
    let untransformed = matches!(color_normalization, ColorNormalization::None);
    normalize_rgba(&mut rgba, color_normalization)?;
    if let Some(orientation) = orientation {
        rgba = orient_rgba(&rgba, source_width, source_height, orientation)?;
    }
    if !untransformed {
        return finish_rgba(rgba);
    }
    // Chromium's untransformed WebP path requests libwebp's premultiplied
    // output, which truncates c*a/255. Keep the straight samples for PDF;
    // color-transformed images continue through the ordinary rounded path.
    let mut premultiplied_rgba = try_copy(&rgba)?;
    for pixel in premultiplied_rgba.as_chunks_mut::<4>().0 {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = (u16::from(*channel) * alpha / 255) as u8;
        }
    }
    Ok(DecodedRaster {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied_rgba),
    })
}

pub(super) fn map_webp_error(error: WebPDecodingError) -> ResolveError {
    if matches!(error, WebPDecodingError::MemoryLimitExceeded) {
        ResolveError::Memory
    } else {
        ResolveError::WebPDecodeFailed
    }
}
