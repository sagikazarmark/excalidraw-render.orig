//! Byte-level builders for the raster codec fixtures: encoders for well-formed
//! PNG, APNG, JPEG, GIF, WebP, BMP and ICO payloads plus the chunk, segment and
//! metadata surgery the malformed and tagged variants need. Everything here is
//! pure bytes in and bytes out, so the file is shared verbatim between the
//! integration binaries (as `common::codec_fixtures`) and the asset cache's own
//! in-crate suite (`src/render/asset/tests.rs`, by path); it must therefore not
//! use the renderer's API in either spelling.
//!
//! Neither compilation unit is obliged to use every builder, and the
//! integration tree's blanket allowance in `common/mod.rs` does not travel with
//! this file, so the allowance is carried here.

#![allow(dead_code)]

use std::borrow::Cow;
use std::io::Cursor;

use gif::{DisposalMethod, Encoder as GifEncoder, Frame, Repeat};
use image_webp::{ColorType as WebPColorType, WebPEncoder};

/// The RGB the JPEG fixtures are built from, chosen so a lossy round trip
/// still lands within a few units of it.
pub const JFIF_BLUE: [u8; 3] = [24, 96, 208];

/// A two-entry GIF palette: index 0 red, index 1 blue.
pub const RED_BLUE_PALETTE: [u8; 6] = [255, 0, 0, 0, 0, 255];

// --- PNG and APNG ----------------------------------------------------------

/// An eight-bit RGBA PNG of `width`x`height` over `pixels` in row-major order.
pub fn png_rgba(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    assert_eq!(
        pixels.len(),
        usize::try_from(width * height).expect("fixture size fits usize")
    );
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("fixture PNG header encodes");
        let rgba: Vec<u8> = pixels.iter().flatten().copied().collect();
        writer
            .write_image_data(&rgba)
            .expect("fixture PNG pixels encode");
    }
    bytes
}

/// A `width`x`height` PNG filled with one `color`.
pub fn solid_png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let count = usize::try_from(width * height).expect("fixture size fits usize");
    png_rgba(width, height, &vec![color; count])
}

/// A two-frame APNG one row high whose frames are `first` and `second`; the
/// default image is frame zero.
pub fn apng(first: &[[u8; 4]], second: &[[u8; 4]]) -> Vec<u8> {
    assert_eq!(first.len(), second.len());
    let width = u32::try_from(first.len()).expect("fixture width fits u32");
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .set_animated(2, 0)
            .expect("fixture animation is valid");
        let mut writer = encoder.write_header().expect("fixture APNG header encodes");
        let first: Vec<u8> = first.iter().flatten().copied().collect();
        let second: Vec<u8> = second.iter().flatten().copied().collect();
        writer
            .write_image_data(&first)
            .expect("first APNG frame encodes");
        writer
            .write_image_data(&second)
            .expect("second APNG frame encodes");
    }
    bytes
}

/// The red/blue [`apng`] rewritten so its red default image is *not* part of
/// the animation: `acTL` declares one frame, the first `fcTL` is dropped and the
/// blue `fdAT` becomes frame zero. With `corrupt_animation_frame` the blue
/// frame's compressed data is damaged behind a valid CRC.
pub fn apng_with_separate_default(corrupt_animation_frame: bool) -> Vec<u8> {
    let source = apng(&[[255, 0, 0, 255]], &[[0, 0, 255, 255]]);
    let mut output = Vec::from(&source[..8]);
    let mut offset = 8;
    let mut frame_control_count = 0;
    while offset < source.len() {
        let length = usize::try_from(u32::from_be_bytes(
            source[offset..offset + 4]
                .try_into()
                .expect("fixture chunk length"),
        ))
        .expect("fixture chunk length fits usize");
        let chunk_type: [u8; 4] = source[offset + 4..offset + 8]
            .try_into()
            .expect("fixture chunk type");
        let mut data = source[offset + 8..offset + 8 + length].to_vec();
        match &chunk_type {
            b"acTL" => data[..4].copy_from_slice(&1_u32.to_be_bytes()),
            b"fcTL" => {
                frame_control_count += 1;
                if frame_control_count == 1 {
                    offset += 12 + length;
                    continue;
                }
                data[..4].copy_from_slice(&0_u32.to_be_bytes());
            }
            b"fdAT" => {
                data[..4].copy_from_slice(&1_u32.to_be_bytes());
                if corrupt_animation_frame {
                    data[4] ^= 0xff;
                }
            }
            _ => {}
        }
        append_png_chunk(&mut output, chunk_type, &data);
        offset += 12 + length;
    }
    output
}

/// An APNG `fcTL` payload for frame `sequence` covering `width`x`height` at
/// the origin with a 1/30 s delay.
pub fn apng_frame_control(sequence: u32, width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity(26);
    data.extend_from_slice(&sequence.to_be_bytes());
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&0_u32.to_be_bytes());
    data.extend_from_slice(&0_u32.to_be_bytes());
    data.extend_from_slice(&1_u16.to_be_bytes());
    data.extend_from_slice(&30_u16.to_be_bytes());
    data.extend_from_slice(&[0, 0]);
    data
}

/// An ISO 21496-1 gain map metadata payload (`gmAP`) with identity parameters.
pub fn iso_gain_map_metadata() -> Vec<u8> {
    let mut metadata = Vec::new();
    metadata.extend_from_slice(&0_u16.to_be_bytes());
    metadata.extend_from_slice(&0_u16.to_be_bytes());
    metadata.push(0x40);
    for (numerator, denominator) in [
        (0_i32, 1_u32),
        (1, 1),
        (0, 1),
        (1, 1),
        (1, 1),
        (0, 1),
        (0, 1),
    ] {
        metadata.extend_from_slice(&numerator.to_be_bytes());
        metadata.extend_from_slice(&denominator.to_be_bytes());
    }
    metadata
}

/// Appends one PNG chunk of `chunk_type` over `data` with its CRC.
pub fn append_png_chunk(bytes: &mut Vec<u8>, chunk_type: [u8; 4], data: &[u8]) {
    bytes.extend_from_slice(
        &u32::try_from(data.len())
            .expect("fixture chunk length fits u32")
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&chunk_type);
    bytes.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(&chunk_type);
    crc.update(data);
    bytes.extend_from_slice(&crc.finalize().to_be_bytes());
}

/// `source` with `chunks` inserted, in order, immediately before its first
/// `IDAT`.
pub fn png_with_chunks_before_idat(source: &[u8], chunks: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let idat = png_chunk_offset(source, *b"IDAT");
    let mut png = Vec::from(&source[..idat]);
    for (chunk_type, data) in chunks {
        append_png_chunk(&mut png, *chunk_type, data);
    }
    png.extend_from_slice(&source[idat..]);
    png
}

/// `source` with one chunk inserted immediately before `IEND`.
pub fn png_with_chunk_before_iend(source: &[u8], chunk_type: [u8; 4], data: &[u8]) -> Vec<u8> {
    let iend = png_chunk_offset(source, *b"IEND");
    let mut png = Vec::from(&source[..iend]);
    append_png_chunk(&mut png, chunk_type, data);
    png.extend_from_slice(&source[iend..]);
    png
}

/// `source` with `chunk` inserted between its first `IDAT` and a second copy of
/// the image data, so the chunk splits the `IDAT` run.
pub fn png_with_chunk_splitting_idat(source: &[u8], chunk_type: [u8; 4], data: &[u8]) -> Vec<u8> {
    let idat = png_chunk_offset(source, *b"IDAT");
    let idat_length = usize::try_from(u32::from_be_bytes(
        source[idat..idat + 4]
            .try_into()
            .expect("fixture IDAT length"),
    ))
    .expect("fixture IDAT length fits usize");
    let idat_end = idat + 12 + idat_length;
    let mut split = Vec::from(&source[..idat_end]);
    append_png_chunk(&mut split, chunk_type, data);
    split.extend_from_slice(&source[idat..]);
    split
}

/// `source` with a `tEXt` chunk inserted between the signature and `IHDR`.
pub fn png_with_chunk_before_ihdr(source: &[u8]) -> Vec<u8> {
    let mut malformed = Vec::from(&source[..8]);
    append_png_chunk(&mut malformed, *b"tEXt", b"key\0value");
    malformed.extend_from_slice(&source[8..]);
    malformed
}

/// Applies `mutate` to the data of the first `chunk_type` chunk in place and
/// re-seals the chunk with a valid CRC.
pub fn mutate_first_png_chunk(png: &mut [u8], chunk_type: [u8; 4], mutate: impl FnOnce(&mut [u8])) {
    let offset = png_chunk_offset(png, chunk_type);
    let length = u32::from_be_bytes(png[offset..offset + 4].try_into().expect("chunk length"));
    let length = usize::try_from(length).expect("chunk length fits usize");
    let data_start = offset + 8;
    let data_end = data_start + length;
    mutate(&mut png[data_start..data_end]);
    let mut crc = crc32fast::Hasher::new();
    crc.update(&chunk_type);
    crc.update(&png[data_start..data_end]);
    png[data_end..data_end + 4].copy_from_slice(&crc.finalize().to_be_bytes());
}

/// The byte offset of the first `wanted` chunk's length field.
pub fn png_chunk_offset(png: &[u8], wanted: [u8; 4]) -> usize {
    let mut offset = 8;
    loop {
        let length = u32::from_be_bytes(png[offset..offset + 4].try_into().expect("chunk length"));
        if png[offset + 4..offset + 8] == wanted {
            return offset;
        }
        offset += 12 + usize::try_from(length).expect("chunk length fits usize");
    }
}

// --- EXIF and ICC metadata -------------------------------------------------

/// A little-endian TIFF structure with one IFD entry: orientation tag 0x0112.
/// This is the payload of a PNG `eXIf` chunk.
pub fn exif_orientation_tiff(orientation: u16) -> Vec<u8> {
    let mut exif = Vec::new();
    exif.extend_from_slice(b"II");
    exif.extend_from_slice(&42_u16.to_le_bytes());
    exif.extend_from_slice(&8_u32.to_le_bytes());
    exif.extend_from_slice(&1_u16.to_le_bytes());
    exif.extend_from_slice(&0x0112_u16.to_le_bytes());
    exif.extend_from_slice(&3_u16.to_le_bytes());
    exif.extend_from_slice(&1_u32.to_le_bytes());
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&0_u16.to_le_bytes());
    exif.extend_from_slice(&0_u32.to_le_bytes());
    exif
}

/// [`exif_orientation_tiff`] behind the `Exif\0\0` header a JPEG APP1 segment
/// and a WebP `EXIF` chunk carry.
pub fn exif_orientation_with_header(orientation: u16) -> Vec<u8> {
    let mut exif = b"Exif\0\0".to_vec();
    exif.extend_from_slice(&exif_orientation_tiff(orientation));
    exif
}

/// A minimal ICC v2 display profile with sRGB primaries and a linear (gamma
/// 1.0) tone curve, so a tagged mid-grey of 128 normalizes to sRGB 188.
pub fn linear_rgb_icc_profile() -> Vec<u8> {
    const PROFILE_SIZE: usize = 312;
    const DATA_OFFSET: u32 = 216;
    const XYZ_SIZE: u32 = 20;
    const CURVE_OFFSET: u32 = DATA_OFFSET + 4 * XYZ_SIZE;
    const TAGS: [(&[u8; 4], u32, u32); 7] = [
        (b"rXYZ", DATA_OFFSET, XYZ_SIZE),
        (b"gXYZ", DATA_OFFSET + XYZ_SIZE, XYZ_SIZE),
        (b"bXYZ", DATA_OFFSET + 2 * XYZ_SIZE, XYZ_SIZE),
        (b"wtpt", DATA_OFFSET + 3 * XYZ_SIZE, XYZ_SIZE),
        (b"rTRC", CURVE_OFFSET, 14),
        (b"gTRC", CURVE_OFFSET, 14),
        (b"bTRC", CURVE_OFFSET, 14),
    ];
    let mut profile = vec![0; 128];
    profile[0..4].copy_from_slice(&(PROFILE_SIZE as u32).to_be_bytes());
    profile[8..12].copy_from_slice(b"\x02\x10\0\0");
    profile[12..16].copy_from_slice(b"mntr");
    profile[16..20].copy_from_slice(b"RGB ");
    profile[20..24].copy_from_slice(b"XYZ ");
    profile[36..40].copy_from_slice(b"acsp");
    profile[68..80].copy_from_slice(&[0, 0, 0xf6, 0xd6, 0, 1, 0, 0, 0, 0, 0xd3, 0x2d]);
    profile.extend_from_slice(&(TAGS.len() as u32).to_be_bytes());
    for (signature, offset, size) in TAGS {
        profile.extend_from_slice(signature);
        profile.extend_from_slice(&offset.to_be_bytes());
        profile.extend_from_slice(&size.to_be_bytes());
    }
    append_xyz(&mut profile, [0x0000_6fa3, 0x0000_38f6, 0x0000_0391]);
    append_xyz(&mut profile, [0x0000_6294, 0x0000_b785, 0x0000_18dc]);
    append_xyz(&mut profile, [0x0000_24a1, 0x0000_0f85, 0x0000_b6d4]);
    append_xyz(&mut profile, [0x0000_f6d6, 0x0001_0000, 0x0000_d32d]);
    profile.extend_from_slice(b"curv\0\0\0\0\0\0\0\x01\x01\0\0\0");
    assert_eq!(profile.len(), PROFILE_SIZE);
    profile
}

fn append_xyz(profile: &mut Vec<u8>, values: [u32; 3]) {
    profile.extend_from_slice(b"XYZ \0\0\0\0");
    for value in values {
        profile.extend_from_slice(&value.to_be_bytes());
    }
}

// --- JPEG ------------------------------------------------------------------

/// A quality-100 baseline RGB JPEG of `width`x`height` over `pixels`.
pub fn jpeg_rgb(width: u16, height: u16, pixels: &[[u8; 3]]) -> Vec<u8> {
    assert_eq!(pixels.len(), usize::from(width) * usize::from(height));
    let mut bytes = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut bytes, 100);
    let rgb: Vec<u8> = pixels.iter().flatten().copied().collect();
    encoder
        .encode(&rgb, width, height, jpeg_encoder::ColorType::Rgb)
        .expect("fixture JPEG encodes");
    bytes
}

/// A one-pixel progressive grayscale JPEG of luma `value`.
pub fn progressive_gray_jpeg(value: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut bytes, 100);
    encoder.set_progressive(true);
    encoder
        .encode(&[value], 1, 1, jpeg_encoder::ColorType::Luma)
        .expect("progressive grayscale JPEG encodes");
    bytes
}

/// A one-pixel CMYK JPEG of the given `cmyk` sample.
pub fn cmyk_jpeg(cmyk: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    jpeg_encoder::Encoder::new(&mut bytes, 100)
        .encode(&cmyk, 1, 1, jpeg_encoder::ColorType::Cmyk)
        .expect("CMYK JPEG encodes");
    bytes
}

/// `source` with an APP1 EXIF segment carrying `orientation`.
pub fn jpeg_with_app1_exif(source: &[u8], orientation: u16) -> Vec<u8> {
    jpeg_with_app_segment(source, 0xe1, &exif_orientation_with_header(orientation))
}

/// `source` with a single-part APP2 `ICC_PROFILE` segment carrying `profile`.
pub fn jpeg_with_icc_profile(source: &[u8], profile: &[u8]) -> Vec<u8> {
    let mut segment = Vec::new();
    segment.extend_from_slice(b"ICC_PROFILE\0\x01\x01");
    segment.extend_from_slice(profile);
    jpeg_with_app_segment(source, 0xe2, &segment)
}

/// `source` with an `APPn` segment (`marker` is the second marker byte, e.g.
/// `0xe1`) over `segment` inserted right after SOI.
pub fn jpeg_with_app_segment(source: &[u8], marker: u8, segment: &[u8]) -> Vec<u8> {
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&source[..2]);
    jpeg.extend_from_slice(&[0xff, marker]);
    jpeg.extend_from_slice(
        &u16::try_from(segment.len() + 2)
            .expect("APP segment length fits u16")
            .to_be_bytes(),
    );
    jpeg.extend_from_slice(segment);
    jpeg.extend_from_slice(&source[2..]);
    jpeg
}

/// `source` without its JFIF APP0 segment: a raw JPEG stream.
pub fn jpeg_without_app0(source: &[u8]) -> Vec<u8> {
    assert_eq!(&source[..4], b"\xff\xd8\xff\xe0");
    let length = usize::from(u16::from_be_bytes([source[4], source[5]]));
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&source[..2]);
    jpeg.extend_from_slice(&source[4 + length..]);
    jpeg
}

// --- GIF -------------------------------------------------------------------

/// A GIF frame over `indices` into its own local `palette` (RGB triples), kept
/// after display, optionally naming a `transparent` index.
pub fn gif_frame<'a>(
    indices: &'a [u8],
    width: u16,
    height: u16,
    palette: &'a [u8],
    transparent: Option<u8>,
) -> Frame<'a> {
    Frame {
        width,
        height,
        palette: Some(palette.to_vec()),
        transparent,
        dispose: DisposalMethod::Keep,
        buffer: Cow::Borrowed(indices),
        ..Frame::default()
    }
}

/// A GIF frame over `indices` into the global palette.
pub fn gif_global_palette_frame(indices: &[u8], width: u16, height: u16) -> Frame<'static> {
    Frame {
        width,
        height,
        buffer: Cow::Owned(indices.to_vec()),
        ..Frame::default()
    }
}

/// A GIF89a whose logical screen is the first frame's size, without a global
/// palette or loop metadata.
pub fn gif(frames: &[Frame<'_>]) -> Vec<u8> {
    gif_with_screen(frames[0].width, frames[0].height, &[], None, frames)
}

/// A GIF89a with a `width`x`height` logical screen, a `global_palette` of RGB
/// triples (none when empty) and optional NETSCAPE loop metadata.
pub fn gif_with_screen(
    width: u16,
    height: u16,
    global_palette: &[u8],
    repeat: Option<Repeat>,
    frames: &[Frame<'_>],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder =
            GifEncoder::new(&mut bytes, width, height, global_palette).expect("GIF header encodes");
        if let Some(repeat) = repeat {
            encoder.set_repeat(repeat).expect("GIF repeat encodes");
        }
        for frame in frames {
            encoder.write_frame(frame).expect("GIF frame encodes");
        }
    }
    bytes
}

/// The offset of the first graphic control extension (`21 F9 04`) in `gif`.
pub fn gif_graphic_control_offset(gif: &[u8]) -> usize {
    gif.windows(3)
        .position(|window| window == [0x21, 0xf9, 0x04])
        .expect("fixture has a graphic control extension")
}

/// The offset of the first application extension introducer (`21 FF`) in
/// `gif`.
pub fn gif_application_extension_offset(gif: &[u8]) -> usize {
    gif.windows(2)
        .position(|window| window == [0x21, 0xff])
        .expect("fixture has an application extension")
}

/// The offset of the first image descriptor separator (`2C`) in `gif`.
pub fn gif_image_descriptor_offset(gif: &[u8]) -> usize {
    gif.iter()
        .position(|byte| *byte == 0x2c)
        .expect("fixture has an image descriptor")
}

// --- WebP ------------------------------------------------------------------

/// A lossless RGBA WebP of `width`x`height` over `pixels`.
pub fn webp_rgba(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    webp_with_metadata(width, height, pixels, None, None)
}

/// [`webp_rgba`] in a VP8X container with optional `ICCP` and `EXIF` chunks.
pub fn webp_with_metadata(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
    icc: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
) -> Vec<u8> {
    let rgba = pixels.iter().flatten().copied().collect::<Vec<_>>();
    let mut bytes = Vec::new();
    let mut encoder = WebPEncoder::new(&mut bytes);
    if let Some(icc) = icc {
        encoder.set_icc_profile(icc);
    }
    if let Some(exif) = exif {
        encoder.set_exif_metadata(exif);
    }
    encoder
        .encode(&rgba, width, height, WebPColorType::Rgba8)
        .expect("WebP fixture encodes");
    bytes
}

/// A 2x2 lossy (VP8) WebP of solid red, held as literal bytes because the
/// workspace has no VP8 encoder.
pub fn lossy_red_webp() -> Vec<u8> {
    vec![
        0x52, 0x49, 0x46, 0x46, 0x3c, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x20, 0x30, 0x00, 0x00, 0x00, 0xd0, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x02, 0x00, 0x02, 0x00,
        0x02, 0x00, 0x34, 0x25, 0xa0, 0x02, 0x74, 0xba, 0x01, 0xf8, 0x00, 0x03, 0xb0, 0x00, 0xfe,
        0xf0, 0xc4, 0x0b, 0xff, 0x20, 0xb9, 0x61, 0x75, 0xc8, 0xd7, 0xff, 0x20, 0x3f, 0xe4, 0x07,
        0xfc, 0x80, 0xff, 0xf8, 0xf2, 0x00, 0x00, 0x00,
    ]
}

/// An animated WebP with a `width`x`height` canvas whose `ANMF` frames place
/// each `(x, y, webp)` still image (its `VP8`/`VP8L` chunk) at that offset. The
/// VP8X alpha flag is set when any frame carries alpha.
pub fn animated_webp(width: u32, height: u32, frames: &[(u32, u32, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    let has_alpha = frames.iter().any(|(_, _, frame)| {
        image_webp::WebPDecoder::new(Cursor::new(frame))
            .expect("frame WebP decodes")
            .has_alpha()
    });
    append_webp_chunk(
        &mut body,
        *b"VP8X",
        &[
            vec![0x02 | if has_alpha { 0x10 } else { 0 }, 0, 0, 0],
            le_u24(width - 1).to_vec(),
            le_u24(height - 1).to_vec(),
        ]
        .concat(),
    );
    append_webp_chunk(&mut body, *b"ANIM", &[0, 0, 0, 0, 0, 0]);
    for (x, y, frame) in frames {
        let frame_chunk = webp_image_chunk(frame);
        let frame_decoder =
            image_webp::WebPDecoder::new(Cursor::new(frame)).expect("frame WebP decodes");
        let (frame_width, frame_height) = frame_decoder.dimensions();
        let mut payload = Vec::new();
        payload.extend_from_slice(&le_u24(x / 2));
        payload.extend_from_slice(&le_u24(y / 2));
        payload.extend_from_slice(&le_u24(frame_width - 1));
        payload.extend_from_slice(&le_u24(frame_height - 1));
        payload.extend_from_slice(&le_u24(100));
        payload.push(0x02);
        payload.extend_from_slice(frame_chunk);
        append_webp_chunk(&mut body, *b"ANMF", &payload);
    }
    let mut webp = b"RIFF".to_vec();
    webp.extend_from_slice(
        &u32::try_from(body.len() + 4)
            .expect("WebP body fits u32")
            .to_le_bytes(),
    );
    webp.extend_from_slice(b"WEBP");
    webp.extend_from_slice(&body);
    webp
}

/// The offset of the second `ANMF` frame's payload in an [`animated_webp`].
pub fn webp_second_frame_payload_offset(webp: &[u8]) -> usize {
    webp.windows(4)
        .enumerate()
        .filter(|(_, window)| *window == b"ANMF")
        .nth(1)
        .map(|(offset, _)| offset + 8)
        .expect("fixture has a second animation frame")
}

fn webp_image_chunk(webp: &[u8]) -> &[u8] {
    let mut offset = 12;
    while offset + 8 <= webp.len() {
        let length = u32::from_le_bytes(
            webp[offset + 4..offset + 8]
                .try_into()
                .expect("chunk length has four bytes"),
        ) as usize;
        let end = offset + 8 + length + (length & 1);
        if matches!(&webp[offset..offset + 4], b"VP8 " | b"VP8L") {
            return &webp[offset..end];
        }
        offset = end;
    }
    panic!("fixture has a WebP image chunk")
}

fn append_webp_chunk(output: &mut Vec<u8>, kind: [u8; 4], payload: &[u8]) {
    output.extend_from_slice(&kind);
    output.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("chunk length fits u32")
            .to_le_bytes(),
    );
    output.extend_from_slice(payload);
    if !payload.len().is_multiple_of(2) {
        output.push(0);
    }
}

fn le_u24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

// --- BMP -------------------------------------------------------------------

/// A bottom-up 24-bit `BI_RGB` BMP of `width`x`height` over `pixels` given
/// top-down in row-major order.
pub fn bmp24(width: u32, height: u32, pixels: &[[u8; 3]]) -> Vec<u8> {
    let row_bytes = width * 3;
    let stride = row_bytes.div_ceil(4) * 4;
    let pixel_bytes = stride * height;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(54 + pixel_bytes).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&54_u32.to_le_bytes());
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&(width as i32).to_le_bytes());
    bytes.extend_from_slice(&(height as i32).to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&24_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&pixel_bytes.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    for y in (0..height as usize).rev() {
        for [red, green, blue] in &pixels[y * width as usize..(y + 1) * width as usize] {
            bytes.extend_from_slice(&[*blue, *green, *red]);
        }
        bytes.resize(bytes.len() + (stride - row_bytes) as usize, 0);
    }
    bytes
}

/// A one-pixel 32-bit BMP: legacy `BI_RGB` with a reserved fourth byte, or a
/// `BI_BITFIELDS` V3 header whose fourth mask makes that byte alpha.
pub fn bmp32(pixel: [u8; 4], alpha_bitfields: bool) -> Vec<u8> {
    let dib_size = if alpha_bitfields { 56_u32 } else { 40 };
    let pixel_offset = 14 + dib_size;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(pixel_offset + 4).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&pixel_offset.to_le_bytes());
    bytes.extend_from_slice(&dib_size.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&(if alpha_bitfields { 3_u32 } else { 0 }).to_le_bytes());
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    if alpha_bitfields {
        bytes.extend_from_slice(&0x00ff_0000_u32.to_le_bytes());
        bytes.extend_from_slice(&0x0000_ff00_u32.to_le_bytes());
        bytes.extend_from_slice(&0x0000_00ff_u32.to_le_bytes());
        bytes.extend_from_slice(&0xff00_0000_u32.to_le_bytes());
    }
    bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    bytes
}

/// The alpha-bitfields [`bmp32`] relabelled `BI_ALPHABITFIELDS` (compression
/// 6) with its DIB header shortened to 52 bytes, so the alpha mask the
/// compression promises has no storage.
pub fn bmp32_without_alpha_mask_storage() -> Vec<u8> {
    let mut bmp = bmp32([24, 96, 208, 128], true);
    bmp[30..34].copy_from_slice(&6_u32.to_le_bytes());
    bmp.drain(66..70);
    let length = bmp.len() as u32;
    bmp[2..6].copy_from_slice(&length.to_le_bytes());
    bmp[10..14].copy_from_slice(&66_u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&52_u32.to_le_bytes());
    bmp
}

/// A 2x1 eight-bit `BI_RLE8` BMP with a two-entry palette (blue, red) whose
/// encoded run paints pixel one red and reaches pixel zero only through a
/// delta, leaving it at palette index zero.
pub fn bmp_rle8() -> Vec<u8> {
    let encoded = [0, 2, 1, 0, 1, 1, 0, 1];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(62_u32 + encoded.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&62_u32.to_le_bytes());
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
    bytes.extend_from_slice(&encoded);
    bytes
}

// --- ICO -------------------------------------------------------------------

/// An ICO whose directory lists each `(width, height, depth, image)` in order,
/// with the images stored back to back after the directory.
pub fn ico(images: &[(u16, u16, u16, Vec<u8>)]) -> Vec<u8> {
    let directory_size = 6 + images.len() * 16;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = directory_size;
    for (width, height, depth, image) in images {
        bytes.push(if *width == 256 { 0 } else { *width as u8 });
        bytes.push(if *height == 256 { 0 } else { *height as u8 });
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&depth.to_le_bytes());
        bytes.extend_from_slice(&(image.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += image.len();
    }
    for (_, _, _, image) in images {
        bytes.extend_from_slice(image);
    }
    bytes
}

/// A one-pixel 32-bit ICO DIB (height doubled for the AND mask) whose colour
/// row is `pixel` and whose mask bit is set when `transparent_mask`.
pub fn dib32(pixel: [u8; 4], transparent_mask: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    bytes.extend_from_slice(&[if transparent_mask { 0x80 } else { 0 }, 0, 0, 0]);
    bytes
}

/// A one-pixel `BI_BITFIELDS` ICO DIB whose red and green masks overlap.
pub fn dib_with_overlapping_masks() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 16]);
    bytes.extend_from_slice(&0x00ff_0000_u32.to_le_bytes());
    bytes.extend_from_slice(&0x00ff_0000_u32.to_le_bytes());
    bytes.extend_from_slice(&0x0000_00ff_u32.to_le_bytes());
    bytes.extend_from_slice(&[208, 96, 24, 255]);
    bytes.extend_from_slice(&[0; 4]);
    bytes
}
