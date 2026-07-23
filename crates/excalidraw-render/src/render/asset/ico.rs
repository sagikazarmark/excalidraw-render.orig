#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) depth: u16,
    pub(super) offset: usize,
    pub(super) length: usize,
    pub(super) index: usize,
}

#[derive(Clone)]
pub(super) struct DibInfo {
    pub(super) width: u32,
    pub(super) height: u32,
    bits_per_pixel: u16,
    pixel_offset: usize,
    row_stride: usize,
    mask_offset: usize,
    mask_stride: usize,
    palette_offset: usize,
    palette_entries: usize,
    masks: [u32; 4],
}

pub(super) fn entries(bytes: &[u8]) -> Result<Vec<Entry>, ()> {
    let count = directory_count(bytes)?;
    let mut entries = Vec::new();
    entries.try_reserve_exact(count).map_err(|_| ())?;
    for index in 0..count {
        entries.push(entry_at(bytes, count, index)?);
    }
    entries.sort_unstable_by_key(|entry| (entry.offset, entry.index));
    for pair in entries.windows(2) {
        if pair[0].offset + pair[0].length > pair[1].offset {
            return Err(());
        }
    }
    Ok(entries)
}

pub(super) fn selected_entry(bytes: &[u8]) -> Result<Entry, ()> {
    let count = directory_count(bytes)?;
    let mut selected = entry_at(bytes, count, 0)?;
    for index in 1..count {
        let entry = entry_at(bytes, count, index)?;
        if (
            u64::from(entry.width) * u64::from(entry.height),
            entry.depth,
        ) > (
            u64::from(selected.width) * u64::from(selected.height),
            selected.depth,
        ) {
            selected = entry;
        }
    }
    Ok(selected)
}

pub(super) fn validation_work(bytes: &[u8]) -> Result<u64, ()> {
    let count = u64::try_from(directory_count(bytes)?).map_err(|_| ())?;
    let comparisons_per_entry = u64::from(usize::BITS - count.max(1).leading_zeros());
    count.checked_mul(comparisons_per_entry).ok_or(())
}

pub(super) fn directory_workspace(bytes: &[u8]) -> Result<u64, ()> {
    u64::try_from(directory_count(bytes)?)
        .map_err(|_| ())?
        .checked_mul(32)
        .ok_or(())
}

pub(super) fn dib_dimensions(bytes: &[u8], expected: Entry) -> Result<(u32, u32), ()> {
    let header_size = usize::try_from(read_u32(bytes, 0)?).map_err(|_| ())?;
    if !matches!(header_size, 40 | 52 | 56 | 108 | 124) || header_size > bytes.len() {
        return Err(());
    }
    let width = u32::try_from(read_i32(bytes, 4)?).map_err(|_| ())?;
    let combined_height = u32::try_from(read_i32(bytes, 8)?).map_err(|_| ())?;
    if combined_height == 0 || combined_height % 2 != 0 {
        return Err(());
    }
    let height = combined_height / 2;
    if (width, height) != (expected.width, expected.height) {
        return Err(());
    }
    Ok((width, height))
}

fn directory_count(bytes: &[u8]) -> Result<usize, ()> {
    if read_u16(bytes, 0)? != 0 || read_u16(bytes, 2)? != 1 {
        return Err(());
    }
    let count = usize::from(read_u16(bytes, 4)?);
    let directory_end = 6_usize
        .checked_add(count.checked_mul(16).ok_or(())?)
        .ok_or(())?;
    if count == 0 || directory_end > bytes.len() {
        return Err(());
    }
    Ok(count)
}

fn entry_at(bytes: &[u8], count: usize, index: usize) -> Result<Entry, ()> {
    let directory_end = 6 + count * 16;
    let start = 6 + index.checked_mul(16).ok_or(())?;
    if bytes[start + 3] != 0 || !matches!(read_u16(bytes, start + 4)?, 0 | 1) {
        return Err(());
    }
    let width = if bytes[start] == 0 {
        256
    } else {
        u32::from(bytes[start])
    };
    let height = if bytes[start + 1] == 0 {
        256
    } else {
        u32::from(bytes[start + 1])
    };
    let depth = read_u16(bytes, start + 6)?;
    let length = usize::try_from(read_u32(bytes, start + 8)?).map_err(|_| ())?;
    let offset = usize::try_from(read_u32(bytes, start + 12)?).map_err(|_| ())?;
    let end = offset.checked_add(length).ok_or(())?;
    if length == 0 || offset < directory_end || end > bytes.len() {
        return Err(());
    }
    Ok(Entry {
        width,
        height,
        depth,
        offset,
        length,
        index,
    })
}

pub(super) fn validate_dib(bytes: &[u8], expected: Entry) -> Result<DibInfo, ()> {
    let header_size = usize::try_from(read_u32(bytes, 0)?).map_err(|_| ())?;
    if !matches!(header_size, 40 | 52 | 56 | 108 | 124) || header_size > bytes.len() {
        return Err(());
    }
    let width = u32::try_from(read_i32(bytes, 4)?).map_err(|_| ())?;
    let combined_height = u32::try_from(read_i32(bytes, 8)?).map_err(|_| ())?;
    if width == 0 || combined_height == 0 || combined_height % 2 != 0 || read_u16(bytes, 12)? != 1 {
        return Err(());
    }
    let height = combined_height / 2;
    if width != expected.width || height != expected.height {
        return Err(());
    }
    let bits_per_pixel = read_u16(bytes, 14)?;
    let compression = read_u32(bytes, 16)?;
    if !matches!(
        (bits_per_pixel, compression),
        (1 | 4 | 8 | 24 | 32, 0) | (16 | 32, 3 | 6)
    ) || (compression == 6 && header_size == 52)
        || (expected.depth != 0 && expected.depth != bits_per_pixel)
    {
        return Err(());
    }
    let external_masks = header_size == 40 && matches!(compression, 3 | 6);
    let mask_count = if external_masks {
        if compression == 6 {
            4
        } else {
            3
        }
    } else {
        0
    };
    let palette_offset = header_size.checked_add(mask_count * 4).ok_or(())?;
    let maximum_palette = if bits_per_pixel <= 8 {
        1_usize << bits_per_pixel
    } else {
        0
    };
    let colors_used = usize::try_from(read_u32(bytes, 32)?).map_err(|_| ())?;
    let palette_entries = if maximum_palette == 0 {
        0
    } else if colors_used == 0 {
        maximum_palette
    } else {
        colors_used
    };
    if palette_entries > maximum_palette {
        return Err(());
    }
    let pixel_offset = palette_offset
        .checked_add(palette_entries.checked_mul(4).ok_or(())?)
        .ok_or(())?;
    let row_stride = stride(width, bits_per_pixel)?;
    let xor_bytes = row_stride
        .checked_mul(usize::try_from(height).map_err(|_| ())?)
        .ok_or(())?;
    let mask_offset = pixel_offset.checked_add(xor_bytes).ok_or(())?;
    let mask_stride = stride(width, 1)?;
    let end = mask_offset
        .checked_add(
            mask_stride
                .checked_mul(usize::try_from(height).map_err(|_| ())?)
                .ok_or(())?,
        )
        .ok_or(())?;
    if end > bytes.len() {
        return Err(());
    }
    let mut masks = match bits_per_pixel {
        16 => [0x7c00, 0x03e0, 0x001f, 0],
        32 => [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000],
        _ => [0; 4],
    };
    if matches!(compression, 3 | 6) {
        let offset = if external_masks { header_size } else { 40 };
        masks[0] = read_u32(bytes, offset)?;
        masks[1] = read_u32(bytes, offset + 4)?;
        masks[2] = read_u32(bytes, offset + 8)?;
        masks[3] = if compression == 6 || header_size >= 56 {
            read_u32(bytes, offset + 12)?
        } else {
            0
        };
    }
    if matches!(bits_per_pixel, 16 | 32) {
        validate_masks(masks, bits_per_pixel)?;
    }
    let info = DibInfo {
        width,
        height,
        bits_per_pixel,
        pixel_offset,
        row_stride,
        mask_offset,
        mask_stride,
        palette_offset,
        palette_entries,
        masks,
    };
    validate_dib_pixels(bytes, &info)?;
    Ok(info)
}

fn validate_dib_pixels(bytes: &[u8], info: &DibInfo) -> Result<(), ()> {
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

pub(super) fn decode_dib(bytes: &[u8], info: &DibInfo) -> Result<Vec<u8>, ()> {
    let width = usize::try_from(info.width).map_err(|_| ())?;
    let height = usize::try_from(info.height).map_err(|_| ())?;
    let rgba_size = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(())?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_size).map_err(|_| ())?;
    rgba.resize(rgba_size, 0);
    let mut any_alpha = false;
    for output_y in 0..height {
        let source_y = height - 1 - output_y;
        let row_start = info.pixel_offset + source_y * info.row_stride;
        let row = bytes
            .get(row_start..row_start + info.row_stride)
            .ok_or(())?;
        for x in 0..width {
            let color = match info.bits_per_pixel {
                1 | 4 | 8 => palette(bytes, info, packed_index(row, x, info.bits_per_pixel)?)?,
                16 => masked(read_u16(row, x * 2)? as u32, info.masks),
                24 => {
                    let pixel = row.get(x * 3..x * 3 + 3).ok_or(())?;
                    [pixel[2], pixel[1], pixel[0], 255]
                }
                32 => {
                    let color = masked(read_u32(row, x * 4)?, info.masks);
                    any_alpha |= color[3] != 0;
                    color
                }
                _ => return Err(()),
            };
            rgba[(output_y * width + x) * 4..(output_y * width + x + 1) * 4]
                .copy_from_slice(&color);
        }
    }
    for output_y in 0..height {
        let source_y = height - 1 - output_y;
        let mask_start = info.mask_offset + source_y * info.mask_stride;
        let mask = bytes
            .get(mask_start..mask_start + info.mask_stride)
            .ok_or(())?;
        for x in 0..width {
            let destination = (output_y * width + x) * 4;
            let transparent = mask[x / 8] & (0x80 >> (x % 8)) != 0;
            if transparent {
                rgba[destination + 3] = 0;
            } else if info.bits_per_pixel != 32 || !any_alpha {
                rgba[destination + 3] = 255;
            }
        }
    }
    Ok(rgba)
}

fn stride(width: u32, depth: u16) -> Result<usize, ()> {
    usize::try_from(
        u64::from(width)
            .checked_mul(u64::from(depth))
            .ok_or(())?
            .div_ceil(32)
            .checked_mul(4)
            .ok_or(())?,
    )
    .map_err(|_| ())
}

fn packed_index(row: &[u8], x: usize, depth: u16) -> Result<usize, ()> {
    Ok(match depth {
        1 => usize::from((row[x / 8] >> (7 - x % 8)) & 1),
        4 => usize::from(if x.is_multiple_of(2) {
            row[x / 2] >> 4
        } else {
            row[x / 2] & 15
        }),
        8 => usize::from(row[x]),
        _ => return Err(()),
    })
}

fn palette(bytes: &[u8], info: &DibInfo, index: usize) -> Result<[u8; 4], ()> {
    if index >= info.palette_entries {
        return Err(());
    }
    let offset = info.palette_offset + index * 4;
    let color = bytes.get(offset..offset + 3).ok_or(())?;
    Ok([color[2], color[1], color[0], 255])
}

fn masked(value: u32, masks: [u32; 4]) -> [u8; 4] {
    [
        scale(value, masks[0]),
        scale(value, masks[1]),
        scale(value, masks[2]),
        if masks[3] == 0 {
            255
        } else {
            scale(value, masks[3])
        },
    ]
}

fn scale(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let maximum = mask >> shift;
    ((((value & mask) >> shift) as u64 * 255 + u64::from(maximum) / 2) / u64::from(maximum)) as u8
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
