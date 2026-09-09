//! The GIF codec: a block-level scan that counts frames and prices the
//! additional frames' pixels, validation that decodes every frame's indices
//! against its palette under a memory limit, and the default frame composited
//! onto a transparent canvas with straight and premultiplied output.

use std::io::Cursor;
use std::num::NonZeroU64;
use std::sync::Arc;

use super::{decoder_workspace_bytes, try_copy, DecodedRaster, ResolveError};

pub(super) const GIF87A_SIGNATURE: &[u8; 6] = b"GIF87a";
pub(super) const GIF89A_SIGNATURE: &[u8; 6] = b"GIF89a";

#[derive(Clone, Copy)]
pub(super) struct GifSemantics {
    pub(super) animation_frames: u32,
}

struct GifMetadata {
    width: u32,
    height: u32,
    animation_frames: u32,
    additional_frame_pixels: u64,
}

pub(super) fn read_gif_dimensions(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    if !bytes.starts_with(GIF87A_SIGNATURE) && !bytes.starts_with(GIF89A_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let descriptor = bytes.get(6..13).ok_or(ResolveError::GifDecodeFailed)?;
    let width = u32::from(u16::from_le_bytes([descriptor[0], descriptor[1]]));
    let height = u32::from(u16::from_le_bytes([descriptor[2], descriptor[3]]));
    if width == 0 || height == 0 {
        return Err(ResolveError::GifDecodeFailed);
    }
    Ok((width, height))
}

pub(super) fn gif_animation_validation_work(
    bytes: &[u8],
    _pixels: u64,
) -> Result<u64, ResolveError> {
    scan_gif(bytes)?
        .additional_frame_pixels
        .checked_mul(4)
        .ok_or(ResolveError::AssetWork {
            attempted: u64::MAX,
        })
}

pub(super) fn validate_gif(bytes: &[u8]) -> Result<(u32, u32, GifSemantics), ResolveError> {
    let metadata = scan_gif(bytes)?;
    let mut options = gif_decoder_options(gif_validation_memory_limit(
        metadata.width,
        metadata.height,
    )?);
    options.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = options
        .read_info(Cursor::new(bytes))
        .map_err(map_gif_error)?;
    let mut decoded_frames = 0_u32;
    while let Some(frame) = decoder.next_frame_info().map_err(map_gif_error)? {
        let expected_pixels = u64::from(frame.width)
            .checked_mul(u64::from(frame.height))
            .ok_or(ResolveError::GifDecodeFailed)?;
        let transparent = frame.transparent;
        let palette_entries = decoder.palette().map_err(map_gif_error)?.len() / 3;
        if transparent.is_some_and(|index| usize::from(index) >= palette_entries) {
            return Err(ResolveError::GifDecodeFailed);
        }
        let mut decoded_pixels = 0_u64;
        let mut index = [0_u8; 1];
        while decoder.fill_buffer(&mut index).map_err(map_gif_error)? {
            decoded_pixels = decoded_pixels
                .checked_add(1)
                .ok_or(ResolveError::GifDecodeFailed)?;
            if decoded_pixels > expected_pixels || usize::from(index[0]) >= palette_entries {
                return Err(ResolveError::GifDecodeFailed);
            }
        }
        if decoded_pixels != expected_pixels {
            return Err(ResolveError::GifDecodeFailed);
        }
        decoded_frames = decoded_frames
            .checked_add(1)
            .ok_or(ResolveError::GifDecodeFailed)?;
    }
    if decoded_frames != metadata.animation_frames {
        return Err(ResolveError::GifDecodeFailed);
    }
    Ok((
        metadata.width,
        metadata.height,
        GifSemantics {
            animation_frames: metadata.animation_frames,
        },
    ))
}

pub(super) fn decode_gif_default_frame(
    bytes: &[u8],
    width: u32,
    height: u32,
    pixels: u64,
) -> Result<DecodedRaster, ResolveError> {
    let mut options = gif_decoder_options(gif_projection_memory_limit(width, height)?);
    options.set_color_output(gif::ColorOutput::Indexed);
    let mut decoder = options
        .read_info(Cursor::new(bytes))
        .map_err(map_gif_error)?;
    let frame = decoder
        .next_frame_info()
        .map_err(map_gif_error)?
        .ok_or(ResolveError::GifDecodeFailed)?;
    let frame_width = frame.width;
    let frame_height = frame.height;
    let frame_left = frame.left;
    let frame_top = frame.top;
    let transparent = frame.transparent;
    let palette = decoder.palette().map_err(map_gif_error)?;
    let mut palette_copy = [0_u8; 256 * 3];
    let palette_copy = palette_copy
        .get_mut(..palette.len())
        .ok_or(ResolveError::GifDecodeFailed)?;
    palette_copy.copy_from_slice(palette);
    let palette_entries = palette_copy.len() / 3;
    if transparent.is_some_and(|index| usize::from(index) >= palette_entries) {
        return Err(ResolveError::GifDecodeFailed);
    }
    let frame_pixels = u64::from(frame_width)
        .checked_mul(u64::from(frame_height))
        .ok_or(ResolveError::Memory)?;
    let frame_size = usize::try_from(frame_pixels).map_err(|_| ResolveError::Memory)?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(frame_size)
        .map_err(|_| ResolveError::Memory)?;
    indices.resize(frame_size, 0);
    decoder
        .read_into_buffer(&mut indices)
        .map_err(map_gif_error)?;
    let rgba_size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    rgba.resize(rgba_size, 0);
    let frame_width = usize::from(frame_width);
    let canvas_width = usize::try_from(width).map_err(|_| ResolveError::Memory)?;
    for y in 0..usize::from(frame_height) {
        for x in 0..frame_width {
            let index = *indices
                .get(y.saturating_mul(frame_width).saturating_add(x))
                .ok_or(ResolveError::GifDecodeFailed)?;
            if usize::from(index) >= palette_entries {
                return Err(ResolveError::GifDecodeFailed);
            }
            let destination = y
                .checked_add(usize::from(frame_top))
                .and_then(|value| value.checked_mul(canvas_width))
                .and_then(|value| value.checked_add(usize::from(frame_left)))
                .and_then(|value| value.checked_add(x))
                .and_then(|value| value.checked_mul(4))
                .ok_or(ResolveError::Memory)?;
            let pixel = rgba
                .get_mut(destination..destination.saturating_add(4))
                .ok_or(ResolveError::GifDecodeFailed)?;
            if transparent == Some(index) {
                pixel.fill(0);
            } else {
                let palette_offset = usize::from(index).saturating_mul(3);
                pixel[..3].copy_from_slice(
                    palette_copy
                        .get(palette_offset..palette_offset.saturating_add(3))
                        .ok_or(ResolveError::GifDecodeFailed)?,
                );
                pixel[3] = 255;
            }
        }
    }
    for pixel in rgba.as_chunks_mut::<4>().0 {
        if pixel[3] == 0 {
            pixel[..3].fill(0);
        }
    }
    let mut premultiplied_rgba = try_copy(&rgba)?;
    for pixel in premultiplied_rgba.as_chunks_mut::<4>().0 {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Ok(DecodedRaster {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied_rgba),
    })
}

fn gif_decoder_options(memory_limit: gif::MemoryLimit) -> gif::DecodeOptions {
    let mut options = gif::DecodeOptions::new();
    options.check_frame_consistency(true);
    options.check_lzw_end_code(true);
    options.set_memory_limit(memory_limit);
    options
}

fn gif_validation_memory_limit(width: u32, height: u32) -> Result<gif::MemoryLimit, ResolveError> {
    NonZeroU64::new(decoder_workspace_bytes(width, height)?)
        .map(gif::MemoryLimit::Bytes)
        .ok_or(ResolveError::Memory)
}

fn gif_projection_memory_limit(width: u32, height: u32) -> Result<gif::MemoryLimit, ResolveError> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| bytes.checked_add(1_048_576))
        .and_then(NonZeroU64::new)
        .ok_or(ResolveError::Memory)?;
    Ok(gif::MemoryLimit::Bytes(bytes))
}

fn map_gif_error(error: gif::DecodingError) -> ResolveError {
    match error {
        gif::DecodingError::OutOfMemory | gif::DecodingError::MemoryLimit => ResolveError::Memory,
        _ => ResolveError::GifDecodeFailed,
    }
}

fn scan_gif(bytes: &[u8]) -> Result<GifMetadata, ResolveError> {
    let (width, height) = read_gif_dimensions(bytes)?;
    let is_gif89a = bytes.starts_with(GIF89A_SIGNATURE);
    let descriptor = bytes.get(6..13).ok_or(ResolveError::GifDecodeFailed)?;
    let mut offset = 13_usize;
    if descriptor[4] & 0x80 != 0 {
        let entries = gif_color_table_entries(descriptor[4])?;
        if usize::from(descriptor[5]) >= entries {
            return Err(ResolveError::GifDecodeFailed);
        }
        offset = skip_gif_color_table(bytes, offset, descriptor[4])?;
    }
    let mut frames = 0_u32;
    let mut additional_frame_pixels = 0_u64;
    loop {
        let block = *bytes.get(offset).ok_or(ResolveError::GifDecodeFailed)?;
        offset = offset.checked_add(1).ok_or(ResolveError::GifDecodeFailed)?;
        match block {
            0x2c => {
                let image = bytes
                    .get(offset..offset.saturating_add(9))
                    .ok_or(ResolveError::GifDecodeFailed)?;
                let frame_width = u32::from(u16::from_le_bytes([image[4], image[5]]));
                let frame_height = u32::from(u16::from_le_bytes([image[6], image[7]]));
                if frame_width == 0 || frame_height == 0 {
                    return Err(ResolveError::GifDecodeFailed);
                }
                if image[8] & 0x18 != 0 {
                    return Err(ResolveError::GifDecodeFailed);
                }
                offset = offset.checked_add(9).ok_or(ResolveError::GifDecodeFailed)?;
                if image[8] & 0x80 != 0 {
                    offset = skip_gif_color_table(bytes, offset, image[8])?;
                }
                let code_size = *bytes.get(offset).ok_or(ResolveError::GifDecodeFailed)?;
                if !(2..=8).contains(&code_size) {
                    return Err(ResolveError::GifDecodeFailed);
                }
                offset = skip_gif_sub_blocks(bytes, offset.saturating_add(1))?;
                frames = frames.checked_add(1).ok_or(ResolveError::GifDecodeFailed)?;
                if frames > 1 {
                    additional_frame_pixels = additional_frame_pixels
                        .checked_add(
                            u64::from(frame_width)
                                .checked_mul(u64::from(frame_height))
                                .ok_or(ResolveError::AssetWork {
                                    attempted: u64::MAX,
                                })?,
                        )
                        .ok_or(ResolveError::AssetWork {
                            attempted: u64::MAX,
                        })?;
                }
            }
            0x21 => {
                if !is_gif89a {
                    return Err(ResolveError::GifDecodeFailed);
                }
                let label = *bytes.get(offset).ok_or(ResolveError::GifDecodeFailed)?;
                offset = offset.checked_add(1).ok_or(ResolveError::GifDecodeFailed)?;
                if label == 0xf9 {
                    let control = bytes
                        .get(offset..offset.saturating_add(6))
                        .ok_or(ResolveError::GifDecodeFailed)?;
                    let disposal = (control[1] >> 2) & 0x07;
                    if control[0] != 4 || control[5] != 0 || control[1] & 0xe0 != 0 || disposal > 3
                    {
                        return Err(ResolveError::GifDecodeFailed);
                    }
                    offset = offset.checked_add(6).ok_or(ResolveError::GifDecodeFailed)?;
                } else {
                    let required_block_size = match label {
                        0x01 => Some(12),
                        0xff => Some(11),
                        0xfe => None,
                        _ => return Err(ResolveError::GifDecodeFailed),
                    };
                    if required_block_size
                        .is_some_and(|required| bytes.get(offset).copied() != Some(required))
                    {
                        return Err(ResolveError::GifDecodeFailed);
                    }
                    offset = skip_gif_sub_blocks(bytes, offset)?;
                }
            }
            0x3b if frames > 0 && offset == bytes.len() => {
                return Ok(GifMetadata {
                    width,
                    height,
                    animation_frames: frames,
                    additional_frame_pixels,
                });
            }
            _ => return Err(ResolveError::GifDecodeFailed),
        }
    }
}

fn skip_gif_color_table(bytes: &[u8], offset: usize, packed: u8) -> Result<usize, ResolveError> {
    let entries = gif_color_table_entries(packed)?;
    let end = offset
        .checked_add(entries.saturating_mul(3))
        .ok_or(ResolveError::GifDecodeFailed)?;
    bytes
        .get(offset..end)
        .ok_or(ResolveError::GifDecodeFailed)?;
    Ok(end)
}

fn gif_color_table_entries(packed: u8) -> Result<usize, ResolveError> {
    1_usize
        .checked_shl(u32::from((packed & 0x07) + 1))
        .ok_or(ResolveError::GifDecodeFailed)
}

fn skip_gif_sub_blocks(bytes: &[u8], mut offset: usize) -> Result<usize, ResolveError> {
    loop {
        let length = usize::from(*bytes.get(offset).ok_or(ResolveError::GifDecodeFailed)?);
        offset = offset
            .checked_add(1)
            .and_then(|offset| offset.checked_add(length))
            .ok_or(ResolveError::GifDecodeFailed)?;
        if length == 0 {
            return Ok(offset);
        }
        bytes
            .get(offset.saturating_sub(length)..offset)
            .ok_or(ResolveError::GifDecodeFailed)?;
    }
}
