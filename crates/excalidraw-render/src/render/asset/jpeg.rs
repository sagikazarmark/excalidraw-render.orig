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
use super::orientation::{orient_rgba, oriented_dimensions};
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
        orientation: jpeg_orientation(bytes),
    })
}

// JPEG owns APP1 precedence. The reference uses the first EXIF block before
// the first scan, even when that block has no usable orientation. Never use
// the decoder's last-APP1 metadata or inspect post-SOS entropy as header bytes.
fn jpeg_orientation(bytes: &[u8]) -> Option<u8> {
    if bytes.get(..2)? != b"\xff\xd8" {
        return None;
    }
    let mut offset = 2_usize;
    while offset < bytes.len() {
        if *bytes.get(offset)? != 0xff {
            return None;
        }
        while bytes.get(offset) == Some(&0xff) {
            offset = offset.checked_add(1)?;
        }
        let marker = *bytes.get(offset)?;
        offset = offset.checked_add(1)?;
        if matches!(marker, 0xda | 0xd9) {
            return None;
        }
        let length = usize::from(u16::from_be_bytes(
            bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
        ));
        if length < 2 {
            return None;
        }
        let end = offset.checked_add(length)?;
        let payload = bytes.get(offset.checked_add(2)?..end)?;
        if marker == 0xe1 && payload.starts_with(b"Exif\0\0") {
            return jpeg_tiff_orientation(&payload[6..]);
        }
        offset = end;
    }
    None
}

// The JPEG reference skips invalid orientation values within IFD0. This is
// intentionally separate from the qualified PNG/WebP TIFF interpretation.
fn jpeg_tiff_orientation(bytes: &[u8]) -> Option<u8> {
    let big = match bytes.get(..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let short = |at: usize| -> Option<u16> {
        let b = bytes.get(at..at.checked_add(2)?)?.try_into().ok()?;
        Some(if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    };
    let long = |at: usize| -> Option<u32> {
        let b = bytes.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    };
    if short(2)? != 42 {
        return None;
    }
    let ifd = usize::try_from(long(4)?).ok()?;
    let count = usize::from(short(ifd)?);
    let entries = ifd.checked_add(2)?;
    for i in 0..count {
        let at = entries.checked_add(i.checked_mul(12)?)?;
        bytes.get(at..at.checked_add(12)?)?;
        if short(at)? != 0x112 {
            continue;
        }
        if short(at.checked_add(2)?)? != 3 || long(at.checked_add(4)?)? != 1 {
            return None;
        }
        let value = short(at.checked_add(8)?)?;
        if (1..=8).contains(&value) {
            return Some(value as u8);
        }
    }
    None
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
    let mut primary_mpf = None;
    let mut primary_has_gain_map_metadata = false;
    while image_start < bytes.len() {
        let current_start = image_start;
        let mut saw_secondary_mpf = false;
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
                    if let Some(entries) = &primary_mpf {
                        let entries: &[std::ops::Range<usize>; 2] = entries;
                        if entries.get(usize::from(image_count)) != Some(&(current_start..offset)) {
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
                    if marker == 0xe2 && segment.starts_with(b"MPF\0") {
                        if image_count == 0 {
                            if primary_mpf.is_some() {
                                return Err(ResolveError::JpegDecodeFailed);
                            }
                            primary_mpf = Some(mpf_image_ranges(
                                &segment[4..],
                                length_end
                                    .checked_add(4)
                                    .ok_or(ResolveError::JpegDecodeFailed)?,
                                bytes.len(),
                            )?);
                        } else {
                            if saw_secondary_mpf {
                                return Err(ResolveError::JpegDecodeFailed);
                            }
                            validate_secondary_mpf(&segment[4..])?;
                            saw_secondary_mpf = true;
                        }
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
            && (image_count != 1 || primary_mpf.is_none() || !primary_has_gain_map_metadata)
        {
            return Err(ResolveError::JpegDecodeFailed);
        }
    }
    if image_count > 2 || (primary_mpf.is_some() && image_count != 2) {
        return Err(ResolveError::JpegDecodeFailed);
    }
    Ok(())
}

// The index belongs to the primary image. This narrow secondary Attribute IFD
// profile requires version 0100 and individual image number 2, with no next IFD.
// Index tags are rejected; only the primary index supplies image ranges.
fn validate_secondary_mpf(tiff: &[u8]) -> Result<(), ResolveError> {
    let invalid = ResolveError::JpegDecodeFailed;
    let big = match tiff.get(..2) {
        Some(b"MM") => true,
        Some(b"II") => false,
        _ => return Err(invalid),
    };
    let short = |at: usize| -> Result<u16, ResolveError> {
        let b = tiff
            .get(at..at.checked_add(2).ok_or(invalid)?)
            .ok_or(invalid)?;
        let b = b.try_into().map_err(|_| invalid)?;
        Ok(if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    };
    let long = |at: usize| -> Result<u32, ResolveError> {
        let b = tiff
            .get(at..at.checked_add(4).ok_or(invalid)?)
            .ok_or(invalid)?;
        let b = b.try_into().map_err(|_| invalid)?;
        Ok(if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    };
    if short(2)? != 42 {
        return Err(invalid);
    }
    let ifd = usize::try_from(long(4)?).map_err(|_| invalid)?;
    if ifd < 8 || short(ifd)? != 2 {
        return Err(invalid);
    }
    let first = ifd.checked_add(2).ok_or(invalid)?;
    let end = first.checked_add(24).ok_or(invalid)?;
    tiff.get(first..end).ok_or(invalid)?;
    let version = if big {
        u32::from_be_bytes(*b"0100")
    } else {
        u32::from_le_bytes(*b"0100")
    };
    if short(first)? != 0xb000
        || short(first + 2)? != 7
        || long(first + 4)? != 4
        || long(first + 8)? != version
        || short(first + 12)? != 0xb101
        || short(first + 14)? != 4
        || long(first + 16)? != 1
        || long(first + 20)? != 2
        || long(end)? != 0
    {
        return Err(invalid);
    }
    Ok(())
}

// Narrow revision-1 two-image MP Index admission: the three required tags plus
// optional ImageUIDList and TotalFrames, with bounded, nonoverlapping values.
// Primary Attribute IFDs and dependencies remain outside this parser's profile;
// rejecting them does not imply that they are invalid MPF metadata.
// All offsets are relative to the TIFF header, except the first image's required
// zero (the file's SOI). Fixed storage and checked slices avoid allocating from
// attacker-controlled counts.
fn mpf_image_ranges(
    tiff: &[u8],
    tiff_base: usize,
    file_size: usize,
) -> Result<[std::ops::Range<usize>; 2], ResolveError> {
    let invalid = ResolveError::JpegDecodeFailed;
    let big = match tiff.get(..2) {
        Some(b"MM") => true,
        Some(b"II") => false,
        _ => return Err(invalid),
    };
    let span = |at: usize, size: usize| {
        tiff.get(at..at.checked_add(size).ok_or(invalid)?)
            .ok_or(invalid)
    };
    let short = |at: usize| -> Result<u16, ResolveError> {
        let b = span(at, 2)?.try_into().map_err(|_| invalid)?;
        Ok(if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    };
    let long = |at: usize| -> Result<u32, ResolveError> {
        let b = span(at, 4)?.try_into().map_err(|_| invalid)?;
        Ok(if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    };
    let offset = |value: u32| usize::try_from(value).map_err(|_| invalid);
    if short(2)? != 42 {
        return Err(invalid);
    }
    let ifd = offset(long(4)?)?;
    if ifd < 8 {
        return Err(invalid);
    }
    let count = usize::from(short(ifd)?);
    let table_start = ifd.checked_add(2).ok_or(invalid)?;
    let table_end = table_start
        .checked_add(count.checked_mul(12).ok_or(invalid)?)
        .ok_or(invalid)?;
    span(table_start, count.checked_mul(12).ok_or(invalid)?)?;
    // Attribute IFDs are outside the admitted two-image index profile.
    if long(table_end)? != 0 {
        return Err(invalid);
    }
    let ifd_end = table_end.checked_add(4).ok_or(invalid)?;
    let value_range = |at: usize, size: usize| -> Result<std::ops::Range<usize>, ResolveError> {
        span(at, size)?;
        let end = at.checked_add(size).ok_or(invalid)?;
        if at < 8 || (at < ifd_end && ifd < end) {
            return Err(invalid);
        }
        Ok(at..end)
    };
    let mut version = false;
    let mut images = false;
    let mut entries = None;
    let mut image_uids = None;
    let mut previous_tag = 0;
    for index in 0..count {
        let at = table_start + index * 12;
        let tag = short(at)?;
        if tag <= previous_tag {
            return Err(invalid);
        }
        previous_tag = tag;
        match tag {
            0xb000 => {
                if version
                    || short(at + 2)? != 7
                    || long(at + 4)? != 4
                    || span(at + 8, 4)? != b"0100"
                {
                    return Err(invalid);
                }
                version = true;
            }
            0xb001 => {
                if images || short(at + 2)? != 4 || long(at + 4)? != 1 || long(at + 8)? != 2 {
                    return Err(invalid);
                }
                images = true;
            }
            0xb002 => {
                if entries.is_some() || short(at + 2)? != 7 || long(at + 4)? != 32 {
                    return Err(invalid);
                }
                entries = Some(offset(long(at + 8)?)?);
            }
            0xb003 => {
                // CIPA DC-007 section 5.2.3.4: 33 bytes per individual image.
                if short(at + 2)? != 7 || long(at + 4)? != 66 {
                    return Err(invalid);
                }
                image_uids = Some(value_range(offset(long(at + 8)?)?, 66)?);
            }
            0xb004 => {
                // Captured frames are independent of the stored image count.
                if short(at + 2)? != 4 || long(at + 4)? != 1 {
                    return Err(invalid);
                }
            }
            _ => return Err(invalid),
        }
    }
    let entries = entries.ok_or(invalid)?;
    if !version || !images {
        return Err(invalid);
    }
    let entry_range = value_range(entries, 32)?;
    if image_uids.is_some_and(|uids| uids.start < entry_range.end && entry_range.start < uids.end) {
        return Err(invalid);
    }
    let mut result = [0..0, 0..0];
    for (i, range) in result.iter_mut().enumerate() {
        let at = entries + i * 16;
        let attributes = long(at)?;
        // Primary type (optionally representative) then undefined-type JPEG;
        // no dependent images or reserved/format bits in this profile.
        if (i == 0 && attributes != 0x030000 && attributes != 0x20030000)
            || (i == 1 && attributes != 0)
            || short(at + 12)? != 0
            || short(at + 14)? != 0
        {
            return Err(invalid);
        }
        let size = offset(long(at + 4)?)?;
        let relative = offset(long(at + 8)?)?;
        let start = if i == 0 {
            if relative != 0 {
                return Err(invalid);
            }
            0
        } else {
            tiff_base.checked_add(relative).ok_or(invalid)?
        };
        let end = start.checked_add(size).ok_or(invalid)?;
        if size < 4 || end > file_size {
            return Err(invalid);
        }
        *range = start..end;
    }
    if result[0].end != result[1].start || result[1].end != file_size {
        return Err(invalid);
    }
    Ok(result)
}

pub(super) fn decode_static_jpeg(
    bytes: &[u8],
    pixels: u64,
    mut color_normalization: ColorNormalization,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    let mut decoder = jpeg_decoder(bytes);
    decoder
        .decode_headers()
        .map_err(|_| ResolveError::JpegDecodeFailed)?;
    let component_count = decoder
        .info()
        .ok_or(ResolveError::JpegDecodeFailed)?
        .components;
    // The pinned JPEG reference ignores a valid gray profile on a
    // three-component source, rather than converting RGB into profile gray.
    // An RGB profile on a gray source is valid: the decoder expands its samples.
    let gray_profile = match &color_normalization {
        ColorNormalization::Profile(profile) => profile.color_space == DataColorSpace::Gray,
        _ => false,
    };
    if component_count == 3 && gray_profile {
        if let ColorNormalization::Profile(profile) = &color_normalization {
            if profile.gray_trc.is_none()
                || profile.pcs != DataColorSpace::Xyz
                || !matches!(
                    profile.profile_class,
                    moxcms::ProfileClass::InputDevice
                        | moxcms::ProfileClass::DisplayDevice
                        | moxcms::ProfileClass::OutputDevice
                        | moxcms::ProfileClass::ColorSpace
                )
            {
                return Err(ResolveError::Unsupported);
            }
            // Ignore only a usable source profile; a recognized header alone
            // must not bypass malformed TRC/PCS validation or typed CMS errors.
            super::color::cms_operation(super::color::CmsSite::Construct, || {
                profile.create_transform_16bit(
                    Layout::Gray,
                    &ColorProfile::new_srgb(),
                    Layout::Rgb,
                    TransformOptions::default(),
                )
            })?;
        }
        color_normalization = ColorNormalization::None;
    }
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
    // Read four-component data without zune's rounded multiply-by-K. The
    // reference's untagged JPEG projection truncates that operation (#187).
    // Preserve the existing gray-profile route and normalize RGB before CMS.
    // Adobe's header color tag is provisional for three-component images;
    // zune resolves those to RGB/YCbCr only during decoder setup.
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
        if component_count == 1 {
            let transform = super::color::cms_operation(super::color::CmsSite::Construct, || {
                profile.create_transform_16bit(
                    source_layout,
                    &srgb,
                    Layout::Rgb,
                    TransformOptions::default(),
                )
            })?;
            let rgb_size = usize::try_from(pixels.checked_mul(3).ok_or(ResolveError::Memory)?)
                .map_err(|_| ResolveError::Memory)?;
            let source_channels = if source_is_gray { 1 } else { 3 };
            let pixel_count = rgb.len() / source_channels;
            rgb.try_reserve_exact(rgb_size.saturating_sub(rgb.len()))
                .map_err(|_| ResolveError::Memory)?;
            rgb.resize(rgb_size, 0);
            // Process backwards so RGB expansion cannot overwrite unread gray.
            // This also handles an RGB profile on expanded gray samples.
            // Fixed scratch avoids an image-sized source copy. Expand exact
            // 8-bit lattice samples; quantize the higher-resolution CMS once.
            let mut source = [0_u16; 768];
            let mut output = [0_u16; 768];
            let mut end = pixel_count;
            while end > 0 {
                let start = end.saturating_sub(256);
                let count = end - start;
                for (target, &value) in source[..count * source_channels]
                    .iter_mut()
                    .zip(&rgb[start * source_channels..end * source_channels])
                {
                    *target = u16::from(value) * 257;
                }
                super::color::cms_operation(super::color::CmsSite::Execute, || {
                    transform
                        .transform(&source[..count * source_channels], &mut output[..count * 3])
                })?;
                for (target, &value) in rgb[start * 3..end * 3].iter_mut().zip(&output[..count * 3])
                {
                    *target = ((u32::from(value) * 255 + 32767) / 65535) as u8;
                }
                end = start;
            }
        } else {
            let transform = super::color::cms_operation(super::color::CmsSite::Construct, || {
                profile.create_transform_8bit(
                    source_layout,
                    &srgb,
                    Layout::Rgb,
                    TransformOptions::default(),
                )
            })?;
            let source = try_copy(&rgb)?;
            super::color::cms_operation(super::color::CmsSite::Execute, || {
                transform.transform(&source, &mut rgb)
            })?;
        }
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
