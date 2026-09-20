//! The PNG codec: chunk-level validation of a static or animated PNG with
//! CRC, ordering and IHDR checks, the animation work estimate, the default
//! frame decode through the `png` crate onto the declared canvas, and the
//! re-encoding a nested raster receives before it is embedded in sanitized
//! SVG.

use std::io::{self, Read};

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
    let mut bit_depth = 0;
    let mut palette_entries = 0;
    let mut saw_trns = false;
    let mut frame_controls = 0_u32;
    let mut sequence = 0_u32;
    let mut frame_dimensions = None;
    let mut frame_has_data = false;
    let mut data_kind = None;
    let mut data_stream: Option<PngDataStream> = None;
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
        // IDAT is consecutive; APNG permits ancillary chunks between fdAT
        // pieces of the same frame (PNG 3 §4.9.2). Only a new frame or image
        // boundary ends that compressed stream.
        if data_kind.is_some_and(|kind| {
            kind != chunk_type
                && (kind == *b"IDAT" || matches!(&chunk_type, b"fcTL" | b"IEND" | b"IDAT"))
        }) {
            if let Some(stream) = data_stream.take() {
                stream.finish()?;
            }
            data_kind = None;
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
                bit_depth = data[8];
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
            b"fcTL" => {
                if length != 26
                    || semantics.animation_frames == 0
                    || (frame_controls != 0 && !frame_has_data)
                    || frame_controls >= semantics.animation_frames
                {
                    return Err(ResolveError::DecodeFailed);
                }
                check_sequence(data, &mut sequence)?;
                let word = |at| -> Result<u32, ResolveError> {
                    Ok(u32::from_be_bytes(
                        data[at..at + 4]
                            .try_into()
                            .map_err(|_| ResolveError::DecodeFailed)?,
                    ))
                };
                let (w, h, x, y) = (word(4)?, word(8)?, word(12)?, word(16)?);
                let (canvas_w, canvas_h) = dimensions.ok_or(ResolveError::DecodeFailed)?;
                if w == 0
                    || h == 0
                    || x.checked_add(w).is_none_or(|v| v > canvas_w)
                    || y.checked_add(h).is_none_or(|v| v > canvas_h)
                    || data[24] > 2
                    || data[25] > 1
                    || (!saw_idat && (w, h, x, y) != (canvas_w, canvas_h, 0, 0))
                {
                    return Err(ResolveError::DecodeFailed);
                }
                frame_dimensions = Some((w, h));
                frame_controls += 1;
                frame_has_data = false;
            }
            b"fdAT" => {
                if length < 4
                    || !saw_idat
                    || frame_controls == 0
                    || (frame_has_data && data_kind != Some(*b"fdAT"))
                {
                    return Err(ResolveError::DecodeFailed);
                }
                check_sequence(data, &mut sequence)?;
                if data_stream.is_none() {
                    data_stream = Some(PngDataStream::new(
                        frame_dimensions.ok_or(ResolveError::DecodeFailed)?,
                        color_type.ok_or(ResolveError::DecodeFailed)?,
                        bit_depth,
                        bytes[28] == 1,
                    )?);
                }
                data_stream
                    .as_mut()
                    .ok_or(ResolveError::DecodeFailed)?
                    .push(&data[4..])?;
                data_kind = Some(*b"fdAT");
                frame_has_data = true;
            }
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
                    && !saw_trns
                    && matches!(color_type, Some(2 | 3 | 6))
                    && length > 0
                    && length <= 768
                    && (!matches!(color_type, Some(3)) || length / 3 <= (1_usize << bit_depth))
                    && length % 3 == 0 =>
            {
                saw_plte = true;
                palette_entries = length / 3;
            }
            b"PLTE" => return Err(ResolveError::DecodeFailed),
            b"tRNS" => {
                if saw_trns
                    || saw_idat
                    || !match color_type {
                        Some(0) => length == 2,
                        Some(2) => length == 6,
                        Some(3) => saw_plte && length > 0 && length <= palette_entries,
                        _ => false,
                    }
                {
                    return Err(ResolveError::DecodeFailed);
                }
                saw_trns = true;
                if matches!(color_type, Some(0 | 2)) {
                    let maximum = (1_u32 << bit_depth) - 1;
                    for sample in data.chunks_exact(2) {
                        if u32::from(u16::from_be_bytes([sample[0], sample[1]])) > maximum {
                            return Err(ResolveError::DecodeFailed);
                        }
                    }
                }
            }
            b"IDAT" if dimensions.is_some() && !idat_ended => {
                if data_stream.is_none() {
                    data_stream = Some(PngDataStream::new(
                        dimensions.ok_or(ResolveError::DecodeFailed)?,
                        color_type.ok_or(ResolveError::DecodeFailed)?,
                        bit_depth,
                        bytes[28] == 1,
                    )?);
                }
                data_stream
                    .as_mut()
                    .ok_or(ResolveError::DecodeFailed)?
                    .push(data)?;
                data_kind = Some(*b"IDAT");
                saw_idat = true;
                frame_has_data = true;
            }
            b"IDAT" => return Err(ResolveError::DecodeFailed),
            b"IEND" if length == 0 && saw_idat && (!matches!(color_type, Some(3)) || saw_plte) => {
                saw_iend = true;
                offset = crc_end;
                break;
            }
            b"IEND" => return Err(ResolveError::DecodeFailed),
            // Chromium149 selects SkPngRustCodec, which does not expose PNG
            // gain maps. gmAP/gdAT are ignored signaling: validate their outer
            // container/CRC, but neither parse nor decode nested gdAT payloads.
            // The selected base still undergoes cICP/ICC/legacy interpretation.
            b"gmAP" | b"gdAT" => {}
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
    if frame_controls != semantics.animation_frames || (frame_controls != 0 && !frame_has_data) {
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

/// png 0.17 closes an fdAT stream at any intervening ancillary chunk. Feed
/// its sample decoder a borrowed chunk view without the legal, non-sample
/// ancillary chunks inside a frame. The outer validator has already checked
/// every original chunk/CRC and EXIF; SVG still retains those original octets.
/// Metadata with pre-IDAT ordering requirements remains visible to the decoder.
struct PngFrameReader<'a> {
    remaining: &'a [u8],
    segment: &'a [u8],
    in_frame: bool,
}
impl<'a> PngFrameReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        let (segment, remaining) = bytes.split_at(bytes.len().min(8));
        Self {
            remaining,
            segment,
            in_frame: false,
        }
    }
}
impl Read for PngFrameReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        while self.segment.is_empty() && !self.remaining.is_empty() {
            let invalid = || io::Error::from(io::ErrorKind::InvalidData);
            let header = self.remaining.get(..8).ok_or_else(invalid)?;
            let length = u32::from_be_bytes(header[..4].try_into().map_err(|_| invalid())?);
            let end = usize::try_from(length)
                .map_err(|_| invalid())?
                .checked_add(12)
                .ok_or_else(invalid)?;
            let segment = self.remaining.get(..end).ok_or_else(invalid)?;
            self.remaining = self.remaining.get(end..).ok_or_else(invalid)?;
            let kind = &header[4..8];
            let ordered = matches!(
                kind,
                b"acTL"
                    | b"fcTL"
                    | b"cICP"
                    | b"gAMA"
                    | b"cHRM"
                    | b"iCCP"
                    | b"sRGB"
                    | b"sBIT"
                    | b"tRNS"
                    | b"bKGD"
                    | b"hIST"
                    | b"pHYs"
                    | b"sPLT"
                    | b"mDCV"
                    | b"cLLI"
            );
            if self.in_frame && kind != b"fdAT" && kind[0].is_ascii_lowercase() && !ordered {
                continue;
            }
            if kind == b"fcTL" {
                self.in_frame = false;
            }
            if kind == b"fdAT" {
                self.in_frame = true;
            }
            self.segment = segment;
        }
        let count = output.len().min(self.segment.len());
        output[..count].copy_from_slice(&self.segment[..count]);
        self.segment = &self.segment[count..];
        Ok(count)
    }
}

fn validate_png_stream(bytes: &[u8], width: u32, height: u32) -> Result<(), ResolveError> {
    let mut decoder = png::Decoder::new(PngFrameReader::new(bytes));
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
        let output = reader
            .next_frame(&mut frame)
            .map_err(|_| ResolveError::DecodeFailed)?;
        if output.color_type == png::ColorType::Indexed {
            let entries = reader
                .info()
                .palette
                .as_ref()
                .ok_or(ResolveError::DecodeFailed)?
                .len()
                / 3;
            let depth = output.bit_depth as usize;
            let row_bytes = (output.width as usize * depth).div_ceil(8);
            let stride = if reader.info().interlaced {
                reader.output_line_size(width)
            } else {
                row_bytes
            };
            for y in 0..output.height as usize {
                let start = y.checked_mul(stride).ok_or(ResolveError::DecodeFailed)?;
                let end = start
                    .checked_add(row_bytes)
                    .ok_or(ResolveError::DecodeFailed)?;
                let row = frame.get(start..end).ok_or(ResolveError::DecodeFailed)?;
                for x in 0..output.width as usize {
                    let index = (row[x * depth / 8] >> (8 - depth - x * depth % 8))
                        & ((1_u16 << depth) - 1) as u8;
                    if usize::from(index) >= entries {
                        return Err(ResolveError::DecodeFailed);
                    }
                }
            }
        }
    }
    reader.finish().map_err(|_| ResolveError::DecodeFailed)
}

fn check_sequence(data: &[u8], expected: &mut u32) -> Result<(), ResolveError> {
    let sequence = u32::from_be_bytes(
        data.get(..4)
            .ok_or(ResolveError::DecodeFailed)?
            .try_into()
            .map_err(|_| ResolveError::DecodeFailed)?,
    );
    if sequence != *expected {
        return Err(ResolveError::DecodeFailed);
    }
    *expected = expected.checked_add(1).ok_or(ResolveError::DecodeFailed)?;
    Ok(())
}

/// The decoder intentionally tolerates excess decompressed bytes. Validation
/// must instead consume exactly the declared scanlines, including the checksum.
/// Fixed inflate state and scratch fit the existing 1 MiB decoder envelope;
/// excess output stops at the first block, never an unbounded expansion.
struct PngDataStream {
    state: miniz_oxide::inflate::stream::InflateState,
    remaining: u64,
    ended: bool,
}
impl PngDataStream {
    fn new(
        (width, height): (u32, u32),
        color: u8,
        depth: u8,
        interlaced: bool,
    ) -> Result<Self, ResolveError> {
        let channels = match color {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => return Err(ResolveError::DecodeFailed),
        };
        let passes = if interlaced {
            &[
                (0, 0, 8, 8),
                (4, 0, 8, 8),
                (0, 4, 4, 8),
                (2, 0, 4, 4),
                (0, 2, 2, 4),
                (1, 0, 2, 2),
                (0, 1, 1, 2),
            ][..]
        } else {
            &[(0, 0, 1, 1)][..]
        };
        let mut remaining = 0_u64;
        for &(x, y, dx, dy) in passes {
            let w = u64::from(width.saturating_sub(x)).div_ceil(dx);
            let h = u64::from(height.saturating_sub(y)).div_ceil(dy);
            if w != 0 && h != 0 {
                let bytes = (w * channels * u64::from(depth)).div_ceil(8) + 1;
                remaining = remaining
                    .checked_add(bytes.checked_mul(h).ok_or(ResolveError::DecodeFailed)?)
                    .ok_or(ResolveError::DecodeFailed)?;
            }
        }
        Ok(Self {
            state: miniz_oxide::inflate::stream::InflateState::new(miniz_oxide::DataFormat::Zlib),
            remaining,
            ended: false,
        })
    }
    fn push(&mut self, mut data: &[u8]) -> Result<(), ResolveError> {
        use miniz_oxide::{MZFlush, MZStatus};
        let mut output = [0; 4096];
        let mut drain = false;
        while !data.is_empty() || drain {
            if self.ended {
                return Err(ResolveError::DecodeFailed);
            }
            let result = miniz_oxide::inflate::stream::inflate(
                &mut self.state,
                data,
                &mut output,
                MZFlush::None,
            );
            self.remaining = self
                .remaining
                .checked_sub(result.bytes_written as u64)
                .ok_or(ResolveError::DecodeFailed)?;
            data = data
                .get(result.bytes_consumed..)
                .ok_or(ResolveError::DecodeFailed)?;
            match result.status {
                Ok(MZStatus::StreamEnd) => self.ended = true,
                Ok(MZStatus::Ok) if result.bytes_consumed != 0 || result.bytes_written != 0 => {}
                Err(miniz_oxide::MZError::Buf) if data.is_empty() && result.bytes_written == 0 => {
                    break
                }
                _ => return Err(ResolveError::DecodeFailed),
            }
            drain = !self.ended && result.bytes_written == output.len();
        }
        Ok(())
    }
    fn finish(self) -> Result<(), ResolveError> {
        if self.ended && self.remaining == 0 {
            Ok(())
        } else {
            Err(ResolveError::DecodeFailed)
        }
    }
}

pub(super) fn decode_static_png(
    bytes: &[u8],
    pixels: u64,
    color_normalization: ColorNormalization,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    if let ColorNormalization::Hdr(hdr) = color_normalization {
        return decode_hdr_png(bytes, pixels, &hdr, orientation);
    }
    let mut decoder = png::Decoder::new(PngFrameReader::new(bytes));
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
    // png 0.17 writes Adam7 rows at the IHDR canvas stride even for narrow
    // APNG subframes. Repack those rows before interpreting a packed frame.
    if reader.info().interlaced {
        let source_stride = reader.output_line_size(reader.info().width);
        for y in 0..output.height as usize {
            let start = y
                .checked_mul(source_stride)
                .ok_or(ResolveError::DecodeFailed)?;
            let end = start
                .checked_add(output.line_size)
                .ok_or(ResolveError::DecodeFailed)?;
            let target = y
                .checked_mul(output.line_size)
                .ok_or(ResolveError::DecodeFailed)?;
            if end > decoded.len()
                || target
                    .checked_add(output.line_size)
                    .is_none_or(|n| n > decoded.len())
            {
                return Err(ResolveError::DecodeFailed);
            }
            decoded.copy_within(start..end, target);
        }
    }
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

fn decode_hdr_png(
    bytes: &[u8],
    pixels: u64,
    hdr: &super::hdr::Hdr,
    orientation: Option<u8>,
) -> Result<DecodedRaster, ResolveError> {
    let mut decoder = png::Decoder::new(PngFrameReader::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    decoder.set_limits(png::Limits {
        bytes: usize::try_from(
            pixels
                .checked_mul(8)
                .and_then(|v| v.checked_add(1_048_576))
                .ok_or(ResolveError::Memory)?,
        )
        .map_err(|_| ResolveError::Memory)?,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|_| ResolveError::DecodeFailed)?;
    if reader.info().animation_control.is_some() && reader.info().frame_control.is_none() {
        reader
            .next_frame_info()
            .map_err(|_| ResolveError::DecodeFailed)?;
    }
    let (color_type, bit_depth) = reader.output_color_type();
    let (width, height) = (reader.info().width, reader.info().height);
    let frame = reader.info().frame_control;
    let size = usize::try_from(pixels.checked_mul(4).ok_or(ResolveError::Memory)?)
        .map_err(|_| ResolveError::Memory)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(size)
        .map_err(|_| ResolveError::Memory)?;
    rgba.resize(size, 0);
    let mut premultiplied = super::try_copy(&rgba)?;
    let sample_bytes = if bit_depth == png::BitDepth::Sixteen {
        2
    } else {
        1
    };
    let channels = match color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(ResolveError::DecodeFailed),
    };
    let (x, y) = frame.map_or((0, 0), |f| (f.x_offset, f.y_offset));
    let stride = (width as usize)
        .checked_mul(4)
        .ok_or(ResolveError::Memory)?;
    let origin = (y as usize)
        .checked_mul(stride)
        .and_then(|v| v.checked_add(x as usize * 4))
        .ok_or(ResolveError::Memory)?;
    // Preserve all source bits without an image-sized 16-bit intermediate.
    // Two projected rows plus the decoder's rows fit the existing bounded
    // decoder workspace; the only full-size outputs are the usual two planes.
    let row_size = frame.map_or(width, |f| f.width) as usize * 4;
    let mut straight_row = Vec::new();
    straight_row
        .try_reserve_exact(row_size)
        .map_err(|_| ResolveError::Memory)?;
    let mut premul_row = Vec::new();
    premul_row
        .try_reserve_exact(row_size)
        .map_err(|_| ResolveError::Memory)?;
    let mut line = 0_usize;
    while let Some(row) = reader
        .next_interlaced_row()
        .map_err(|_| ResolveError::DecodeFailed)?
    {
        straight_row.clear();
        premul_row.clear();
        for pixel in row.data().chunks_exact(channels * sample_bytes) {
            let sample = |c| {
                if sample_bytes == 2 {
                    f64::from(u16::from_be_bytes([pixel[c * 2], pixel[c * 2 + 1]])) / 65535.0
                } else {
                    f64::from(pixel[c]) / 255.0
                }
            };
            let rgb = if channels <= 2 {
                [sample(0); 3]
            } else {
                [sample(0), sample(1), sample(2)]
            };
            let alpha = if channels == 2 || channels == 4 {
                sample(channels - 1)
            } else {
                1.0
            };
            let (straight, premul) = hdr.project_pixel(rgb, alpha)?;
            straight_row.extend_from_slice(&straight);
            premul_row.extend_from_slice(&premul);
        }
        match row.interlace() {
            png::InterlaceInfo::Adam7(info) => {
                png::expand_interlaced_row(&mut rgba[origin..], stride, &straight_row, info, 32);
                png::expand_interlaced_row(
                    &mut premultiplied[origin..],
                    stride,
                    &premul_row,
                    info,
                    32,
                );
            }
            png::InterlaceInfo::Null(_) => {
                let start = line
                    .checked_mul(stride)
                    .and_then(|v| v.checked_add(origin))
                    .ok_or(ResolveError::Memory)?;
                let end = start
                    .checked_add(straight_row.len())
                    .ok_or(ResolveError::Memory)?;
                rgba.get_mut(start..end)
                    .ok_or(ResolveError::DecodeFailed)?
                    .copy_from_slice(&straight_row);
                premultiplied
                    .get_mut(start..end)
                    .ok_or(ResolveError::DecodeFailed)?
                    .copy_from_slice(&premul_row);
                line = line.checked_add(1).ok_or(ResolveError::Memory)?;
            }
        }
    }
    if let Some(orientation) = orientation {
        rgba = orient_rgba(&rgba, width, height, orientation)?;
        premultiplied = orient_rgba(&premultiplied, width, height, orientation)?;
    }
    Ok(DecodedRaster {
        rgba: std::sync::Arc::new(rgba),
        premultiplied_rgba: std::sync::Arc::new(premultiplied),
    })
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
