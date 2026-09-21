//! The JPEG codec: marker-level validation that admits one baseline or
//! progressive image (or an MPF gain-map pair) with a complete ICC sequence,
//! header and EXIF-orientation reads, and the strict `zune-jpeg` decode with
//! ICC or linear color normalization applied in the decoder's color space.

use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions};
use std::sync::Arc;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use super::color::{ColorNormalization, LINEAR_TO_SRGB};
use super::orientation::{exif_orientation, orient_rgba, oriented_dimensions};
use super::{try_copy, DecodedRaster, ResolveError};

pub(super) const JPEG_SIGNATURE: &[u8; 3] = b"\xff\xd8\xff";

#[derive(Clone, Copy, Default)]
pub(super) struct JpegSemantics {
    pub(super) orientation: Option<u8>,
}

struct JpegMetadata {
    width: u32,
    height: u32,
    orientation: Option<u8>,
}

pub(super) fn jpeg_decoder(bytes: &[u8]) -> JpegDecoder<ZCursor<&[u8]>> {
    let options = DecoderOptions::default()
        .set_max_width(usize::from(u16::MAX))
        .set_max_height(usize::from(u16::MAX))
        .set_use_unsafe(false)
        .set_strict_mode(true)
        .jpeg_set_out_colorspace(ColorSpace::RGB);
    JpegDecoder::new_with_options(ZCursor::new(bytes), options)
}

pub(super) fn read_jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32), ResolveError> {
    if !bytes.starts_with(JPEG_SIGNATURE) {
        return Err(ResolveError::Unsupported);
    }
    let metadata = read_jpeg_metadata(bytes)?;
    Ok((metadata.width, metadata.height))
}

fn read_jpeg_metadata(bytes: &[u8]) -> Result<JpegMetadata, ResolveError> {
    let mut decoder = jpeg_decoder(bytes);
    decoder
        .decode_headers()
        .map_err(|_| ResolveError::JpegDecodeFailed)?;
    let info = decoder.info().ok_or(ResolveError::JpegDecodeFailed)?;
    if info.width == 0 || info.height == 0 {
        return Err(ResolveError::JpegDecodeFailed);
    }
    Ok(JpegMetadata {
        width: u32::from(info.width),
        height: u32::from(info.height),
        orientation: decoder.exif().and_then(|data| exif_orientation(data)),
    })
}

pub(super) fn validate_static_jpeg(
    bytes: &[u8],
) -> Result<(u32, u32, JpegSemantics), ResolveError> {
    if !bytes.starts_with(JPEG_SIGNATURE) || validate_jpeg_markers(bytes).is_err() {
        return Err(if bytes.starts_with(JPEG_SIGNATURE) {
            ResolveError::JpegDecodeFailed
        } else {
            ResolveError::Unsupported
        });
    }
    let metadata = read_jpeg_metadata(bytes)?;
    let orientation = metadata.orientation;
    let (width, height) =
        oriented_dimensions(metadata.width, metadata.height, orientation.unwrap_or(1));
    Ok((width, height, JpegSemantics { orientation }))
}

fn validate_jpeg_markers(bytes: &[u8]) -> Result<(), ResolveError> {
    let mut image_start = 0_usize;
    let mut image_count = 0_u8;
    let mut primary_has_mpf = false;
    let mut primary_has_gain_map_metadata = false;
    while image_start < bytes.len() {
        let mut icc_parts = [false; 256];
        let mut icc_part_count = None;
        if bytes.get(image_start..image_start.saturating_add(2)) != Some(b"\xff\xd8") {
            return Err(ResolveError::JpegDecodeFailed);
        }
        let mut offset = image_start + 2;
        let mut saw_frame = false;
        let mut saw_scan = false;
        loop {
            let marker_start = offset;
            if bytes.get(offset) != Some(&0xff) {
                return Err(ResolveError::JpegDecodeFailed);
            }
            while bytes.get(offset) == Some(&0xff) {
                offset = offset
                    .checked_add(1)
                    .ok_or(ResolveError::JpegDecodeFailed)?;
            }
            let marker = *bytes.get(offset).ok_or(ResolveError::JpegDecodeFailed)?;
            offset = offset
                .checked_add(1)
                .ok_or(ResolveError::JpegDecodeFailed)?;
            match marker {
                0xd9 if saw_frame && saw_scan => {
                    if let Some(part_count) = icc_part_count {
                        if !(1..=part_count).all(|part| icc_parts[usize::from(part)]) {
                            return Err(ResolveError::JpegDecodeFailed);
                        }
                    }
                    image_start = offset;
                    break;
                }
                0xd8 | 0x00 | 0x01 | 0xd0..=0xd8 => {
                    return Err(ResolveError::JpegDecodeFailed);
                }
                _ => {
                    let length_end = offset
                        .checked_add(2)
                        .ok_or(ResolveError::JpegDecodeFailed)?;
                    let length = usize::from(u16::from_be_bytes(
                        bytes
                            .get(offset..length_end)
                            .ok_or(ResolveError::JpegDecodeFailed)?
                            .try_into()
                            .map_err(|_| ResolveError::JpegDecodeFailed)?,
                    ));
                    if length < 2 {
                        return Err(ResolveError::JpegDecodeFailed);
                    }
                    let segment_end = offset
                        .checked_add(length)
                        .ok_or(ResolveError::JpegDecodeFailed)?;
                    let segment = bytes
                        .get(length_end..segment_end)
                        .ok_or(ResolveError::JpegDecodeFailed)?;
                    if image_count == 0 && marker == 0xe2 && segment.starts_with(b"MPF\0") {
                        primary_has_mpf = true;
                    }
                    if marker == 0xe2 && segment.starts_with(b"ICC_PROFILE\0") {
                        let sequence = *segment.get(12).ok_or(ResolveError::JpegDecodeFailed)?;
                        let part_count = *segment.get(13).ok_or(ResolveError::JpegDecodeFailed)?;
                        if sequence == 0
                            || part_count == 0
                            || sequence > part_count
                            || icc_part_count.is_some_and(|expected| expected != part_count)
                            || icc_parts[usize::from(sequence)]
                        {
                            return Err(ResolveError::JpegDecodeFailed);
                        }
                        icc_part_count = Some(part_count);
                        icc_parts[usize::from(sequence)] = true;
                    }
                    if image_count == 0
                        && marker == 0xe1
                        && segment
                            .windows(b"hdrgm:Version".len())
                            .any(|window| window == b"hdrgm:Version")
                    {
                        primary_has_gain_map_metadata = true;
                    }
                    if matches!(marker, 0xc0..=0xc2) {
                        if segment.len() < 6 || segment[0] != 8 {
                            return Err(ResolveError::JpegDecodeFailed);
                        }
                        saw_frame = true;
                    } else if matches!(marker, 0xc3..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc)
                    {
                        return Err(ResolveError::JpegDecodeFailed);
                    }
                    offset = segment_end;
                    if marker != 0xda {
                        continue;
                    }
                    if !saw_frame || segment.is_empty() {
                        return Err(ResolveError::JpegDecodeFailed);
                    }
                    saw_scan = true;
                    loop {
                        let byte = *bytes.get(offset).ok_or(ResolveError::JpegDecodeFailed)?;
                        offset = offset
                            .checked_add(1)
                            .ok_or(ResolveError::JpegDecodeFailed)?;
                        if byte != 0xff {
                            continue;
                        }
                        while bytes.get(offset) == Some(&0xff) {
                            offset = offset
                                .checked_add(1)
                                .ok_or(ResolveError::JpegDecodeFailed)?;
                        }
                        match bytes.get(offset).copied() {
                            Some(0x00 | 0xd0..=0xd7) => offset += 1,
                            Some(_) => {
                                offset = offset
                                    .checked_sub(1)
                                    .ok_or(ResolveError::JpegDecodeFailed)?;
                                break;
                            }
                            None => return Err(ResolveError::JpegDecodeFailed),
                        }
                    }
                }
            }
            if offset <= marker_start {
                return Err(ResolveError::JpegDecodeFailed);
            }
        }
        image_count = image_count
            .checked_add(1)
            .ok_or(ResolveError::JpegDecodeFailed)?;
        if image_start < bytes.len()
            && (image_count != 1 || !primary_has_mpf || !primary_has_gain_map_metadata)
        {
            return Err(ResolveError::JpegDecodeFailed);
        }
    }
    if image_count > 2 {
        return Err(ResolveError::JpegDecodeFailed);
    }
    Ok(())
}

pub(super) fn decode_static_jpeg(
    bytes: &[u8],
    pixels: u64,
    color_normalization: ColorNormalization,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    let source_color_space = match &color_normalization {
        ColorNormalization::Lut(lut) => {
            if lut.first().ok_or(ResolveError::DecodeFailed)?.gray() {
                ColorSpace::Luma
            } else {
                ColorSpace::RGB
            }
        }
        ColorNormalization::Profile(profile) => match profile.color_space {
            DataColorSpace::Rgb => ColorSpace::RGB,
            DataColorSpace::Gray => ColorSpace::Luma,
            _ => return Err(ResolveError::Unsupported),
        },
        ColorNormalization::None | ColorNormalization::LinearSrgb => ColorSpace::RGB,
        ColorNormalization::Hdr(_) => return Err(ResolveError::Unsupported),
    };
    let mut decoder = jpeg_decoder(bytes);
    decoder
        .decode_headers()
        .map_err(|_| ResolveError::JpegDecodeFailed)?;
    // Read four-component data without zune's rounded multiply-by-K. The
    // reference's untagged JPEG projection truncates that operation (#187).
    // Preserve the existing gray-profile route and normalize RGB before CMS.
    // Adobe's header color tag is provisional for three-component images;
    // zune resolves those to RGB/YCbCr only during decoder setup.
    let component_count = decoder
        .info()
        .ok_or(ResolveError::JpegDecodeFailed)?
        .components;
    let raw_space = match (
        decoder.input_colorspace(),
        source_color_space,
        component_count,
    ) {
        (Some(space @ (ColorSpace::CMYK | ColorSpace::YCCK)), ColorSpace::RGB, 4) => space,
        _ => source_color_space,
    };
    decoder.set_options((*decoder.options()).jpeg_set_out_colorspace(raw_space));
    let mut rgb = decoder
        .decode()
        .map_err(|_| ResolveError::JpegDecodeFailed)?;
    let info = decoder.info().ok_or(ResolveError::JpegDecodeFailed)?;
    let components = u64::try_from(raw_space.num_components()).map_err(|_| ResolveError::Memory)?;
    let expected_rgb = usize::try_from(pixels.checked_mul(components).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    if rgb.len() != expected_rgb {
        return Err(ResolveError::JpegDecodeFailed);
    }
    if matches!(raw_space, ColorSpace::CMYK | ColorSpace::YCCK) {
        // Compact in place: every four input bytes become three output bytes.
        // All arithmetic is bounded integer arithmetic, identical on both targets.
        for index in 0..rgb.len() / 4 {
            let p: [u8; 4] = rgb[index * 4..index * 4 + 4]
                .try_into()
                .map_err(|_| ResolveError::JpegDecodeFailed)?;
            let channels = if raw_space == ColorSpace::YCCK {
                let y = i32::from(p[0]);
                let cb = i32::from(p[1]) - 128;
                let cr = i32::from(p[2]) - 128;
                // Full-range JPEG YCbCr coefficients, nearest integer before
                // inversion to CMY. The independent browser matrix qualifies
                // this conversion separately from the following K operation.
                [
                    y + ((91881 * cr + 32768) >> 16),
                    y + ((-22554 * cb - 46802 * cr + 32768) >> 16),
                    y + ((116130 * cb + 32768) >> 16),
                ]
                .map(|v| (255 - v.clamp(0, 255)) as u8)
            } else {
                [p[0], p[1], p[2]]
            };
            let channels = channels.map(|v| (u32::from(v) * u32::from(p[3]) / 255) as u8);
            rgb[index * 3..index * 3 + 3].copy_from_slice(&channels);
        }
        rgb.truncate(rgb.len() / 4 * 3);
    }
    if let ColorNormalization::Lut(lut) = color_normalization {
        let lut = lut.first().ok_or(ResolveError::DecodeFailed)?;
        if lut.gray() {
            let size = usize::try_from(pixels.checked_mul(3).ok_or(ResolveError::Memory)?)
                .map_err(|_| ResolveError::Memory)?;
            rgb.try_reserve_exact(size.saturating_sub(rgb.len()))
                .map_err(|_| ResolveError::Memory)?;
            rgb.resize(size, 0);
            for i in (0..pixels as usize).rev() {
                let g = rgb[i];
                let p = lut.pixel([g; 3])?;
                rgb[i * 3..i * 3 + 3].copy_from_slice(&p);
            }
        } else {
            for p in rgb.chunks_exact_mut(3) {
                let output = lut.pixel([p[0], p[1], p[2]])?;
                p.copy_from_slice(&output);
            }
        }
    } else if let ColorNormalization::Profile(profile) = color_normalization {
        let srgb = ColorProfile::new_srgb();
        let (source_layout, source_is_gray) = match profile.color_space {
            DataColorSpace::Rgb => (Layout::Rgb, false),
            DataColorSpace::Gray => (Layout::Gray, true),
            _ => return Err(ResolveError::Unsupported),
        };
        let transform = super::color::cms_operation(super::color::CmsSite::Construct, || {
            profile.create_transform_8bit(
                source_layout,
                &srgb,
                Layout::Rgb,
                TransformOptions::default(),
            )
        })?;
        let source = try_copy(&rgb)?;
        if source_is_gray {
            let rgb_size = usize::try_from(pixels.checked_mul(3).ok_or(ResolveError::Memory)?)
                .map_err(|_| ResolveError::Memory)?;
            rgb.try_reserve_exact(rgb_size.saturating_sub(rgb.len()))
                .map_err(|_| ResolveError::Memory)?;
            rgb.resize(rgb_size, 0);
        }
        super::color::cms_operation(super::color::CmsSite::Execute, || {
            transform.transform(&source, &mut rgb)
        })?;
    } else if matches!(color_normalization, ColorNormalization::LinearSrgb) {
        for channel in &mut rgb {
            *channel = LINEAR_TO_SRGB[usize::from(*channel)];
        }
    }

    let rgba_size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size)
        .map_err(|_| ResolveError::Memory)?;
    for pixel in rgb.as_chunks::<3>().0 {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    if let Some(orientation) = orientation {
        rgba = orient_rgba(
            &rgba,
            u32::from(info.width),
            u32::from(info.height),
            orientation,
        )?;
    }
    let premultiplied_rgba = try_copy(&rgba)?;
    Ok(DecodedRaster {
        rgba: Arc::new(rgba),
        premultiplied_rgba: Arc::new(premultiplied_rgba),
    })
}
