#[derive(Clone)]
pub(super) struct Info {
    pub(super) width: u32,
    pub(super) height: u32,
    top_down: bool,
    bits_per_pixel: u16,
    compression: u32,
    pixel_offset: usize,
    row_stride: usize,
    palette_offset: usize,
    palette_entries: usize,
    palette_stride: usize,
    masks: [u32; 4],
    use_alpha: bool,
}

pub(super) fn validate(bytes: &[u8]) -> Result<Info, ()> {
    let info = parse(bytes)?;
    validate_pixels(bytes, &info)?;
    Ok(info)
}

pub(super) fn dimensions(bytes: &[u8]) -> Result<(u32, u32), ()> {
    let info = parse(bytes)?;
    Ok((info.width, info.height))
}

pub(super) fn validation_work(bytes: &[u8]) -> Result<u64, ()> {
    let info = parse(bytes)?;
    if matches!(info.compression, 1 | 2) {
        u64::from(info.width)
            .checked_mul(u64::from(info.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(())
    } else {
        Ok(0)
    }
}

pub(super) fn decode(bytes: &[u8], info: &Info) -> Result<Vec<u8>, ()> {
    let pixel_count =
        usize::try_from(u64::from(info.width) * u64::from(info.height)).map_err(|_| ())?;
    let rgba_size = pixel_count.checked_mul(4).ok_or(())?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size).map_err(|_| ())?;
    rgba.resize(rgba_size, 0);
    if matches!(info.compression, 1 | 2) {
        let background = palette_color(bytes, info, 0)?;
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.copy_from_slice(&background);
        }
        walk_rle(bytes, info, Some(&mut rgba))?;
        return Ok(rgba);
    }
    let width = usize::try_from(info.width).map_err(|_| ())?;
    let height = usize::try_from(info.height).map_err(|_| ())?;
    for output_y in 0..height {
        let source_y = if info.top_down {
            output_y
        } else {
            height - 1 - output_y
        };
        let row_start = info
            .pixel_offset
            .checked_add(source_y.checked_mul(info.row_stride).ok_or(())?)
            .ok_or(())?;
        let row = bytes
            .get(row_start..row_start.checked_add(info.row_stride).ok_or(())?)
            .ok_or(())?;
        for x in 0..width {
            let color = match info.bits_per_pixel {
                1 | 4 | 8 => {
                    palette_color(bytes, info, packed_index(row, x, info.bits_per_pixel)?)?
                }
                16 => masked_color(read_u16(row, x.checked_mul(2).ok_or(())?)? as u32, info),
                24 => {
                    let offset = x.checked_mul(3).ok_or(())?;
                    let pixel = row
                        .get(offset..offset.checked_add(3).ok_or(())?)
                        .ok_or(())?;
                    [pixel[2], pixel[1], pixel[0], 255]
                }
                32 if info.compression == 0 && !info.use_alpha => {
                    let offset = x.checked_mul(4).ok_or(())?;
                    let pixel = row
                        .get(offset..offset.checked_add(4).ok_or(())?)
                        .ok_or(())?;
                    [pixel[2], pixel[1], pixel[0], 255]
                }
                32 => masked_color(read_u32(row, x.checked_mul(4).ok_or(())?)?, info),
                _ => return Err(()),
            };
            let destination = (output_y * width + x) * 4;
            rgba[destination..destination + 4].copy_from_slice(&color);
        }
    }
    Ok(rgba)
}

fn parse(bytes: &[u8]) -> Result<Info, ()> {
    if bytes.get(..2) != Some(b"BM") || bytes.len() < 26 {
        return Err(());
    }
    let declared_size = usize::try_from(read_u32(bytes, 2)?).map_err(|_| ())?;
    if declared_size != 0 && declared_size != bytes.len() {
        return Err(());
    }
    if read_u32(bytes, 6)? != 0 {
        return Err(());
    }
    let pixel_offset = usize::try_from(read_u32(bytes, 10)?).map_err(|_| ())?;
    let dib_size = usize::try_from(read_u32(bytes, 14)?).map_err(|_| ())?;
    let dib_end = 14_usize.checked_add(dib_size).ok_or(())?;
    if dib_end > bytes.len() {
        return Err(());
    }

    let (width, signed_height, planes, bits_per_pixel, compression, colors_used, palette_stride) =
        if dib_size == 12 {
            (
                u32::from(read_u16(bytes, 18)?),
                i32::from(read_u16(bytes, 20)?),
                read_u16(bytes, 22)?,
                read_u16(bytes, 24)?,
                0,
                0,
                3,
            )
        } else if matches!(dib_size, 40 | 52 | 56 | 108 | 124) {
            (
                u32::try_from(read_i32(bytes, 18)?).map_err(|_| ())?,
                read_i32(bytes, 22)?,
                read_u16(bytes, 26)?,
                read_u16(bytes, 28)?,
                read_u32(bytes, 30)?,
                read_u32(bytes, 46)?,
                4,
            )
        } else {
            return Err(());
        };
    if width == 0 || signed_height == 0 || signed_height == i32::MIN || planes != 1 {
        return Err(());
    }
    let top_down = signed_height < 0;
    let height = signed_height.unsigned_abs();
    let valid_encoding = matches!(
        (bits_per_pixel, compression),
        (1 | 4 | 8 | 16 | 24 | 32, 0) | (8, 1) | (4, 2) | (16 | 32, 3 | 6)
    ) && !(compression == 6 && dib_size == 52);
    if !valid_encoding || (top_down && matches!(compression, 1 | 2)) {
        return Err(());
    }

    let external_masks = dib_size == 40 && matches!(compression, 3 | 6);
    let mask_count = if external_masks {
        if compression == 6 {
            4
        } else {
            3
        }
    } else {
        0
    };
    let palette_offset = dib_end.checked_add(mask_count * 4).ok_or(())?;
    let maximum_palette_entries = if bits_per_pixel <= 8 {
        1_usize << bits_per_pixel
    } else {
        0
    };
    let palette_entries = if maximum_palette_entries == 0 {
        0
    } else if colors_used == 0 {
        maximum_palette_entries
    } else {
        usize::try_from(colors_used).map_err(|_| ())?
    };
    if palette_entries > maximum_palette_entries {
        return Err(());
    }
    let palette_end = palette_offset
        .checked_add(palette_entries.checked_mul(palette_stride).ok_or(())?)
        .ok_or(())?;
    if pixel_offset < palette_end || pixel_offset > bytes.len() {
        return Err(());
    }

    let mut masks = match bits_per_pixel {
        16 => [0x7c00, 0x03e0, 0x001f, 0],
        32 => [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0],
        _ => [0; 4],
    };
    if matches!(compression, 3 | 6) {
        let mask_offset = if external_masks { dib_end } else { 14 + 40 };
        masks[0] = read_u32(bytes, mask_offset)?;
        masks[1] = read_u32(bytes, mask_offset + 4)?;
        masks[2] = read_u32(bytes, mask_offset + 8)?;
        masks[3] = if compression == 6 || dib_size >= 56 {
            read_u32(bytes, mask_offset + 12)?
        } else {
            0
        };
    } else if bits_per_pixel == 32 && dib_size >= 56 {
        let alpha = read_u32(bytes, 14 + 52)?;
        if alpha != 0 {
            masks = [
                read_u32(bytes, 14 + 40)?,
                read_u32(bytes, 14 + 44)?,
                read_u32(bytes, 14 + 48)?,
                alpha,
            ];
        }
    }
    if matches!(bits_per_pixel, 16 | 32) {
        validate_masks(masks, bits_per_pixel)?;
    }

    let row_bits = u64::from(width)
        .checked_mul(u64::from(bits_per_pixel))
        .ok_or(())?;
    let row_stride =
        usize::try_from(row_bits.div_ceil(32).checked_mul(4).ok_or(())?).map_err(|_| ())?;
    if matches!(compression, 0 | 3 | 6) {
        let pixel_end = pixel_offset
            .checked_add(
                row_stride
                    .checked_mul(usize::try_from(height).map_err(|_| ())?)
                    .ok_or(())?,
            )
            .ok_or(())?;
        if pixel_end > bytes.len() {
            return Err(());
        }
    } else if pixel_offset == bytes.len() {
        return Err(());
    }
    Ok(Info {
        width,
        height,
        top_down,
        bits_per_pixel,
        compression,
        pixel_offset,
        row_stride,
        palette_offset,
        palette_entries,
        palette_stride,
        masks,
        use_alpha: masks[3] != 0,
    })
}

fn validate_pixels(bytes: &[u8], info: &Info) -> Result<(), ()> {
    if matches!(info.compression, 1 | 2) {
        return walk_rle(bytes, info, None);
    }
    if info.bits_per_pixel > 8 {
        return Ok(());
    }
    let width = usize::try_from(info.width).map_err(|_| ())?;
    let height = usize::try_from(info.height).map_err(|_| ())?;
    for y in 0..height {
        let row_start = info
            .pixel_offset
            .checked_add(y.checked_mul(info.row_stride).ok_or(())?)
            .ok_or(())?;
        let row = bytes
            .get(row_start..row_start + info.row_stride)
            .ok_or(())?;
        for x in 0..width {
            if packed_index(row, x, info.bits_per_pixel)? >= info.palette_entries {
                return Err(());
            }
        }
    }
    Ok(())
}

fn walk_rle(bytes: &[u8], info: &Info, mut rgba: Option<&mut [u8]>) -> Result<(), ()> {
    let width = usize::try_from(info.width).map_err(|_| ())?;
    let height = usize::try_from(info.height).map_err(|_| ())?;
    let mut x = 0_usize;
    let mut y = 0_usize;
    let mut offset = info.pixel_offset;
    let mut ended = false;
    while offset < bytes.len() && !ended {
        let count = usize::from(*bytes.get(offset).ok_or(())?);
        let value = *bytes.get(offset + 1).ok_or(())?;
        offset += 2;
        if count != 0 {
            for index in 0..count {
                let palette_index = if info.compression == 1 {
                    usize::from(value)
                } else if index % 2 == 0 {
                    usize::from(value >> 4)
                } else {
                    usize::from(value & 0x0f)
                };
                put_index(bytes, info, rgba.as_deref_mut(), x, y, palette_index)?;
                x = x.checked_add(1).ok_or(())?;
            }
            continue;
        }
        match value {
            0 => {
                if x > width {
                    return Err(());
                }
                x = 0;
                y = y.checked_add(1).ok_or(())?;
                if y > height {
                    return Err(());
                }
            }
            1 => ended = true,
            2 => {
                x = x
                    .checked_add(usize::from(*bytes.get(offset).ok_or(())?))
                    .ok_or(())?;
                y = y
                    .checked_add(usize::from(*bytes.get(offset + 1).ok_or(())?))
                    .ok_or(())?;
                offset += 2;
                if x > width || y >= height {
                    return Err(());
                }
            }
            literal => {
                let literal = usize::from(literal);
                let encoded_bytes = if info.compression == 1 {
                    literal
                } else {
                    literal.div_ceil(2)
                };
                let data = bytes
                    .get(offset..offset.checked_add(encoded_bytes).ok_or(())?)
                    .ok_or(())?;
                for index in 0..literal {
                    let palette_index = if info.compression == 1 {
                        usize::from(data[index])
                    } else if index % 2 == 0 {
                        usize::from(data[index / 2] >> 4)
                    } else {
                        usize::from(data[index / 2] & 0x0f)
                    };
                    put_index(bytes, info, rgba.as_deref_mut(), x, y, palette_index)?;
                    x = x.checked_add(1).ok_or(())?;
                }
                offset = offset.checked_add(encoded_bytes).ok_or(())?;
                if encoded_bytes % 2 != 0 {
                    offset = offset.checked_add(1).ok_or(())?;
                }
            }
        }
    }
    if !ended {
        return Err(());
    }
    Ok(())
}

fn put_index(
    bytes: &[u8],
    info: &Info,
    rgba: Option<&mut [u8]>,
    x: usize,
    source_y: usize,
    palette_index: usize,
) -> Result<(), ()> {
    let width = usize::try_from(info.width).map_err(|_| ())?;
    let height = usize::try_from(info.height).map_err(|_| ())?;
    if x >= width || source_y >= height {
        return Err(());
    }
    let color = palette_color(bytes, info, palette_index)?;
    if let Some(rgba) = rgba {
        let output_y = height - 1 - source_y;
        let destination = (output_y * width + x) * 4;
        rgba[destination..destination + 4].copy_from_slice(&color);
    }
    Ok(())
}

fn packed_index(row: &[u8], x: usize, bits_per_pixel: u16) -> Result<usize, ()> {
    Ok(match bits_per_pixel {
        1 => usize::from((row[x / 8] >> (7 - x % 8)) & 1),
        4 => {
            let byte = row[x / 2];
            usize::from(if x.is_multiple_of(2) {
                byte >> 4
            } else {
                byte & 0x0f
            })
        }
        8 => usize::from(row[x]),
        _ => return Err(()),
    })
}

fn palette_color(bytes: &[u8], info: &Info, index: usize) -> Result<[u8; 4], ()> {
    if index >= info.palette_entries {
        return Err(());
    }
    let offset = info
        .palette_offset
        .checked_add(index.checked_mul(info.palette_stride).ok_or(())?)
        .ok_or(())?;
    let color = bytes
        .get(offset..offset.checked_add(3).ok_or(())?)
        .ok_or(())?;
    Ok([color[2], color[1], color[0], 255])
}

fn masked_color(value: u32, info: &Info) -> [u8; 4] {
    [
        scale_mask(value, info.masks[0]),
        scale_mask(value, info.masks[1]),
        scale_mask(value, info.masks[2]),
        if info.use_alpha {
            scale_mask(value, info.masks[3])
        } else {
            255
        },
    ]
}

fn scale_mask(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let maximum = mask >> shift;
    let component = (value & mask) >> shift;
    ((u64::from(component) * 255 + u64::from(maximum) / 2) / u64::from(maximum)) as u8
}

fn validate_masks(masks: [u32; 4], bits_per_pixel: u16) -> Result<(), ()> {
    let allowed = if bits_per_pixel == 16 {
        0xffff
    } else {
        u32::MAX
    };
    if masks[..3].contains(&0)
        || masks.iter().any(|mask| mask & !allowed != 0)
        || masks
            .iter()
            .enumerate()
            .any(|(index, mask)| masks[index + 1..].iter().any(|other| mask & other != 0))
        || masks.iter().filter(|mask| **mask != 0).any(|mask| {
            let shifted = mask >> mask.trailing_zeros();
            shifted & shifted.wrapping_add(1) != 0
        })
    {
        return Err(());
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ()> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(2).ok_or(())?)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ()> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(4).ok_or(())?)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?,
    ))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, ()> {
    Ok(i32::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(4).ok_or(())?)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?,
    ))
}
